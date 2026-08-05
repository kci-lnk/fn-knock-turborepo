use std::{
    cmp::Ordering,
    collections::HashSet,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    str::FromStr,
    time::{Duration, Instant},
};

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use ipnet::Ipv4Net;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tokio::{task::JoinSet, time};

use crate::{crypto_utils, response, state::AppState, time_utils};

use super::{
    cloudflare_api::{CloudflareApi, CloudflareApiError},
    managed::{
        ManagedDnsRequest, api_for_background, configured_hosts, dns_record_owned_for_update,
        load_managed_config, load_managed_state, managed_instance_id, managed_root_domain,
        save_managed_config, save_managed_state, upsert_managed_dns,
    },
};

const OPTIMIZATION_RUNTIME_KEY: &str = "fn_knock:cloudflared:optimization:runtime:v1";
const SPEEDTEST_HOST: &str = "speed.cloudflare.com";
const SPEEDTEST_PATH: &str = "/__down";
const MAX_CANDIDATES: usize = 128;
const CANDIDATES_PER_PREFIX: usize = 8;
const PROBE_CONCURRENCY: usize = 32;
const SNI_VALIDATION_CONCURRENCY: usize = 16;
const LATENCY_PROBES: usize = 3;
const DOWNLOAD_SHORTLIST: usize = 8;
const DOWNLOAD_BYTES: usize = 1024 * 1024;
const MAX_DOWNLOAD_BUDGET: usize = 20 * 1024 * 1024;
const _: () = {
    assert!(MAX_CANDIDATES <= 128);
    assert!(PROBE_CONCURRENCY <= 32);
    assert!(LATENCY_PROBES == 3);
    assert!(DOWNLOAD_SHORTLIST * 2 * DOWNLOAD_BYTES <= MAX_DOWNLOAD_BUDGET);
};
const MAX_CUSTOM_HOSTNAMES: usize = 100;
const MAX_CUSTOM_HOSTNAME_CREATES_PER_RECONCILE: usize = 10;
const WEEK_MS: i64 = 7 * 24 * 60 * 60 * 1000;
const HEALTH_INTERVAL_MS: i64 = 15 * 60 * 1000;
const CONFIRMATION_DELAY_MS: i64 = 10 * 60 * 1000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CapabilityProbeResult {
    Ready,
    Pending,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FallbackOriginResult {
    Ready,
    Pending,
}

const CLOUDFLARE_IPV4_FALLBACK: &[&str] = &[
    "103.21.244.0/22",
    "103.22.200.0/22",
    "103.31.4.0/22",
    "104.16.0.0/13",
    "104.24.0.0/14",
    "108.162.192.0/18",
    "131.0.72.0/22",
    "141.101.64.0/18",
    "162.158.0.0/15",
    "172.64.0.0/13",
    "173.245.48.0/20",
    "188.114.96.0/20",
    "190.93.240.0/20",
    "197.234.240.0/22",
    "198.41.128.0/17",
];

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct OptimizationCandidate {
    ip: String,
    median_latency_ms: f64,
    jitter_ms: f64,
    loss_ratio: f64,
    download_mbps: f64,
    score: f64,
    #[serde(default)]
    verified_at: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApplyOptimizationRequest {
    scan_id: String,
    #[serde(default)]
    candidate_ip: Option<String>,
}

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/admin/cloudflared/optimization/scans",
            post(start_scan),
        )
        .route(
            "/api/admin/cloudflared/optimization/scans/{id}",
            get(get_scan).delete(cancel_scan),
        )
        .route(
            "/api/admin/cloudflared/optimization/apply",
            post(apply_optimization),
        )
        .route(
            "/api/admin/cloudflared/optimization/fallback",
            post(fallback_optimization),
        )
}

pub(super) fn start_tasks(state: AppState) {
    tokio::spawn(async move {
        let mut interval = time::interval(super::managed::plan_wakeup_delay());
        interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = state.shutdown.cancelled() => break,
                _ = interval.tick() => {},
                _ = state.cloudflared_schedule_notify.notified() => {},
            }
            if let Err(error) = scheduled_tick(&state).await {
                tracing::warn!(%error, "Cloudflare optimization scheduler failed");
                let mut runtime = load_runtime(&state).await;
                ensure_object(&mut runtime)
                    .insert("lastError".to_string(), json!(error.to_string()));
                let _ = save_runtime(&state, &runtime).await;
            }
        }
    });
}

pub(super) fn schedule_after_host_mappings_change(state: AppState) {
    tokio::spawn(async move {
        let managed = load_managed_config(&state).await;
        if managed.get("mode").and_then(Value::as_str) != Some("managed") {
            return;
        }
        state.cloudflared_schedule_notify.notify_one();
    });
}

async fn start_scan(State(state): State<AppState>) -> Response {
    let managed = load_managed_config(&state).await;
    if !optimization_is_enabled(&managed) {
        return response::error(
            StatusCode::CONFLICT,
            "Enable optimization by previewing and applying a Cloudflare reconcile plan before starting a speed test",
        );
    }
    let id = uuid::Uuid::new_v4().to_string();
    let job = json!({
        "id": id,
        "status": "queued",
        "phase": "queued",
        "progress": 0,
        "createdAt": time_utils::now_iso(),
        "startedAt": Value::Null,
        "completedAt": Value::Null,
        "cancelRequested": false,
        "candidates": [],
        "recommendedIp": Value::Null,
        "error": Value::Null,
    });
    {
        let mut jobs = state.cloudflared_scan_jobs.write().await;
        if let Some(existing) = jobs.values().find(|job| scan_job_active(job)) {
            return response::error(
                StatusCode::CONFLICT,
                format!(
                    "Optimization scan {} is already running",
                    existing.get("id").and_then(Value::as_str).unwrap_or("")
                ),
            );
        }
        if jobs.len() >= 20 {
            let oldest = jobs
                .iter()
                .filter(|(_, value)| !scan_job_active(value))
                .min_by_key(|(_, value)| {
                    value
                        .get("createdAt")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string()
                })
                .map(|(id, _)| id.clone());
            if let Some(oldest) = oldest {
                jobs.remove(&oldest);
            }
        }
        jobs.insert(id.clone(), job.clone());
    }
    let scan_state = state.clone();
    let scan_id = id.clone();
    tokio::spawn(async move {
        let _scan_guard = scan_state.cloudflared_scan_lock.lock().await;
        update_job(
            &scan_state,
            &scan_id,
            json!({
                "status": "running",
                "phase": "latency",
                "progress": 1,
                "startedAt": time_utils::now_iso(),
            }),
        )
        .await;
        let result = time::timeout(
            Duration::from_secs(180),
            run_scan(&scan_state, Some(&scan_id)),
        )
        .await;
        match result {
            Ok(Ok(_candidates)) if is_job_cancelled(&scan_state, &scan_id).await => {
                update_job(
                    &scan_state,
                    &scan_id,
                    json!({
                        "status": "cancelled",
                        "phase": "cancelled",
                        "completedAt": time_utils::now_iso(),
                    }),
                )
                .await;
            }
            Ok(Ok(candidates)) => {
                let recommended = candidates.first().map(|candidate| candidate.ip.clone());
                update_job(
                    &scan_state,
                    &scan_id,
                    json!({
                        "status": "completed",
                        "phase": "completed",
                        "progress": 100,
                        "completedAt": time_utils::now_iso(),
                        "candidates": candidates,
                        "recommendedIp": recommended,
                    }),
                )
                .await;
            }
            Ok(Err(error)) => {
                update_job(
                    &scan_state,
                    &scan_id,
                    json!({
                        "status": "failed",
                        "phase": "failed",
                        "completedAt": time_utils::now_iso(),
                        "error": error.to_string(),
                    }),
                )
                .await;
            }
            Err(_) => {
                update_job(
                    &scan_state,
                    &scan_id,
                    json!({
                        "status": "failed",
                        "phase": "failed",
                        "completedAt": time_utils::now_iso(),
                        "error": "Optimization scan exceeded the three-minute limit",
                    }),
                )
                .await;
            }
        }
    });
    response::ok(job).into_response()
}

async fn get_scan(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    match state.cloudflared_scan_jobs.read().await.get(&id).cloned() {
        Some(job) => response::ok(job).into_response(),
        None => response::error(StatusCode::NOT_FOUND, "Optimization scan was not found"),
    }
}

async fn cancel_scan(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let mut jobs = state.cloudflared_scan_jobs.write().await;
    let Some(job) = jobs.get_mut(&id) else {
        return response::error(StatusCode::NOT_FOUND, "Optimization scan was not found");
    };
    ensure_object(job).insert("cancelRequested".to_string(), json!(true));
    response::success_empty().into_response()
}

async fn apply_optimization(
    State(state): State<AppState>,
    Json(body): Json<ApplyOptimizationRequest>,
) -> Response {
    let job = match state
        .cloudflared_scan_jobs
        .read()
        .await
        .get(body.scan_id.trim())
        .cloned()
    {
        Some(job) if job.get("status").and_then(Value::as_str) == Some("completed") => job,
        Some(_) => {
            return response::error(StatusCode::CONFLICT, "Optimization scan is not complete");
        }
        None => return response::error(StatusCode::NOT_FOUND, "Optimization scan was not found"),
    };
    let candidates = serde_json::from_value::<Vec<OptimizationCandidate>>(
        job.get("candidates").cloned().unwrap_or_else(|| json!([])),
    )
    .unwrap_or_default();
    let requested = body
        .candidate_ip
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| job.get("recommendedIp").and_then(Value::as_str));
    let Some(candidate) = requested.and_then(|ip| candidates.iter().find(|item| item.ip == ip))
    else {
        return response::error(
            StatusCode::BAD_REQUEST,
            "Select a candidate returned by the completed scan",
        );
    };
    let _guard = state.cloudflared_manage_lock.lock().await;
    let mut managed = load_managed_config(&state).await;
    if !optimization_is_enabled(&managed) {
        return response::error(
            StatusCode::CONFLICT,
            "Enable optimization by previewing and applying a Cloudflare reconcile plan before publishing a candidate",
        );
    }
    let api = match api_for_background(&state).await {
        Ok(Some(api)) => api,
        Ok(None) => {
            return response::error(
                StatusCode::CONFLICT,
                "Cloudflare API Token is not configured",
            );
        }
        Err(error) => return api_error_response(error),
    };
    let mut ownership = load_managed_state(&state).await;
    let previous_selected = ownership.pointer("/optimization/selected").cloned();
    let previous_fallback = ownership.pointer("/optimization/fallbackActive").cloned();
    let previous_publish_suppressed = ownership
        .pointer("/optimization/publishSuppressed")
        .cloned();
    let selected = json!({
        "ip": candidate.ip,
        "medianLatencyMs": candidate.median_latency_ms,
        "jitterMs": candidate.jitter_ms,
        "lossRatio": candidate.loss_ratio,
        "downloadMbps": candidate.download_mbps,
        "score": candidate.score,
        "selectedAt": time_utils::now_iso(),
        "source": "manual",
    });
    ensure_nested_object(&mut ownership, &["optimization"])
        .insert("selected".to_string(), selected.clone());
    ensure_nested_object(&mut ownership, &["optimization"])
        .insert("fallbackActive".to_string(), json!(false));
    ensure_nested_object(&mut ownership, &["optimization"])
        .insert("publishSuppressed".to_string(), json!(false));
    if let Err(error) = save_managed_state(&state, &ownership).await {
        return api_error_response(error);
    }
    if let Err(error) =
        reconcile_resources(&state, &api, &managed, &mut ownership, true, None).await
    {
        let optimization = ensure_nested_object(&mut ownership, &["optimization"]);
        match previous_selected {
            Some(value) => {
                optimization.insert("selected".to_string(), value);
            }
            None => {
                optimization.remove("selected");
            }
        }
        match previous_fallback {
            Some(value) => {
                optimization.insert("fallbackActive".to_string(), value);
            }
            None => {
                optimization.remove("fallbackActive");
            }
        }
        match previous_publish_suppressed {
            Some(value) => {
                optimization.insert("publishSuppressed".to_string(), value);
            }
            None => {
                optimization.remove("publishSuppressed");
            }
        }
        let _ = save_managed_state(&state, &ownership).await;
        return api_error_response(error);
    }
    ensure_object(&mut managed).insert(
        "lastOptimizationApplyAt".to_string(),
        json!(time_utils::now_iso()),
    );
    if let Err(error) = save_managed_config(&state, &managed).await {
        return api_error_response(error);
    }
    let mut runtime = load_runtime(&state).await;
    let runtime_object = ensure_object(&mut runtime);
    runtime_object.insert("lastCandidates".to_string(), json!(candidates));
    runtime_object.insert("lastSwitchReason".to_string(), json!("manual-speed-test"));
    if let Err(error) = save_runtime(&state, &runtime).await {
        return api_error_response(error);
    }
    response::ok(json!({ "selected": selected, "state": ownership.get("optimization") }))
        .into_response()
}

