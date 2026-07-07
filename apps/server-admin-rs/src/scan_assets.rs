use std::{
    collections::{BTreeSet, HashMap},
    env,
    net::{Ipv4Addr, ToSocketAddrs},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use get_if_addrs::{IfAddr, get_if_addrs};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::{net::TcpStream, task::JoinSet, time::timeout};
use url::Url;

use crate::{i18n::Translator, response, runtime_profile, state::AppState};

const MAX_SCAN_CIDRS: usize = 16;
const MAX_SCAN_HOSTS: u64 = 1024;
const DOCKER_DISCOVER_IP_HEADER: &str = "x-fn-knock-docker-discover-ip";
const DISCOVER_JOB_ACTIVE_TTL_MS: i64 = 30 * 60 * 1000;
const DISCOVER_JOB_DONE_TTL_MS: i64 = 5 * 60 * 1000;
const DISCOVER_JOB_MAX_ACTIVE: usize = 4;
const DISCOVER_JOB_MAX_RETAINED: usize = 64;
const LOOPBACK_DISCOVERY_CIDR: &str = "127.0.0.1/32";
const LOOPBACK_DISCOVERY_HOST: &str = "127.0.0.1";
const DISCOVERY_PORT_RANGE_START: u16 = 80;
const DISCOVERY_PORT_RANGE_END: u16 = 60_000;
const DISCOVERY_LIMITED_PORT_RANGE_END: u16 = 9_999;
const DISCOVERY_TIMEOUT_MS: u64 = 80;
const DISCOVERY_HTTP_TIMEOUT_MS: u64 = 2_000;
const DISCOVERY_HTTP_USER_AGENT: &str = "Node-Elysia-Scanner/1.0";
const NETWORK_MAX_CONCURRENT: usize = 64;
const NETWORK_HOST_CONCURRENCY: usize = 6;
const LOOPBACK_MAX_CONCURRENT: usize = 200;
const LOCAL_SELF_DISCOVERY_SKIP_PORTS: &[u16] = &[80];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DiscoveryPortRangeMode {
    Full,
    Limited,
}

#[derive(Clone, Copy)]
struct DiscoveryPortRange {
    start: u16,
    end: u16,
}

struct DiscoveryHostGroup {
    hosts: Vec<String>,
    mode: DiscoveryPortRangeMode,
    port_range: DiscoveryPortRange,
    skip_ports: Vec<u16>,
}

#[derive(Deserialize)]
struct DiscoverTargetsBody {
    #[serde(default)]
    custom_cidrs: Vec<String>,
    #[serde(default)]
    selected_cidrs: Vec<String>,
}

#[derive(Deserialize)]
struct DiscoverJobBody {
    #[serde(default)]
    target_cidrs: Vec<String>,
}

#[derive(Deserialize)]
struct DiscoverJobQuery {
    cursor: Option<String>,
}

#[derive(Deserialize)]
struct HostMappingProbeBody {
    hosts: Option<Vec<String>>,
}

#[derive(Clone)]
struct ParsedIpv4Cidr {
    cidr: String,
    first_host: u32,
    last_host: u32,
    host_count: u64,
}

struct DiscoverJob {
    id: String,
    cancel: Arc<AtomicBool>,
    created_at: i64,
    updated_at: i64,
    state: String,
    meta: Option<Value>,
    progress: Option<Value>,
    service_events: Vec<Value>,
    service_map: Vec<(String, Value)>,
    result: Option<Value>,
    error: Option<String>,
}

type DiscoverJobHandle = Arc<Mutex<DiscoverJob>>;

static DISCOVER_JOBS: OnceLock<Mutex<HashMap<String, DiscoverJobHandle>>> = OnceLock::new();

#[derive(Clone)]
struct DiscoveryProxyRule {
    path: String,
    rewrite_html: bool,
    use_root_mode: bool,
}

#[derive(Clone)]
struct DiscoveryAnalyzerRule {
    name: String,
    label: String,
    proxy: DiscoveryProxyRule,
    is_default: bool,
}

struct DiscoveryHttpResult {
    host: String,
    port: u16,
    status: u16,
    headers: HashMap<String, String>,
    body: String,
}

pub fn scan_asset_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/admin/scan/discover-targets",
            get(get_discover_targets).post(save_discover_targets),
        )
        .route(
            "/api/admin/assets/discover-targets",
            get(get_discover_targets).post(save_discover_targets),
        )
        .route("/api/admin/scan/discover/jobs", post(start_discover_job))
        .route("/api/admin/assets/discover/jobs", post(start_discover_job))
        .route(
            "/api/admin/scan/discover/jobs/{job_id}",
            get(get_discover_job).delete(cancel_discover_job_route),
        )
        .route(
            "/api/admin/assets/discover/jobs/{job_id}",
            get(get_discover_job).delete(cancel_discover_job_route),
        )
        .route(
            "/api/admin/scan/host-mappings/probe",
            post(probe_host_mappings),
        )
        .route(
            "/api/admin/assets/host-mappings/probe",
            post(probe_host_mappings),
        )
}

async fn get_discover_targets(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.redis.get_config().await {
        Ok(config) => response::ok(build_discover_targets_payload(
            &state,
            &headers,
            &config,
            &translator,
        ))
        .into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to read scan discover targets config");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                translator.t("server.scanDiscovery.loadTargetsFailed"),
            )
        }
    }
}

async fn save_discover_targets(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<DiscoverTargetsBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let custom_cidrs = normalize_allowed_scan_cidrs(body.custom_cidrs);
    let selected_cidrs = normalize_allowed_scan_cidrs(body.selected_cidrs);
    if let Err(message) = validate_scan_cidrs(&selected_cidrs) {
        return response::error(
            StatusCode::BAD_REQUEST,
            localize_scan_discovery_error(&translator, &message),
        );
    }

    let mut config = match state.redis.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to read scan discover targets config");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                translator.t("server.scanDiscovery.loadConfigFailed"),
            );
        }
    };
    ensure_object(&mut config).insert(
        "scan_discovery".to_string(),
        json!({
            "custom_cidrs": custom_cidrs,
            "selected_cidrs": selected_cidrs
        }),
    );
    if let Err(error) = state.redis.save_config(&config).await {
        tracing::warn!(%error, "failed to save scan discover targets config");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            translator.t("server.scanDiscovery.saveTargetsFailed"),
        );
    }
    response::ok(build_discover_targets_payload(
        &state,
        &headers,
        &config,
        &translator,
    ))
    .into_response()
}

async fn start_discover_job(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<DiscoverJobBody>,
) -> Response {
    let translator = crate::i18n::Translator::from_state(&state).await;
    let scan_cidrs = match validate_scan_cidrs(&body.target_cidrs) {
        Ok(cidrs) if !cidrs.is_empty() => cidrs,
        Ok(_) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                translator.t("server.scanDiscovery.selectAtLeastOneCidr"),
            );
        }
        Err(message) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                localize_scan_discovery_error(&translator, &message),
            );
        }
    };
    let config = match state.redis.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load config before scan discover job");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                translator.t("server.scanDiscovery.loadConfigFailed"),
            );
        }
    };
    let full_range_cidrs =
        resolve_full_range_discover_cidrs(&state, &headers, &config, &translator);
    let self_scan_hosts = resolve_discover_self_hosts(&state, &headers);
    let exclude_ports = collect_excluded_ports(&state);
    let job = create_discover_job(
        scan_cidrs,
        full_range_cidrs,
        self_scan_hosts,
        exclude_ports,
        translator,
    );
    let data = serialize_discover_job(&job, None);
    response::ok(data).into_response()
}

async fn get_discover_job(
    State(state): State<AppState>,
    Path(job_id): Path<String>,
    Query(query): Query<DiscoverJobQuery>,
) -> Response {
    cleanup_discover_jobs();
    let Some(job) = get_discover_job_handle(&job_id) else {
        let translator = crate::i18n::Translator::from_state(&state).await;
        return response::error(
            StatusCode::NOT_FOUND,
            translator.t("server.scanDiscovery.scanJobNotFound"),
        );
    };
    response::ok(serialize_discover_job(&job, query.cursor.as_deref())).into_response()
}

async fn cancel_discover_job_route(
    State(state): State<AppState>,
    Path(job_id): Path<String>,
) -> Response {
    cleanup_discover_jobs();
    let Some(job) = get_discover_job_handle(&job_id) else {
        let translator = crate::i18n::Translator::from_state(&state).await;
        return response::error(
            StatusCode::NOT_FOUND,
            translator.t("server.scanDiscovery.scanJobNotFound"),
        );
    };
    cancel_discover_job(&job);
    response::ok(serialize_discover_job(&job, None)).into_response()
}

async fn probe_host_mappings(
    State(state): State<AppState>,
    Json(body): Json<HostMappingProbeBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let config = match state.redis.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to read host mappings for probe");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                translator.t("server.scanDiscovery.loadConfigFailed"),
            );
        }
    };
    let results = probe_configured_host_mappings(
        config
            .get("host_mappings")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
        body.hosts.unwrap_or_default(),
    )
    .await;
    response::ok(json!({ "results": results })).into_response()
}

fn discover_jobs() -> &'static Mutex<HashMap<String, DiscoverJobHandle>> {
    DISCOVER_JOBS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn create_discover_job(
    scan_cidrs: Vec<String>,
    full_range_cidrs: Vec<String>,
    self_scan_hosts: Vec<String>,
    exclude_ports: Vec<u16>,
    translator: Translator,
) -> DiscoverJobHandle {
    cleanup_discover_jobs();
    let now = now_millis();
    let id = uuid::Uuid::new_v4().to_string();
    let cancel = Arc::new(AtomicBool::new(false));
    let job = Arc::new(Mutex::new(DiscoverJob {
        id: id.clone(),
        cancel: cancel.clone(),
        created_at: now,
        updated_at: now,
        state: "queued".to_string(),
        meta: None,
        progress: None,
        service_events: Vec::new(),
        service_map: Vec::new(),
        result: None,
        error: None,
    }));
    discover_jobs()
        .lock()
        .expect("discover jobs lock")
        .insert(id, job.clone());
    enforce_discover_job_limits();

    let job_for_task = job.clone();
    tokio::spawn(async move {
        run_discover_job(
            job_for_task,
            scan_cidrs,
            full_range_cidrs,
            self_scan_hosts,
            exclude_ports,
            translator,
        )
        .await;
    });
    job
}

fn get_discover_job_handle(job_id: &str) -> Option<DiscoverJobHandle> {
    discover_jobs()
        .lock()
        .expect("discover jobs lock")
        .get(job_id)
        .cloned()
}

fn is_terminal_discover_state(state: &str) -> bool {
    matches!(state, "completed" | "cancelled" | "failed")
}

fn cancel_discover_job(job: &DiscoverJobHandle) {
    let mut locked = job.lock().expect("discover job lock");
    if is_terminal_discover_state(&locked.state) {
        return;
    }
    locked.cancel.store(true, Ordering::SeqCst);
    locked.state = "cancelled".to_string();
    locked.service_events.clear();
    locked.service_map.clear();
    locked.updated_at = now_millis();
}

fn cleanup_discover_jobs() {
    let now = now_millis();
    let jobs = discover_jobs();
    let handles = jobs
        .lock()
        .expect("discover jobs lock")
        .iter()
        .map(|(id, job)| (id.clone(), job.clone()))
        .collect::<Vec<_>>();
    let mut delete_ids = Vec::new();
    for (id, job) in handles {
        let mut locked = job.lock().expect("discover job lock");
        if is_terminal_discover_state(&locked.state) {
            if now - locked.updated_at > DISCOVER_JOB_DONE_TTL_MS {
                delete_ids.push(id);
            }
            continue;
        }
        if now - locked.created_at > DISCOVER_JOB_ACTIVE_TTL_MS {
            locked.cancel.store(true, Ordering::SeqCst);
            locked.state = "cancelled".to_string();
            locked.service_events.clear();
            locked.service_map.clear();
            locked.updated_at = now;
        }
    }
    if !delete_ids.is_empty() {
        let mut locked = jobs.lock().expect("discover jobs lock");
        for id in delete_ids {
            locked.remove(&id);
        }
    }
    enforce_discover_job_limits();
}

