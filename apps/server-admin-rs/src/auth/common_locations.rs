use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    env,
    net::IpAddr,
    str::FromStr,
    sync::{LazyLock, Mutex},
    time::Duration,
};

use ipnet::IpNet;
use serde_json::{Value, json};
use tokio::{
    task::JoinHandle,
    time::{self, MissedTickBehavior},
};

use crate::{
    http_utils::{is_private_or_local_ip, normalize_ip},
    ip_location::ensure_ip_locations_enqueued,
    scanner,
    state::AppState,
    time_utils,
};

const RUNTIME_KEY: &str = "fn_knock:common_auth_locations:runtime";
const RECENT_WINDOW_SECONDS: i64 = 7 * 24 * 3600;
const KNOWN_COUNTRY_CHINA: &str = "中国";
static SCHEDULED_REBUILD: LazyLock<Mutex<ScheduledRebuild>> =
    LazyLock::new(|| Mutex::new(ScheduledRebuild::default()));
static RECENT_AUTH_IP_TOUCHES: LazyLock<Mutex<HashMap<(String, String), i64>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
const RECENT_AUTH_IP_TOUCH_MIN_INTERVAL_SECONDS: i64 = 30;
const MAX_RECENT_AUTH_IP_TOUCHES: usize = 4096;

#[derive(Default)]
struct ScheduledRebuild {
    next_id: u64,
    task: Option<(u64, JoinHandle<()>)>,
}

#[derive(Clone, Debug)]
struct RecentAuthIpEntry {
    ip: String,
    first_seen_at: i64,
    last_seen_at: i64,
    seen_count: i64,
}

#[derive(Clone, Debug)]
struct ResolvedSample {
    entry: RecentAuthIpEntry,
    location: Value,
}

#[derive(Clone, Debug)]
struct LocationGroup {
    key: String,
    country: String,
    province: String,
    city: String,
    isp: String,
    samples: Vec<ResolvedSample>,
}

pub fn start_common_auth_location_tasks(state: AppState) {
    tokio::spawn(async move {
        tokio::select! {
            _ = state.shutdown.cancelled() => return,
            result = rebuild_common_auth_locations_runtime_state(&state) => {
                if let Err(error) = result {
                    tracing::warn!(%error, "failed to rebuild common auth locations on boot");
                }
            }
        }
        let mut ticker = time::interval(std::time::Duration::from_secs(5 * 60));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        ticker.tick().await;
        loop {
            tokio::select! {
                _ = state.shutdown.cancelled() => break,
                _ = ticker.tick() => {}
            }
            tokio::select! {
                _ = state.shutdown.cancelled() => break,
                result = rebuild_common_auth_locations_runtime_state(&state) => {
                    if let Err(error) = result {
                        tracing::warn!(%error, "failed to rebuild common auth locations");
                    }
                }
            }
        }
    });
}

pub async fn record_recent_verified_ip(state: &AppState, ip: &str) -> anyhow::Result<()> {
    let normalized = normalize_ip(ip);
    if normalized.is_empty() {
        return Ok(());
    }
    let now = now_seconds();
    let store_key = state.settings.sqlite_path.to_string_lossy().into_owned();
    if !claim_recent_auth_ip_touch(&store_key, &normalized, now) {
        return Ok(());
    }
    if let Err(error) = state.store.record_recent_auth_ip(&normalized, now).await {
        release_recent_auth_ip_touch(&store_key, &normalized, now);
        return Err(error.into());
    }
    schedule_common_auth_locations_rebuild(state.clone(), "recent-auth-ip");
    Ok(())
}

fn claim_recent_auth_ip_touch(store_key: &str, ip: &str, now: i64) -> bool {
    let mut touches = RECENT_AUTH_IP_TOUCHES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    claim_recent_auth_ip_touch_in(&mut touches, store_key, ip, now)
}