async fn fallback_optimization(State(state): State<AppState>) -> Response {
    let _guard = state.cloudflared_manage_lock.lock().await;
    let managed = load_managed_config(&state).await;
    let api = match api_for_background(&state).await {
        Ok(Some(api)) => api,
        Ok(None) => {
            return response::error(
                StatusCode::CONFLICT,
                "Cloudflare API Token is not configured",
            );
        }
        Err(error) => return api_error_response(error),
    };
    let mut ownership = load_managed_state(&state).await;
    match fallback_to_wildcard(&state, &api, &managed, &mut ownership).await {
        Ok(()) => {
            let mut runtime = load_runtime(&state).await;
            let object = ensure_object(&mut runtime);
            object.remove("pendingCandidate");
            object.insert("lastSwitchReason".to_string(), json!("manual-fallback"));
            if let Err(error) = save_runtime(&state, &runtime).await {
                return api_error_response(error);
            }
            response::ok(json!({ "fallbackActive": true })).into_response()
        }
        Err(error) => api_error_response(error),
    }
}

pub(super) async fn public_state(state: &AppState, managed: &Value, ownership: &Value) -> Value {
    let runtime = load_runtime(state).await;
    let local = state.store.get_config().await.unwrap_or_else(|_| json!({}));
    let host_states = ownership
        .pointer("/optimization/customHostnames")
        .and_then(Value::as_object);
    let domains = configured_hosts(&local)
        .into_iter()
        .map(|host| {
            let current = host_states.and_then(|items| items.get(&host));
            json!({
                "hostname": host,
                "status": current.and_then(|value| value.get("status")).cloned().unwrap_or_else(|| json!("fallback")),
                "sslStatus": current.and_then(|value| value.get("sslStatus")).cloned().unwrap_or(Value::Null),
                "customHostnameId": current.and_then(|value| value.get("id")).cloned().unwrap_or(Value::Null),
                "optimized": current.is_some_and(exact_route_is_optimized),
                "message": current.and_then(|value| value.get("message")).cloned().unwrap_or(Value::Null),
            })
        })
        .collect::<Vec<_>>();
    let latest_jobs = {
        let jobs = state.cloudflared_scan_jobs.read().await;
        let mut values = jobs.values().cloned().collect::<Vec<_>>();
        values.sort_by(|left, right| {
            right
                .get("createdAt")
                .and_then(Value::as_str)
                .cmp(&left.get("createdAt").and_then(Value::as_str))
        });
        values.into_iter().take(5).collect::<Vec<_>>()
    };
    json!({
        "enabled": managed.get("optimizationEnabled").and_then(Value::as_bool).unwrap_or(false),
        "beta": true,
        "ipv4Only": true,
        "selected": ownership.pointer("/optimization/selected").cloned().unwrap_or(Value::Null),
        "fallbackActive": ownership.pointer("/optimization/fallbackActive").and_then(Value::as_bool).unwrap_or(true),
        "publishSuppressed": ownership.pointer("/optimization/publishSuppressed").and_then(Value::as_bool).unwrap_or(false),
        "originHostname": ownership.pointer("/optimization/originDns/name").cloned().unwrap_or(Value::Null),
        "edgeHostname": ownership.pointer("/optimization/edgeDns/name").cloned().unwrap_or(Value::Null),
        "fallbackOrigin": ownership.pointer("/optimization/fallbackOrigin").cloned().unwrap_or(Value::Null),
        "capabilityProbe": ownership.pointer("/optimization/capabilityProbe").cloned().unwrap_or(Value::Null),
        "domains": domains,
        "schedule": {
            "fullScanIntervalDays": 7,
            "healthCheckIntervalMinutes": 15,
            "nextFullScanAt": runtime.get("nextFullScanAt").cloned().unwrap_or(Value::Null),
            "lastFullScanAt": runtime.get("lastFullScanAt").cloned().unwrap_or(Value::Null),
            "lastHealthAt": runtime.get("lastHealthAt").cloned().unwrap_or(Value::Null),
            "healthFailures": runtime.get("healthFailures").cloned().unwrap_or_else(|| json!(0)),
            "lastSwitchReason": runtime.get("lastSwitchReason").cloned().unwrap_or(Value::Null),
            "lastError": runtime.get("lastError").cloned().unwrap_or(Value::Null),
        },
        "scans": latest_jobs,
    })
}

pub(super) fn plan_warnings(enabled: bool) -> Vec<Value> {
    if !enabled {
        return Vec::new();
    }
    vec![
        json!("Optimization is a Beta feature measured from this server's network vantage point."),
        json!(
            "Cloudflare for SaaS includes up to 100 exact Custom Hostnames on non-Enterprise plans; excess domains use the wildcard Tunnel."
        ),
        json!(
            "The wildcard Tunnel remains configured and is restored automatically if the preferred edge path fails."
        ),
    ]
}