fn enforce_discover_job_limits() {
    let jobs = discover_jobs();
    let handles = jobs
        .lock()
        .expect("discover jobs lock")
        .iter()
        .map(|(id, job)| (id.clone(), job.clone()))
        .collect::<Vec<_>>();
    let mut active = Vec::new();
    for (_id, job) in &handles {
        let locked = job.lock().expect("discover job lock");
        if !is_terminal_discover_state(&locked.state) {
            active.push((locked.created_at, job.clone()));
        }
    }
    active.sort_by_key(|(created_at, _)| *created_at);
    for (_, job) in active
        .iter()
        .take(active.len().saturating_sub(DISCOVER_JOB_MAX_ACTIVE))
    {
        cancel_discover_job(job);
    }

    let mut ids_by_age = Vec::new();
    for (id, job) in handles {
        let locked = job.lock().expect("discover job lock");
        ids_by_age.push((
            locked.created_at,
            id,
            is_terminal_discover_state(&locked.state),
        ));
    }
    ids_by_age.sort_by_key(|(created_at, _, _)| *created_at);
    let overflow = ids_by_age.len().saturating_sub(DISCOVER_JOB_MAX_RETAINED);
    if overflow == 0 {
        return;
    }
    let mut locked_jobs = jobs.lock().expect("discover jobs lock");
    for (_, id, terminal) in ids_by_age.into_iter().take(overflow) {
        if terminal {
            locked_jobs.remove(&id);
        } else if let Some(job) = locked_jobs.get(&id) {
            cancel_discover_job(job);
        }
    }
}

fn serialize_discover_job(job: &DiscoverJobHandle, cursor: Option<&str>) -> Value {
    let locked = job.lock().expect("discover job lock");
    let service_cursor = normalize_service_cursor(cursor, locked.service_events.len());
    json!({
        "jobId": locked.id,
        "state": locked.state,
        "createdAt": locked.created_at,
        "updatedAt": locked.updated_at,
        "meta": locked.meta,
        "progress": locked.progress,
        "services": locked.service_events[service_cursor..].to_vec(),
        "nextCursor": locked.service_events.len(),
        "result": locked.result,
        "error": locked.error,
    })
}

fn normalize_service_cursor(value: Option<&str>, max: usize) -> usize {
    parse_js_parse_int_radix_10(value.unwrap_or("0"))
        .filter(|cursor| *cursor >= 0)
        .map(|cursor| (cursor as usize).min(max))
        .unwrap_or(0)
}

fn full_discovery_port_range() -> DiscoveryPortRange {
    DiscoveryPortRange {
        start: DISCOVERY_PORT_RANGE_START,
        end: DISCOVERY_PORT_RANGE_END,
    }
}

fn limited_discovery_port_range() -> DiscoveryPortRange {
    DiscoveryPortRange {
        start: DISCOVERY_PORT_RANGE_START,
        end: DISCOVERY_LIMITED_PORT_RANGE_END,
    }
}

fn count_ports_in_range(range: DiscoveryPortRange, skip_ports: &[u16]) -> usize {
    let skipped = skip_ports
        .iter()
        .copied()
        .filter(|port| *port >= range.start && *port <= range.end)
        .collect::<BTreeSet<_>>()
        .len();
    usize::from(range.end - range.start + 1).saturating_sub(skipped)
}

fn build_port_list(range: DiscoveryPortRange, skip_ports: &[u16]) -> Vec<u16> {
    let skip = skip_ports.iter().copied().collect::<BTreeSet<_>>();
    (range.start..=range.end)
        .filter(|port| !skip.contains(port))
        .collect()
}

fn merge_discovery_skip_ports(base: &[u16], extra: &[u16]) -> Vec<u16> {
    base.iter()
        .chain(extra)
        .copied()
        .filter(|port| *port > 0)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn build_discovery_port_mode_label(scan_cidrs: &[String], full_range_cidrs: &[String]) -> String {
    let groups = build_discovery_host_groups(scan_cidrs, full_range_cidrs, None, &[]);
    let has_full = groups
        .iter()
        .any(|group| group.mode == DiscoveryPortRangeMode::Full);
    let has_limited = groups
        .iter()
        .any(|group| group.mode == DiscoveryPortRangeMode::Limited);
    if has_full && has_limited {
        format!(
            "local={DISCOVERY_PORT_RANGE_START}-{DISCOVERY_PORT_RANGE_END}, other={DISCOVERY_PORT_RANGE_START}-{DISCOVERY_LIMITED_PORT_RANGE_END}"
        )
    } else if has_limited {
        format!("{DISCOVERY_PORT_RANGE_START}-{DISCOVERY_LIMITED_PORT_RANGE_END}")
    } else {
        format!("{DISCOVERY_PORT_RANGE_START}-{DISCOVERY_PORT_RANGE_END}")
    }
}

fn count_discovery_scan_ports_for_groups(
    groups: &[DiscoveryHostGroup],
    exclude_ports: &[u16],
) -> usize {
    groups
        .iter()
        .map(|group| {
            let skip_ports = merge_discovery_skip_ports(exclude_ports, &group.skip_ports);
            count_ports_in_range(group.port_range, &skip_ports) * group.hosts.len()
        })
        .sum()
}

fn build_discovery_host_groups(
    scan_cidrs: &[String],
    full_range_cidrs: &[String],
    scan_hosts: Option<&[String]>,
    self_scan_hosts: &[String],
) -> Vec<DiscoveryHostGroup> {
    let mut normalized_full_range = vec![LOOPBACK_DISCOVERY_CIDR.to_string()];
    normalized_full_range.extend(full_range_cidrs.iter().cloned());
    let normalized_full_range = normalize_allowed_scan_cidrs(normalized_full_range);
    let allowed_hosts = scan_hosts.map(|hosts| hosts.iter().cloned().collect::<BTreeSet<_>>());
    let self_scan_hosts = build_self_scan_host_set(self_scan_hosts);
    let mut seen_hosts = BTreeSet::new();
    let mut groups = Vec::new();

    for cidr in normalize_allowed_scan_cidrs(scan_cidrs.iter().cloned()) {
        let hosts = expand_scan_cidrs(&[cidr.clone()])
            .into_iter()
            .filter(|host| {
                if allowed_hosts
                    .as_ref()
                    .is_some_and(|allowed_hosts| !allowed_hosts.contains(host))
                {
                    return false;
                }
                seen_hosts.insert(host.clone())
            })
            .collect::<Vec<_>>();
        if hosts.is_empty() {
            continue;
        }

        let mut full_hosts = Vec::new();
        let mut limited_hosts = Vec::new();
        for host in hosts {
            if is_full_range_discovery_host(&host, &normalized_full_range) {
                full_hosts.push(host);
            } else {
                limited_hosts.push(host);
            }
        }
        push_discovery_host_groups(
            &mut groups,
            full_hosts,
            DiscoveryPortRangeMode::Full,
            &self_scan_hosts,
        );
        push_discovery_host_groups(
            &mut groups,
            limited_hosts,
            DiscoveryPortRangeMode::Limited,
            &self_scan_hosts,
        );
    }

    groups
}

fn build_self_scan_host_set(self_scan_hosts: &[String]) -> BTreeSet<String> {
    std::iter::once(LOOPBACK_DISCOVERY_HOST.to_string())
        .chain(self_scan_hosts.iter().map(|host| host.trim().to_string()))
        .filter(|host| !host.is_empty())
        .collect()
}

fn push_discovery_host_groups(
    groups: &mut Vec<DiscoveryHostGroup>,
    hosts: Vec<String>,
    mode: DiscoveryPortRangeMode,
    self_scan_hosts: &BTreeSet<String>,
) {
    if hosts.is_empty() {
        return;
    }
    let mut regular_hosts = Vec::new();
    let mut local_self_hosts = Vec::new();
    for host in hosts {
        if self_scan_hosts.contains(&host) {
            local_self_hosts.push(host);
        } else {
            regular_hosts.push(host);
        }
    }
    if !regular_hosts.is_empty() {
        groups.push(build_discovery_host_group(regular_hosts, mode, Vec::new()));
    }
    if !local_self_hosts.is_empty() {
        groups.push(build_discovery_host_group(
            local_self_hosts,
            mode,
            LOCAL_SELF_DISCOVERY_SKIP_PORTS.to_vec(),
        ));
    }
}

fn build_discovery_host_group(
    hosts: Vec<String>,
    mode: DiscoveryPortRangeMode,
    skip_ports: Vec<u16>,
) -> DiscoveryHostGroup {
    DiscoveryHostGroup {
        hosts,
        mode,
        port_range: if mode == DiscoveryPortRangeMode::Full {
            full_discovery_port_range()
        } else {
            limited_discovery_port_range()
        },
        skip_ports,
    }
}

fn is_full_range_discovery_host(host: &str, full_range_cidrs: &[String]) -> bool {
    if host == LOOPBACK_DISCOVERY_HOST {
        return true;
    }
    let Ok(ip) = host.parse::<Ipv4Addr>() else {
        return false;
    };
    let host_number = u32::from(ip);
    full_range_cidrs.iter().any(|cidr| {
        parse_allowed_scan_cidr(cidr).is_some_and(|parsed| {
            host_number >= parsed.first_host && host_number <= parsed.last_host
        })
    })
}

async fn run_discover_job(
    job: DiscoverJobHandle,
    scan_cidrs: Vec<String>,
    full_range_cidrs: Vec<String>,
    self_scan_hosts: Vec<String>,
    exclude_ports: Vec<u16>,
    translator: Translator,
) {
    let scan_hosts = expand_scan_cidrs(&scan_cidrs);
    let scan_scope = build_scan_scope(&scan_cidrs);
    let groups = build_discovery_host_groups(
        &scan_cidrs,
        &full_range_cidrs,
        Some(&scan_hosts),
        &self_scan_hosts,
    );
    let total_ports = count_discovery_scan_ports_for_groups(&groups, &exclude_ports);
    let port_mode_label = build_discovery_port_mode_label(&scan_cidrs, &full_range_cidrs);
    update_discover_job(&job, |job| {
        job.state = "running".to_string();
        job.meta = Some(json!({
            "host": scan_hosts.first().cloned().unwrap_or_default(),
            "totalPortsScanned": total_ports,
            "foundServices": 0,
            "scannedHosts": scan_hosts.len(),
            "scanHostCount": scan_hosts.len(),
            "scanScope": scan_scope,
            "scanCidrs": scan_cidrs,
            "portRange": port_mode_label
        }));
        job.progress = Some(json!({
            "scannedPorts": 0,
            "totalPorts": total_ports,
            "scannedHosts": 0,
            "totalHosts": scan_hosts.len(),
            "currentHost": scan_hosts.first().cloned().unwrap_or_default(),
        }));
    });

    let client = match discovery_http_client_builder()
        .redirect(reqwest::redirect::Policy::limited(20))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            fail_discover_job(&job, error.to_string());
            return;
        }
    };
    let manual_redirect_client = match discovery_http_client_builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            fail_discover_job(&job, error.to_string());
            return;
        }
    };

    let scanned_ports = Arc::new(AtomicUsize::new(0));
    let completed_hosts = Arc::new(AtomicUsize::new(0));
    let total_hosts = scan_hosts.len();

    for group in groups {
        let skip_ports = merge_discovery_skip_ports(&exclude_ports, &group.skip_ports);
        let ports = Arc::new(build_port_list(group.port_range, &skip_ports));
        if ports.is_empty() || group.hosts.is_empty() {
            continue;
        }
        let is_loopback_only = group.hosts.len() == 1
            && group
                .hosts
                .first()
                .is_some_and(|host| host == LOOPBACK_DISCOVERY_HOST);
        let host_concurrency = if is_loopback_only {
            1
        } else {
            NETWORK_HOST_CONCURRENCY.min(group.hosts.len()).max(1)
        };
        let max_concurrent = if is_loopback_only {
            LOOPBACK_MAX_CONCURRENT
        } else {
            NETWORK_MAX_CONCURRENT
        };

        for hosts in group.hosts.chunks(host_concurrency) {
            if job_cancelled(&job) {
                return;
            }
            let mut tasks = JoinSet::new();
            for host in hosts {
                tasks.spawn(scan_discovery_host(
                    job.clone(),
                    client.clone(),
                    manual_redirect_client.clone(),
                    host.clone(),
                    ports.clone(),
                    total_ports,
                    total_hosts,
                    scanned_ports.clone(),
                    completed_hosts.clone(),
                    max_concurrent,
                    translator.clone(),
                ));
            }
            while let Some(result) = tasks.join_next().await {
                if let Err(error) = result {
                    tracing::warn!(%error, "scan discovery host task failed");
                }
            }
        }
    }

    if job_cancelled(&job) {
        return;
    }
    complete_discover_job(
        &job,
        scan_cidrs,
        scan_hosts,
        scan_scope,
        scanned_ports.load(Ordering::SeqCst),
    );
}

fn discovery_http_client_builder() -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .timeout(Duration::from_millis(DISCOVERY_HTTP_TIMEOUT_MS))
        .danger_accept_invalid_certs(true)
}