fn claim_recent_auth_ip_touch_in(
    touches: &mut HashMap<(String, String), i64>,
    store_key: &str,
    ip: &str,
    now: i64,
) -> bool {
    let key = (store_key.to_string(), ip.to_string());
    if touches.get(&key).is_some_and(|last_seen| {
        now >= *last_seen && now - *last_seen < RECENT_AUTH_IP_TOUCH_MIN_INTERVAL_SECONDS
    }) {
        return false;
    }
    if touches.len() >= MAX_RECENT_AUTH_IP_TOUCHES {
        touches.retain(|_, last_seen| now >= *last_seen && now - *last_seen < 3600);
        if touches.len() >= MAX_RECENT_AUTH_IP_TOUCHES {
            // Clearing only loses write coalescing; it never grants access.
            touches.clear();
        }
    }
    touches.insert(key, now);
    true
}

fn release_recent_auth_ip_touch(store_key: &str, ip: &str, claimed_at: i64) {
    let mut touches = RECENT_AUTH_IP_TOUCHES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let key = (store_key.to_string(), ip.to_string());
    if touches.get(&key) == Some(&claimed_at) {
        touches.remove(&key);
    }
}

pub async fn is_common_auth_location_exempt_ip(state: &AppState, ip: &str) -> anyhow::Result<bool> {
    let normalized = normalize_ip(ip);
    if normalized.is_empty() || is_private_or_local_ip(&normalized) {
        return Ok(false);
    }

    let runtime = state
        .store
        .get_json_value(RUNTIME_KEY)
        .await?
        .unwrap_or_else(|| json!({}));
    if runtime.get("enabled").and_then(Value::as_bool) != Some(true) {
        return Ok(false);
    }
    let Ok(ip) = normalized.parse::<IpAddr>() else {
        return Ok(false);
    };
    Ok(runtime
        .get("cidrs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter_map(|cidr| IpNet::from_str(cidr.trim()).ok())
        .any(|network| network.contains(&ip)))
}

pub async fn rebuild_common_auth_locations_runtime_state(
    state: &AppState,
) -> anyhow::Result<Value> {
    if !common_auth_location_exemptions_enabled(state).await? {
        return sync_disabled_common_auth_locations_runtime(state).await;
    }

    let max_recent_ips = strict_env_i64("COMMON_AUTH_LOCATIONS_MAX_IPS", 1000).max(10) as usize;
    let max_locations = strict_env_i64("COMMON_AUTH_LOCATIONS_MAX_LOCATIONS", 5).max(1) as usize;
    let max_cidrs = strict_env_i64("COMMON_AUTH_LOCATIONS_MAX_CIDRS", 1000).max(1) as usize;
    let max_region_cidrs =
        strict_env_i64("COMMON_AUTH_LOCATIONS_MAX_REGION_CIDRS_PER_LOCATION", 128).max(0) as usize;

    let entries = state
        .store
        .list_recent_auth_ips_with_scores(now_seconds(), max_recent_ips)
        .await?
        .into_iter()
        .filter_map(parse_recent_auth_ip_entry)
        .filter(|entry| !is_private_or_local_ip(&entry.ip))
        .collect::<Vec<_>>();
    let mut samples = Vec::new();
    let mut pending_ips = Vec::new();
    for entry in &entries {
        let cached = state.store.get_ip_location_cache(&entry.ip).await?;
        collect_resolved_sample_or_pending(entry, cached, &mut samples, &mut pending_ips);
    }
    if !pending_ips.is_empty() {
        ensure_ip_locations_enqueued(state, pending_ips.clone()).await?;
    }

    let resolved_sample_count = samples.len();
    let groups = scored_location_groups(samples, now_seconds(), max_locations);
    let mut locations = Vec::new();
    let mut all_cidrs = Vec::new();
    let mut seen_cidrs = BTreeSet::new();
    for group in groups {
        if all_cidrs.len() >= max_cidrs {
            break;
        }
        let (region_cidrs, cidr_error) = resolve_region_cidrs(state, &group)
            .await
            .unwrap_or_else(|error| (Vec::new(), Some(error.to_string())));
        let region_cidrs = region_cidrs
            .into_iter()
            .take(max_region_cidrs)
            .collect::<Vec<_>>();
        let sample_cidrs = derive_sample_cidrs(&group);
        let sample_set = sample_cidrs.iter().cloned().collect::<BTreeSet<_>>();
        let region_set = region_cidrs.iter().cloned().collect::<BTreeSet<_>>();
        let cidrs = normalize_cidr_lines(sample_cidrs.into_iter().chain(region_cidrs));
        let mut selected = Vec::new();
        let mut selected_sample = false;
        let mut selected_region = false;
        for cidr in cidrs {
            if seen_cidrs.contains(&cidr) {
                continue;
            }
            selected_sample |= sample_set.contains(&cidr);
            selected_region |= region_set.contains(&cidr);
            seen_cidrs.insert(cidr.clone());
            selected.push(cidr);
            if all_cidrs.len() + selected.len() >= max_cidrs {
                break;
            }
        }
        all_cidrs.extend(selected.clone());
        let cidr_source = match (selected_sample, selected_region) {
            (true, true) => "mixed",
            (false, true) => "region",
            _ => "sample",
        };
        let stats = score_group(&group, now_seconds());
        let mut location = json!({
            "key": group.key,
            "label": location_label(&group),
            "country": group.country,
            "province": group.province,
            "city": group.city,
            "isp": group.isp,
            "ip_count": group.samples.len(),
            "seen_count": stats.seen_count,
            "ips": group.samples.iter().map(|sample| sample.entry.ip.clone()).collect::<BTreeSet<_>>().into_iter().collect::<Vec<_>>(),
            "first_seen_at": stats.first_seen_at,
            "last_seen_at": stats.last_seen_at,
            "score": (stats.score * 100.0).round() / 100.0,
            "confidence": stats.confidence,
            "cidrs": selected,
            "cidr_source": cidr_source,
        });
        if let Some(error) = cidr_error
            && let Some(object) = location.as_object_mut()
        {
            object.insert("cidr_error".to_string(), Value::String(error));
        }
        locations.push(location);
    }

    let runtime = json!({
        "enabled": !all_cidrs.is_empty(),
        "cidrs": normalize_cidr_lines(all_cidrs),
        "locations": locations,
        "sample_count": entries.len(),
        "resolved_sample_count": resolved_sample_count,
        "pending_ip_count": pending_ips.len(),
        "updated_at": time_utils::now_iso(),
    });
    state
        .store
        .set_string_value(RUNTIME_KEY, &serde_json::to_string(&runtime)?)
        .await?;
    sync_common_auth_locations_to_gateway(state, &runtime).await?;
    if !pending_ips.is_empty() {
        schedule_common_auth_locations_rebuild_after(
            state.clone(),
            "ip-location-refresh",
            common_auth_locations_location_retry_delay(),
        );
    }
    Ok(runtime)
}

async fn common_auth_location_exemptions_enabled(state: &AppState) -> anyhow::Result<bool> {
    let config = state.store.get_config().await?;
    let waf = config.get("waf").unwrap_or(&Value::Null);
    let waf_enabled = waf.get("enabled").and_then(Value::as_bool) == Some(true)
        && waf
            .get("common_location_exempt_enabled")
            .and_then(Value::as_bool)
            == Some(true);
    let scanner_settings = state.store.scanner_settings_raw().await?;
    let scanner_enabled = scanner_settings
        .as_ref()
        .and_then(|value| value.get("enabled"))
        .and_then(Value::as_bool)
        == Some(true)
        && scanner_settings
            .as_ref()
            .and_then(|value| value.get("commonLocationExemptEnabled"))
            .and_then(Value::as_bool)
            == Some(true);
    Ok(waf_enabled || scanner_enabled)
}

async fn sync_disabled_common_auth_locations_runtime(state: &AppState) -> anyhow::Result<Value> {
    if let Some(runtime) = state
        .store
        .get_string_value(RUNTIME_KEY)
        .await?
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        && runtime.get("enabled").and_then(Value::as_bool) != Some(true)
        && runtime
            .get("cidrs")
            .and_then(Value::as_array)
            .is_none_or(Vec::is_empty)
    {
        return Ok(runtime);
    }

    let runtime = json!({
        "enabled": false,
        "cidrs": [],
        "locations": [],
        "sample_count": 0,
        "resolved_sample_count": 0,
        "pending_ip_count": 0,
        "updated_at": time_utils::now_iso(),
    });
    state
        .store
        .set_string_value(RUNTIME_KEY, &serde_json::to_string(&runtime)?)
        .await?;
    sync_common_auth_locations_to_gateway(state, &runtime).await?;
    Ok(runtime)
}

fn schedule_common_auth_locations_rebuild(state: AppState, reason: &'static str) {
    schedule_common_auth_locations_rebuild_after(
        state,
        reason,
        common_auth_locations_rebuild_debounce(),
    );
}

fn schedule_common_auth_locations_rebuild_after(
    state: AppState,
    reason: &'static str,
    delay: Duration,
) {
    let Ok(mut scheduled) = SCHEDULED_REBUILD.lock() else {
        return;
    };
    if let Some((_, handle)) = scheduled.task.take() {
        handle.abort();
    }
    scheduled.next_id = scheduled.next_id.wrapping_add(1).max(1);
    let task_id = scheduled.next_id;
    scheduled.task = Some((
        task_id,
        tokio::spawn(async move {
            tokio::select! {
                _ = state.shutdown.cancelled() => return,
                _ = time::sleep(delay) => {}
            }
            {
                let Ok(mut scheduled) = SCHEDULED_REBUILD.lock() else {
                    return;
                };
                if !matches!(scheduled.task.as_ref(), Some((id, _)) if *id == task_id) {
                    return;
                }
                scheduled.task = None;
            }
            tokio::select! {
                _ = state.shutdown.cancelled() => {}
                result = rebuild_common_auth_locations_runtime_state(&state) => {
                    if let Err(error) = result {
                        tracing::warn!(%error, %reason, "failed to rebuild common auth locations");
                    }
                }
            }
        }),
    ));
}

fn common_auth_locations_rebuild_debounce() -> Duration {
    Duration::from_millis(
        strict_env_i64("COMMON_AUTH_LOCATIONS_REBUILD_DEBOUNCE_MS", 5000).max(1000) as u64,
    )
}

fn common_auth_locations_location_retry_delay() -> Duration {
    Duration::from_millis(
        strict_env_i64("COMMON_AUTH_LOCATIONS_LOCATION_RETRY_MS", 30000).max(5000) as u64,
    )
}

async fn sync_common_auth_locations_to_gateway(
    state: &AppState,
    runtime: &Value,
) -> anyhow::Result<()> {
    let config = state.store.get_config().await?;
    let waf = config.get("waf").unwrap_or(&Value::Null);
    let cidrs = runtime
        .get("cidrs")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let enabled = waf.get("enabled").and_then(Value::as_bool).unwrap_or(false)
        && waf
            .get("common_location_exempt_enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        && runtime
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        && !cidrs.is_empty();
    let payload = json!({
        "enabled": enabled,
        "waf_enabled": enabled,
        "cidrs": if enabled { cidrs } else { Vec::<String>::new() },
        "updated_at": runtime.get("updated_at").cloned().unwrap_or(Value::Null),
    });
    let (status, response) = state
        .go_backend
        .set_common_location_exemptions(&payload)
        .await?;
    if let Some(error) = common_location_sync_failure(status, &response) {
        anyhow::bail!(error);
    }
    Ok(())
}

fn common_location_sync_failure(status: reqwest::StatusCode, response: &Value) -> Option<String> {
    if status == reqwest::StatusCode::NOT_FOUND || status == reqwest::StatusCode::NOT_IMPLEMENTED {
        return None;
    }
    if status.is_success() && response.get("success").and_then(Value::as_bool) == Some(true) {
        return None;
    }
    Some(
        response
            .get("message")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|message| !message.is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(|| format!("Failed to sync common location exemptions ({status})")),
    )
}

fn parse_recent_auth_ip_entry(value: Value) -> Option<RecentAuthIpEntry> {
    let ip = value
        .get("ip")
        .and_then(Value::as_str)
        .map(normalize_ip)
        .filter(|value| !value.is_empty())?;
    Some(RecentAuthIpEntry {
        ip,
        first_seen_at: value
            .get("firstSeenAt")
            .and_then(Value::as_i64)
            .unwrap_or(0),
        last_seen_at: value.get("lastSeenAt").and_then(Value::as_i64).unwrap_or(0),
        seen_count: value
            .get("seenCount")
            .and_then(Value::as_i64)
            .unwrap_or(1)
            .max(1),
    })
}

fn collect_resolved_sample_or_pending(
    entry: &RecentAuthIpEntry,
    cached_location: Option<Value>,
    samples: &mut Vec<ResolvedSample>,
    pending_ips: &mut Vec<String>,
) {
    match cached_location {
        Some(location) => samples.push(ResolvedSample {
            entry: entry.clone(),
            location,
        }),
        None => pending_ips.push(entry.ip.clone()),
    }
}

fn scored_location_groups(
    samples: Vec<ResolvedSample>,
    now_seconds: i64,
    max_locations: usize,
) -> Vec<LocationGroup> {
    let mut groups = BTreeMap::<String, LocationGroup>::new();
    for sample in samples {
        let Some(key) = location_key(&sample.location) else {
            continue;
        };
        let country = string_field(&sample.location, "country");
        let province = string_field(&sample.location, "province");
        let city = string_field(&sample.location, "city");
        let isp = string_field(&sample.location, "isp");
        let group = groups.entry(key.clone()).or_insert_with(|| LocationGroup {
            key,
            country,
            province,
            city,
            isp: isp.clone(),
            samples: Vec::new(),
        });
        if !group.isp.is_empty() && group.isp != isp {
            group.isp.clear();
        }
        group.samples.push(sample);
    }
    let mut groups = groups.into_values().collect::<Vec<_>>();
    groups.retain(|group| score_group(group, now_seconds).confidence != "low");
    groups.sort_by(|left, right| {
        let left_score = score_group(left, now_seconds);
        let right_score = score_group(right, now_seconds);
        right_score
            .score
            .partial_cmp(&left_score.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| right_score.last_seen_at.cmp(&left_score.last_seen_at))
    });
    groups.truncate(max_locations);
    groups
}

struct GroupScore {
    first_seen_at: i64,
    last_seen_at: i64,
    seen_count: i64,
    score: f64,
    confidence: &'static str,
}

fn score_group(group: &LocationGroup, now_seconds: i64) -> GroupScore {
    let first_seen_at = group
        .samples
        .iter()
        .map(|sample| sample.entry.first_seen_at)
        .min()
        .unwrap_or_default();
    let last_seen_at = group
        .samples
        .iter()
        .map(|sample| sample.entry.last_seen_at)
        .max()
        .unwrap_or_default();
    let seen_count = group
        .samples
        .iter()
        .map(|sample| sample.entry.seen_count.max(1))
        .sum::<i64>();
    let age_seconds = (now_seconds - last_seen_at).max(0);
    let recent = age_seconds <= RECENT_WINDOW_SECONDS;
    let recency_score = (30.0 - age_seconds as f64 / 86_400.0).max(0.0);
    let score =
        group.samples.len() as f64 * 100.0 + seen_count.min(50) as f64 * 5.0 + recency_score;
    let confidence = if (recent && (group.samples.len() >= 3 || seen_count >= 10))
        || (group.samples.len() >= 2 && seen_count >= 8)
    {
        "high"
    } else if group.samples.len() >= 2 || seen_count >= 5 {
        "medium"
    } else {
        "low"
    };
    GroupScore {
        first_seen_at,
        last_seen_at,
        seen_count,
        score,
        confidence,
    }
}

async fn resolve_region_cidrs(
    state: &AppState,
    group: &LocationGroup,
) -> anyhow::Result<(Vec<String>, Option<String>)> {
    if group.country != KNOWN_COUNTRY_CHINA || group.province.is_empty() || group.city.is_empty() {
        return Ok((Vec::new(), None));
    }
    match scanner::lookup_cidr_region(state, &group.province, Some(&group.city)).await {
        Ok(result) => Ok((normalize_cidr_lines(result.cidrs), None)),
        Err(error) => Ok((Vec::new(), Some(error))),
    }
}

fn derive_sample_cidrs(group: &LocationGroup) -> Vec<String> {
    let ips = group
        .samples
        .iter()
        .map(|sample| sample.entry.ip.clone())
        .collect::<Vec<_>>();
    let mut cidrs = Vec::new();
    let mut buckets = BTreeMap::<String, usize>::new();
    for ip in &ips {
        if let Ok(IpAddr::V4(addr)) = ip.parse::<IpAddr>() {
            let octets = addr.octets();
            *buckets
                .entry(format!("{}.{}.{}.0/24", octets[0], octets[1], octets[2]))
                .or_default() += 1;
        }
    }
    cidrs.extend(
        buckets
            .into_iter()
            .filter(|(_, count)| *count >= 2)
            .map(|(cidr, _)| cidr),
    );
    for ip in ips {
        match ip.parse::<IpAddr>() {
            Ok(IpAddr::V4(_)) => cidrs.push(format!("{ip}/32")),
            Ok(IpAddr::V6(_)) => cidrs.push(format!("{ip}/128")),
            Err(_) => {}
        }
    }
    normalize_cidr_lines(cidrs)
}

fn normalize_cidr_lines<I>(cidrs: I) -> Vec<String>
where
    I: IntoIterator<Item = String>,
{
    let mut seen = BTreeSet::new();
    let mut normalized = Vec::new();
    for cidr in cidrs {
        let Ok(parsed) = IpNet::from_str(cidr.trim()) else {
            continue;
        };
        let item = match parsed {
            IpNet::V4(network) => format!("{}/{}", network.network(), network.prefix_len()),
            IpNet::V6(network) => format!("{}/{}", network.network(), network.prefix_len()),
        };
        if seen.insert(item.to_ascii_lowercase()) {
            normalized.push(item);
        }
    }
    normalized
}

fn location_key(location: &Value) -> Option<String> {
    let parts = [
        string_field(location, "country"),
        string_field(location, "province"),
        string_field(location, "city"),
    ]
    .into_iter()
    .filter(|part| !part.is_empty())
    .collect::<Vec<_>>();
    (!parts.is_empty()).then(|| parts.join("|"))
}

fn location_label(group: &LocationGroup) -> String {
    let parts: Vec<&str> = if group.country == KNOWN_COUNTRY_CHINA {
        vec![&group.province, &group.city, &group.isp]
    } else {
        vec![&group.country, &group.province, &group.city, &group.isp]
    };
    let label = parts
        .into_iter()
        .filter(|part| !part.trim().is_empty())
        .map(|part| part.trim())
        .collect::<Vec<_>>()
        .join(" / ");
    if label.is_empty() {
        group.key.clone()
    } else {
        label
    }
}

fn string_field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string()
}

fn strict_env_i64(key: &str, fallback: i64) -> i64 {
    env::var(key)
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(fallback)
}

use crate::time_utils::now_seconds;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::EnvGuard;

    fn recent_entry(ip: &str, seen_count: i64) -> RecentAuthIpEntry {
        RecentAuthIpEntry {
            ip: ip.to_string(),
            first_seen_at: 1_000,
            last_seen_at: 2_000,
            seen_count,
        }
    }

    #[test]
    fn recent_verified_ip_writes_are_coalesced_per_store_and_ip() {
        let mut touches = HashMap::new();
        assert!(claim_recent_auth_ip_touch_in(
            &mut touches,
            "store-a",
            "203.0.113.10",
            100,
        ));
        assert!(!claim_recent_auth_ip_touch_in(
            &mut touches,
            "store-a",
            "203.0.113.10",
            129,
        ));
        assert!(claim_recent_auth_ip_touch_in(
            &mut touches,
            "store-a",
            "203.0.113.10",
            130,
        ));
        assert!(claim_recent_auth_ip_touch_in(
            &mut touches,
            "store-b",
            "203.0.113.10",
            130,
        ));
        assert!(claim_recent_auth_ip_touch_in(
            &mut touches,
            "store-a",
            "203.0.113.11",
            130,
        ));
        assert!(claim_recent_auth_ip_touch_in(
            &mut touches,
            "store-a",
            "203.0.113.10",
            99,
        ));
    }

    #[test]
    fn cached_location_without_key_is_resolved_but_not_grouped_like_node() {
        let entry = recent_entry("203.0.113.9", 6);
        let mut samples = Vec::new();
        let mut pending_ips = Vec::new();

        collect_resolved_sample_or_pending(
            &entry,
            Some(json!({ "country": "", "province": "", "city": "", "isp": "Test ISP" })),
            &mut samples,
            &mut pending_ips,
        );

        assert_eq!(samples.len(), 1);
        assert!(pending_ips.is_empty());
        assert!(scored_location_groups(samples, 2_000, 5).is_empty());
    }

    #[test]
    fn non_china_location_label_includes_isp_like_node() {
        let group = LocationGroup {
            key: "United States|California|San Francisco".to_string(),
            country: "United States".to_string(),
            province: "California".to_string(),
            city: "San Francisco".to_string(),
            isp: "Example ISP".to_string(),
            samples: Vec::new(),
        };

        assert_eq!(
            location_label(&group),
            "United States / California / San Francisco / Example ISP"
        );
    }

    #[test]
    fn rebuild_debounce_has_node_floor() {
        let env = EnvGuard::new(&["COMMON_AUTH_LOCATIONS_REBUILD_DEBOUNCE_MS"]);
        env.set("COMMON_AUTH_LOCATIONS_REBUILD_DEBOUNCE_MS", "0");
        assert_eq!(
            common_auth_locations_rebuild_debounce(),
            Duration::from_millis(1_000)
        );
    }

    #[test]
    fn location_retry_delay_has_node_floor() {
        let env = EnvGuard::new(&["COMMON_AUTH_LOCATIONS_LOCATION_RETRY_MS"]);
        env.set("COMMON_AUTH_LOCATIONS_LOCATION_RETRY_MS", "1");
        assert_eq!(
            common_auth_locations_location_retry_delay(),
            Duration::from_millis(5_000)
        );
    }

    #[test]
    fn common_location_sync_failure_matches_node_gateway_semantics() {
        assert!(
            common_location_sync_failure(reqwest::StatusCode::OK, &json!({ "success": true }))
                .is_none()
        );
        assert!(common_location_sync_failure(reqwest::StatusCode::NOT_FOUND, &json!({})).is_none());
        assert!(
            common_location_sync_failure(reqwest::StatusCode::NOT_IMPLEMENTED, &json!({}))
                .is_none()
        );
        assert_eq!(
            common_location_sync_failure(
                reqwest::StatusCode::OK,
                &json!({ "success": false, "message": "gateway rejected" }),
            )
            .as_deref(),
            Some("gateway rejected")
        );
        assert!(
            common_location_sync_failure(reqwest::StatusCode::INTERNAL_SERVER_ERROR, &json!({}))
                .is_some()
        );
    }
}