pub(super) async fn append_cleanup_remote_snapshot(
    api: &CloudflareApi,
    zone_id: &str,
    ownership: &Value,
    instance_id: &str,
    conflicts: &mut Vec<Value>,
    remote_snapshot: &mut Vec<Value>,
) -> Result<(), CloudflareApiError> {
    let fallback_origin = api.get_fallback_origin(zone_id).await?;
    if let Some(owned) = ownership.pointer("/optimization/fallbackOrigin") {
        let expected = owned.get("origin").and_then(Value::as_str);
        let remote_origin = fallback_origin
            .as_ref()
            .and_then(|value| value.get("origin"))
            .and_then(Value::as_str);
        if expected.is_some() && remote_origin.is_some() && expected != remote_origin {
            conflicts.push(json!({
                "id": "optimization:cleanup-fallback-origin",
                "kind": "custom-hostname",
                "target": "Cloudflare for SaaS fallback origin",
                "message": "The previously managed fallback origin has been changed by another configuration",
                "takeoverAllowed": false,
            }));
        }
    }
    remote_snapshot.push(json!({ "fallbackOrigin": fallback_origin }));

    let mut tracked = Vec::new();
    for path in ["/optimization/originDns", "/optimization/edgeDns"] {
        if let Some(record) = ownership.pointer(path) {
            tracked.push(record.clone());
        }
    }
    for (hostname, state) in ownership
        .pointer("/optimization/customHostnames")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|items| items.iter())
    {
        if let Some(id) = state.get("exactDnsId").and_then(Value::as_str) {
            tracked.push(json!({
                "id": id,
                "name": hostname,
                "type": "CNAME",
                "content": ownership.pointer("/optimization/edgeDns/name").cloned().unwrap_or(Value::Null),
                "proxied": false,
            }));
        }
        for record in state
            .get("validationDns")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            tracked.push(record.clone());
        }
    }
    if let Some(probe) = ownership.pointer("/optimization/capabilityProbe") {
        if let Some(record) = probe.get("activationDns") {
            tracked.push(record.clone());
        }
        for record in probe
            .get("validationDns")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            tracked.push(record.clone());
        }
    }
    let mut names = tracked
        .iter()
        .filter_map(|record| record.get("name").and_then(Value::as_str))
        .map(str::to_string)
        .collect::<Vec<_>>();
    names.sort();
    names.dedup();
    for name in names {
        let records = api.list_dns_records(zone_id, Some(&name)).await?;
        for owned in tracked
            .iter()
            .filter(|record| record.get("name").and_then(Value::as_str) == Some(name.as_str()))
        {
            let Some(id) = owned.get("id").and_then(Value::as_str) else {
                continue;
            };
            let Some(remote) = records
                .iter()
                .find(|record| record.get("id").and_then(Value::as_str) == Some(id))
            else {
                continue;
            };
            let record_type = owned.get("type").and_then(Value::as_str).unwrap_or("");
            let content = owned.get("content").and_then(Value::as_str);
            let proxied = owned
                .get("proxied")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if record_type.is_empty()
                || !dns_record_owned_for_update(
                    remote,
                    Some(id),
                    instance_id,
                    record_type,
                    content,
                    proxied,
                )
            {
                conflicts.push(json!({
                    "id": format!("optimization:cleanup-dns:{id}"),
                    "kind": "dns",
                    "target": name.clone(),
                    "message": "A previously managed optimization DNS record has been claimed or changed by another configuration",
                    "takeoverAllowed": true,
                }));
            }
        }
        remote_snapshot.push(json!({ "hostname": name, "dnsRecords": records }));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn append_preview(
    api: &CloudflareApi,
    zone_id: &str,
    root: &str,
    instance: &str,
    local: &Value,
    ownership: &Value,
    custom_hostnames: &[Value],
    operations: &mut Vec<Value>,
    conflicts: &mut Vec<Value>,
    remote_snapshot: &mut Vec<Value>,
) -> Result<(), CloudflareApiError> {
    let origin = format!("fnknock-origin-{instance}.{root}");
    let origin_target = ownership
        .pointer("/tunnel/id")
        .and_then(Value::as_str)
        .map(|id| format!("{id}.cfargotunnel.com"));
    inspect_auxiliary_dns(
        api,
        zone_id,
        &origin,
        ownership
            .pointer("/optimization/originDns/id")
            .and_then(Value::as_str),
        "optimization:origin-dns",
        instance,
        "CNAME",
        origin_target.as_deref(),
        true,
        operations,
        conflicts,
        remote_snapshot,
    )
    .await?;
    let remote_fallback = api.get_fallback_origin(zone_id).await?;
    remote_snapshot.push(json!({ "fallbackOrigin": remote_fallback.clone() }));
    let owned_fallback = ownership.pointer("/optimization/fallbackOrigin");
    let remote_origin = remote_fallback
        .as_ref()
        .and_then(|value| value.get("origin"))
        .and_then(Value::as_str);
    let owned_origin = owned_fallback
        .and_then(|value| value.get("origin"))
        .and_then(Value::as_str);
    match remote_origin {
        None => operations.push(preview_operation(
            "optimization:fallback-origin",
            "custom-hostname",
            "create",
            &origin,
            false,
        )),
        Some(remote) if remote.eq_ignore_ascii_case(&origin) && owned_origin == Some(remote) => {
            operations.push(preview_operation(
                "optimization:fallback-origin",
                "custom-hostname",
                "keep",
                &origin,
                true,
            ));
        }
        Some(remote)
            if owned_origin == Some(remote)
                && owned_fallback
                    .and_then(|value| value.get("ownership"))
                    .and_then(Value::as_str)
                    .is_some() =>
        {
            operations.push(preview_operation(
                "optimization:fallback-origin",
                "custom-hostname",
                "update",
                &origin,
                true,
            ));
        }
        Some(_) => conflicts.push(json!({
            "id": "optimization:fallback-origin",
            "kind": "custom-hostname",
            "target": "Cloudflare for SaaS fallback origin",
            "message": "A Zone-wide fallback origin already exists and is not owned by fn-knock",
            "takeoverAllowed": true,
        })),
    }
    let capability_status = ownership
        .pointer("/optimization/capabilityProbe/status")
        .and_then(Value::as_str);
    operations.push(preview_operation(
        "optimization:capability-probe",
        "custom-hostname",
        if capability_status == Some("compatible") {
            "keep"
        } else {
            "probe"
        },
        &format!("fnknock-probe-{instance}.{root}"),
        capability_status == Some("compatible"),
    ));
    if capability_status != Some("compatible") {
        let probe_hostname = format!("fnknock-probe-{instance}.{root}");
        inspect_auxiliary_dns(
            api,
            zone_id,
            &probe_hostname,
            ownership
                .pointer("/optimization/capabilityProbe/activationDns/id")
                .and_then(Value::as_str),
            "optimization:capability-probe-dns",
            instance,
            "CNAME",
            Some(origin.as_str()),
            false,
            operations,
            conflicts,
            remote_snapshot,
        )
        .await?;
    }
    if ownership.pointer("/optimization/selected/ip").is_some() {
        let edge = format!("fnknock-edge-{instance}.{root}");
        inspect_auxiliary_dns(
            api,
            zone_id,
            &edge,
            ownership
                .pointer("/optimization/edgeDns/id")
                .and_then(Value::as_str),
            "optimization:edge-dns",
            instance,
            "A",
            ownership
                .pointer("/optimization/selected/ip")
                .and_then(Value::as_str),
            false,
            operations,
            conflicts,
            remote_snapshot,
        )
        .await?;
    }
    let owned = ownership
        .pointer("/optimization/customHostnames")
        .and_then(Value::as_object);
    let total = custom_hostnames.len();
    let mut remaining = MAX_CUSTOM_HOSTNAMES.saturating_sub(total);
    for host in configured_hosts(local) {
        let existing = custom_hostnames.iter().find(|item| {
            item.get("hostname")
                .and_then(Value::as_str)
                .is_some_and(|value| value.eq_ignore_ascii_case(&host))
        });
        let owned_id = owned
            .and_then(|items| items.get(&host))
            .and_then(|value| value.get("id"))
            .and_then(Value::as_str);
        match existing {
            Some(item) if owned_id == item.get("id").and_then(Value::as_str) => {
                operations.push(preview_operation(
                    &format!("custom-hostname:{host}"),
                    "custom-hostname",
                    "keep",
                    &host,
                    true,
                ));
            }
            Some(_) => conflicts.push(json!({
                "id": format!("custom-hostname:{host}"),
                "kind": "custom-hostname",
                "target": host,
                "message": "An unowned Cloudflare for SaaS Custom Hostname already exists",
                "takeoverAllowed": true,
            })),
            None if remaining > 0 => {
                operations.push(preview_operation(
                    &format!("custom-hostname:{host}"),
                    "custom-hostname",
                    "create",
                    &host,
                    false,
                ));
                remaining -= 1;
            }
            None => operations.push(preview_operation(
                &format!("custom-hostname:{host}"),
                "custom-hostname",
                "fallback",
                &host,
                false,
            )),
        }
        if ownership.pointer("/optimization/selected/ip").is_some() {
            let exact_records = api.list_dns_records(zone_id, Some(&host)).await?;
            remote_snapshot.push(json!({
                "hostname": host,
                "dnsRecords": exact_records.clone(),
            }));
            let exact_owned_id = owned
                .and_then(|items| items.get(&host))
                .and_then(|value| value.get("exactDnsId"))
                .and_then(Value::as_str);
            let exact_record = exact_owned_id
                .and_then(|id| {
                    exact_records
                        .iter()
                        .find(|record| record.get("id").and_then(Value::as_str) == Some(id))
                })
                .or_else(|| {
                    exact_records
                        .iter()
                        .find(|record| is_managed_dns(record, instance))
                })
                .or_else(|| {
                    exact_records.iter().find(|record| {
                        matches!(
                            record.get("type").and_then(Value::as_str),
                            Some("A" | "AAAA" | "CNAME")
                        )
                    })
                })
                .or_else(|| exact_records.first());
            if let Some(record) = exact_record {
                let edge = format!("fnknock-edge-{instance}.{root}");
                let exact_owned = dns_record_owned_for_update(
                    record,
                    exact_owned_id,
                    instance,
                    "CNAME",
                    Some(&edge),
                    false,
                );
                if exact_owned {
                    operations.push(preview_operation(
                        &format!("optimization:dns:{host}"),
                        "dns",
                        "update",
                        &host,
                        true,
                    ));
                } else {
                    conflicts.push(json!({
                        "id": format!("optimization:dns:{host}"),
                        "kind": "dns",
                        "target": host,
                        "message": "An unowned exact DNS record prevents optimization",
                        "takeoverAllowed": true,
                    }));
                }
            } else {
                operations.push(preview_operation(
                    &format!("optimization:dns:{host}"),
                    "dns",
                    "create",
                    &host,
                    false,
                ));
            }
        }
    }
    Ok(())
}

pub(super) async fn reconcile_resources(
    state: &AppState,
    api: &CloudflareApi,
    managed: &Value,
    ownership: &mut Value,
    force_publish: bool,
    takeover: Option<&HashSet<String>>,
) -> Result<(), CloudflareApiError> {
    if managed.get("optimizationEnabled").and_then(Value::as_bool) != Some(true) {
        return Ok(());
    }
    let zone_id = managed.get("zoneId").and_then(Value::as_str).unwrap_or("");
    let root = managed_root_domain(managed);
    let tunnel_id = managed
        .pointer("/tunnel/id")
        .and_then(Value::as_str)
        .unwrap_or("");
    if zone_id.is_empty() || root.is_empty() || tunnel_id.is_empty() {
        return Err(local_error(
            "Managed Tunnel, Zone, and root domain must be configured first",
        ));
    }
    let suffix = managed_instance_id(managed);
    let origin_hostname = format!("fnknock-origin-{suffix}.{root}");
    let origin_target = format!("{tunnel_id}.cfargotunnel.com");
    let existing_origin_id = ownership
        .pointer("/optimization/originDns/id")
        .and_then(Value::as_str)
        .map(str::to_string);
    let origin_dns = upsert_managed_dns(
        api,
        ManagedDnsRequest {
            zone_id,
            name: &origin_hostname,
            record_type: "CNAME",
            content: &origin_target,
            proxied: true,
            owned_id: existing_origin_id.as_deref(),
            takeover: takeover.is_some_and(|items| items.contains("optimization:origin-dns")),
            instance_id: &suffix,
        },
    )
    .await?;
    ensure_nested_object(ownership, &["optimization"]).insert("originDns".to_string(), origin_dns);
    save_managed_state(state, ownership).await?;

    match ensure_fallback_origin(
        state,
        api,
        zone_id,
        ownership,
        &origin_hostname,
        takeover.is_some_and(|items| items.contains("optimization:fallback-origin")),
    )
    .await?
    {
        FallbackOriginResult::Ready => {}
        FallbackOriginResult::Pending => return Ok(()),
    }

    let selected = ownership.pointer("/optimization/selected").cloned();
    let selected_ip = selected
        .as_ref()
        .and_then(|value| value.get("ip"))
        .and_then(Value::as_str)
        .and_then(|value| Ipv4Addr::from_str(value).ok());
    let edge_hostname = format!("fnknock-edge-{suffix}.{root}");
    match ensure_capability_probe(
        state,
        api,
        managed,
        ownership,
        &origin_hostname,
        selected_ip,
    )
    .await?
    {
        CapabilityProbeResult::Ready => {}
        CapabilityProbeResult::Pending | CapabilityProbeResult::Unsupported => return Ok(()),
    }

    if let Some(ip) = selected_ip {
        let ip_text = ip.to_string();
        let current_edge_ip = ownership
            .pointer("/optimization/edgeDns/content")
            .and_then(Value::as_str);
        if current_edge_ip != Some(ip_text.as_str()) {
            validate_candidate_for_active_hostnames(ownership, ip)
                .await
                .map_err(|error| {
                    local_error(format!(
                        "Preferred edge candidate failed pre-publish validation: {error}"
                    ))
                })?;
        }
        let existing_edge_id = ownership
            .pointer("/optimization/edgeDns/id")
            .and_then(Value::as_str)
            .map(str::to_string);
        let edge_dns = upsert_managed_dns(
            api,
            ManagedDnsRequest {
                zone_id,
                name: &edge_hostname,
                record_type: "A",
                content: &ip_text,
                proxied: false,
                owned_id: existing_edge_id.as_deref(),
                takeover: takeover.is_some_and(|items| items.contains("optimization:edge-dns")),
                instance_id: &suffix,
            },
        )
        .await?;
        ensure_nested_object(ownership, &["optimization"]).insert("edgeDns".to_string(), edge_dns);
        save_managed_state(state, ownership).await?;
    }

    let local = state
        .store
        .get_config()
        .await
        .map_err(local_error_display)?;
    let hosts = configured_hosts(&local);
    let configured_set = hosts.iter().cloned().collect::<HashSet<_>>();
    cleanup_removed_hosts(state, api, zone_id, ownership, &configured_set, &suffix).await?;
    if !should_publish_exact_routes(ownership, force_publish) {
        return Ok(());
    }
    let remote_custom = api.list_custom_hostnames(zone_id, None).await?;
    let mut available = MAX_CUSTOM_HOSTNAMES.saturating_sub(remote_custom.len());
    let mut created_this_run = 0usize;
    for host in hosts {
        let current_owned = ownership
            .pointer(&format!(
                "/optimization/customHostnames/{}",
                json_pointer_escape(&host)
            ))
            .cloned()
            .unwrap_or_else(|| json!({}));
        let owned_id = current_owned.get("id").and_then(Value::as_str);
        let existing = remote_custom.iter().find(|item| {
            item.get("hostname")
                .and_then(Value::as_str)
                .is_some_and(|value| value.eq_ignore_ascii_case(&host))
        });
        let custom = match existing {
            Some(item) if owned_id == item.get("id").and_then(Value::as_str) => item.clone(),
            Some(item)
                if takeover
                    .is_some_and(|items| items.contains(&format!("custom-hostname:{host}"))) =>
            {
                if let Some(id) = item.get("id").and_then(Value::as_str) {
                    api.delete_custom_hostname(zone_id, id).await?;
                }
                api.create_custom_hostname(zone_id, &host, &origin_hostname)
                    .await?
            }
            Some(_) => {
                set_host_state(
                    ownership,
                    &host,
                    json!({
                        "status": "conflict",
                        "message": "Custom Hostname is not owned by fn-knock"
                    }),
                );
                save_managed_state(state, ownership).await?;
                continue;
            }
            None if available == 0 => {
                set_host_state(
                    ownership,
                    &host,
                    json!({ "status": "quota", "message": "Custom Hostname quota is exhausted" }),
                );
                save_managed_state(state, ownership).await?;
                continue;
            }
            None if created_this_run >= MAX_CUSTOM_HOSTNAME_CREATES_PER_RECONCILE => {
                set_host_state(
                    ownership,
                    &host,
                    json!({
                        "status": "queued",
                        "message": "Queued to respect Cloudflare certificate issuance rate limits"
                    }),
                );
                save_managed_state(state, ownership).await?;
                continue;
            }
            None => {
                available -= 1;
                created_this_run += 1;
                api.create_custom_hostname(zone_id, &host, &origin_hostname)
                    .await?
            }
        };
        let custom_id = custom
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if custom_id.is_empty() {
            continue;
        }
        let mut host_state = current_owned;
        let exact_route_was_optimized = exact_route_is_optimized(&host_state);
        let status = custom
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("pending");
        let ssl_status = custom
            .pointer("/ssl/status")
            .and_then(Value::as_str)
            .unwrap_or("pending");
        let object = ensure_object(&mut host_state);
        object.insert("id".to_string(), json!(custom_id));
        object.insert("status".to_string(), json!(status));
        object.insert("sslStatus".to_string(), json!(ssl_status));
        object.insert("hostname".to_string(), json!(host));
        set_host_state(ownership, &host, host_state.clone());
        save_managed_state(state, ownership).await?;

        let validation_records = extract_validation_records(&custom);
        let mut validation_ids = Vec::new();
        let mut activation_conflict = false;
        for (name, value) in validation_records {
            let existing_id = host_state
                .get("validationDns")
                .and_then(Value::as_array)
                .and_then(|records| {
                    records.iter().find(|record| {
                        record.get("name").and_then(Value::as_str) == Some(name.as_str())
                    })
                })
                .and_then(|record| record.get("id"))
                .and_then(Value::as_str);
            match upsert_managed_dns(
                api,
                ManagedDnsRequest {
                    zone_id,
                    name: &name,
                    record_type: "TXT",
                    content: &value,
                    proxied: false,
                    owned_id: existing_id,
                    takeover: false,
                    instance_id: &suffix,
                },
            )
            .await
            {
                Ok(record) => {
                    validation_ids.push(record);
                    ensure_object(&mut host_state)
                        .insert("validationDns".to_string(), json!(validation_ids));
                    set_host_state(ownership, &host, host_state.clone());
                    save_managed_state(state, ownership).await?;
                }
                Err(error) if error.status == Some(StatusCode::CONFLICT) => {
                    activation_conflict = true;
                    ensure_object(&mut host_state).insert("status".to_string(), json!("conflict"));
                    ensure_object(&mut host_state)
                        .insert("message".to_string(), json!(error.to_string()));
                }
                Err(error) => return Err(error),
            }
        }
        if !activation_conflict {
            // Cloudflare does not support TXT pre-validation when the custom
            // hostname is already in this Cloudflare Zone (Orange-to-Orange).
            // Point the exact hostname at the standard Tunnel first. This
            // activates the Custom Hostname without changing the request path;
            // only switch it to the preferred edge after certificate and SNI
            // validation have completed.
            let activation_target = if exact_route_was_optimized {
                edge_hostname.as_str()
            } else {
                origin_hostname.as_str()
            };
            let exact_id = host_state.get("exactDnsId").and_then(Value::as_str);
            match upsert_managed_dns(
                api,
                ManagedDnsRequest {
                    zone_id,
                    name: &host,
                    record_type: "CNAME",
                    content: activation_target,
                    proxied: false,
                    owned_id: exact_id,
                    takeover: takeover
                        .is_some_and(|items| items.contains(&format!("optimization:dns:{host}"))),
                    instance_id: &suffix,
                },
            )
            .await
            {
                Ok(record) => {
                    ensure_object(&mut host_state).insert(
                        "exactDnsId".to_string(),
                        record.get("id").cloned().unwrap_or(Value::Null),
                    );
                    ensure_object(&mut host_state).insert(
                        "exactDnsTarget".to_string(),
                        json!(if exact_route_was_optimized {
                            "edge"
                        } else {
                            "origin"
                        }),
                    );
                    set_host_state(ownership, &host, host_state.clone());
                    save_managed_state(state, ownership).await?;
                }
                Err(error) if error.status == Some(StatusCode::CONFLICT) => {
                    activation_conflict = true;
                    ensure_object(&mut host_state).insert("status".to_string(), json!("conflict"));
                    ensure_object(&mut host_state)
                        .insert("message".to_string(), json!(error.to_string()));
                }
                Err(error) => return Err(error),
            }
        }
        let refreshed = api.get_custom_hostname(zone_id, &custom_id).await?;
        let status = refreshed
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or(status);
        let ssl_status = refreshed
            .pointer("/ssl/status")
            .and_then(Value::as_str)
            .unwrap_or(ssl_status);
        if !activation_conflict {
            ensure_object(&mut host_state).insert("status".to_string(), json!(status));
        }
        ensure_object(&mut host_state).insert("sslStatus".to_string(), json!(ssl_status));
        if !activation_conflict && status == "active" && ssl_status == "active" {
            if let Some(ip) = selected_ip {
                match probe_custom_hostname(&host, ip).await {
                    Ok(()) => {
                        let exact_id = host_state.get("exactDnsId").and_then(Value::as_str);
                        match upsert_managed_dns(
                            api,
                            ManagedDnsRequest {
                                zone_id,
                                name: &host,
                                record_type: "CNAME",
                                content: &edge_hostname,
                                proxied: false,
                                owned_id: exact_id,
                                takeover: takeover.is_some_and(|items| {
                                    items.contains(&format!("optimization:dns:{host}"))
                                }),
                                instance_id: &suffix,
                            },
                        )
                        .await
                        {
                            Ok(record) => {
                                ensure_object(&mut host_state).insert(
                                    "exactDnsId".to_string(),
                                    record.get("id").cloned().unwrap_or(Value::Null),
                                );
                                ensure_object(&mut host_state)
                                    .insert("exactDnsTarget".to_string(), json!("edge"));
                                ensure_object(&mut host_state)
                                    .insert("status".to_string(), json!("optimized"));
                                ensure_object(&mut host_state).insert(
                                    "lastVerifiedAt".to_string(),
                                    json!(time_utils::now_iso()),
                                );
                                ensure_object(&mut host_state).remove("message");
                            }
                            Err(error) if error.status == Some(StatusCode::CONFLICT) => {
                                ensure_object(&mut host_state)
                                    .insert("status".to_string(), json!("conflict"));
                                ensure_object(&mut host_state)
                                    .insert("message".to_string(), json!(error.to_string()));
                            }
                            Err(error) => return Err(error),
                        }
                    }
                    Err(error) => {
                        ensure_object(&mut host_state)
                            .insert("status".to_string(), json!("probe-failed"));
                        ensure_object(&mut host_state).insert("message".to_string(), json!(error));
                        if force_publish {
                            ensure_nested_object(ownership, &["optimization"])
                                .insert("fallbackActive".to_string(), json!(true));
                        }
                    }
                }
            } else {
                ensure_object(&mut host_state).insert("status".to_string(), json!("ready"));
            }
        }
        set_host_state(ownership, &host, host_state);
        save_managed_state(state, ownership).await?;
    }
    let any_optimized = ownership
        .pointer("/optimization/customHostnames")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|items| items.values())
        .any(exact_route_is_optimized);
    ensure_nested_object(ownership, &["optimization"])
        .insert("fallbackActive".to_string(), json!(!any_optimized));
    save_managed_state(state, ownership).await
}