async fn scan_discovery_host(
    job: DiscoverJobHandle,
    client: reqwest::Client,
    manual_redirect_client: reqwest::Client,
    host: String,
    ports: Arc<Vec<u16>>,
    total_ports: usize,
    total_hosts: usize,
    scanned_ports: Arc<AtomicUsize>,
    completed_hosts: Arc<AtomicUsize>,
    max_concurrent: usize,
    translator: Translator,
) {
    for chunk in ports.chunks(max_concurrent.max(1)) {
        if job_cancelled(&job) {
            return;
        }
        let mut tcp_tasks = JoinSet::new();
        for port in chunk.iter().copied() {
            let host = host.clone();
            tcp_tasks.spawn(async move { (port, check_tcp_port(&host, port).await) });
        }

        let mut open_ports = Vec::new();
        while let Some(result) = tcp_tasks.join_next().await {
            let Ok((port, open)) = result else {
                continue;
            };
            let scanned = scanned_ports.fetch_add(1, Ordering::SeqCst) + 1;
            update_discover_progress(
                &job,
                scanned,
                total_ports,
                completed_hosts.load(Ordering::SeqCst),
                total_hosts,
                &host,
            );
            if open {
                open_ports.push(port);
            }
        }

        if open_ports.is_empty() {
            continue;
        }
        let mut http_tasks = JoinSet::new();
        for port in open_ports {
            let client = client.clone();
            let manual_redirect_client = manual_redirect_client.clone();
            let host = host.clone();
            let translator = translator.clone();
            http_tasks.spawn(async move {
                probe_discovery_service(&client, &manual_redirect_client, &host, port, &translator)
                    .await
            });
        }
        while let Some(result) = http_tasks.join_next().await {
            if let Ok(Some(service)) = result {
                push_discovered_service(&job, service);
            }
        }
    }

    let completed = completed_hosts.fetch_add(1, Ordering::SeqCst) + 1;
    update_discover_progress(
        &job,
        scanned_ports.load(Ordering::SeqCst),
        total_ports,
        completed,
        total_hosts,
        &host,
    );
}

async fn check_tcp_port(host: &str, port: u16) -> bool {
    timeout(
        Duration::from_millis(DISCOVERY_TIMEOUT_MS),
        TcpStream::connect((host, port)),
    )
    .await
    .is_ok_and(|result| result.is_ok())
}

fn update_discover_progress(
    job: &DiscoverJobHandle,
    scanned_ports: usize,
    total_ports: usize,
    scanned_hosts: usize,
    total_hosts: usize,
    current_host: &str,
) {
    update_discover_job(job, |job| {
        if is_terminal_discover_state(&job.state) {
            return;
        }
        job.progress = Some(json!({
            "scannedPorts": scanned_ports,
            "totalPorts": total_ports,
            "scannedHosts": scanned_hosts,
            "totalHosts": total_hosts,
            "currentHost": current_host,
        }));
    });
}

fn update_discover_job(job: &DiscoverJobHandle, update: impl FnOnce(&mut DiscoverJob)) {
    let mut locked = job.lock().expect("discover job lock");
    update(&mut locked);
    locked.updated_at = now_millis();
}

fn fail_discover_job(job: &DiscoverJobHandle, message: String) {
    update_discover_job(job, |job| {
        if is_terminal_discover_state(&job.state) {
            return;
        }
        job.state = "failed".to_string();
        job.error = Some(message);
    });
}

fn job_cancelled(job: &DiscoverJobHandle) -> bool {
    let locked = job.lock().expect("discover job lock");
    locked.cancel.load(Ordering::SeqCst) || locked.state == "cancelled"
}

fn push_discovered_service(job: &DiscoverJobHandle, service: Value) {
    let service_key = discovered_service_key(&service);
    update_discover_job(job, |job| {
        if let Some((_, existing)) = job
            .service_map
            .iter_mut()
            .find(|(key, _)| key == &service_key)
        {
            if discovered_service_port(existing) <= discovered_service_port(&service) {
                return;
            }
            *existing = service.clone();
        } else {
            job.service_map.push((service_key, service.clone()));
        }
        job.service_events.push(service);
        if let Some(meta) = job.meta.as_mut().and_then(Value::as_object_mut) {
            meta.insert("foundServices".to_string(), json!(job.service_map.len()));
        }
    });
}

fn discovered_service_key(service: &Value) -> String {
    if let Some(key) = service.get("serviceKey").and_then(Value::as_str)
        && !key.is_empty()
    {
        return key.to_string();
    }
    let host = service.get("host").and_then(Value::as_str).unwrap_or("");
    let port = service
        .get("port")
        .and_then(Value::as_u64)
        .map(|port| port.to_string())
        .unwrap_or_default();
    format!("{host}:{port}")
}

fn discovered_service_port(service: &Value) -> u64 {
    service
        .get("port")
        .and_then(Value::as_u64)
        .unwrap_or(u64::MAX)
}

fn complete_discover_job(
    job: &DiscoverJobHandle,
    scan_cidrs: Vec<String>,
    scan_hosts: Vec<String>,
    scan_scope: Option<String>,
    scanned_ports: usize,
) {
    update_discover_job(job, |job| {
        if is_terminal_discover_state(&job.state) {
            return;
        }
        let services = job
            .service_map
            .iter()
            .map(|(_, service)| service.clone())
            .collect::<Vec<_>>();
        job.progress = Some(json!({
            "scannedPorts": scanned_ports,
            "totalPorts": scanned_ports,
            "scannedHosts": scan_hosts.len(),
            "totalHosts": scan_hosts.len(),
        }));
        job.result = Some(json!({
            "host": scan_hosts.first().cloned().unwrap_or_default(),
            "totalPortsScanned": scanned_ports,
            "foundServices": services.len(),
            "scannedHosts": scan_hosts.len(),
            "scanHostCount": scan_hosts.len(),
            "scanScope": scan_scope,
            "scanCidrs": scan_cidrs,
            "services": services,
        }));
        job.state = "completed".to_string();
    });
}

async fn probe_discovery_service(
    client: &reqwest::Client,
    manual_redirect_client: &reqwest::Client,
    host: &str,
    port: u16,
    translator: &Translator,
) -> Option<Value> {
    let target = format!("http://{host}:{port}");
    let response = match send_discovery_probe_request(client, &target).await {
        Ok(response) => response,
        Err(error) => {
            if error.is_timeout() {
                return None;
            }
            tracing::debug!(%error, %target, "discovery HTTP follow probe failed; retrying without redirects");
            send_discovery_probe_request(manual_redirect_client, &target)
                .await
                .ok()?
        }
    };
    let status = response.status().as_u16();
    let headers = collect_response_headers(response.headers());
    let body = response.text().await.unwrap_or_default();
    analyze_discovered_http_service(
        client,
        DiscoveryHttpResult {
            host: host.to_string(),
            port,
            status,
            headers,
            body,
        },
        translator,
    )
    .await
}

async fn send_discovery_probe_request(
    client: &reqwest::Client,
    target: &str,
) -> Result<reqwest::Response, reqwest::Error> {
    client
        .get(target)
        .header(reqwest::header::USER_AGENT, DISCOVERY_HTTP_USER_AGENT)
        .header(reqwest::header::CONNECTION, "close")
        .send()
        .await
}

fn collect_response_headers(headers: &reqwest::header::HeaderMap) -> HashMap<String, String> {
    let mut output = HashMap::new();
    for (name, value) in headers {
        let key = name.as_str().to_ascii_lowercase();
        let value = value.to_str().unwrap_or("").to_string();
        output
            .entry(key)
            .and_modify(|existing: &mut String| {
                if !existing.is_empty() && !value.is_empty() {
                    existing.push_str(", ");
                }
                existing.push_str(&value);
            })
            .or_insert(value);
    }
    output
}

async fn analyze_discovered_http_service(
    client: &reqwest::Client,
    result: DiscoveryHttpResult,
    translator: &Translator,
) -> Option<Value> {
    if is_plain_http_to_https_response(result.status, &result.body) {
        return None;
    }

    let rule = match_discovery_analyzer_rule(client, &result, translator).await;
    Some(build_discovered_service_value(&result, rule))
}

#[cfg(test)]
fn build_discovered_http_service(
    host: &str,
    port: u16,
    status: u16,
    www_authenticate: Option<&str>,
    body: &str,
) -> Option<Value> {
    if is_plain_http_to_https_response(status, body) {
        return None;
    }
    let mut headers = HashMap::new();
    if let Some(www_authenticate) = www_authenticate {
        headers.insert("www-authenticate".to_string(), www_authenticate.to_string());
    }
    let result = DiscoveryHttpResult {
        host: host.to_string(),
        port,
        status,
        headers,
        body: body.to_string(),
    };
    Some(build_discovered_service_value(
        &result,
        build_generic_http_rule(&result),
    ))
}

fn build_discovered_service_value(
    result: &DiscoveryHttpResult,
    rule: DiscoveryAnalyzerRule,
) -> Value {
    let service_name = rule.name.to_string();
    let mut service = json!({
        "serviceKey": format!("{}::{service_name}", result.host),
        "host": result.host,
        "port": result.port,
        "httpStatus": result.status,
        "detail": {
            "name": service_name,
            "label": rule.label,
            "rule": {
                "path": rule.proxy.path,
                "rewrite_html": rule.proxy.rewrite_html,
                "use_auth": true,
                "use_root_mode": rule.proxy.use_root_mode,
                "strip_path": true,
                "target": "",
            },
            "isDefault": rule.is_default,
        },
    });
    if has_basic_auth_challenge(result.headers.get("www-authenticate").map(String::as_str)) {
        if let Some(object) = service.as_object_mut() {
            object.insert("requiresBasicAuth".to_string(), json!(true));
        }
    }
    service
}

fn build_generic_http_rule(result: &DiscoveryHttpResult) -> DiscoveryAnalyzerRule {
    DiscoveryAnalyzerRule {
        name: format!("http-{}", result.port),
        label: extract_html_title(&result.body).unwrap_or_else(|| format!("HTTP {}", result.port)),
        proxy: DiscoveryProxyRule {
            path: format!("/app-{}", result.port),
            rewrite_html: true,
            use_root_mode: false,
        },
        is_default: false,
    }
}

async fn match_discovery_analyzer_rule(
    client: &reqwest::Client,
    result: &DiscoveryHttpResult,
    translator: &Translator,
) -> DiscoveryAnalyzerRule {
    if header_contains(result, "set-cookie", "mongo-express=") {
        return discovery_rule(
            "mongoexpress",
            "Mongo Express",
            "/mongoe",
            true,
            false,
            false,
        );
    }
    if body_contains(result, "<title>Redis Insight</title>") {
        return discovery_rule(
            "redisinsight",
            "Redis Insight",
            "/redisi",
            false,
            true,
            false,
        );
    }
    if body_contains(result, "<title>go2rtc</title>") {
        return discovery_rule("go2rtc", "Go2RTC", "/go2rtc", true, false, false);
    }
    if is_openwrt_luci_result(result) {
        return discovery_rule("openwrt", "OpenWrt LuCI", "/openwrt", false, true, false);
    }
    if body_contains(result, "<title>飞牛 fnOS</title>") {
        return discovery_rule(
            "fnos",
            &scanner_service_label(translator, "fnos"),
            "/fnos",
            false,
            true,
            true,
        );
    }
    if body_contains(result, "<title>Lucky</title>") {
        return discovery_rule("lucky", "Lucky", "/lucky", true, false, false);
    }

    if let Some(site_title) = fetch_list_public_site_title(client, result).await {
        if site_title == "小雅的分类 Alist" {
            return discovery_rule(
                "xiaoya",
                &scanner_service_label(translator, "xiaoyaAlist"),
                "/xy",
                false,
                true,
                false,
            );
        }
        if site_title == "Alist" {
            return discovery_rule("alist", "AList", "/alist", false, true, false);
        }
        if site_title == "OpenList" {
            return discovery_rule("openlist", "OpenList", "/op", false, true, false);
        }
    }

    if body_contains(result, "<title>Home Assistant</title>") {
        return discovery_rule("homeassistant", "Home Assistant", "/ha", true, false, false);
    }
    if body_contains(result, "<title>Sun-Panel</title>") {
        return discovery_rule("sun-panel", "Sun-Panel", "/sp", true, true, false);
    }
    if result.port == 5005
        && header_contains(result, "www-authenticate", "Basic realm=\"Restricted\"")
    {
        return discovery_rule("webdav", "WebDAV", "/webdav", true, false, false);
    }
    if body_contains(result, "<title>迅雷下载</title>") {
        return discovery_rule(
            "xunlei",
            &scanner_service_label(translator, "xunlei"),
            "/xunlei",
            true,
            false,
            false,
        );
    }
    if body_contains(result, "<TITLE>MiniDLNA") {
        return discovery_rule("miniDLNA", "miniDLNA", "/dlna", true, false, false);
    }
    if body_contains(result, "<title>Digital Zen Garden</title>") {
        return discovery_rule(
            "nowen",
            &scanner_service_label(translator, "nowen"),
            "/nowen",
            false,
            true,
            true,
        );
    }
    if body_contains(result, "<title>飞牛影视</title>") {
        return discovery_rule(
            "fnys",
            &scanner_service_label(translator, "fnys"),
            "/v",
            false,
            true,
            false,
        );
    }
    if body_contains(result, "dpanel/ui") {
        return discovery_rule("DPanel", "DPanel", "/dp", false, true, false);
    }
    if body_contains(result, "<title>彩票助手</title>") {
        return discovery_rule(
            "cpzs",
            &scanner_service_label(translator, "lottery"),
            "/cpzs",
            false,
            true,
            false,
        );
    }
    if result.port == 5005 && body_contains(result, "<title>登录</title>") {
        return discovery_rule(
            "Kuake",
            &scanner_service_label(translator, "kuake"),
            "/kuake",
            false,
            true,
            false,
        );
    }
    if body_contains(result, "<title>Jellyfin</title>") {
        return discovery_rule("Jellyfin", "Jellyfin", "/jellyfin", false, true, false);
    }
    if body_contains(result, "<title>WebUI 登录 | ME Frp</title>") {
        return discovery_rule("ME Frp", "ME Frp", "/mefrp", false, true, false);
    }
    if body_contains(result, "<title>MoonTV</title>") {
        return discovery_rule("MoonTV", "MoonTV", "/moontv", false, true, false);
    }
    if body_contains(result, "<title>fnOS Apps</title>") {
        return discovery_rule("fnOS Apps", "fnOS Apps", "/fnosapps", false, true, false);
    }
    if body_contains(result, "emby-elements/emby-collapse/emby-collapse") {
        return discovery_rule("Emby", "Emby", "/emby", false, true, false);
    }
    if body_contains(result, "<title>道理鱼音乐管理</title>") {
        return discovery_rule(
            "DLYMusic",
            &scanner_service_label(translator, "dlymusic"),
            "/music",
            false,
            true,
            false,
        );
    }
    if has_one_panel_loading_title(&result.body)
        && has_one_panel_public_favicon(client, result).await
    {
        return discovery_rule("1Panel", "1Panel", "/1panel", false, true, false);
    }

    build_generic_http_rule(result)
}

fn discovery_rule(
    name: &str,
    label: &str,
    path: &str,
    rewrite_html: bool,
    use_root_mode: bool,
    is_default: bool,
) -> DiscoveryAnalyzerRule {
    DiscoveryAnalyzerRule {
        name: name.to_string(),
        label: label.to_string(),
        proxy: DiscoveryProxyRule {
            path: path.to_string(),
            rewrite_html,
            use_root_mode,
        },
        is_default,
    }
}

fn scanner_service_label(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.scanDiscovery.serviceLabels.{key}"))
}

fn body_contains(result: &DiscoveryHttpResult, needle: &str) -> bool {
    result.body.contains(needle)
}

fn header_contains(result: &DiscoveryHttpResult, header: &str, needle: &str) -> bool {
    result
        .headers
        .get(header)
        .is_some_and(|value| value.contains(needle))
}

fn extract_html_title_text(body: &str) -> String {
    extract_html_title(body).unwrap_or_default()
}

fn has_list_title(body: &str) -> bool {
    extract_html_title_text(body)
        .trim()
        .to_ascii_lowercase()
        .contains("list")
}

async fn fetch_list_public_site_title(
    client: &reqwest::Client,
    result: &DiscoveryHttpResult,
) -> Option<String> {
    if !has_list_title(&result.body) {
        return None;
    }
    let url = format!("http://{}:{}/api/public/settings", result.host, result.port);
    let response = client
        .get(url)
        .header(reqwest::header::USER_AGENT, DISCOVERY_HTTP_USER_AGENT)
        .header(reqwest::header::CONNECTION, "close")
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let payload = response.json::<Value>().await.ok()?;
    if payload.get("code").and_then(Value::as_i64) != Some(200) {
        return None;
    }
    payload
        .pointer("/data/site_title")
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn is_openwrt_luci_result(result: &DiscoveryHttpResult) -> bool {
    result
        .headers
        .get("x-luci-login-required")
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("yes"))
        || has_luci_entrypoint(&result.body)
        || has_luci_login_page(&result.body)
}

fn has_luci_entrypoint(body: &str) -> bool {
    let normalized = body.to_ascii_lowercase();
    normalized.contains("cgi-bin/luci")
        && (normalized.contains("luci - lua configuration interface")
            || normalized.contains("http-equiv=\"refresh\"")
            || normalized.contains("http-equiv='refresh'")
            || normalized.contains("http-equiv=refresh"))
}

fn has_luci_login_page(body: &str) -> bool {
    let title = extract_html_title_text(body)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    let normalized = body.to_ascii_lowercase();
    title_has_luci_word(&title)
        && (normalized.contains("/luci-static/")
            || normalized.contains("application-name")
            || normalized.contains("apple-mobile-web-app-title"))
}

fn title_has_luci_word(title: &str) -> bool {
    let bytes = title.as_bytes();
    for (index, _) in title.match_indices("luci") {
        let before_ok = index == 0 || !bytes[index - 1].is_ascii_alphanumeric();
        let after = index + "luci".len();
        let after_ok = after >= bytes.len() || !bytes[after].is_ascii_alphanumeric();
        if before_ok && after_ok {
            return true;
        }
    }
    false
}

fn has_one_panel_loading_title(body: &str) -> bool {
    extract_html_title_text(body)
        .trim()
        .eq_ignore_ascii_case("loading...")
}

async fn has_one_panel_public_favicon(
    client: &reqwest::Client,
    result: &DiscoveryHttpResult,
) -> bool {
    let url = format!("http://{}:{}/public/favicon.png", result.host, result.port);
    let Ok(response) = client
        .get(url)
        .header(reqwest::header::USER_AGENT, DISCOVERY_HTTP_USER_AGENT)
        .header(reqwest::header::CONNECTION, "close")
        .header(reqwest::header::ACCEPT, "image/*,*/*;q=0.8")
        .send()
        .await
    else {
        return false;
    };
    if !response.status().is_success() {
        return false;
    }
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(|value| value.trim().to_ascii_lowercase());
    content_type.is_none_or(|value| {
        value.starts_with("image/")
            || value == "application/octet-stream"
            || value == "binary/octet-stream"
    })
}

fn is_plain_http_to_https_response(status: u16, body: &str) -> bool {
    if status != 400 {
        return false;
    }
    let normalized = body.to_ascii_lowercase();
    normalized.contains("plain http request was sent to https port")
        || normalized.contains("client sent an http request to an https server")
}

fn extract_html_title(body: &str) -> Option<String> {
    let lower = body.to_ascii_lowercase();
    let title_start = lower.find("<title")?;
    let content_start = title_start + lower[title_start..].find('>')? + 1;
    let content_end = content_start + lower[content_start..].find("</title>")?;
    let title = body[content_start..content_end].trim().to_string();
    (!title.is_empty()).then_some(title)
}

fn has_basic_auth_challenge(www_authenticate: Option<&str>) -> bool {
    let Some(www_authenticate) = www_authenticate else {
        return false;
    };
    www_authenticate
        .split(',')
        .map(str::trim_start)
        .any(|part| {
            let lower = part.to_ascii_lowercase();
            lower == "basic" || lower.starts_with("basic ") || lower.starts_with("basic\t")
        })
}