fn active_probe_hostname(ownership: &Value) -> Option<String> {
    active_probe_hostnames(ownership).into_iter().next()
}

fn active_probe_hostnames(ownership: &Value) -> Vec<String> {
    ownership
        .pointer("/optimization/customHostnames")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|items| items.iter())
        .filter_map(|(hostname, state)| {
            let status = state.get("status").and_then(Value::as_str);
            let ssl_status = state.get("sslStatus").and_then(Value::as_str);
            (matches!(status, Some("active" | "optimized")) && ssl_status == Some("active"))
                .then(|| hostname.clone())
        })
        .collect()
}

async fn validate_candidate_for_active_hostnames(
    ownership: &Value,
    ip: Ipv4Addr,
) -> Result<(), String> {
    let hostnames = active_probe_hostnames(ownership);
    for chunk in hostnames.chunks(SNI_VALIDATION_CONCURRENCY) {
        let mut probes = JoinSet::new();
        for hostname in chunk.iter().cloned() {
            probes.spawn(async move {
                let result = probe_custom_hostname(&hostname, ip).await;
                (hostname, result)
            });
        }
        while let Some(result) = probes.join_next().await {
            match result {
                Ok((_, Ok(()))) => {}
                Ok((hostname, Err(error))) => {
                    probes.abort_all();
                    return Err(format!("{hostname}: {error}"));
                }
                Err(error) => {
                    probes.abort_all();
                    return Err(format!("SNI validation task failed: {error}"));
                }
            }
        }
    }
    Ok(())
}

async fn ensure_fallback_origin(
    state: &AppState,
    api: &CloudflareApi,
    zone_id: &str,
    ownership: &mut Value,
    desired_origin: &str,
    takeover: bool,
) -> Result<FallbackOriginResult, CloudflareApiError> {
    let current = ownership.pointer("/optimization/fallbackOrigin").cloned();
    let remote_value = api.get_fallback_origin(zone_id).await?;
    let remote_origin = remote_value
        .as_ref()
        .and_then(|value| value.get("origin"))
        .and_then(Value::as_str);
    let owned_origin = current
        .as_ref()
        .and_then(|value| value.get("origin"))
        .and_then(Value::as_str);
    let owned_kind = current
        .as_ref()
        .and_then(|value| value.get("ownership"))
        .and_then(Value::as_str);

    let (next, ownership_kind, previous_origin) = match remote_origin {
        None => (
            api.update_fallback_origin(zone_id, desired_origin).await?,
            "dedicated",
            None,
        ),
        Some(remote) if remote.eq_ignore_ascii_case(desired_origin) => {
            if owned_origin.is_none() || owned_kind.is_none() {
                if !takeover {
                    return Err(CloudflareApiError {
                        status: Some(StatusCode::CONFLICT),
                        message: "The Cloudflare for SaaS fallback origin already exists and is not owned by fn-knock; preview and explicitly confirm takeover"
                            .to_string(),
                    });
                }
                (
                    remote_value
                        .clone()
                        .unwrap_or_else(|| json!({ "origin": remote })),
                    "adopted",
                    None,
                )
            } else {
                (
                    remote_value
                        .clone()
                        .unwrap_or_else(|| json!({ "origin": remote })),
                    owned_kind.unwrap(),
                    current
                        .as_ref()
                        .and_then(|value| value.get("previousOrigin"))
                        .and_then(Value::as_str),
                )
            }
        }
        Some(remote) if owned_origin == Some(remote) && owned_kind.is_some() => (
            api.update_fallback_origin(zone_id, desired_origin).await?,
            owned_kind.unwrap(),
            current
                .as_ref()
                .and_then(|value| value.get("previousOrigin"))
                .and_then(Value::as_str),
        ),
        Some(remote) if takeover => (
            api.update_fallback_origin(zone_id, desired_origin).await?,
            "adopted",
            Some(remote),
        ),
        Some(_) => {
            return Err(CloudflareApiError {
                status: Some(StatusCode::CONFLICT),
                message: "A different Cloudflare for SaaS fallback origin exists; preview and explicitly confirm takeover"
                    .to_string(),
            });
        }
    };

    let status = next
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("pending_deployment");
    let errors = next.get("errors").cloned().unwrap_or_else(|| json!([]));
    let mut stored = json!({
        "origin": desired_origin,
        "status": status,
        "errors": errors,
        "ownership": ownership_kind,
        "updatedAt": time_utils::now_iso(),
    });
    if let Some(previous_origin) = previous_origin {
        ensure_object(&mut stored).insert("previousOrigin".to_string(), json!(previous_origin));
    }
    ensure_nested_object(ownership, &["optimization"]).insert("fallbackOrigin".to_string(), stored);
    save_managed_state(state, ownership).await?;

    match status {
        "active" => Ok(FallbackOriginResult::Ready),
        "deployment_timed_out" | "pending_deletion" | "deleted" => Err(local_error(format!(
            "Cloudflare for SaaS fallback origin entered status {status}"
        ))),
        _ => Ok(FallbackOriginResult::Pending),
    }
}