fn build_discover_targets_payload(
    state: &AppState,
    headers: &HeaderMap,
    config: &Value,
    translator: &Translator,
) -> Value {
    let automatic_targets = build_automatic_discover_targets(state, headers, config, translator);
    let scan_discovery = config.get("scan_discovery");
    let custom_targets = build_custom_discover_targets(
        scan_discovery
            .and_then(|value| value.get("custom_cidrs"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|value| value.as_str().map(str::to_string)),
        translator,
    );
    let saved_selected_cidrs = normalize_allowed_scan_cidrs(
        scan_discovery
            .and_then(|value| value.get("selected_cidrs"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|value| value.as_str().map(str::to_string)),
    );
    let automatic_cidrs = automatic_targets
        .iter()
        .filter_map(|item| item.get("cidr").and_then(Value::as_str).map(str::to_string))
        .collect::<Vec<_>>();
    let selection_mode = if saved_selected_cidrs.is_empty() {
        "automatic"
    } else {
        "custom"
    };
    let selected_cidrs = if saved_selected_cidrs.is_empty() {
        automatic_cidrs.clone()
    } else {
        saved_selected_cidrs
    };
    let effective_cidrs = if selected_cidrs.is_empty() {
        automatic_cidrs
    } else {
        selected_cidrs
    };
    let selected_targets = build_saved_discover_targets(effective_cidrs.clone(), translator);
    json!({
        "automaticTargets": automatic_targets,
        "customTargets": custom_targets,
        "selectedTargets": selected_targets,
        "selectionMode": selection_mode,
        "selectedCidrs": effective_cidrs,
        "effectiveCidrs": effective_cidrs,
        "limits": {
            "maxCidrs": MAX_SCAN_CIDRS,
            "maxHosts": MAX_SCAN_HOSTS
        }
    })
}

fn build_automatic_discover_targets(
    state: &AppState,
    headers: &HeaderMap,
    config: &Value,
    translator: &Translator,
) -> Vec<Value> {
    let mut targets = Vec::new();
    if deployment_target(state) == "docker" {
        targets.push(build_docker_discover_target(
            resolve_docker_discover_host(headers),
            translator,
        ));
    } else {
        let cidr = "127.0.0.1/32";
        targets.push(to_discover_target(
            cidr,
            &scan_discovery_target_label(translator, "loopback", &[("cidr", cidr.to_string())]),
            "loopback",
            true,
        ));
    }
    targets.extend(build_interface_discover_targets(translator));
    targets.extend(build_mapping_discover_targets(config, translator));
    dedupe_targets(targets)
}

fn resolve_full_range_discover_cidrs(
    state: &AppState,
    headers: &HeaderMap,
    config: &Value,
    translator: &Translator,
) -> Vec<String> {
    let automatic_targets = build_automatic_discover_targets(state, headers, config, translator);
    normalize_allowed_scan_cidrs(
        std::iter::once(LOOPBACK_DISCOVERY_CIDR.to_string()).chain(
            automatic_targets
                .iter()
                .filter(|target| {
                    matches!(
                        target.get("source").and_then(Value::as_str),
                        Some("loopback" | "interface" | "docker")
                    )
                })
                .filter_map(|target| target.get("cidr").and_then(Value::as_str))
                .map(ToString::to_string),
        ),
    )
}

fn resolve_discover_self_hosts(state: &AppState, headers: &HeaderMap) -> Vec<String> {
    let mut hosts = vec![LOOPBACK_DISCOVERY_HOST.to_string()];
    hosts.extend(
        list_private_ipv4_candidates()
            .into_iter()
            .map(|candidate| candidate.address),
    );
    if deployment_target(state) == "docker"
        && let Some(host) = resolve_docker_discover_host(headers)
    {
        hosts.push(host);
    }
    normalize_discover_self_hosts(hosts)
}

fn normalize_discover_self_hosts(hosts: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut output = Vec::new();
    for host in hosts {
        let host = host.trim().to_string();
        if host.parse::<Ipv4Addr>().is_err() || !is_allowed_scan_ipv4(&host) {
            continue;
        }
        if seen.insert(host.clone()) {
            output.push(host);
        }
    }
    output
}

fn collect_excluded_ports(state: &AppState) -> Vec<u16> {
    let mut ports = Vec::new();
    if !runtime_profile::admin_panel_protected_runtime(state) {
        if let Some(port) = excluded_env_port("ADMIN_VIEW_PORT", 7991) {
            ports.push(port);
        }
    }
    let default_backend_port = if deployment_target(state) == "openwrt" {
        17_998
    } else {
        7_998
    };
    for (name, fallback) in [
        ("BACKEND_PORT", default_backend_port),
        ("AUTH_PORT", 7_997),
        ("GO_BACKEND_PORT", 7_996),
        ("GO_REPROXY_PORT", 7_999),
    ] {
        if let Some(port) = excluded_env_port(name, fallback) {
            ports.push(port);
        }
    }
    ports.extend([7_995, 8_000, 8_200, 30_661, 30_662]);
    ports
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn build_docker_discover_target(ip: Option<String>, translator: &Translator) -> Option<Value> {
    let ip = ip.filter(|value| is_allowed_scan_ipv4(value))?;
    let cidr = build_ipv4_cidr(&ip, 24)?;
    to_discover_target(
        &cidr,
        &scan_discovery_target_label(translator, "docker", &[("cidr", cidr.clone())]),
        "docker",
        true,
    )
}

fn build_interface_discover_targets(translator: &Translator) -> Vec<Option<Value>> {
    list_private_ipv4_candidates()
        .into_iter()
        .filter_map(|candidate| {
            let cidr = build_interface_ipv4_cidr(&candidate.address, candidate.prefix)?;
            Some(to_discover_target(
                &cidr,
                &scan_discovery_target_label(
                    translator,
                    "interface",
                    &[("cidr", cidr.clone()), ("name", candidate.name)],
                ),
                "interface",
                true,
            ))
        })
        .collect()
}

fn build_mapping_discover_targets(config: &Value, translator: &Translator) -> Vec<Option<Value>> {
    let mut targets = Vec::new();
    for key in ["proxy_mappings", "host_mappings"] {
        for mapping in config
            .get(key)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
        {
            let Some(target) = mapping.get("target").and_then(Value::as_str) else {
                continue;
            };
            let Some(ip) = extract_ipv4_from_target(target) else {
                continue;
            };
            let cidr = build_ipv4_cidr(&ip, if ip.starts_with("127.") { 32 } else { 24 });
            if let Some(cidr) = cidr {
                targets.push(to_discover_target(
                    &cidr,
                    &scan_discovery_target_label(translator, "mapping", &[("cidr", cidr.clone())]),
                    "mapping",
                    true,
                ));
            }
        }
    }
    targets
}

fn build_custom_discover_targets(
    cidrs: impl IntoIterator<Item = String>,
    translator: &Translator,
) -> Vec<Value> {
    normalize_allowed_scan_cidrs(cidrs)
        .into_iter()
        .filter_map(|cidr| {
            to_discover_target(
                &cidr,
                &scan_discovery_target_label(translator, "custom", &[("cidr", cidr.clone())]),
                "custom",
                false,
            )
        })
        .collect()
}

fn build_saved_discover_targets(
    cidrs: impl IntoIterator<Item = String>,
    translator: &Translator,
) -> Vec<Value> {
    normalize_allowed_scan_cidrs(cidrs)
        .into_iter()
        .filter_map(|cidr| {
            to_discover_target(
                &cidr,
                &scan_discovery_target_label(translator, "saved", &[("cidr", cidr.clone())]),
                "saved",
                false,
            )
        })
        .collect()
}

fn scan_discovery_target_label(
    translator: &Translator,
    key: &str,
    params: &[(&str, String)],
) -> String {
    translator.t_params(&format!("server.scanDiscovery.targetLabels.{key}"), params)
}

fn localize_scan_discovery_error(translator: &Translator, message: &str) -> String {
    let normalized = message.trim();
    if let Some(cidrs) = normalized.strip_prefix("Only local IPv4 CIDR ranges are supported: ") {
        return translator.t_params(
            "server.scanDiscovery.localIpv4CidrOnly",
            &[("cidrs", cidrs.to_string())],
        );
    }
    if normalized == format!("At most {MAX_SCAN_CIDRS} CIDR ranges can be selected") {
        return translator.t_params(
            "server.scanDiscovery.maxCidrsExceeded",
            &[("max", MAX_SCAN_CIDRS.to_string())],
        );
    }
    if let Some(current) = normalized
        .strip_prefix(&format!(
            "At most {MAX_SCAN_HOSTS} hosts can be scanned, current selection has "
        ))
        .and_then(|value| value.trim().parse::<u64>().ok())
    {
        return translator.t_params(
            "server.scanDiscovery.maxHostsExceededWithCurrent",
            &[
                ("max", MAX_SCAN_HOSTS.to_string()),
                ("current", current.to_string()),
            ],
        );
    }
    if normalized == format!("At most {MAX_SCAN_HOSTS} hosts can be scanned") {
        return translator.t_params(
            "server.scanDiscovery.maxHostsExceeded",
            &[("max", MAX_SCAN_HOSTS.to_string())],
        );
    }
    normalized.to_string()
}

fn expand_scan_cidrs(cidrs: &[String]) -> Vec<String> {
    let mut hosts = Vec::new();
    let mut seen = BTreeSet::new();
    for cidr in cidrs {
        let Some(parsed) = parse_allowed_scan_cidr(cidr) else {
            continue;
        };
        for value in parsed.first_host..=parsed.last_host {
            let host = Ipv4Addr::from(value).to_string();
            if seen.insert(host.clone()) {
                hosts.push(host);
            }
            if hosts.len() as u64 >= MAX_SCAN_HOSTS {
                return hosts;
            }
        }
    }
    hosts
}

fn build_scan_scope(cidrs: &[String]) -> Option<String> {
    if cidrs.is_empty() {
        None
    } else if cidrs.len() == 1 {
        cidrs.first().cloned()
    } else {
        Some(cidrs.join(", "))
    }
}

fn to_discover_target(cidr: &str, label: &str, source: &str, is_automatic: bool) -> Option<Value> {
    let parsed = parse_allowed_scan_cidr(cidr)?;
    Some(json!({
        "cidr": parsed.cidr,
        "label": label,
        "source": source,
        "hostCount": parsed.host_count,
        "isAutomatic": is_automatic
    }))
}

fn dedupe_targets(targets: Vec<Option<Value>>) -> Vec<Value> {
    let mut seen = BTreeSet::new();
    let mut output = Vec::new();
    for target in targets.into_iter().flatten() {
        let cidr = target
            .get("cidr")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if !cidr.is_empty() && seen.insert(cidr) {
            output.push(target);
        }
    }
    output
}

async fn probe_configured_host_mappings(mappings: Vec<Value>, hosts: Vec<String>) -> Vec<Value> {
    let requested_hosts = if hosts.is_empty() {
        None
    } else {
        Some(
            hosts
                .into_iter()
                .map(|host| normalize_host_key(&host))
                .filter(|host| !host.is_empty())
                .collect::<BTreeSet<_>>(),
        )
    };
    let mut target_cache = HashMap::<String, Value>::new();
    let mut results = Vec::new();
    for mapping in mappings {
        let host = mapping
            .get("host")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        let target = mapping
            .get("target")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        if host.is_empty()
            || target.is_empty()
            || is_auth_service_target(&target)
            || requested_hosts
                .as_ref()
                .is_some_and(|set| !set.contains(&normalize_host_key(&host)))
        {
            continue;
        }
        let target_key = normalize_probe_url(&target).unwrap_or_else(|| target.clone());
        let probe = if let Some(cached) = target_cache.get(&target_key) {
            cached.clone()
        } else {
            let result = probe_host_mapping_target(&target).await;
            target_cache.insert(target_key, result.clone());
            result
        };
        let mut result = serde_json::Map::new();
        result.insert("host".to_string(), json!(host));
        result.insert("target".to_string(), json!(target));
        if let Some(object) = probe.as_object() {
            for (key, value) in object {
                result.insert(key.clone(), value.clone());
            }
        }
        results.push(Value::Object(result));
    }
    results
}

async fn probe_host_mapping_target(target: &str) -> Value {
    let started = Instant::now();
    let Some(url) = normalize_probe_url(target) else {
        return json!({
            "status": "unsupported",
            "error": "Only http:// and https:// targets can be probed",
            "latencyMs": 0
        });
    };
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(2500))
        .danger_accept_invalid_certs(true)
        .redirect(reqwest::redirect::Policy::none())
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            return json!({
                "status": "stale",
                "error": error.to_string(),
                "latencyMs": elapsed_ms(started)
            });
        }
    };

    for method in [reqwest::Method::HEAD, reqwest::Method::GET] {
        let is_get = method == reqwest::Method::GET;
        match client
            .request(method, url.as_str())
            .header("User-Agent", "fn-knock-host-mapping-probe/1.0")
            .header("Connection", "close")
            .send()
            .await
        {
            Ok(response) => {
                return json!({
                    "status": "online",
                    "httpStatus": response.status().as_u16(),
                    "latencyMs": elapsed_ms(started)
                });
            }
            Err(error) if is_get => {
                return json!({
                    "status": "stale",
                    "error": error.to_string(),
                    "latencyMs": elapsed_ms(started)
                });
            }
            Err(_) => {}
        }
    }
    json!({
        "status": "stale",
        "error": "Probe failed",
        "latencyMs": elapsed_ms(started)
    })
}

fn elapsed_ms(started: Instant) -> i64 {
    started.elapsed().as_millis().min(i64::MAX as u128) as i64
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or_default()
}

fn normalize_probe_url(target: &str) -> Option<String> {
    let url = Url::parse(target.trim()).ok()?;
    matches!(url.scheme(), "http" | "https").then(|| url.to_string())
}

fn normalize_host_key(value: &str) -> String {
    let lower = value.trim().to_lowercase();
    let without_scheme = strip_alpha_scheme(&lower);
    without_scheme
        .split('/')
        .next()
        .unwrap_or("")
        .trim_end_matches('.')
        .to_string()
}

fn strip_alpha_scheme(value: &str) -> &str {
    let Some((scheme, rest)) = value.split_once("://") else {
        return value;
    };
    if !scheme.is_empty() && scheme.chars().all(|ch| ch.is_ascii_alphabetic()) {
        rest
    } else {
        value
    }
}

fn is_auth_service_target(target: &str) -> bool {
    let Ok(url) = Url::parse(target.trim()) else {
        return false;
    };
    if !matches!(url.scheme(), "http" | "https") {
        return false;
    }
    let port = url.port_or_known_default().unwrap_or(0);
    port == resolve_env_port_with_fallback("AUTH_PORT", 7997)
}

fn normalize_allowed_scan_cidrs(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut output = Vec::new();
    for value in values {
        let Some(parsed) = parse_allowed_scan_cidr(&value) else {
            continue;
        };
        if seen.insert(parsed.cidr.clone()) {
            output.push(parsed.cidr);
        }
    }
    output
}

fn validate_scan_cidrs(values: &[String]) -> Result<Vec<String>, String> {
    let mut output = Vec::new();
    let mut seen = BTreeSet::new();
    let mut invalid = Vec::new();
    for value in values {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Some(parsed) = parse_allowed_scan_cidr(trimmed) else {
            invalid.push(trimmed.to_string());
            continue;
        };
        if seen.insert(parsed.cidr.clone()) {
            output.push(parsed.cidr);
        }
    }
    if !invalid.is_empty() {
        return Err(format!(
            "Only local IPv4 CIDR ranges are supported: {}",
            invalid.into_iter().take(3).collect::<Vec<_>>().join(", ")
        ));
    }
    if output.len() > MAX_SCAN_CIDRS {
        return Err(format!(
            "At most {MAX_SCAN_CIDRS} CIDR ranges can be selected"
        ));
    }
    count_scan_hosts(&output)?;
    Ok(output)
}

fn count_scan_hosts(cidrs: &[String]) -> Result<usize, String> {
    let mut seen = BTreeSet::new();
    for cidr in cidrs {
        let Some(parsed) = parse_allowed_scan_cidr(cidr) else {
            continue;
        };
        for value in parsed.first_host..=parsed.last_host {
            if seen.insert(value) && seen.len() > MAX_SCAN_HOSTS as usize {
                return Err(format!("At most {MAX_SCAN_HOSTS} hosts can be scanned"));
            }
        }
    }
    Ok(seen.len())
}

fn parse_allowed_scan_cidr(value: &str) -> Option<ParsedIpv4Cidr> {
    let parsed = parse_ipv4_cidr(value)?;
    (parsed.host_count > 0 && allowed_scan_range(parsed.first_host, parsed.last_host))
        .then_some(parsed)
}

fn parse_ipv4_cidr(value: &str) -> Option<ParsedIpv4Cidr> {
    let (address, prefix) = value.trim().split_once('/')?;
    let ip = address.trim().parse::<Ipv4Addr>().ok()?;
    let prefix = prefix.trim().parse::<u8>().ok()?;
    if prefix > 32 {
        return None;
    }
    let address_number = u32::from(ip);
    let mask = if prefix == 0 {
        0
    } else {
        u32::MAX.checked_shl((32 - prefix) as u32).unwrap_or(0)
    };
    let network = address_number & mask;
    let host_size = 1_u64.checked_shl((32 - prefix) as u32)?;
    let broadcast = network as u64 + host_size - 1;
    if broadcast > u32::MAX as u64 {
        return None;
    }
    let first_host = if prefix >= 31 { network } else { network + 1 };
    let last_host = if prefix >= 31 {
        broadcast as u32
    } else {
        broadcast as u32 - 1
    };
    let host_count = if prefix >= 31 {
        host_size
    } else {
        host_size.saturating_sub(2)
    };
    Some(ParsedIpv4Cidr {
        cidr: format!("{}/{}", Ipv4Addr::from(network), prefix),
        first_host,
        last_host,
        host_count,
    })
}

fn build_ipv4_cidr(value: &str, prefix: u8) -> Option<String> {
    (prefix <= 32).then_some(())?;
    let ip = value.trim().parse::<Ipv4Addr>().ok()?;
    parse_ipv4_cidr(&format!("{ip}/{prefix}")).map(|parsed| parsed.cidr)
}

fn build_interface_ipv4_cidr(value: &str, prefix: Option<u8>) -> Option<String> {
    let mut candidates = Vec::new();
    if let Some(prefix) = prefix {
        candidates.push(prefix);
    }
    if !candidates.contains(&24) {
        candidates.push(24);
    }
    for candidate in candidates {
        let Some(cidr) = build_ipv4_cidr(value, candidate) else {
            continue;
        };
        if parse_allowed_scan_cidr(&cidr).is_some_and(|parsed| parsed.host_count <= MAX_SCAN_HOSTS)
        {
            return Some(cidr);
        }
    }
    None
}

fn is_allowed_scan_ipv4(value: &str) -> bool {
    let Ok(ip) = value.parse::<Ipv4Addr>() else {
        return false;
    };
    let number = u32::from(ip);
    allowed_ranges()
        .iter()
        .any(|(start, end)| number >= *start && number <= *end)
}

fn allowed_scan_range(first: u32, last: u32) -> bool {
    allowed_ranges()
        .iter()
        .any(|(start, end)| first >= *start && last <= *end)
}

fn allowed_ranges() -> Vec<(u32, u32)> {
    [
        ("127.0.0.0", "127.255.255.255"),
        ("10.0.0.0", "10.255.255.255"),
        ("172.16.0.0", "172.31.255.255"),
        ("192.168.0.0", "192.168.255.255"),
        ("100.64.0.0", "100.127.255.255"),
        ("169.254.0.0", "169.254.255.255"),
    ]
    .into_iter()
    .filter_map(|(start, end)| {
        Some((
            u32::from(start.parse::<Ipv4Addr>().ok()?),
            u32::from(end.parse::<Ipv4Addr>().ok()?),
        ))
    })
    .collect()
}

struct Ipv4Candidate {
    name: String,
    address: String,
    prefix: Option<u8>,
}

fn list_private_ipv4_candidates() -> Vec<Ipv4Candidate> {
    let mut seen = BTreeSet::new();
    let mut output = Vec::new();
    let Ok(addrs) = get_if_addrs() else {
        return output;
    };
    for iface in addrs {
        if is_excluded_interface(&iface.name) || iface.is_loopback() {
            continue;
        }
        let IfAddr::V4(addr) = iface.addr else {
            continue;
        };
        let address = addr.ip.to_string();
        if !is_private_ipv4(addr.ip) || !seen.insert(address.clone()) {
            continue;
        }
        output.push(Ipv4Candidate {
            name: iface.name,
            address,
            prefix: Some(ipv4_prefix_len(addr.netmask) as u8),
        });
    }
    output.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.address.cmp(&right.address))
    });
    output
}

fn is_excluded_interface(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower == "lo"
        || lower.starts_with("docker")
        || lower.starts_with("br-")
        || lower.starts_with("veth")
        || lower.starts_with("tailscale")
        || lower.starts_with("zt")
        || lower.starts_with("tun")
        || lower.starts_with("tap")
        || lower.starts_with("wg")
}

fn is_private_ipv4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    octets[0] == 10
        || (octets[0] == 172 && (16..=31).contains(&octets[1]))
        || (octets[0] == 192 && octets[1] == 168)
}

fn ipv4_prefix_len(mask: Ipv4Addr) -> u32 {
    mask.octets().iter().map(|byte| byte.count_ones()).sum()
}

fn extract_ipv4_from_target(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let host = Url::parse(trimmed)
        .ok()
        .and_then(|url| url.host_str().map(str::to_string))
        .or_else(|| {
            Url::parse(&format!("http://{trimmed}"))
                .ok()
                .and_then(|url| url.host_str().map(str::to_string))
        })?;
    let ip = host.parse::<Ipv4Addr>().ok()?;
    is_allowed_scan_ipv4(&ip.to_string()).then(|| ip.to_string())
}

fn resolve_docker_discover_host(headers: &HeaderMap) -> Option<String> {
    headers
        .get(DOCKER_DISCOVER_IP_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| is_usable_private_discover_ipv4(value))
        .map(str::to_string)
        .or_else(|| {
            env::var("DOCKER_DISCOVER_LAN_IP")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| is_usable_private_discover_ipv4(value))
        })
        .or_else(|| {
            for header in ["x-forwarded-host", "host"] {
                let Some(host) = headers.get(header).and_then(|value| value.to_str().ok()) else {
                    continue;
                };
                for candidate in host.split(',').map(normalize_host_like) {
                    if is_usable_private_discover_ipv4(&candidate) {
                        return Some(candidate);
                    }
                    if let Some(resolved) = resolve_private_ipv4_host(&candidate) {
                        return Some(resolved);
                    }
                }
            }
            None
        })
}

fn normalize_host_like(value: &str) -> String {
    Url::parse(&format!("http://{}", value.trim()))
        .ok()
        .and_then(|url| {
            url.host_str()
                .map(|host| host.trim_matches(['[', ']']).to_lowercase())
        })
        .unwrap_or_else(|| value.trim().trim_matches(['[', ']']).to_lowercase())
}

fn resolve_private_ipv4_host(host: &str) -> Option<String> {
    if host.is_empty() || host == "localhost" || host.parse::<Ipv4Addr>().is_ok() {
        return None;
    }
    (host, 0)
        .to_socket_addrs()
        .ok()?
        .filter_map(|addr| match addr.ip() {
            std::net::IpAddr::V4(ip) if is_usable_private_discover_ipv4(&ip.to_string()) => {
                Some(ip.to_string())
            }
            _ => None,
        })
        .next()
}

fn is_usable_private_discover_ipv4(value: &str) -> bool {
    value.parse::<Ipv4Addr>().is_ok() && is_allowed_scan_ipv4(value) && !value.starts_with("127.")
}

fn deployment_target(state: &AppState) -> String {
    runtime_profile::deployment_target(state)
}

fn resolve_env_port_with_fallback(name: &str, fallback: u16) -> u16 {
    resolve_env_port_with_fallback_value(env::var(name).ok(), fallback)
}

fn resolve_env_port_with_fallback_value(value: Option<String>, fallback: u16) -> u16 {
    let raw = value
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| fallback.to_string());
    parse_js_parse_int_radix_10(raw.trim_start())
        .filter(|port| *port > 0 && *port <= u16::MAX as i64)
        .map(|port| port as u16)
        .unwrap_or(fallback)
}

fn excluded_env_port(name: &str, fallback: u16) -> Option<u16> {
    excluded_env_port_value(env::var(name).ok(), fallback)
}

fn excluded_env_port_value(value: Option<String>, fallback: u16) -> Option<u16> {
    let raw = value
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| fallback.to_string());
    parse_js_parse_int_radix_10(raw.trim_start())
        .filter(|port| *port > 0 && *port <= u16::MAX as i64)
        .map(|port| port as u16)
}

fn parse_js_parse_int_radix_10(value: &str) -> Option<i64> {
    let value = value.trim_start();
    let mut chars = value.char_indices().peekable();
    if matches!(chars.peek(), Some((_, '+' | '-'))) {
        chars.next();
    }
    let mut end = 0;
    let mut has_digit = false;
    for (index, ch) in chars {
        if ch.is_ascii_digit() {
            has_digit = true;
            end = index + ch.len_utf8();
        } else {
            break;
        }
    }
    has_digit
        .then(|| value[..end].parse::<i64>().ok())
        .flatten()
}