async fn ensure_capability_probe(
    state: &AppState,
    api: &CloudflareApi,
    managed: &Value,
    ownership: &mut Value,
    origin_hostname: &str,
    selected_ip: Option<Ipv4Addr>,
) -> Result<CapabilityProbeResult, CloudflareApiError> {
    let existing_status = ownership
        .pointer("/optimization/capabilityProbe/status")
        .and_then(Value::as_str)
        .map(str::to_string);
    if existing_status.as_deref() == Some("compatible") {
        let tested_ip = ownership
            .pointer("/optimization/capabilityProbe/testedIp")
            .and_then(Value::as_str);
        let selected_ip_text = selected_ip.map(|ip| ip.to_string());
        let candidate_changed_without_business_probe = selected_ip_text.is_some()
            && tested_ip != selected_ip_text.as_deref()
            && active_probe_hostname(ownership).is_none();
        if !candidate_changed_without_business_probe {
            return Ok(CapabilityProbeResult::Ready);
        }
        if let Some(optimization) = ownership
            .pointer_mut("/optimization")
            .and_then(Value::as_object_mut)
        {
            optimization.remove("capabilityProbe");
        }
        save_managed_state(state, ownership).await?;
    }
    if existing_status.as_deref() == Some("unsupported") {
        return Ok(CapabilityProbeResult::Unsupported);
    }
    let zone_id = managed.get("zoneId").and_then(Value::as_str).unwrap_or("");
    let root = managed_root_domain(managed);
    let suffix = managed_instance_id(managed);
    let hostname = format!("fnknock-probe-{suffix}.{root}");
    let current = ownership
        .pointer("/optimization/capabilityProbe")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let owned_id = current.get("id").and_then(Value::as_str);
    let custom = if let Some(id) = owned_id {
        match api.get_custom_hostname(zone_id, id).await {
            Ok(value) => value,
            Err(error) if error.status == Some(StatusCode::NOT_FOUND) => {
                create_capability_hostname(
                    state,
                    api,
                    managed,
                    ownership,
                    &hostname,
                    origin_hostname,
                )
                .await?
            }
            Err(error) => return Err(error),
        }
    } else {
        let existing = api.list_custom_hostnames(zone_id, Some(&hostname)).await?;
        if existing.is_empty() {
            match create_capability_hostname(
                state,
                api,
                managed,
                ownership,
                &hostname,
                origin_hostname,
            )
            .await
            {
                Ok(value) => value,
                Err(error) if is_capability_unsupported_api_error(&error) => {
                    disable_unsupported_optimization(state, managed, ownership, &error.to_string())
                        .await?;
                    return Ok(CapabilityProbeResult::Unsupported);
                }
                Err(error) => return Err(error),
            }
        } else {
            return Err(CloudflareApiError {
                status: Some(StatusCode::CONFLICT),
                message: "The isolated optimization probe hostname already exists and is not owned by fn-knock"
                    .to_string(),
            });
        }
    };
    let custom_id = custom.get("id").and_then(Value::as_str).unwrap_or("");
    if custom_id.is_empty() {
        return Err(local_error(
            "Cloudflare did not return an ID for the optimization capability probe",
        ));
    }
    let current = ownership
        .pointer("/optimization/capabilityProbe")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let mut validation_dns = Vec::new();
    for (name, value) in extract_validation_records(&custom) {
        let existing_id = current
            .get("validationDns")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .find(|record| record.get("name").and_then(Value::as_str) == Some(name.as_str()))
            .and_then(|record| record.get("id"))
            .and_then(Value::as_str);
        match upsert_managed_dns(
            api,
            ManagedDnsRequest {
                zone_id,
                name: &name,
                record_type: "TXT",
                content: &value,
                proxied: false,
                owned_id: existing_id,
                takeover: false,
                instance_id: &suffix,
            },
        )
        .await
        {
            Ok(record) => {
                validation_dns.push(record);
                let probe = ensure_nested_object(ownership, &["optimization", "capabilityProbe"]);
                probe.insert("id".to_string(), json!(custom_id));
                probe.insert("hostname".to_string(), json!(hostname));
                probe.insert("validationDns".to_string(), json!(validation_dns.clone()));
                save_managed_state(state, ownership).await?;
            }
            Err(error) if is_capability_unsupported_api_error(&error) => {
                let mut cleanup = current.clone();
                let cleanup_object = ensure_object(&mut cleanup);
                cleanup_object.insert("id".to_string(), json!(custom_id));
                cleanup_object.insert("validationDns".to_string(), json!(validation_dns));
                cleanup_capability_probe(api, zone_id, &cleanup).await?;
                disable_unsupported_optimization(state, managed, ownership, &error.to_string())
                    .await?;
                return Ok(CapabilityProbeResult::Unsupported);
            }
            Err(error) => return Err(error),
        }
    }
    let activation_dns = upsert_managed_dns(
        api,
        ManagedDnsRequest {
            zone_id,
            name: &hostname,
            record_type: "CNAME",
            content: origin_hostname,
            proxied: false,
            owned_id: current.pointer("/activationDns/id").and_then(Value::as_str),
            takeover: false,
            instance_id: &suffix,
        },
    )
    .await?;
    {
        let probe = ensure_nested_object(ownership, &["optimization", "capabilityProbe"]);
        probe.insert("id".to_string(), json!(custom_id));
        probe.insert("hostname".to_string(), json!(hostname));
        probe.insert("activationDns".to_string(), activation_dns.clone());
        probe.insert("validationDns".to_string(), json!(validation_dns.clone()));
    }
    save_managed_state(state, ownership).await?;
    let refreshed = api.get_custom_hostname(zone_id, custom_id).await?;
    let status = refreshed
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("pending");
    let ssl_status = refreshed
        .pointer("/ssl/status")
        .and_then(Value::as_str)
        .unwrap_or("pending");
    let verification_errors = refreshed
        .get("verification_errors")
        .cloned()
        .unwrap_or_else(|| json!([]));
    let verification_message = cloudflare_error_list_message(&verification_errors);
    let probe_state = json!({
        "id": custom_id,
        "hostname": hostname,
        "status": if status == "active" && ssl_status == "active" && selected_ip.is_none() {
            "awaiting-candidate"
        } else {
            "pending"
        },
        "hostnameStatus": status,
        "sslStatus": ssl_status,
        "activationDns": activation_dns,
        "validationDns": validation_dns,
        "verificationErrors": verification_errors,
        "message": verification_message,
        "updatedAt": time_utils::now_iso(),
    });
    ensure_nested_object(ownership, &["optimization"])
        .insert("capabilityProbe".to_string(), probe_state.clone());
    save_managed_state(state, ownership).await?;
    if status != "active" || ssl_status != "active" {
        return Ok(CapabilityProbeResult::Pending);
    }
    let Some(ip) = selected_ip else {
        return Ok(CapabilityProbeResult::Pending);
    };
    match probe_custom_hostname(&hostname, ip).await {
        Ok(()) => {
            cleanup_capability_probe(api, zone_id, &probe_state).await?;
            ensure_nested_object(ownership, &["optimization"]).insert(
                "capabilityProbe".to_string(),
                json!({
                    "hostname": hostname,
                    "status": "compatible",
                    "testedIp": ip,
                    "testedAt": time_utils::now_iso(),
                }),
            );
            save_managed_state(state, ownership).await?;
            Ok(CapabilityProbeResult::Ready)
        }
        Err(error) => {
            cleanup_capability_probe(api, zone_id, &probe_state).await?;
            disable_unsupported_optimization(state, managed, ownership, &error).await?;
            Ok(CapabilityProbeResult::Unsupported)
        }
    }
}

async fn create_capability_hostname(
    state: &AppState,
    api: &CloudflareApi,
    managed: &Value,
    ownership: &mut Value,
    hostname: &str,
    origin_hostname: &str,
) -> Result<Value, CloudflareApiError> {
    let zone_id = managed.get("zoneId").and_then(Value::as_str).unwrap_or("");
    let custom = api
        .create_custom_hostname(zone_id, hostname, origin_hostname)
        .await?;
    ensure_nested_object(ownership, &["optimization"]).insert(
        "capabilityProbe".to_string(),
        json!({
            "id": custom.get("id").cloned().unwrap_or(Value::Null),
            "hostname": hostname,
            "status": "pending",
            "createdAt": time_utils::now_iso(),
        }),
    );
    save_managed_state(state, ownership).await?;
    Ok(custom)
}

async fn cleanup_capability_probe(
    api: &CloudflareApi,
    zone_id: &str,
    probe: &Value,
) -> Result<(), CloudflareApiError> {
    if let Some(id) = probe.pointer("/activationDns/id").and_then(Value::as_str) {
        ignore_not_found(api.delete_dns_record(zone_id, id).await)?;
    }
    for record in probe
        .get("validationDns")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if let Some(id) = record.get("id").and_then(Value::as_str) {
            ignore_not_found(api.delete_dns_record(zone_id, id).await)?;
        }
    }
    if let Some(id) = probe.get("id").and_then(Value::as_str) {
        ignore_not_found(api.delete_custom_hostname(zone_id, id).await)?;
    }
    Ok(())
}

async fn disable_unsupported_optimization(
    state: &AppState,
    managed: &Value,
    ownership: &mut Value,
    reason: &str,
) -> Result<(), CloudflareApiError> {
    ensure_nested_object(ownership, &["optimization"]).insert(
        "capabilityProbe".to_string(),
        json!({
            "status": "unsupported",
            "message": reason,
            "testedAt": time_utils::now_iso(),
        }),
    );
    ensure_nested_object(ownership, &["optimization"])
        .insert("fallbackActive".to_string(), json!(true));
    ensure_nested_object(ownership, &["optimization"])
        .insert("publishSuppressed".to_string(), json!(true));
    save_managed_state(state, ownership).await?;
    let mut next_managed = managed.clone();
    ensure_object(&mut next_managed).insert("optimizationEnabled".to_string(), json!(false));
    save_managed_config(state, &next_managed).await
}

pub(super) async fn fallback_to_wildcard(
    state: &AppState,
    api: &CloudflareApi,
    managed: &Value,
    ownership: &mut Value,
) -> Result<(), CloudflareApiError> {
    let zone_id = managed.get("zoneId").and_then(Value::as_str).unwrap_or("");
    if zone_id.is_empty() {
        return Ok(());
    }
    let hosts = ownership
        .pointer("/optimization/customHostnames")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let instance_id = managed_instance_id(managed);
    let edge_hostname = ownership
        .pointer("/optimization/edgeDns/name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    for (hostname, value) in hosts {
        if let Some(record_id) = value.get("exactDnsId").and_then(Value::as_str) {
            delete_dns_if_owned(
                api,
                zone_id,
                &json!({
                    "id": record_id,
                    "name": hostname,
                    "type": "CNAME",
                    "content": edge_hostname,
                    "proxied": false,
                }),
                &instance_id,
            )
            .await?;
        }
        let mut next = value;
        ensure_object(&mut next).remove("exactDnsId");
        ensure_object(&mut next).remove("exactDnsTarget");
        ensure_object(&mut next).insert("status".to_string(), json!("fallback"));
        set_host_state(ownership, &hostname, next);
        save_managed_state(state, ownership).await?;
    }
    let optimization = ensure_nested_object(ownership, &["optimization"]);
    optimization.insert("fallbackActive".to_string(), json!(true));
    optimization.insert("publishSuppressed".to_string(), json!(true));
    optimization.insert("lastFallbackAt".to_string(), json!(time_utils::now_iso()));
    save_managed_state(state, ownership).await
}

pub(super) async fn cleanup_resources(
    state: &AppState,
    api: &CloudflareApi,
    managed: &Value,
    ownership: &mut Value,
) -> Result<(), CloudflareApiError> {
    let zone_id = managed.get("zoneId").and_then(Value::as_str).unwrap_or("");
    if zone_id.is_empty() {
        return Ok(());
    }
    if let Some(probe) = ownership.pointer("/optimization/capabilityProbe") {
        cleanup_capability_probe(api, zone_id, probe).await?;
    }
    let hosts = ownership
        .pointer("/optimization/customHostnames")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    for (_, host) in hosts {
        if let Some(id) = host.get("exactDnsId").and_then(Value::as_str) {
            ignore_not_found(api.delete_dns_record(zone_id, id).await)?;
        }
        for validation in host
            .get("validationDns")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(id) = validation.get("id").and_then(Value::as_str) {
                ignore_not_found(api.delete_dns_record(zone_id, id).await)?;
            }
        }
        if let Some(id) = host.get("id").and_then(Value::as_str) {
            ignore_not_found(api.delete_custom_hostname(zone_id, id).await)?;
        }
    }
    if let Some(fallback) = ownership.pointer("/optimization/fallbackOrigin").cloned() {
        let expected = fallback.get("origin").and_then(Value::as_str).unwrap_or("");
        let remote = api.get_fallback_origin(zone_id).await?;
        let remote_origin = remote
            .as_ref()
            .and_then(|value| value.get("origin"))
            .and_then(Value::as_str);
        if !expected.is_empty() && remote_origin.is_some() && remote_origin != Some(expected) {
            return Err(CloudflareApiError {
                status: Some(StatusCode::CONFLICT),
                message: "The Cloudflare for SaaS fallback origin changed after preview; refusing to clean it up"
                    .to_string(),
            });
        }
        match fallback.get("ownership").and_then(Value::as_str) {
            Some("dedicated") if remote_origin == Some(expected) => {
                ignore_not_found(api.delete_fallback_origin(zone_id).await)?;
            }
            Some("adopted") if remote_origin == Some(expected) => {
                if let Some(previous) = fallback.get("previousOrigin").and_then(Value::as_str) {
                    api.update_fallback_origin(zone_id, previous).await?;
                }
            }
            _ => {}
        }
    }
    for path in ["/optimization/originDns/id", "/optimization/edgeDns/id"] {
        if let Some(id) = ownership.pointer(path).and_then(Value::as_str) {
            ignore_not_found(api.delete_dns_record(zone_id, id).await)?;
        }
    }
    ensure_object(ownership).remove("optimization");
    save_managed_state(state, ownership).await
}

async fn cleanup_removed_hosts(
    state: &AppState,
    api: &CloudflareApi,
    zone_id: &str,
    ownership: &mut Value,
    configured: &HashSet<String>,
    instance_id: &str,
) -> Result<(), CloudflareApiError> {
    let current = ownership
        .pointer("/optimization/customHostnames")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    for (hostname, host) in current {
        if configured.contains(&hostname) {
            continue;
        }
        if let Some(id) = host.get("exactDnsId").and_then(Value::as_str) {
            delete_dns_if_owned(
                api,
                zone_id,
                &json!({
                    "id": id,
                    "name": hostname,
                    "type": "CNAME",
                    "content": ownership.pointer("/optimization/edgeDns/name").cloned().unwrap_or(Value::Null),
                    "proxied": false,
                }),
                instance_id,
            )
            .await?;
        }
        for record in host
            .get("validationDns")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if record.get("id").and_then(Value::as_str).is_some() {
                delete_dns_if_owned(api, zone_id, record, instance_id).await?;
            }
        }
        if let Some(id) = host.get("id").and_then(Value::as_str) {
            ignore_not_found(api.delete_custom_hostname(zone_id, id).await)?;
        }
        if let Some(items) = ownership
            .pointer_mut("/optimization/customHostnames")
            .and_then(Value::as_object_mut)
        {
            items.remove(&hostname);
        }
        save_managed_state(state, ownership).await?;
    }
    Ok(())
}

async fn scheduled_tick(state: &AppState) -> Result<(), CloudflareApiError> {
    let _guard = state.cloudflared_manage_lock.lock().await;
    let managed = load_managed_config(state).await;
    if managed.get("mode").and_then(Value::as_str) != Some("managed") {
        return Ok(());
    }
    let Some(api) = api_for_background(state).await? else {
        return Ok(());
    };
    let mut ownership = load_managed_state(state).await;
    let mut runtime = load_runtime(state).await;
    if managed.get("optimizationEnabled").and_then(Value::as_bool) != Some(true) {
        let zone_id = managed.get("zoneId").and_then(Value::as_str).unwrap_or("");
        if !zone_id.is_empty() {
            let local = state
                .store
                .get_config()
                .await
                .map_err(local_error_display)?;
            let configured = configured_hosts(&local).into_iter().collect::<HashSet<_>>();
            cleanup_removed_hosts(
                state,
                &api,
                zone_id,
                &mut ownership,
                &configured,
                &managed_instance_id(&managed),
            )
            .await?;
        }
        return Ok(());
    }
    if ownership
        .pointer("/optimization/publishSuppressed")
        .is_none()
    {
        let suppression = legacy_publish_suppression(&ownership, &runtime);
        ensure_nested_object(&mut ownership, &["optimization"])
            .insert("publishSuppressed".to_string(), json!(suppression));
        save_managed_state(state, &ownership).await?;
    }
    reconcile_resources(state, &api, &managed, &mut ownership, false, None).await?;
    let now = time_utils::now_ms();

    let last_health = runtime
        .get("lastHealthAtMs")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    if now.saturating_sub(last_health) >= HEALTH_INTERVAL_MS {
        run_health_check(state, &api, &managed, &mut ownership, &mut runtime).await?;
    }

    let next_scan = runtime
        .get("nextFullScanAtMs")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    if next_scan == 0 || now >= next_scan {
        let _scan_guard = state.cloudflared_scan_lock.lock().await;
        let candidates = time::timeout(Duration::from_secs(180), run_scan(state, None))
            .await
            .map_err(|_| local_error("Optimization scan exceeded the three-minute limit"))??;
        apply_automatic_scan_result(
            state,
            &api,
            &managed,
            &mut ownership,
            &mut runtime,
            &candidates,
        )
        .await?;
        let jitter = weekly_jitter_ms();
        let next = now + WEEK_MS + jitter;
        let runtime_object = ensure_object(&mut runtime);
        runtime_object.insert("lastFullScanAtMs".to_string(), json!(now));
        runtime_object.insert(
            "lastFullScanAt".to_string(),
            json!(time_utils::iso_from_ms(now)),
        );
        runtime_object.insert("nextFullScanAtMs".to_string(), json!(next));
        runtime_object.insert(
            "nextFullScanAt".to_string(),
            json!(time_utils::iso_from_ms(next)),
        );
    } else if let Some(confirm_at) = runtime
        .pointer("/pendingCandidate/confirmAtMs")
        .and_then(Value::as_i64)
        && now >= confirm_at
    {
        confirm_pending_candidate(state, &api, &managed, &mut ownership, &mut runtime).await?;
    }
    ensure_object(&mut runtime).insert("lastError".to_string(), Value::Null);
    save_runtime(state, &runtime).await
}

async fn run_health_check(
    state: &AppState,
    api: &CloudflareApi,
    managed: &Value,
    ownership: &mut Value,
    runtime: &mut Value,
) -> Result<(), CloudflareApiError> {
    let now = time_utils::now_ms();
    let selected_ip = ownership
        .pointer("/optimization/selected/ip")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<Ipv4Addr>().ok());
    let host = optimized_health_hostname(ownership);
    let success = match (selected_ip, host.as_deref()) {
        (Some(ip), Some(host)) => probe_custom_hostname(host, ip).await.is_ok(),
        _ => true,
    };
    let failures = if success {
        0
    } else {
        runtime
            .get("healthFailures")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            .saturating_add(1)
    };
    let object = ensure_object(runtime);
    object.insert("lastHealthAtMs".to_string(), json!(now));
    object.insert(
        "lastHealthAt".to_string(),
        json!(time_utils::iso_from_ms(now)),
    );
    object.insert("healthFailures".to_string(), json!(failures));
    if failures >= 3 {
        if try_verified_backup_candidate(
            state,
            api,
            managed,
            ownership,
            runtime,
            selected_ip,
            host.as_deref(),
        )
        .await?
        {
            let object = ensure_object(runtime);
            object.insert("healthFailures".to_string(), json!(0));
            object.insert("lastSwitchReason".to_string(), json!("health-failover"));
        } else {
            fallback_to_wildcard(state, api, managed, ownership).await?;
            let object = ensure_object(runtime);
            object.remove("pendingCandidate");
            object.insert(
                "lastError".to_string(),
                json!(
                    "Preferred edge failed three health checks; wildcard Tunnel fallback activated"
                ),
            );
            object.insert("lastSwitchReason".to_string(), json!("health-fallback"));
        }
    }
    save_runtime(state, runtime).await
}

#[allow(clippy::too_many_arguments)]
async fn try_verified_backup_candidate(
    state: &AppState,
    api: &CloudflareApi,
    managed: &Value,
    ownership: &mut Value,
    runtime: &Value,
    current_ip: Option<Ipv4Addr>,
    hostname: Option<&str>,
) -> Result<bool, CloudflareApiError> {
    let Some(hostname) = hostname else {
        return Ok(false);
    };
    let candidates = serde_json::from_value::<Vec<OptimizationCandidate>>(
        runtime
            .get("lastCandidates")
            .cloned()
            .unwrap_or_else(|| json!([])),
    )
    .unwrap_or_default();
    for candidate in candidates {
        let Ok(ip) = candidate.ip.parse::<Ipv4Addr>() else {
            continue;
        };
        if Some(ip) == current_ip || candidate.verified_at.is_none() {
            continue;
        }
        if probe_latency(ip).await.is_none() || probe_custom_hostname(hostname, ip).await.is_err() {
            continue;
        }
        let previous = ownership.pointer("/optimization/selected").cloned();
        set_selected(ownership, &candidate, "health-failover");
        save_managed_state(state, ownership).await?;
        if let Err(error) = reconcile_resources(state, api, managed, ownership, true, None).await {
            restore_selected(ownership, previous);
            save_managed_state(state, ownership).await?;
            return Err(error);
        }
        return Ok(true);
    }
    Ok(false)
}

async fn apply_automatic_scan_result(
    state: &AppState,
    api: &CloudflareApi,
    managed: &Value,
    ownership: &mut Value,
    runtime: &mut Value,
    candidates: &[OptimizationCandidate],
) -> Result<(), CloudflareApiError> {
    let Some(best) = candidates.first() else {
        return Ok(());
    };
    ensure_object(runtime).insert("lastCandidates".to_string(), json!(candidates));
    let current_score = ownership
        .pointer("/optimization/selected/score")
        .and_then(Value::as_f64);
    if current_score.is_none() {
        let previous = ownership.pointer("/optimization/selected").cloned();
        set_selected(ownership, best, "automatic");
        save_managed_state(state, ownership).await?;
        if let Err(error) = reconcile_resources(state, api, managed, ownership, true, None).await {
            restore_selected(ownership, previous);
            save_managed_state(state, ownership).await?;
            return Err(error);
        }
        return Ok(());
    }
    if best.score > current_score.unwrap_or(f64::MAX) * 0.85 {
        ensure_object(runtime).remove("pendingCandidate");
        return Ok(());
    }
    let now = time_utils::now_ms();
    ensure_object(runtime).insert(
        "pendingCandidate".to_string(),
        json!({
            "candidate": best,
            "firstSeenAtMs": now,
            "confirmAtMs": now + CONFIRMATION_DELAY_MS,
        }),
    );
    Ok(())
}

async fn confirm_pending_candidate(
    state: &AppState,
    api: &CloudflareApi,
    managed: &Value,
    ownership: &mut Value,
    runtime: &mut Value,
) -> Result<(), CloudflareApiError> {
    let Some(candidate) = runtime
        .pointer("/pendingCandidate/candidate")
        .cloned()
        .and_then(|value| serde_json::from_value::<OptimizationCandidate>(value).ok())
    else {
        return Ok(());
    };
    let ip = candidate
        .ip
        .parse::<Ipv4Addr>()
        .map_err(|_| local_error("Pending optimization candidate is invalid"))?;
    let confirmed = probe_latency(ip).await;
    let current_score = ownership
        .pointer("/optimization/selected/score")
        .and_then(Value::as_f64)
        .unwrap_or(f64::MAX);
    ensure_object(runtime).remove("pendingCandidate");
    if let Some((latency, jitter, loss)) = confirmed
        && loss <= candidate.loss_ratio
    {
        let mut updated = candidate;
        updated.median_latency_ms = latency;
        updated.jitter_ms = jitter;
        updated.loss_ratio = loss;
        updated.score = score_candidate(latency, jitter, loss, updated.download_mbps);
        if updated.score <= current_score * 0.85 {
            let previous = ownership.pointer("/optimization/selected").cloned();
            set_selected(ownership, &updated, "automatic");
            save_managed_state(state, ownership).await?;
            if let Err(error) =
                reconcile_resources(state, api, managed, ownership, true, None).await
            {
                restore_selected(ownership, previous);
                save_managed_state(state, ownership).await?;
                return Err(error);
            }
        }
    }
    Ok(())
}

fn set_selected(ownership: &mut Value, candidate: &OptimizationCandidate, source: &str) {
    ensure_nested_object(ownership, &["optimization"]).insert(
        "selected".to_string(),
        json!({
            "ip": candidate.ip,
            "medianLatencyMs": candidate.median_latency_ms,
            "jitterMs": candidate.jitter_ms,
            "lossRatio": candidate.loss_ratio,
            "downloadMbps": candidate.download_mbps,
            "score": candidate.score,
            "selectedAt": time_utils::now_iso(),
            "source": source,
        }),
    );
}

fn restore_selected(ownership: &mut Value, previous: Option<Value>) {
    let optimization = ensure_nested_object(ownership, &["optimization"]);
    match previous {
        Some(value) => {
            optimization.insert("selected".to_string(), value);
        }
        None => {
            optimization.remove("selected");
        }
    }
}

async fn run_scan(
    state: &AppState,
    job_id: Option<&str>,
) -> Result<Vec<OptimizationCandidate>, CloudflareApiError> {
    let prefixes = load_cloudflare_prefixes(state).await;
    let ips = sample_candidate_ips(&prefixes);
    let total = ips.len().max(1);
    let mut join_set = JoinSet::new();
    let mut results = Vec::new();
    let mut processed_count = 0usize;
    for chunk in ips.chunks(PROBE_CONCURRENCY) {
        if let Some(job_id) = job_id
            && is_job_cancelled(state, job_id).await
        {
            return Ok(Vec::new());
        }
        for ip in chunk.iter().copied() {
            join_set.spawn(async move { (ip, probe_latency(ip).await) });
        }
        while let Some(result) = join_set.join_next().await {
            if let Ok((ip, Some((latency, jitter, loss)))) = result
                && loss <= 1.0 / 3.0
            {
                results.push(OptimizationCandidate {
                    ip: ip.to_string(),
                    median_latency_ms: latency,
                    jitter_ms: jitter,
                    loss_ratio: loss,
                    download_mbps: 0.0,
                    score: f64::MAX,
                    verified_at: Some(time_utils::now_iso()),
                });
            }
        }
        processed_count += chunk.len();
        if let Some(job_id) = job_id {
            let progress = 5 + ((processed_count.min(total) * 55) / total) as i64;
            update_job(
                state,
                job_id,
                json!({ "phase": "latency", "progress": progress }),
            )
            .await;
        }
    }
    results.sort_by(|left, right| {
        left.median_latency_ms
            .partial_cmp(&right.median_latency_ms)
            .unwrap_or(Ordering::Equal)
    });
    results.truncate(DOWNLOAD_SHORTLIST);
    if let Some(job_id) = job_id {
        update_job(
            state,
            job_id,
            json!({ "phase": "download", "progress": 65 }),
        )
        .await;
    }
    let download_total = results.len().max(1);
    let mut download_tasks = JoinSet::new();
    for mut candidate in results {
        download_tasks.spawn(async move {
            let ip = candidate.ip.parse::<Ipv4Addr>().ok()?;
            let mut samples = Vec::new();
            for _ in 0..2 {
                if let Some(mbps) = probe_download(ip, DOWNLOAD_BYTES).await {
                    samples.push(mbps);
                }
            }
            samples.sort_by(|left, right| left.partial_cmp(right).unwrap_or(Ordering::Equal));
            candidate.download_mbps = median(&samples).unwrap_or(0.0);
            candidate.score = score_candidate(
                candidate.median_latency_ms,
                candidate.jitter_ms,
                candidate.loss_ratio,
                candidate.download_mbps,
            );
            Some(candidate)
        });
    }
    let mut completed = 0usize;
    let mut measured = Vec::new();
    while let Some(result) = download_tasks.join_next().await {
        if let Some(job_id) = job_id
            && is_job_cancelled(state, job_id).await
        {
            download_tasks.abort_all();
            return Ok(Vec::new());
        }
        if let Ok(Some(candidate)) = result {
            measured.push(candidate);
        }
        completed += 1;
        if let Some(job_id) = job_id {
            let progress = 65 + ((completed * 30) / download_total) as i64;
            update_job(
                state,
                job_id,
                json!({ "phase": "download", "progress": progress }),
            )
            .await;
        }
    }
    results = measured;
    results.retain(|candidate| candidate.download_mbps > 0.0 && candidate.score.is_finite());
    results.sort_by(|left, right| {
        left.score
            .partial_cmp(&right.score)
            .unwrap_or(Ordering::Equal)
    });
    Ok(results)
}