fn ensure_object(value: &mut Value) -> &mut serde_json::Map<String, Value> {
    if !value.is_object() {
        *value = json!({});
    }
    value.as_object_mut().expect("value just set to object")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    #[test]
    fn normalizes_allowed_scan_cidrs_like_node() {
        assert_eq!(
            normalize_allowed_scan_cidrs([
                "192.168.1.99/24".to_string(),
                "8.8.8.8/32".to_string(),
                "192.168.1.0/24".to_string(),
            ]),
            vec!["192.168.1.0/24".to_string()]
        );
    }

    #[test]
    fn validates_scan_limits() {
        let cidrs = vec!["10.0.0.0/16".to_string()];
        let error = validate_scan_cidrs(&cidrs).unwrap_err();
        assert!(error.contains("1024"));
    }

    #[test]
    fn validates_scan_host_count_after_dedupe_like_node() {
        let cidrs = vec!["10.0.0.0/22".to_string(), "10.0.0.0/23".to_string()];
        assert_eq!(
            validate_scan_cidrs(&cidrs).unwrap(),
            vec!["10.0.0.0/22".to_string(), "10.0.0.0/23".to_string()]
        );
    }

    #[test]
    fn interface_cidr_prefers_reported_prefix_before_node_fallback() {
        assert_eq!(
            build_interface_ipv4_cidr("192.168.1.2", Some(30)).as_deref(),
            Some("192.168.1.0/30")
        );
        assert_eq!(
            build_interface_ipv4_cidr("192.168.1.2", Some(16)).as_deref(),
            Some("192.168.1.0/24")
        );
    }

    #[test]
    fn expands_scan_cidrs_and_scope() {
        let cidrs = vec!["192.168.1.0/30".to_string()];
        assert_eq!(
            expand_scan_cidrs(&cidrs),
            vec!["192.168.1.1".to_string(), "192.168.1.2".to_string()]
        );
        assert_eq!(build_scan_scope(&cidrs).as_deref(), Some("192.168.1.0/30"));
    }

    #[test]
    fn discovery_host_groups_match_node_port_modes_and_self_skip() {
        let scan_cidrs = vec![
            LOOPBACK_DISCOVERY_CIDR.to_string(),
            "192.168.1.0/30".to_string(),
        ];
        let full_range_cidrs = vec!["192.168.1.0/30".to_string()];
        let self_scan_hosts = vec!["192.168.1.1".to_string()];

        let groups =
            build_discovery_host_groups(&scan_cidrs, &full_range_cidrs, None, &self_scan_hosts);

        assert_eq!(groups.len(), 3);
        assert!(
            groups
                .iter()
                .all(|group| group.mode == DiscoveryPortRangeMode::Full)
        );
        assert_eq!(
            count_discovery_scan_ports_for_groups(&groups, &[]),
            59_920 + 59_920 + 59_921
        );
        assert_eq!(
            build_discovery_port_mode_label(&scan_cidrs, &full_range_cidrs),
            "80-60000"
        );
    }

    #[test]
    fn limited_discovery_uses_node_range_and_excluded_ports() {
        let scan_cidrs = vec!["192.168.2.0/30".to_string()];
        let groups = build_discovery_host_groups(&scan_cidrs, &[], None, &[]);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].mode, DiscoveryPortRangeMode::Limited);
        assert_eq!(groups[0].hosts.len(), 2);
        assert_eq!(
            count_discovery_scan_ports_for_groups(&groups, &[7_999]),
            2 * (9_920 - 1)
        );
        assert_eq!(build_discovery_port_mode_label(&scan_cidrs, &[]), "80-9999");
    }

    #[test]
    fn mixed_discovery_label_matches_node_copy() {
        let scan_cidrs = vec![
            LOOPBACK_DISCOVERY_CIDR.to_string(),
            "192.168.2.0/30".to_string(),
        ];

        assert_eq!(
            build_discovery_port_mode_label(&scan_cidrs, &[]),
            "local=80-60000, other=80-9999"
        );
    }

    #[test]
    fn discovery_port_list_merges_self_and_service_exclusions() {
        let ports = build_port_list(
            limited_discovery_port_range(),
            &merge_discovery_skip_ports(LOCAL_SELF_DISCOVERY_SKIP_PORTS, &[7_999, 7_999]),
        );

        assert_eq!(ports.first().copied(), Some(81));
        assert!(!ports.contains(&7_999));
        assert_eq!(ports.len(), 9_920 - 2);
    }

    #[test]
    fn discovered_generic_http_service_matches_node_fallback_rule() {
        let service = build_discovered_http_service(
            "192.168.31.1",
            8_080,
            200,
            None,
            "<html><title>Login</title></html>",
        )
        .expect("service");

        assert_eq!(
            service.get("serviceKey").and_then(Value::as_str),
            Some("192.168.31.1::http-8080")
        );
        assert_eq!(
            service.pointer("/detail/name").and_then(Value::as_str),
            Some("http-8080")
        );
        assert_eq!(
            service.pointer("/detail/label").and_then(Value::as_str),
            Some("Login")
        );
        assert_eq!(
            service.pointer("/detail/rule/path").and_then(Value::as_str),
            Some("/app-8080")
        );
        assert_eq!(
            service
                .pointer("/detail/rule/strip_path")
                .and_then(Value::as_bool),
            Some(true)
        );
        assert!(service.get("requiresBasicAuth").is_none());
    }

    #[test]
    fn discovered_generic_http_service_uses_node_label_and_basic_auth_rules() {
        let service = build_discovered_http_service(
            "192.168.31.1",
            80,
            302,
            Some("Digest realm=\"admin\", Basic realm=\"admin\""),
            "",
        )
        .expect("service");

        assert_eq!(
            service.pointer("/detail/label").and_then(Value::as_str),
            Some("HTTP 80")
        );
        assert_eq!(
            service.get("requiresBasicAuth").and_then(Value::as_bool),
            Some(true)
        );
    }

    #[tokio::test]
    async fn discovery_analyzer_matches_node_static_rules() {
        let client = reqwest::Client::new();
        let translator = Translator::new("zh-CN");
        let fnos = analyze_discovered_http_service(
            &client,
            DiscoveryHttpResult {
                host: "192.168.31.2".to_string(),
                port: 5666,
                status: 200,
                headers: HashMap::new(),
                body: "<title>飞牛 fnOS</title>".to_string(),
            },
            &translator,
        )
        .await
        .expect("fnos service");
        assert_eq!(
            fnos.pointer("/detail/name").and_then(Value::as_str),
            Some("fnos")
        );
        assert_eq!(
            fnos.pointer("/detail/rule/path").and_then(Value::as_str),
            Some("/fnos")
        );
        assert_eq!(
            fnos.pointer("/detail/isDefault").and_then(Value::as_bool),
            Some(true)
        );

        let mut luci_headers = HashMap::new();
        luci_headers.insert("x-luci-login-required".to_string(), "yes".to_string());
        let openwrt = analyze_discovered_http_service(
            &client,
            DiscoveryHttpResult {
                host: "192.168.31.1".to_string(),
                port: 80,
                status: 403,
                headers: luci_headers,
                body: String::new(),
            },
            &translator,
        )
        .await
        .expect("openwrt service");
        assert_eq!(
            openwrt.pointer("/detail/name").and_then(Value::as_str),
            Some("openwrt")
        );
        assert_eq!(
            openwrt.pointer("/detail/rule/path").and_then(Value::as_str),
            Some("/openwrt")
        );

        let mut webdav_headers = HashMap::new();
        webdav_headers.insert(
            "www-authenticate".to_string(),
            "Basic realm=\"Restricted\"".to_string(),
        );
        let webdav = analyze_discovered_http_service(
            &client,
            DiscoveryHttpResult {
                host: "192.168.31.3".to_string(),
                port: 5005,
                status: 401,
                headers: webdav_headers,
                body: String::new(),
            },
            &translator,
        )
        .await
        .expect("webdav service");
        assert_eq!(
            webdav.pointer("/detail/name").and_then(Value::as_str),
            Some("webdav")
        );
        assert_eq!(
            webdav.get("requiresBasicAuth").and_then(Value::as_bool),
            Some(true)
        );
    }

    #[tokio::test]
    async fn discovery_analyzer_matches_all_node_static_rule_shapes() {
        struct StaticRuleCase {
            case_name: &'static str,
            port: u16,
            status: u16,
            headers: Vec<(&'static str, &'static str)>,
            body: &'static str,
            expected_name: &'static str,
            expected_path: &'static str,
            expected_rewrite_html: bool,
            expected_use_root_mode: bool,
            expected_is_default: bool,
        }

        let client = reqwest::Client::new();
        let translator = Translator::new("zh-CN");
        let cases = vec![
            StaticRuleCase {
                case_name: "mongo-express",
                port: 8081,
                status: 200,
                headers: vec![("set-cookie", "mongo-express=sid")],
                body: "",
                expected_name: "mongoexpress",
                expected_path: "/mongoe",
                expected_rewrite_html: true,
                expected_use_root_mode: false,
                expected_is_default: false,
            },
            StaticRuleCase {
                case_name: "redis-insight",
                port: 5540,
                status: 200,
                headers: vec![],
                body: "<html><title>Redis Insight</title></html>",
                expected_name: "redisinsight",
                expected_path: "/redisi",
                expected_rewrite_html: false,
                expected_use_root_mode: true,
                expected_is_default: false,
            },
            StaticRuleCase {
                case_name: "go2rtc",
                port: 1984,
                status: 200,
                headers: vec![],
                body: "<html><title>go2rtc</title></html>",
                expected_name: "go2rtc",
                expected_path: "/go2rtc",
                expected_rewrite_html: true,
                expected_use_root_mode: false,
                expected_is_default: false,
            },
            StaticRuleCase {
                case_name: "openwrt",
                port: 80,
                status: 403,
                headers: vec![("x-luci-login-required", "yes")],
                body: "",
                expected_name: "openwrt",
                expected_path: "/openwrt",
                expected_rewrite_html: false,
                expected_use_root_mode: true,
                expected_is_default: false,
            },
            StaticRuleCase {
                case_name: "fnos",
                port: 5666,
                status: 200,
                headers: vec![],
                body: "<html><title>飞牛 fnOS</title></html>",
                expected_name: "fnos",
                expected_path: "/fnos",
                expected_rewrite_html: false,
                expected_use_root_mode: true,
                expected_is_default: true,
            },
            StaticRuleCase {
                case_name: "lucky",
                port: 16601,
                status: 200,
                headers: vec![],
                body: "<html><title>Lucky</title></html>",
                expected_name: "lucky",
                expected_path: "/lucky",
                expected_rewrite_html: true,
                expected_use_root_mode: false,
                expected_is_default: false,
            },
            StaticRuleCase {
                case_name: "homeassistant",
                port: 8123,
                status: 200,
                headers: vec![],
                body: "<html><title>Home Assistant</title></html>",
                expected_name: "homeassistant",
                expected_path: "/ha",
                expected_rewrite_html: true,
                expected_use_root_mode: false,
                expected_is_default: false,
            },
            StaticRuleCase {
                case_name: "sun-panel",
                port: 3002,
                status: 200,
                headers: vec![],
                body: "<html><title>Sun-Panel</title></html>",
                expected_name: "sun-panel",
                expected_path: "/sp",
                expected_rewrite_html: true,
                expected_use_root_mode: true,
                expected_is_default: false,
            },
            StaticRuleCase {
                case_name: "webdav",
                port: 5005,
                status: 401,
                headers: vec![("www-authenticate", "Basic realm=\"Restricted\"")],
                body: "",
                expected_name: "webdav",
                expected_path: "/webdav",
                expected_rewrite_html: true,
                expected_use_root_mode: false,
                expected_is_default: false,
            },
            StaticRuleCase {
                case_name: "xunlei",
                port: 2345,
                status: 200,
                headers: vec![],
                body: "<html><title>迅雷下载</title></html>",
                expected_name: "xunlei",
                expected_path: "/xunlei",
                expected_rewrite_html: true,
                expected_use_root_mode: false,
                expected_is_default: false,
            },
            StaticRuleCase {
                case_name: "minidlna",
                port: 8200,
                status: 200,
                headers: vec![],
                body: "<HTML><TITLE>MiniDLNA status</TITLE></HTML>",
                expected_name: "miniDLNA",
                expected_path: "/dlna",
                expected_rewrite_html: true,
                expected_use_root_mode: false,
                expected_is_default: false,
            },
            StaticRuleCase {
                case_name: "nowen",
                port: 8080,
                status: 200,
                headers: vec![],
                body: "<html><title>Digital Zen Garden</title></html>",
                expected_name: "nowen",
                expected_path: "/nowen",
                expected_rewrite_html: false,
                expected_use_root_mode: true,
                expected_is_default: true,
            },
            StaticRuleCase {
                case_name: "fnys",
                port: 5667,
                status: 200,
                headers: vec![],
                body: "<html><title>飞牛影视</title></html>",
                expected_name: "fnys",
                expected_path: "/v",
                expected_rewrite_html: false,
                expected_use_root_mode: true,
                expected_is_default: false,
            },
            StaticRuleCase {
                case_name: "dpanel",
                port: 8807,
                status: 200,
                headers: vec![],
                body: "<script src=\"/dpanel/ui/main.js\"></script>",
                expected_name: "DPanel",
                expected_path: "/dp",
                expected_rewrite_html: false,
                expected_use_root_mode: true,
                expected_is_default: false,
            },
            StaticRuleCase {
                case_name: "lottery",
                port: 8088,
                status: 200,
                headers: vec![],
                body: "<html><title>彩票助手</title></html>",
                expected_name: "cpzs",
                expected_path: "/cpzs",
                expected_rewrite_html: false,
                expected_use_root_mode: true,
                expected_is_default: false,
            },
            StaticRuleCase {
                case_name: "kuake",
                port: 5005,
                status: 200,
                headers: vec![],
                body: "<html><title>登录</title></html>",
                expected_name: "Kuake",
                expected_path: "/kuake",
                expected_rewrite_html: false,
                expected_use_root_mode: true,
                expected_is_default: false,
            },
            StaticRuleCase {
                case_name: "jellyfin",
                port: 8096,
                status: 200,
                headers: vec![],
                body: "<html><title>Jellyfin</title></html>",
                expected_name: "Jellyfin",
                expected_path: "/jellyfin",
                expected_rewrite_html: false,
                expected_use_root_mode: true,
                expected_is_default: false,
            },
            StaticRuleCase {
                case_name: "mefrp",
                port: 7000,
                status: 200,
                headers: vec![],
                body: "<html><title>WebUI 登录 | ME Frp</title></html>",
                expected_name: "ME Frp",
                expected_path: "/mefrp",
                expected_rewrite_html: false,
                expected_use_root_mode: true,
                expected_is_default: false,
            },
            StaticRuleCase {
                case_name: "moontv",
                port: 3000,
                status: 200,
                headers: vec![],
                body: "<html><title>MoonTV</title></html>",
                expected_name: "MoonTV",
                expected_path: "/moontv",
                expected_rewrite_html: false,
                expected_use_root_mode: true,
                expected_is_default: false,
            },
            StaticRuleCase {
                case_name: "fnosapps",
                port: 3180,
                status: 200,
                headers: vec![],
                body: "<html><title>fnOS Apps</title></html>",
                expected_name: "fnOS Apps",
                expected_path: "/fnosapps",
                expected_rewrite_html: false,
                expected_use_root_mode: true,
                expected_is_default: false,
            },
            StaticRuleCase {
                case_name: "emby",
                port: 8097,
                status: 200,
                headers: vec![],
                body: "<script src=\"emby-elements/emby-collapse/emby-collapse.js\"></script>",
                expected_name: "Emby",
                expected_path: "/emby",
                expected_rewrite_html: false,
                expected_use_root_mode: true,
                expected_is_default: false,
            },
            StaticRuleCase {
                case_name: "dlymusic",
                port: 4567,
                status: 200,
                headers: vec![],
                body: "<html><title>道理鱼音乐管理</title></html>",
                expected_name: "DLYMusic",
                expected_path: "/music",
                expected_rewrite_html: false,
                expected_use_root_mode: true,
                expected_is_default: false,
            },
        ];

        for case in cases {
            let service = analyze_discovered_http_service(
                &client,
                DiscoveryHttpResult {
                    host: "192.168.31.2".to_string(),
                    port: case.port,
                    status: case.status,
                    headers: case
                        .headers
                        .into_iter()
                        .map(|(key, value)| (key.to_string(), value.to_string()))
                        .collect(),
                    body: case.body.to_string(),
                },
                &translator,
            )
            .await
            .unwrap_or_else(|| panic!("{} should match", case.case_name));

            assert_eq!(
                service.pointer("/detail/name").and_then(Value::as_str),
                Some(case.expected_name),
                "{} name",
                case.case_name
            );
            assert_eq!(
                service.pointer("/detail/rule/path").and_then(Value::as_str),
                Some(case.expected_path),
                "{} path",
                case.case_name
            );
            assert_eq!(
                service
                    .pointer("/detail/rule/rewrite_html")
                    .and_then(Value::as_bool),
                Some(case.expected_rewrite_html),
                "{} rewrite_html",
                case.case_name
            );
            assert_eq!(
                service
                    .pointer("/detail/rule/use_root_mode")
                    .and_then(Value::as_bool),
                Some(case.expected_use_root_mode),
                "{} use_root_mode",
                case.case_name
            );
            assert_eq!(
                service
                    .pointer("/detail/isDefault")
                    .and_then(Value::as_bool),
                Some(case.expected_is_default),
                "{} isDefault",
                case.case_name
            );
        }
    }

    #[tokio::test]
    async fn discovery_analyzer_fetches_alist_public_settings_like_node() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            for _ in 0..2 {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                tokio::spawn(async move {
                    let mut buffer = [0_u8; 2048];
                    let Ok(read_len) = socket.read(&mut buffer).await else {
                        return;
                    };
                    let request = String::from_utf8_lossy(&buffer[..read_len]);
                    let path = request
                        .lines()
                        .next()
                        .and_then(|line| line.split_whitespace().nth(1))
                        .unwrap_or("/");
                    let (content_type, body) = if path == "/api/public/settings" {
                        (
                            "application/json",
                            br#"{"code":200,"data":{"site_title":"OpenList","version":"1.0"}}"#
                                .to_vec(),
                        )
                    } else {
                        (
                            "text/html",
                            br#"<html><title>OpenList</title></html>"#.to_vec(),
                        )
                    };
                    let header = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = socket.write_all(header.as_bytes()).await;
                    let _ = socket.write_all(&body).await;
                });
            }
        });

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap();
        let service = probe_discovery_service(
            &client,
            &client,
            "127.0.0.1",
            addr.port(),
            &Translator::new("zh-CN"),
        )
        .await
        .expect("openlist service");
        assert_eq!(
            service.pointer("/detail/name").and_then(Value::as_str),
            Some("openlist")
        );
        assert_eq!(
            service.pointer("/detail/rule/path").and_then(Value::as_str),
            Some("/op")
        );
    }

    #[tokio::test]
    async fn discovery_probe_retries_manual_redirect_like_node() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let request_count = Arc::new(AtomicUsize::new(0));
        let request_count_for_server = request_count.clone();
        tokio::spawn(async move {
            for _ in 0..2 {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                let request_count = request_count_for_server.clone();
                tokio::spawn(async move {
                    request_count.fetch_add(1, Ordering::SeqCst);
                    let mut buffer = [0_u8; 2048];
                    let _ = socket.read(&mut buffer).await;
                    let body = br#"<html><title>Redirect Login</title></html>"#;
                    let header = format!(
                        "HTTP/1.1 302 Found\r\nLocation: /next\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = socket.write_all(header.as_bytes()).await;
                    let _ = socket.write_all(body).await;
                });
            }
        });

        let follow_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                attempt.error("redirect blocked for test")
            }))
            .build()
            .unwrap();
        let manual_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        let service = probe_discovery_service(
            &follow_client,
            &manual_client,
            "127.0.0.1",
            addr.port(),
            &Translator::new("zh-CN"),
        )
        .await
        .expect("manual redirect fallback service");

        let expected_name = format!("http-{}", addr.port());
        assert_eq!(request_count.load(Ordering::SeqCst), 2);
        assert_eq!(
            service.pointer("/detail/name").and_then(Value::as_str),
            Some(expected_name.as_str())
        );
        assert_eq!(
            service.pointer("/detail/label").and_then(Value::as_str),
            Some("Redirect Login")
        );
    }

    #[tokio::test]
    async fn discovery_analyzer_detects_onepanel_public_favicon_like_node() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            for _ in 0..2 {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                tokio::spawn(async move {
                    let mut buffer = [0_u8; 2048];
                    let Ok(read_len) = socket.read(&mut buffer).await else {
                        return;
                    };
                    let request = String::from_utf8_lossy(&buffer[..read_len]);
                    let path = request
                        .lines()
                        .next()
                        .and_then(|line| line.split_whitespace().nth(1))
                        .unwrap_or("/");
                    let (content_type, body) = if path == "/public/favicon.png" {
                        ("application/octet-stream", vec![1, 2, 3])
                    } else {
                        (
                            "text/html",
                            br#"<html><title>loading...</title></html>"#.to_vec(),
                        )
                    };
                    let header = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = socket.write_all(header.as_bytes()).await;
                    let _ = socket.write_all(&body).await;
                });
            }
        });

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap();
        let service = probe_discovery_service(
            &client,
            &client,
            "127.0.0.1",
            addr.port(),
            &Translator::new("zh-CN"),
        )
        .await
        .expect("1panel service");
        assert_eq!(
            service.pointer("/detail/name").and_then(Value::as_str),
            Some("1Panel")
        );
        assert_eq!(
            service.pointer("/detail/rule/path").and_then(Value::as_str),
            Some("/1panel")
        );
    }

    #[test]
    fn discovered_services_keep_lowest_port_for_same_service_key_like_node() {
        let job = Arc::new(Mutex::new(DiscoverJob {
            id: "job".to_string(),
            cancel: Arc::new(AtomicBool::new(false)),
            created_at: now_millis(),
            updated_at: now_millis(),
            state: "running".to_string(),
            meta: Some(json!({ "foundServices": 0 })),
            progress: None,
            service_events: Vec::new(),
            service_map: Vec::new(),
            result: None,
            error: None,
        }));
        push_discovered_service(
            &job,
            json!({ "serviceKey": "host::fnos", "port": 5666, "detail": { "name": "fnos" } }),
        );
        push_discovered_service(
            &job,
            json!({ "serviceKey": "host::fnos", "port": 80, "detail": { "name": "fnos" } }),
        );
        push_discovered_service(
            &job,
            json!({ "serviceKey": "host::fnos", "port": 8080, "detail": { "name": "fnos" } }),
        );

        let locked = job.lock().unwrap();
        assert_eq!(locked.service_events.len(), 2);
        assert_eq!(
            locked.service_events[0].get("port").and_then(Value::as_u64),
            Some(5666)
        );
        assert_eq!(
            locked.service_events[1].get("port").and_then(Value::as_u64),
            Some(80)
        );
        assert_eq!(locked.service_map.len(), 1);
        assert_eq!(
            locked.service_map[0].1.get("port").and_then(Value::as_u64),
            Some(80)
        );
        assert_eq!(
            locked
                .meta
                .as_ref()
                .and_then(|value| value.pointer("/foundServices"))
                .and_then(Value::as_u64),
            Some(1)
        );
    }

    #[test]
    fn plain_http_to_https_port_response_is_not_discoverable_like_node() {
        assert!(
            build_discovered_http_service(
                "192.168.31.1",
                443,
                400,
                None,
                "<title>400 The plain HTTP request was sent to HTTPS port</title>",
            )
            .is_none()
        );
        assert!(
            build_discovered_http_service(
                "192.168.31.1",
                8443,
                400,
                None,
                "Client sent an HTTP request to an HTTPS server",
            )
            .is_none()
        );
    }

    #[test]
    fn serializes_discover_job_from_cursor() {
        let job = Arc::new(Mutex::new(DiscoverJob {
            id: "job-1".to_string(),
            cancel: Arc::new(AtomicBool::new(false)),
            created_at: 10,
            updated_at: 20,
            state: "running".to_string(),
            meta: Some(json!({ "foundServices": 2 })),
            progress: Some(json!({ "scannedPorts": 1 })),
            service_events: vec![json!({ "port": 80 }), json!({ "port": 443 })],
            service_map: Vec::new(),
            result: None,
            error: None,
        }));
        let data = serialize_discover_job(&job, Some("1"));

        assert_eq!(data.get("jobId").and_then(Value::as_str), Some("job-1"));
        assert_eq!(data.get("nextCursor").and_then(Value::as_u64), Some(2));
        assert_eq!(
            data.pointer("/services/0/port").and_then(Value::as_u64),
            Some(443)
        );
    }

    #[test]
    fn service_cursor_parser_matches_node_parse_int() {
        assert_eq!(normalize_service_cursor(Some("1x"), 5), 1);
        assert_eq!(normalize_service_cursor(Some("  +2.9"), 5), 2);
        assert_eq!(normalize_service_cursor(Some("-1"), 5), 0);
        assert_eq!(normalize_service_cursor(Some("0x10"), 5), 0);
        assert_eq!(normalize_service_cursor(Some("99"), 5), 5);
        assert_eq!(normalize_service_cursor(Some("nope"), 5), 0);
    }

    #[test]
    fn localizes_scan_discovery_route_text() {
        let translator = Translator::new("zh-CN");
        assert_eq!(
            translator.t("server.scanDiscovery.loadTargetsFailed"),
            "读取扫描目标失败"
        );
        assert_eq!(translator.t("server.apiPathNotFound"), "接口不存在");
    }

    #[test]
    fn localizes_scan_discovery_validation_errors() {
        let translator = Translator::new("zh-CN");

        assert_eq!(
            localize_scan_discovery_error(
                &translator,
                "Only local IPv4 CIDR ranges are supported: 8.8.8.0/24",
            ),
            "扫描网段仅支持本地 IPv4 CIDR：8.8.8.0/24"
        );
        assert_eq!(
            localize_scan_discovery_error(
                &translator,
                "At most 1024 hosts can be scanned, current selection has 65534",
            ),
            "单次最多扫描 1024 台主机，当前为 65534 台"
        );
    }

    #[test]
    fn localizes_scan_discovery_target_labels() {
        let translator = Translator::new("zh-CN");

        assert_eq!(
            scan_discovery_target_label(
                &translator,
                "custom",
                &[("cidr", "192.168.1.0/24".to_string())],
            ),
            "192.168.1.0/24（自定义）"
        );
        assert_eq!(
            scan_discovery_target_label(
                &translator,
                "interface",
                &[
                    ("cidr", "192.168.2.0/24".to_string()),
                    ("name", "en0".to_string()),
                ],
            ),
            "192.168.2.0/24（en0）"
        );
        assert_eq!(
            build_custom_discover_targets(["192.168.3.99/24".to_string()], &translator)
                .first()
                .and_then(|target| target.get("label"))
                .and_then(Value::as_str),
            Some("192.168.3.0/24（自定义）")
        );
        assert_eq!(
            build_saved_discover_targets(["10.0.0.0/24".to_string()], &translator)
                .first()
                .and_then(|target| target.get("label"))
                .and_then(Value::as_str),
            Some("10.0.0.0/24（已保存）")
        );
    }

    #[test]
    fn extracts_mapping_ipv4_targets() {
        assert_eq!(
            extract_ipv4_from_target("http://192.168.2.10:8080/app"),
            Some("192.168.2.10".to_string())
        );
        assert_eq!(extract_ipv4_from_target("https://example.com"), None);
    }

    #[test]
    fn normalizes_host_mapping_probe_keys_like_node() {
        assert_eq!(
            normalize_host_key("HTTPS://Example.COM:8443/path?q=1."),
            "example.com:8443"
        );
        assert_eq!(normalize_host_key("Example.COM."), "example.com");
        assert_eq!(normalize_host_key("1://Example.COM/path"), "1:");
        assert_eq!(
            normalize_host_key("[2001:db8::1]:8443"),
            "[2001:db8::1]:8443"
        );
    }

    #[test]
    fn docker_discover_ip_filter_excludes_loopback_like_node() {
        assert!(is_usable_private_discover_ipv4("192.168.31.10"));
        assert!(is_usable_private_discover_ipv4("100.64.1.2"));
        assert!(!is_usable_private_discover_ipv4("127.0.0.1"));
        assert!(!is_usable_private_discover_ipv4("8.8.8.8"));
    }

    #[test]
    fn scan_excluded_env_ports_match_node_truthy_parse_int() {
        assert_eq!(excluded_env_port_value(None, 7_997), Some(7_997));
        assert_eq!(
            excluded_env_port_value(Some(String::new()), 7_997),
            Some(7_997)
        );
        assert_eq!(
            excluded_env_port_value(Some(" 8080x ".to_string()), 7_997),
            Some(8080)
        );
        assert_eq!(
            excluded_env_port_value(Some("abc".to_string()), 7_997),
            None
        );
        assert_eq!(
            excluded_env_port_value(Some("0x10".to_string()), 7_997),
            None
        );
        assert_eq!(
            resolve_env_port_with_fallback_value(Some(" 8080x ".to_string()), 7_997),
            8080
        );
        assert_eq!(
            resolve_env_port_with_fallback_value(Some("abc".to_string()), 7_997),
            7_997
        );
    }

    #[test]
    fn detects_auth_service_target_by_port() {
        unsafe {
            env::set_var("AUTH_PORT", "7997");
        }
        assert!(is_auth_service_target("http://127.0.0.1:7997"));
        assert!(!is_auth_service_target("ws://127.0.0.1:7997"));
        assert!(!is_auth_service_target("http://127.0.0.1:8080"));
    }
}