async fn load_cloudflare_prefixes(state: &AppState) -> Vec<Ipv4Net> {
    let remote = state
        .fallback_client
        .get("https://www.cloudflare.com/ips-v4")
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .ok()
        .and_then(|response| response.error_for_status().ok());
    let text = match remote {
        Some(response) => response.text().await.ok(),
        None => None,
    };
    let parsed = parse_prefixes(text.as_deref().unwrap_or(""));
    if parsed.is_empty() {
        parse_prefixes(&CLOUDFLARE_IPV4_FALLBACK.join("\n"))
    } else {
        parsed
    }
}

fn parse_prefixes(value: &str) -> Vec<Ipv4Net> {
    let mut seen = HashSet::new();
    value
        .lines()
        .filter_map(|line| line.trim().parse::<Ipv4Net>().ok())
        .filter(|network| seen.insert(*network))
        .collect()
}

fn sample_candidate_ips(prefixes: &[Ipv4Net]) -> Vec<Ipv4Addr> {
    let mut output = Vec::new();
    let mut seen = HashSet::new();
    for prefix in prefixes {
        let host_bits = 32u32.saturating_sub(prefix.prefix_len() as u32);
        let address_count = 1u64 << host_bits.min(32);
        if address_count <= 2 {
            continue;
        }
        let base = u32::from(prefix.network()) as u64;
        let usable = address_count - 2;
        for index in 0..CANDIDATES_PER_PREFIX {
            let seed = crypto_utils::sha256_hex_str(&format!("{prefix}:{index}:fn-knock"));
            let sample = u64::from_str_radix(seed.get(..16).unwrap_or("0"), 16).unwrap_or(0);
            let offset = 1 + sample % usable;
            let ip = Ipv4Addr::from((base + offset) as u32);
            if prefix.contains(&ip) && seen.insert(ip) {
                output.push(ip);
            }
            if output.len() >= MAX_CANDIDATES {
                return output;
            }
        }
    }
    output
}

async fn probe_latency(ip: Ipv4Addr) -> Option<(f64, f64, f64)> {
    let client = speedtest_client(SPEEDTEST_HOST, ip, Duration::from_secs(4)).ok()?;
    let mut samples = Vec::new();
    for _ in 0..LATENCY_PROBES {
        let started = Instant::now();
        let response = client
            .get(format!("https://{SPEEDTEST_HOST}{SPEEDTEST_PATH}?bytes=0"))
            .header(reqwest::header::CACHE_CONTROL, "no-store")
            .send()
            .await;
        if response.is_ok_and(|response| response.status().is_success()) {
            samples.push(started.elapsed().as_secs_f64() * 1000.0);
        }
    }
    let loss = 1.0 - samples.len() as f64 / LATENCY_PROBES as f64;
    if samples.len() < 2 {
        return None;
    }
    samples.sort_by(|left, right| left.partial_cmp(right).unwrap_or(Ordering::Equal));
    let latency = median(&samples)?;
    let jitter =
        samples.last().copied().unwrap_or(latency) - samples.first().copied().unwrap_or(latency);
    Some((latency, jitter, loss))
}

async fn probe_download(ip: Ipv4Addr, bytes: usize) -> Option<f64> {
    let client = speedtest_client(SPEEDTEST_HOST, ip, Duration::from_secs(12)).ok()?;
    let started = Instant::now();
    let mut response = client
        .get(format!(
            "https://{SPEEDTEST_HOST}{SPEEDTEST_PATH}?bytes={bytes}"
        ))
        .header(reqwest::header::CACHE_CONTROL, "no-store")
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?;
    let mut received = 0usize;
    while received < bytes {
        let chunk = response.chunk().await.ok()??;
        received = received.saturating_add(chunk.len().min(bytes - received));
    }
    if received < bytes / 2 {
        return None;
    }
    let seconds = started.elapsed().as_secs_f64().max(0.001);
    Some(received as f64 * 8.0 / seconds / 1_000_000.0)
}

fn speedtest_client(
    hostname: &str,
    ip: Ipv4Addr,
    timeout: Duration,
) -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(2))
        .timeout(timeout)
        .https_only(true)
        .resolve(hostname, SocketAddr::new(IpAddr::V4(ip), 443))
        .build()
}

async fn probe_custom_hostname(hostname: &str, ip: Ipv4Addr) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(8))
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .resolve(hostname, SocketAddr::new(IpAddr::V4(ip), 443))
        .build()
        .map_err(|error| error.to_string())?;
    let response = client
        .get(format!("https://{hostname}/"))
        .header(reqwest::header::CACHE_CONTROL, "no-store")
        .send()
        .await
        .map_err(|error| format!("Preferred edge TLS probe failed: {error}"))?;
    let status = response.status();
    let has_cf_ray = response.headers().contains_key("cf-ray");
    let mut response = response;
    let mut body = Vec::new();
    while body.len() < 32 * 1024 {
        let chunk = match response.chunk().await {
            Ok(Some(chunk)) => chunk,
            Ok(None) => break,
            Err(error) => return Err(format!("Preferred edge response failed: {error}")),
        };
        let remaining = 32 * 1024 - body.len();
        body.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
    }
    let body = String::from_utf8_lossy(&body).to_ascii_lowercase();
    if body.contains("error 1000")
        || body.contains("dns points to prohibited ip")
        || body.contains("error 1016")
        || matches!(status.as_u16(), 520..=527 | 530)
        || (body.contains("cloudflare") && body.contains("error code"))
    {
        return Err("Cloudflare rejected the preferred edge route".to_string());
    }
    if !has_cf_ray {
        return Err(format!(
            "Preferred edge returned HTTP {status} without a Cloudflare Ray ID"
        ));
    }
    Ok(())
}

fn score_candidate(latency: f64, jitter: f64, loss: f64, download_mbps: f64) -> f64 {
    latency + 2.0 * jitter + 1500.0 * loss + 800.0 / download_mbps.max(1.0)
}

fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        None
    } else if values.len() % 2 == 1 {
        values.get(values.len() / 2).copied()
    } else {
        let right = values.len() / 2;
        Some((values[right - 1] + values[right]) / 2.0)
    }
}

#[allow(clippy::too_many_arguments)]
async fn inspect_auxiliary_dns(
    api: &CloudflareApi,
    zone_id: &str,
    name: &str,
    owned_id: Option<&str>,
    logical_id: &str,
    instance_id: &str,
    record_type: &str,
    content: Option<&str>,
    proxied: bool,
    operations: &mut Vec<Value>,
    conflicts: &mut Vec<Value>,
    remote_snapshot: &mut Vec<Value>,
) -> Result<(), CloudflareApiError> {
    let records = api.list_dns_records(zone_id, Some(name)).await?;
    remote_snapshot.push(json!({
        "hostname": name,
        "dnsRecords": records.clone(),
    }));
    if records.is_empty() {
        operations.push(preview_operation(logical_id, "dns", "create", name, false));
        return Ok(());
    }
    let record = owned_id
        .and_then(|id| {
            records
                .iter()
                .find(|record| record.get("id").and_then(Value::as_str) == Some(id))
        })
        .or_else(|| {
            records
                .iter()
                .find(|record| is_managed_dns(record, instance_id))
        })
        .unwrap_or(&records[0]);
    if dns_record_owned_for_update(record, owned_id, instance_id, record_type, content, proxied) {
        operations.push(preview_operation(logical_id, "dns", "update", name, true));
    } else {
        conflicts.push(json!({
            "id": logical_id,
            "kind": "dns",
            "target": name,
            "message": "An unowned DNS record already uses the optimization hostname",
            "takeoverAllowed": true,
        }));
    }
    Ok(())
}

fn extract_validation_records(custom: &Value) -> Vec<(String, String)> {
    let mut output = Vec::new();
    if let Some(record) = custom.get("ownership_verification") {
        let name = record
            .get("name")
            .or_else(|| record.get("txt_name"))
            .and_then(Value::as_str);
        let value = record
            .get("value")
            .or_else(|| record.get("txt_value"))
            .or_else(|| record.get("txt_record"))
            .and_then(Value::as_str);
        if let (Some(name), Some(value)) = (name, value) {
            output.push((name.to_string(), value.to_string()));
        }
    }
    for record in custom
        .pointer("/ssl/validation_records")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let name = record
            .get("txt_name")
            .or_else(|| record.get("name"))
            .and_then(Value::as_str);
        let value = record
            .get("txt_value")
            .or_else(|| record.get("txt_record"))
            .or_else(|| record.get("value"))
            .and_then(Value::as_str);
        if let (Some(name), Some(value)) = (name, value) {
            output.push((name.to_string(), value.to_string()));
        }
    }
    output.sort();
    output.dedup();
    output
}

fn set_host_state(ownership: &mut Value, hostname: &str, value: Value) {
    ensure_nested_object(ownership, &["optimization", "customHostnames"])
        .insert(hostname.to_string(), value);
}

fn ensure_nested_object<'a>(value: &'a mut Value, path: &[&str]) -> &'a mut Map<String, Value> {
    let mut current = value;
    for segment in path {
        let object = ensure_object(current);
        current = object
            .entry((*segment).to_string())
            .or_insert_with(|| json!({}));
    }
    ensure_object(current)
}

fn ensure_object(value: &mut Value) -> &mut Map<String, Value> {
    if !value.is_object() {
        *value = json!({});
    }
    value
        .as_object_mut()
        .expect("value was normalized to object")
}

fn preview_operation(id: &str, kind: &str, action: &str, target: &str, owned: bool) -> Value {
    json!({ "id": id, "kind": kind, "action": action, "target": target, "owned": owned })
}

fn is_managed_dns(record: &Value, instance_id: &str) -> bool {
    let expected_comment = format!("Managed by fn-knock ({instance_id})");
    let expected_tag = format!("fn-knock-instance:{instance_id}");
    record
        .get("comment")
        .and_then(Value::as_str)
        .is_some_and(|value| value == expected_comment)
        || record
            .get("tags")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .any(|value| value.as_str() == Some(expected_tag.as_str()))
}

fn should_publish_exact_routes(ownership: &Value, force_publish: bool) -> bool {
    force_publish
        || ownership
            .pointer("/optimization/publishSuppressed")
            .and_then(Value::as_bool)
            != Some(true)
}

fn exact_route_is_optimized(state: &Value) -> bool {
    state
        .get("exactDnsId")
        .and_then(Value::as_str)
        .is_some_and(|id| !id.is_empty())
        && (state.get("exactDnsTarget").and_then(Value::as_str) == Some("edge")
            || (state.get("exactDnsTarget").is_none()
                && state.get("status").and_then(Value::as_str) == Some("optimized")))
}

fn optimized_health_hostname(ownership: &Value) -> Option<String> {
    ownership
        .pointer("/optimization/customHostnames")
        .and_then(Value::as_object)
        .and_then(|items| {
            items.iter().find_map(|(hostname, state)| {
                (exact_route_is_optimized(state)
                    && state.get("sslStatus").and_then(Value::as_str) == Some("active"))
                .then(|| hostname.clone())
            })
        })
}

fn legacy_publish_suppression(ownership: &Value, runtime: &Value) -> bool {
    ownership
        .pointer("/optimization/fallbackActive")
        .and_then(Value::as_bool)
        == Some(true)
        && matches!(
            runtime.get("lastSwitchReason").and_then(Value::as_str),
            Some("manual-fallback" | "health-fallback")
        )
}

fn cloudflare_error_list_message(errors: &Value) -> Option<String> {
    let messages = errors
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| {
            item.as_str()
                .or_else(|| item.get("message").and_then(Value::as_str))
                .map(str::trim)
                .filter(|value| !value.is_empty())
        })
        .collect::<Vec<_>>();
    (!messages.is_empty()).then(|| messages.join("; "))
}

fn scan_job_active(job: &Value) -> bool {
    matches!(
        job.get("status").and_then(Value::as_str),
        Some("queued" | "running")
    ) && job.get("cancelRequested").and_then(Value::as_bool) != Some(true)
}

fn json_pointer_escape(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

fn weekly_jitter_ms() -> i64 {
    let value = u64::from_le_bytes(crypto_utils::random_bytes::<8>());
    (value % (6 * 60 * 60 * 1000)) as i64
}

async fn update_job(state: &AppState, id: &str, patch: Value) {
    let mut jobs = state.cloudflared_scan_jobs.write().await;
    let Some(job) = jobs.get_mut(id) else {
        return;
    };
    let target = ensure_object(job);
    if let Some(patch) = patch.as_object() {
        for (key, value) in patch {
            target.insert(key.clone(), value.clone());
        }
    }
}

async fn is_job_cancelled(state: &AppState, id: &str) -> bool {
    state
        .cloudflared_scan_jobs
        .read()
        .await
        .get(id)
        .and_then(|job| job.get("cancelRequested"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

async fn load_runtime(state: &AppState) -> Value {
    state
        .store
        .get_json_value(OPTIMIZATION_RUNTIME_KEY)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| {
            json!({
                "healthFailures": 0,
                "lastHealthAt": Value::Null,
                "lastFullScanAt": Value::Null,
                "nextFullScanAt": Value::Null,
                "lastError": Value::Null,
            })
        })
}

async fn save_runtime(state: &AppState, value: &Value) -> Result<(), CloudflareApiError> {
    state
        .store
        .set_json_value(OPTIMIZATION_RUNTIME_KEY, value)
        .await
        .map_err(local_error_display)
}

fn api_error_response(error: CloudflareApiError) -> Response {
    let status = match error.status {
        Some(StatusCode::UNAUTHORIZED) | Some(StatusCode::FORBIDDEN) => StatusCode::FORBIDDEN,
        Some(StatusCode::CONFLICT) => StatusCode::CONFLICT,
        Some(StatusCode::NOT_FOUND) => StatusCode::NOT_FOUND,
        Some(status) if status.is_client_error() => StatusCode::BAD_REQUEST,
        _ => StatusCode::BAD_GATEWAY,
    };
    response::error(status, error.to_string())
}

pub(super) fn is_capability_unsupported_api_error(error: &CloudflareApiError) -> bool {
    if !matches!(
        error.status,
        Some(StatusCode::BAD_REQUEST | StatusCode::FORBIDDEN | StatusCode::PAYMENT_REQUIRED)
    ) {
        return false;
    }
    let message = error.message.to_ascii_lowercase();
    [
        "not entitled",
        "not enabled for this zone",
        "not available on your plan",
        "plan does not support",
        "requires an enterprise plan",
        "upgrade your plan",
        "no quota has been allocated",
        "(1404)",
    ]
    .iter()
    .any(|needle| message.contains(needle))
}

fn optimization_is_enabled(managed: &Value) -> bool {
    managed.get("mode").and_then(Value::as_str) == Some("managed")
        && managed.get("optimizationEnabled").and_then(Value::as_bool) == Some(true)
}

fn local_error(message: impl Into<String>) -> CloudflareApiError {
    CloudflareApiError {
        status: None,
        message: message.into(),
    }
}

fn local_error_display(error: impl std::fmt::Display) -> CloudflareApiError {
    local_error(error.to_string())
}

fn ignore_not_found(result: Result<(), CloudflareApiError>) -> Result<(), CloudflareApiError> {
    match result {
        Err(error) if error.status == Some(StatusCode::NOT_FOUND) => Ok(()),
        other => other,
    }
}

async fn delete_dns_if_owned(
    api: &CloudflareApi,
    zone_id: &str,
    owned: &Value,
    instance_id: &str,
) -> Result<(), CloudflareApiError> {
    let id = owned.get("id").and_then(Value::as_str).unwrap_or("");
    let name = owned.get("name").and_then(Value::as_str).unwrap_or("");
    let record_type = owned.get("type").and_then(Value::as_str).unwrap_or("");
    let content = owned.get("content").and_then(Value::as_str);
    let proxied = owned
        .get("proxied")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if id.is_empty() || name.is_empty() || record_type.is_empty() {
        return Err(CloudflareApiError {
            status: Some(StatusCode::CONFLICT),
            message: "Managed DNS ownership metadata is incomplete; refusing automatic deletion"
                .to_string(),
        });
    }
    let records = api.list_dns_records(zone_id, Some(name)).await?;
    let Some(remote) = records
        .iter()
        .find(|record| record.get("id").and_then(Value::as_str) == Some(id))
    else {
        return Ok(());
    };
    if !dns_record_owned_for_update(remote, Some(id), instance_id, record_type, content, proxied) {
        return Err(CloudflareApiError {
            status: Some(StatusCode::CONFLICT),
            message: format!(
                "DNS record {name} was claimed or changed by another configuration; refusing automatic deletion"
            ),
        });
    }
    ignore_not_found(api.delete_dns_record(zone_id, id).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_official_ranges_deterministically_and_within_bounds() {
        let prefixes = parse_prefixes(&CLOUDFLARE_IPV4_FALLBACK.join("\n"));
        let first = sample_candidate_ips(&prefixes);
        let second = sample_candidate_ips(&prefixes);
        assert_eq!(first, second);
        assert!(!first.is_empty());
        assert!(first.len() <= MAX_CANDIDATES);
        assert!(
            first
                .iter()
                .all(|ip| prefixes.iter().any(|prefix| prefix.contains(ip)))
        );
    }

    #[test]
    fn score_penalizes_loss_latency_jitter_and_low_bandwidth() {
        let baseline = score_candidate(30.0, 2.0, 0.0, 100.0);
        assert!(score_candidate(60.0, 2.0, 0.0, 100.0) > baseline);
        assert!(score_candidate(30.0, 20.0, 0.0, 100.0) > baseline);
        assert!(score_candidate(30.0, 2.0, 0.2, 100.0) > baseline);
        assert!(score_candidate(30.0, 2.0, 0.0, 5.0) > baseline);
    }

    #[test]
    fn extracts_txt_dcv_records_from_both_cloudflare_shapes() {
        let value = json!({
            "ownership_verification": { "type": "txt", "name": "_cf.example.com", "value": "owner" },
            "ssl": { "validation_records": [
                { "status": "pending", "txt_name": "_acme.example.com", "txt_record": "ssl" }
            ] }
        });
        assert_eq!(
            extract_validation_records(&value),
            vec![
                ("_acme.example.com".to_string(), "ssl".to_string()),
                ("_cf.example.com".to_string(), "owner".to_string()),
            ]
        );
    }

    #[test]
    fn weekly_jitter_is_bounded_to_six_hours() {
        for _ in 0..32 {
            assert!((0..6 * 60 * 60 * 1000).contains(&weekly_jitter_ms()));
        }
    }

    #[test]
    fn capability_errors_only_disable_known_unsupported_plans() {
        let unsupported = CloudflareApiError {
            status: Some(StatusCode::FORBIDDEN),
            message: "This feature is not available on your plan".to_string(),
        };
        assert!(is_capability_unsupported_api_error(&unsupported));

        let missing_quota = CloudflareApiError {
            status: Some(StatusCode::BAD_REQUEST),
            message: "No quota has been allocated for this zone or for this account. (1404)"
                .to_string(),
        };
        assert!(is_capability_unsupported_api_error(&missing_quota));

        for error in [
            CloudflareApiError {
                status: Some(StatusCode::TOO_MANY_REQUESTS),
                message: "rate limited".to_string(),
            },
            CloudflareApiError {
                status: Some(StatusCode::FORBIDDEN),
                message: "permission denied".to_string(),
            },
            CloudflareApiError {
                status: Some(StatusCode::CONFLICT),
                message: "hostname already exists".to_string(),
            },
        ] {
            assert!(!is_capability_unsupported_api_error(&error));
        }
    }

    #[test]
    fn scans_require_an_applied_managed_optimization_plan() {
        assert!(!optimization_is_enabled(&json!({})));
        assert!(!optimization_is_enabled(&json!({
            "mode": "managed",
            "optimizationEnabled": false,
        })));
        assert!(!optimization_is_enabled(&json!({
            "mode": "manual",
            "optimizationEnabled": true,
        })));
        assert!(optimization_is_enabled(&json!({
            "mode": "managed",
            "optimizationEnabled": true,
        })));
    }

    #[test]
    fn activation_cname_is_not_reported_as_an_optimized_route() {
        assert!(!exact_route_is_optimized(&json!({
            "exactDnsId": "dns-id",
            "exactDnsTarget": "origin",
            "status": "pending",
        })));
        assert!(exact_route_is_optimized(&json!({
            "exactDnsId": "dns-id",
            "exactDnsTarget": "edge",
            "status": "optimized",
        })));
        assert!(exact_route_is_optimized(&json!({
            "exactDnsId": "legacy-dns-id",
            "status": "optimized",
        })));
    }

    #[test]
    fn health_checks_ignore_origin_activation_and_unready_certificates() {
        let ownership = json!({
            "optimization": {
                "customHostnames": {
                    "activation.example.com": {
                        "exactDnsId": "activation-id",
                        "exactDnsTarget": "origin",
                        "status": "pending",
                        "sslStatus": "pending_validation"
                    },
                    "unready.example.com": {
                        "exactDnsId": "unready-id",
                        "exactDnsTarget": "edge",
                        "status": "optimized",
                        "sslStatus": "pending_validation"
                    },
                    "ready.example.com": {
                        "exactDnsId": "ready-id",
                        "exactDnsTarget": "edge",
                        "status": "optimized",
                        "sslStatus": "active"
                    }
                }
            }
        });
        assert_eq!(
            optimized_health_hostname(&ownership).as_deref(),
            Some("ready.example.com")
        );

        let only_activation = json!({
            "optimization": {
                "customHostnames": {
                    "activation.example.com": {
                        "exactDnsId": "activation-id",
                        "exactDnsTarget": "origin",
                        "sslStatus": "active"
                    }
                }
            }
        });
        assert_eq!(optimized_health_hostname(&only_activation), None);
    }

    #[test]
    fn legacy_publish_suppression_preserves_only_explicit_fallbacks() {
        let fallback = json!({ "optimization": { "fallbackActive": true } });
        assert!(legacy_publish_suppression(
            &fallback,
            &json!({ "lastSwitchReason": "health-fallback" })
        ));
        assert!(legacy_publish_suppression(
            &fallback,
            &json!({ "lastSwitchReason": "manual-fallback" })
        ));
        assert!(!legacy_publish_suppression(
            &fallback,
            &json!({ "lastSwitchReason": "manual-speed-test" })
        ));
        assert!(!legacy_publish_suppression(
            &json!({ "optimization": { "fallbackActive": false } }),
            &json!({ "lastSwitchReason": "health-fallback" })
        ));
    }

    #[test]
    fn dns_ownership_is_scoped_to_the_current_instance() {
        let own = json!({
            "comment": "Managed by fn-knock (instance-a)",
            "tags": ["fn-knock:managed", "fn-knock-instance:instance-a"]
        });
        let other = json!({
            "comment": "Managed by fn-knock (instance-b)",
            "tags": ["fn-knock:managed", "fn-knock-instance:instance-b"]
        });
        let legacy_generic = json!({ "tags": ["fn-knock:managed"] });
        assert!(is_managed_dns(&own, "instance-a"));
        assert!(!is_managed_dns(&other, "instance-a"));
        assert!(!is_managed_dns(&legacy_generic, "instance-a"));
    }

    #[test]
    fn fallback_suppresses_automatic_exact_route_republication() {
        let ownership = json!({ "optimization": { "publishSuppressed": true } });
        assert!(!should_publish_exact_routes(&ownership, false));
        assert!(should_publish_exact_routes(&ownership, true));
        assert!(should_publish_exact_routes(
            &json!({ "optimization": { "fallbackActive": true } }),
            false
        ));
        assert!(should_publish_exact_routes(&json!({}), false));
    }

    #[test]
    fn cancelled_scan_requests_do_not_block_the_next_serialized_scan() {
        assert!(scan_job_active(
            &json!({ "status": "running", "cancelRequested": false })
        ));
        assert!(!scan_job_active(
            &json!({ "status": "running", "cancelRequested": true })
        ));
        assert!(!scan_job_active(&json!({ "status": "completed" })));
    }
}
