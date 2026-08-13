use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    net::Ipv4Addr,
    str::FromStr,
    time::Duration,
};

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use ipnet::Ipv4Net;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::{task::JoinSet, time};
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{response, state::AppState, time_utils};

use super::{
    cloudflare_api::{CloudflareApi, CloudflareApiError},
    managed::{
        ManagedDnsRequest, acquire_http_manage_lock, api_for_background, configured_hosts,
        dns_record_owned_for_update, load_managed_config, load_managed_state, managed_instance_id,
        managed_root_domain, save_managed_config, save_managed_state, upsert_managed_dns,
    },
};

mod api;
mod probes;
mod resolvers;
mod runtime;
mod scheduler;
mod settings;
mod state_helpers;

pub(super) use api::openapi_routes;
use api::public_source_settings;
use probes::*;
use resolvers::*;
pub(super) use runtime::is_capability_unsupported_api_error;
use runtime::{
    api_error_response, delete_dns_if_owned, ignore_not_found, is_job_cancelled, load_runtime,
    local_error, local_error_display, optimization_is_enabled, optimization_scan_error_code,
    save_runtime, update_job, weekly_jitter_ms,
};
#[cfg(test)]
use scheduler::apply_automatic_scan_result;
use scheduler::scheduled_tick;
use state_helpers::*;

#[cfg(test)]
use settings::normalize_domain_settings;
use settings::{
    default_builtin_source_ids, default_true, load_domain_settings, load_source_settings,
    normalize_candidate_hostname, normalize_source_settings, partition_optimization_hosts,
    source_settings_fingerprint,
};

const OPTIMIZATION_RUNTIME_KEY: &str = "fn_knock:cloudflared:optimization:runtime:v1";
const OPTIMIZATION_SETTINGS_KEY: &str = "fn_knock:cloudflared:optimization:settings:v1";
const OPTIMIZATION_DOMAIN_SETTINGS_KEY: &str =
    "fn_knock:cloudflared:optimization:domain-settings:v1";
const CLOUDFLARE_SAAS_REQUIRED_ERROR_CODE: &str = "cloudflare-saas-required";
const CLOUDFLARE_SAAS_REQUIRED_SCAN_ERROR: &str =
    "Cloudflare for SaaS is not enabled or available for the selected zone";
const CLOUDFLARE_SAAS_VALIDATION_PENDING_ERROR_CODE: &str = "cloudflare-saas-validation-pending";
const CLOUDFLARE_SAAS_VALIDATION_PENDING_SCAN_ERROR: &str =
    "Cloudflare for SaaS is enabled, but hostname and certificate validation is still in progress";
const CLOUDFLARE_RESOURCE_CONFLICT_ERROR_CODE: &str = "cloudflare-resource-conflict";
const CLOUDFLARE_RESOURCE_CONFLICT_SCAN_ERROR: &str =
    "Cloudflare Custom Hostname or DNS ownership conflicts must be reconciled";
const OPTIMIZATION_NOT_READY_ERROR_CODE: &str = "cloudflare-optimization-not-ready";
const OPTIMIZATION_NOT_READY_SCAN_ERROR: &str =
    "Cloudflare optimization is not ready for TLS and SNI validation";
const CANDIDATE_RESOLUTION_UNAVAILABLE_ERROR_CODE: &str =
    "cloudflare-candidate-resolution-unavailable";
const CANDIDATE_RESOLUTION_UNAVAILABLE_SCAN_ERROR: &str =
    "No verified Cloudflare candidate address could be resolved";
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
const MAX_CUSTOM_SOURCE_HOSTNAMES: usize = 16;
const WEEK_MS: i64 = 7 * 24 * 60 * 60 * 1000;
const HEALTH_INTERVAL_MS: i64 = 15 * 60 * 1000;
const CONFIRMATION_DELAY_MS: i64 = 10 * 60 * 1000;
const SCAN_APPLY_TTL_MS: i64 = 10 * 60 * 1000;

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

#[derive(Clone, Debug)]
struct RecoverableCustomHostname {
    legacy_instance_id: String,
    origin_hostname: String,
    origin_dns: Value,
    exact_dns: Value,
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

#[derive(Clone, Copy, Debug)]
struct BuiltinCandidateSource {
    id: &'static str,
    hostname: &'static str,
    category: &'static str,
}

// These hostnames are only resolved into candidate Cloudflare IPv4 addresses.
// fn-knock never publishes a customer CNAME to, or sends HTTP traffic with the
// third-party hostname/SNI to, any source in this catalog.
const BUILTIN_CANDIDATE_SOURCES: &[BuiltinCandidateSource] = &[
    BuiltinCandidateSource {
        id: "sweden-government",
        hostname: "www.gov.se",
        category: "government",
    },
    BuiltinCandidateSource {
        id: "us-library-of-congress",
        hostname: "www.loc.gov",
        category: "public-institution",
    },
    BuiltinCandidateSource {
        id: "icann",
        hostname: "www.icann.org",
        category: "internet-infrastructure",
    },
    BuiltinCandidateSource {
        id: "visa",
        hostname: "www.visa.com",
        category: "payment-infrastructure",
    },
];

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct OptimizationSourceSettings {
    #[serde(default = "default_true")]
    official_ranges: bool,
    #[serde(default = "default_builtin_source_ids")]
    builtin_ids: Vec<String>,
    #[serde(default)]
    custom_hostnames: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct OptimizationDomainSettings {
    #[serde(default)]
    external_hostnames: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateOptimizationDomainRequest {
    mode: String,
}

impl Default for OptimizationSourceSettings {
    fn default() -> Self {
        Self {
            official_ranges: true,
            builtin_ids: default_builtin_source_ids(),
            custom_hostnames: Vec::new(),
        }
    }
}

#[derive(Clone, Debug)]
struct CandidateSeed {
    ip: Ipv4Addr,
    source_types: Vec<String>,
    source_hostnames: Vec<String>,
}

#[derive(Clone, Debug)]
struct LatencyProbeMetrics {
    median_latency_ms: f64,
    jitter_ms: f64,
    loss_ratio: f64,
    colo: Option<String>,
    cf_ray: Option<String>,
}

#[derive(Clone, Debug)]
struct BusinessProbeResult {
    status: u16,
    colo: Option<String>,
    cf_ray: Option<String>,
}

#[derive(Debug)]
struct OptimizationScanResult {
    candidates: Vec<OptimizationCandidate>,
    vantage: Value,
    source_warnings: Vec<String>,
    resolver_diagnostics: Vec<ResolverDiagnostic>,
    resolution_path: String,
    source_fingerprint: String,
}

#[derive(Clone, Debug)]
struct ConfirmationSnapshot {
    pending: OptimizationCandidate,
    current: OptimizationCandidate,
    hostname: String,
    selected_at: Option<String>,
}

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
    #[serde(default)]
    source_types: Vec<String>,
    #[serde(default)]
    source_hostnames: Vec<String>,
    #[serde(default)]
    colo: Option<String>,
    #[serde(default)]
    cf_ray: Option<String>,
    #[serde(default)]
    business_hostname: Option<String>,
    #[serde(default)]
    business_status: Option<u16>,
    #[serde(default)]
    business_colo: Option<String>,
    #[serde(default)]
    business_cf_ray: Option<String>,
    #[serde(default)]
    business_validated: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApplyOptimizationRequest {
    scan_id: String,
    #[serde(default)]
    candidate_ip: Option<String>,
}

pub(super) async fn configured_optimization_hosts(
    state: &AppState,
    config: &Value,
) -> Result<Vec<String>, CloudflareApiError> {
    let settings = load_domain_settings(state).await?;
    Ok(partition_optimization_hosts(configured_hosts(config), &settings).0)
}

pub(super) fn start_tasks(state: AppState) {
    let task_state = state.clone();
    state.spawn_background("cloudflare-optimization-scheduler", async move {
        let mut interval = time::interval(super::managed::plan_wakeup_delay());
        interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = task_state.shutdown.cancelled() => break,
                _ = interval.tick() => {},
                _ = task_state.tunnel.cloudflared_schedule_notify.notified() => {},
            }
            if let Err(error) = scheduled_tick(&task_state).await {
                tracing::warn!(%error, "Cloudflare optimization scheduler failed");
                let mut runtime = load_runtime(&task_state).await;
                ensure_object(&mut runtime)
                    .insert("lastError".to_string(), json!(error.to_string()));
                let _ = save_runtime(&task_state, &runtime).await;
            }
        }
    });
}

pub(super) fn schedule_after_host_mappings_change(state: AppState) {
    let task_state = state.clone();
    state.spawn_background("cloudflare-mapping-reconcile", async move {
        let managed = load_managed_config(&task_state).await;
        if managed.get("mode").and_then(Value::as_str) != Some("managed") {
            return;
        }
        task_state.tunnel.cloudflared_schedule_notify.notify_one();
    });
}

pub(super) async fn public_state(state: &AppState, managed: &Value, ownership: &Value) -> Value {
    let runtime = load_runtime(state).await;
    let domain_settings = load_domain_settings(state).await.unwrap_or_default();
    let external_hostnames = domain_settings
        .external_hostnames
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let (sources, source_settings_error) = match load_source_settings(state).await {
        Ok(value) => (value, None),
        Err(error) => (
            OptimizationSourceSettings {
                official_ranges: false,
                builtin_ids: Vec::new(),
                custom_hostnames: Vec::new(),
            },
            Some(error.to_string()),
        ),
    };
    let local = state
        .storage
        .store
        .get_config()
        .await
        .unwrap_or_else(|_| json!({}));
    let host_states = ownership
        .pointer("/optimization/customHostnames")
        .and_then(Value::as_object);
    let domains = configured_hosts(&local)
        .into_iter()
        .map(|host| {
            let current = host_states.and_then(|items| items.get(&host));
            let external = external_hostnames.contains(host.as_str());
            json!({
                "hostname": host,
                "managementMode": if external { "external" } else { "optimize" },
                "status": if external {
                    json!("external")
                } else {
                    current.and_then(|value| value.get("status")).cloned().unwrap_or_else(|| json!("fallback"))
                },
                "hostnameStatus": current
                    .and_then(custom_hostname_activation_status)
                    .map(Value::from)
                    .unwrap_or(Value::Null),
                "sslStatus": current.and_then(|value| value.get("sslStatus")).cloned().unwrap_or(Value::Null),
                "customHostnameId": current.and_then(|value| value.get("id")).cloned().unwrap_or(Value::Null),
                "optimized": !external && current.is_some_and(exact_route_is_optimized),
                "actionRequired": !external && current.and_then(|value| value.get("status")).and_then(Value::as_str) == Some("conflict"),
                "cleanupPending": external && current.is_some(),
                "conflictResourceId": current.and_then(|value| value.get("conflictResourceId")).cloned().unwrap_or(Value::Null),
                "messageCode": current.and_then(|value| value.get("messageCode")).cloned().unwrap_or(Value::Null),
                "messageDetail": current.and_then(|value| value.get("messageDetail")).cloned().unwrap_or(Value::Null),
                "message": if external {
                    Value::Null
                } else {
                    current.and_then(|value| value.get("message")).cloned().unwrap_or(Value::Null)
                },
            })
        })
        .collect::<Vec<_>>();
    let latest_jobs = {
        let jobs = state.tunnel.cloudflared_scan_jobs.read().await;
        let mut values = jobs.values().cloned().collect::<Vec<_>>();
        values.sort_by(|left, right| {
            right
                .get("createdAt")
                .and_then(Value::as_str)
                .cmp(&left.get("createdAt").and_then(Value::as_str))
        });
        values.into_iter().take(5).collect::<Vec<_>>()
    };
    let mut public_sources = public_source_settings(&sources);
    ensure_object(&mut public_sources).insert(
        "error".to_string(),
        source_settings_error
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    let scan_ready = scan_validation_hostname(ownership).is_some();
    let scan_readiness_error_code = (!scan_ready)
        .then(|| scan_validation_hostname_error(ownership))
        .and_then(|error| optimization_scan_error_code(&error))
        .map(Value::from)
        .unwrap_or(Value::Null);
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
        "scanReady": scan_ready,
        "scanReadinessErrorCode": scan_readiness_error_code,
        "candidateSources": public_sources,
        "vantage": runtime.get("lastVantage").cloned().unwrap_or(Value::Null),
        "sourceWarnings": runtime.get("lastSourceWarnings").cloned().unwrap_or_else(|| json!([])),
        "resolverDiagnostics": runtime.get("lastResolverDiagnostics").cloned().unwrap_or_else(|| json!([])),
        "resolutionPath": runtime.get("lastResolutionPath").cloned().unwrap_or(Value::Null),
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

const PLAN_WARNINGS: [(&str, &str); 4] = [
    (
        "betaVantage",
        "Optimization is a Beta feature measured from this server's network vantage point.",
    ),
    (
        "candidateDiscoveryOnly",
        "Built-in and custom third-party hostnames are used only to discover candidate Cloudflare IPs. Business DNS is never pointed at those hostnames.",
    ),
    (
        "customHostnameQuota",
        "Cloudflare for SaaS includes up to 100 exact Custom Hostnames on non-Enterprise plans; excess domains use the wildcard Tunnel.",
    ),
    (
        "wildcardFallback",
        "The wildcard Tunnel remains configured and is restored automatically if the preferred edge path fails.",
    ),
];

pub(super) fn plan_warnings(enabled: bool) -> Vec<Value> {
    if !enabled {
        return Vec::new();
    }
    PLAN_WARNINGS
        .iter()
        .map(|(_, message)| json!(message))
        .collect()
}

pub(super) fn plan_warning_codes(enabled: bool) -> Vec<&'static str> {
    if !enabled {
        return Vec::new();
    }
    PLAN_WARNINGS.iter().map(|(code, _)| *code).collect()
}

pub(super) async fn append_cleanup_remote_snapshot(
    api: &CloudflareApi,
    zone_id: &str,
    ownership: &Value,
    instance_id: &str,
    custom_hostnames: &[Value],
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
                "messageCode": "fallbackOriginChanged",
                "message": "The previously managed fallback origin has been changed by another configuration",
                "takeoverAllowed": false,
            }));
        }
    }
    remote_snapshot.push(json!({ "fallbackOrigin": fallback_origin }));

    let default_custom_origin = ownership
        .pointer("/optimization/originDns/name")
        .and_then(Value::as_str);
    for (hostname, state) in ownership
        .pointer("/optimization/customHostnames")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|items| items.iter())
    {
        let Some(id) = state.get("id").and_then(Value::as_str) else {
            continue;
        };
        let Some(remote) = custom_hostnames
            .iter()
            .find(|item| item.get("id").and_then(Value::as_str) == Some(id))
        else {
            continue;
        };
        if !managed_custom_hostname_matches(remote, hostname, state, default_custom_origin) {
            conflicts.push(json!({
                "id": format!("optimization:cleanup-custom-hostname:{id}"),
                "kind": "custom-hostname",
                "target": hostname,
                "messageCode": "managedCustomHostnameChanged",
                "message": "A previously managed Custom Hostname was changed by another configuration",
                "takeoverAllowed": false,
            }));
        }
    }
    if let Some(probe) = ownership.pointer("/optimization/capabilityProbe")
        && let Some(id) = probe.get("id").and_then(Value::as_str)
        && let Some(remote) = custom_hostnames
            .iter()
            .find(|item| item.get("id").and_then(Value::as_str) == Some(id))
    {
        let hostname = probe.get("hostname").and_then(Value::as_str).unwrap_or("");
        if !managed_custom_hostname_matches(remote, hostname, probe, default_custom_origin) {
            conflicts.push(json!({
                "id": format!("optimization:cleanup-capability-hostname:{id}"),
                "kind": "custom-hostname",
                "target": hostname,
                "messageCode": "capabilityHostnameChanged",
                "message": "The previously managed capability Custom Hostname was changed by another configuration",
                "takeoverAllowed": false,
            }));
        }
    }

    let mut tracked = Vec::new();
    for path in ["/optimization/originDns", "/optimization/edgeDns"] {
        if let Some(record) = ownership.pointer(path) {
            tracked.push(record.clone());
        }
    }
    tracked.extend(
        ownership
            .pointer("/optimization/recoveredOrigins")
            .and_then(Value::as_object)
            .into_iter()
            .flat_map(|items| items.values().cloned()),
    );
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
                    "messageCode": "managedOptimizationDnsChanged",
                    "message": "A previously managed optimization DNS record has been claimed or changed by another configuration",
                    "takeoverAllowed": false,
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
    hosts: &[String],
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
    let recovery_origin = owned_fallback
        .and_then(|value| value.get("previousOrigin"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            remote_origin
                .filter(|remote| !remote.eq_ignore_ascii_case(&origin))
                .map(str::to_string)
        });
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
            "messageCode": "unownedFallbackOrigin",
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
    for host in hosts {
        let existing = custom_hostnames.iter().find(|item| {
            item.get("hostname")
                .and_then(Value::as_str)
                .is_some_and(|value| value.eq_ignore_ascii_case(host))
        });
        let owned_id = owned
            .and_then(|items| items.get(host))
            .and_then(|value| value.get("id"))
            .and_then(Value::as_str);
        let exact_records = api.list_dns_records(zone_id, Some(host)).await?;
        remote_snapshot.push(json!({
            "hostname": host,
            "dnsRecords": exact_records.clone(),
        }));
        let (recoverable, recovery_origin_records) = match existing {
            Some(item) if owned_id != item.get("id").and_then(Value::as_str) => {
                let recovered_origin = item
                    .get("custom_origin_server")
                    .and_then(Value::as_str)
                    .and_then(|origin| {
                        ownership.pointer(&format!(
                            "/optimization/recoveredOrigins/{}",
                            json_pointer_escape(origin)
                        ))
                    });
                inspect_recoverable_fn_knock_custom_hostname(
                    api,
                    zone_id,
                    root,
                    item,
                    &exact_records,
                    recovery_origin.as_deref(),
                    instance,
                    recovered_origin,
                )
                .await?
            }
            _ => (None, Vec::new()),
        };
        if !recovery_origin_records.is_empty() {
            remote_snapshot.push(json!({
                "hostname": recoverable
                    .as_ref()
                    .map(|value| value.origin_hostname.clone())
                    .or_else(|| existing
                        .and_then(|item| item.get("custom_origin_server"))
                        .and_then(Value::as_str)
                        .map(str::to_string)),
                "dnsRecords": recovery_origin_records,
            }));
        }
        let owned_state = owned.and_then(|items| items.get(host));
        let owned_custom_matches = existing.is_some_and(|item| {
            owned_state.is_some_and(|state| {
                owned_id == item.get("id").and_then(Value::as_str)
                    && managed_custom_hostname_matches(item, host, state, Some(&origin))
            })
        });
        match existing {
            Some(_) if owned_custom_matches => {
                operations.push(preview_operation(
                    &format!("custom-hostname:{host}"),
                    "custom-hostname",
                    "keep",
                    host,
                    true,
                ));
            }
            Some(_) if recoverable.is_some() => operations.push(preview_operation(
                &format!("custom-hostname:{host}"),
                "custom-hostname",
                "recover",
                host,
                true,
            )),
            Some(_) => conflicts.push(json!({
                "id": format!("custom-hostname:{host}"),
                "kind": "custom-hostname",
                "target": host,
                "messageCode": "unownedCustomHostname",
                "message": "An unowned Cloudflare for SaaS Custom Hostname already exists",
                "takeoverAllowed": true,
            })),
            None if remaining > 0 => {
                operations.push(preview_operation(
                    &format!("custom-hostname:{host}"),
                    "custom-hostname",
                    "create",
                    host,
                    false,
                ));
                remaining -= 1;
            }
            None => operations.push(preview_operation(
                &format!("custom-hostname:{host}"),
                "custom-hostname",
                "fallback",
                host,
                false,
            )),
        }
        if recoverable.is_some() {
            operations.push(preview_operation(
                &format!("optimization:dns:{host}"),
                "dns",
                "recover",
                host,
                true,
            ));
        } else if ownership.pointer("/optimization/selected/ip").is_some() {
            let exact_owned_id = owned
                .and_then(|items| items.get(host))
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
                if exact_owned && exact_records.len() == 1 {
                    operations.push(preview_operation(
                        &format!("optimization:dns:{host}"),
                        "dns",
                        "update",
                        host,
                        true,
                    ));
                } else {
                    let single_record = exact_records.len() == 1;
                    conflicts.push(json!({
                        "id": format!("optimization:dns:{host}"),
                        "kind": "dns",
                        "target": host,
                        "messageCode": if single_record { "exactDnsConflict" } else { "multipleExactDnsConflict" },
                        "message": if single_record {
                            "An unowned exact DNS record prevents optimization"
                        } else {
                            "Multiple exact DNS records must be resolved before optimization"
                        },
                        "takeoverAllowed": single_record,
                        "details": dns_conflict_details(&exact_records, instance, "CNAME", &edge, false),
                    }));
                }
            } else {
                operations.push(preview_operation(
                    &format!("optimization:dns:{host}"),
                    "dns",
                    "create",
                    host,
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
        .storage
        .store
        .get_config()
        .await
        .map_err(local_error_display)?;
    let hosts =
        reconcile_optimization_host_membership(state, api, zone_id, ownership, &local, &suffix)
            .await?;
    if !should_publish_exact_routes(ownership, force_publish) {
        // A manual or health fallback suppresses exact business-hostname DNS
        // publication, not reconciliation of the Cloudflare Custom Hostname
        // control plane. Keep activation and certificate state fresh so a
        // completed validation can make the next recovery scan available.
        refresh_tracked_custom_hostname_statuses(state, api, zone_id, ownership, &origin_hostname)
            .await?;
        return Ok(());
    }
    let remote_custom = api.list_custom_hostnames(zone_id, None).await?;
    let recovery_origin = ownership
        .pointer("/optimization/fallbackOrigin/previousOrigin")
        .and_then(Value::as_str)
        .map(str::to_string);
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
        let owned_custom_matches = existing.is_some_and(|item| {
            owned_id == item.get("id").and_then(Value::as_str)
                && managed_custom_hostname_matches(
                    item,
                    &host,
                    &current_owned,
                    Some(&origin_hostname),
                )
        });
        let recovery = match existing {
            Some(item) if owned_id != item.get("id").and_then(Value::as_str) => {
                let exact_records = api.list_dns_records(zone_id, Some(&host)).await?;
                let recovered_origin = item
                    .get("custom_origin_server")
                    .and_then(Value::as_str)
                    .and_then(|origin| {
                        ownership
                            .pointer(&format!(
                                "/optimization/recoveredOrigins/{}",
                                json_pointer_escape(origin)
                            ))
                            .cloned()
                    });
                inspect_recoverable_fn_knock_custom_hostname(
                    api,
                    zone_id,
                    root,
                    item,
                    &exact_records,
                    recovery_origin.as_deref(),
                    &suffix,
                    recovered_origin.as_ref(),
                )
                .await?
                .0
            }
            _ => None,
        };
        if let Some(recoverable) = recovery.as_ref() {
            adopt_recoverable_fn_knock_origin(
                state,
                api,
                zone_id,
                ownership,
                recoverable,
                &origin_target,
                &suffix,
            )
            .await?;
        }
        let recovered_lineage = recovery.is_some();
        let custom = match existing {
            Some(item) if owned_custom_matches => item.clone(),
            Some(item) if recovered_lineage => item.clone(),
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
                        "messageCode": "customHostnameOwnershipConflict",
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
                    json!({
                        "status": "quota",
                        "messageCode": "customHostnameQuotaExhausted",
                        "message": "Custom Hostname quota is exhausted"
                    }),
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
                        "messageCode": "certificateRateLimited",
                        "message": "Queued to respect Cloudflare certificate issuance rate limits"
                    }),
                );
                save_managed_state(state, ownership).await?;
                continue;
            }
            None => {
                match api
                    .create_custom_hostname(zone_id, &host, &origin_hostname)
                    .await
                {
                    Ok(custom) => {
                        available = available.saturating_sub(1);
                        created_this_run += 1;
                        custom
                    }
                    Err(error) if is_capability_unsupported_api_error(&error) => {
                        // The included limit is not an entitlement guarantee:
                        // account-specific quotas can be lower or already
                        // exhausted. Keep the wildcard Tunnel serving this and
                        // all remaining hosts instead of aborting reconciliation.
                        available = 0;
                        set_host_state(
                            ownership,
                            &host,
                            json!({
                                "status": "quota",
                                "messageCode": "customHostnameQuotaUnavailable",
                                "messageDetail": error.to_string(),
                                "message": format!("Custom Hostname quota is unavailable: {error}")
                            }),
                        );
                        save_managed_state(state, ownership).await?;
                        continue;
                    }
                    Err(error) => return Err(error),
                }
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
        {
            let object = ensure_object(&mut host_state);
            object.remove("message");
            object.remove("messageCode");
            object.remove("messageDetail");
            object.remove("conflictResourceId");
        }
        if let Some(recoverable) = recovery.as_ref() {
            let object = ensure_object(&mut host_state);
            object.insert("ownership".to_string(), json!("recovered"));
            object.insert(
                "recoveredFromInstance".to_string(),
                json!(recoverable.legacy_instance_id),
            );
            object.insert(
                "customOriginServer".to_string(),
                json!(recoverable.origin_hostname),
            );
            object.insert(
                "exactDnsId".to_string(),
                recoverable
                    .exact_dns
                    .get("id")
                    .cloned()
                    .unwrap_or(Value::Null),
            );
            object.insert("exactDnsTarget".to_string(), json!("origin"));
        }
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
        object.insert("hostnameStatus".to_string(), json!(status));
        object.insert("sslStatus".to_string(), json!(ssl_status));
        object.insert("hostname".to_string(), json!(host));
        object.insert(
            "customOriginServer".to_string(),
            custom
                .get("custom_origin_server")
                .cloned()
                .unwrap_or_else(|| json!(origin_hostname)),
        );
        set_host_state(ownership, &host, host_state.clone());
        save_managed_state(state, ownership).await?;

        let validation_records = extract_validation_records(&custom);
        let mut validation_ids = Vec::new();
        let mut used_validation_dns_ids = HashSet::new();
        let mut activation_conflict = false;
        for (name, value) in validation_records {
            let existing_id = host_state
                .get("validationDns")
                .and_then(Value::as_array)
                .and_then(|records| {
                    records.iter().find(|record| {
                        record.get("name").and_then(Value::as_str) == Some(name.as_str())
                            && record.get("content").and_then(Value::as_str) == Some(value.as_str())
                            && record
                                .get("id")
                                .and_then(Value::as_str)
                                .is_some_and(|id| !used_validation_dns_ids.contains(id))
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
                    takeover: recovered_lineage,
                    instance_id: &suffix,
                },
            )
            .await
            {
                Ok(record) => {
                    if let Some(id) = record.get("id").and_then(Value::as_str) {
                        used_validation_dns_ids.insert(id.to_string());
                    }
                    validation_ids.push(record);
                    ensure_object(&mut host_state)
                        .insert("validationDns".to_string(), json!(validation_ids));
                    set_host_state(ownership, &host, host_state.clone());
                    save_managed_state(state, ownership).await?;
                }
                Err(error) if error.status == Some(StatusCode::CONFLICT) => {
                    activation_conflict = true;
                    ensure_object(&mut host_state).insert("status".to_string(), json!("conflict"));
                    let object = ensure_object(&mut host_state);
                    object.insert(
                        "messageCode".to_string(),
                        json!("validationDnsOwnershipConflict"),
                    );
                    object.insert("messageDetail".to_string(), json!(name));
                    object.insert("message".to_string(), json!(error.to_string()));
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
                    takeover: recovered_lineage
                        || takeover.is_some_and(|items| {
                            items.contains(&format!("optimization:dns:{host}"))
                        }),
                    instance_id: &suffix,
                },
            )
            .await
            {
                Ok(record) => {
                    set_exact_dns_route(
                        &mut host_state,
                        &record,
                        if exact_route_was_optimized {
                            "edge"
                        } else {
                            "origin"
                        },
                    );
                    set_host_state(ownership, &host, host_state.clone());
                    save_managed_state(state, ownership).await?;
                }
                Err(error) if error.status == Some(StatusCode::CONFLICT) => {
                    activation_conflict = true;
                    ensure_object(&mut host_state).insert("status".to_string(), json!("conflict"));
                    let object = ensure_object(&mut host_state);
                    object.insert(
                        "messageCode".to_string(),
                        json!("exactDnsOwnershipConflict"),
                    );
                    object.insert(
                        "conflictResourceId".to_string(),
                        json!(format!("optimization:dns:{host}")),
                    );
                    object.insert("message".to_string(), json!(error.to_string()));
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
        ensure_object(&mut host_state).insert("hostnameStatus".to_string(), json!(status));
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
                                takeover: recovered_lineage
                                    || takeover.is_some_and(|items| {
                                        items.contains(&format!("optimization:dns:{host}"))
                                    }),
                                instance_id: &suffix,
                            },
                        )
                        .await
                        {
                            Ok(record) => {
                                set_exact_dns_route(&mut host_state, &record, "edge");
                                ensure_object(&mut host_state)
                                    .insert("status".to_string(), json!("optimized"));
                                ensure_object(&mut host_state).insert(
                                    "lastVerifiedAt".to_string(),
                                    json!(time_utils::now_iso()),
                                );
                                let object = ensure_object(&mut host_state);
                                object.remove("message");
                                object.remove("messageCode");
                                object.remove("messageDetail");
                                object.remove("conflictResourceId");
                            }
                            Err(error) if error.status == Some(StatusCode::CONFLICT) => {
                                let object = ensure_object(&mut host_state);
                                object.insert("status".to_string(), json!("conflict"));
                                object.insert(
                                    "messageCode".to_string(),
                                    json!("exactDnsOwnershipConflict"),
                                );
                                object.insert(
                                    "conflictResourceId".to_string(),
                                    json!(format!("optimization:dns:{host}")),
                                );
                                object.insert("message".to_string(), json!(error.to_string()));
                            }
                            Err(error) => return Err(error),
                        }
                    }
                    Err(error) => {
                        let mut fallback_conflict = false;
                        if exact_route_is_optimized(&host_state) {
                            let exact_id = host_state.get("exactDnsId").and_then(Value::as_str);
                            match upsert_managed_dns(
                                api,
                                ManagedDnsRequest {
                                    zone_id,
                                    name: &host,
                                    record_type: "CNAME",
                                    content: &origin_hostname,
                                    proxied: false,
                                    owned_id: exact_id,
                                    takeover: recovered_lineage
                                        || takeover.is_some_and(|items| {
                                            items.contains(&format!("optimization:dns:{host}"))
                                        }),
                                    instance_id: &suffix,
                                },
                            )
                            .await
                            {
                                Ok(record) => {
                                    set_exact_dns_route(&mut host_state, &record, "origin");
                                }
                                Err(fallback_error)
                                    if fallback_error.status == Some(StatusCode::CONFLICT) =>
                                {
                                    fallback_conflict = true;
                                    let object = ensure_object(&mut host_state);
                                    object.insert("status".to_string(), json!("conflict"));
                                    object.insert(
                                        "messageCode".to_string(),
                                        json!("exactDnsOwnershipConflict"),
                                    );
                                    object.insert(
                                        "conflictResourceId".to_string(),
                                        json!(format!("optimization:dns:{host}")),
                                    );
                                    object.insert(
                                        "message".to_string(),
                                        json!(fallback_error.to_string()),
                                    );
                                }
                                Err(fallback_error) => return Err(fallback_error),
                            }
                        }
                        if !fallback_conflict {
                            record_preferred_edge_probe_failure(&mut host_state, &error);
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

fn set_exact_dns_route(host_state: &mut Value, record: &Value, target: &str) {
    let object = ensure_object(host_state);
    object.insert(
        "exactDnsId".to_string(),
        record.get("id").cloned().unwrap_or(Value::Null),
    );
    object.insert("exactDnsTarget".to_string(), json!(target));
}

fn record_preferred_edge_probe_failure(state: &mut Value, error: &str) {
    let object = ensure_object(state);
    object.insert("status".to_string(), json!("probe-failed"));
    object.insert("messageCode".to_string(), json!("preferredEdgeProbeFailed"));
    object.insert("messageDetail".to_string(), json!(error));
    object.insert("message".to_string(), json!(error));
    object.insert(
        "lastProbeFailedAt".to_string(),
        json!(time_utils::now_iso()),
    );
}

async fn refresh_tracked_custom_hostname_statuses(
    state: &AppState,
    api: &CloudflareApi,
    zone_id: &str,
    ownership: &mut Value,
    default_origin: &str,
) -> Result<(), CloudflareApiError> {
    let tracked = ownership
        .pointer("/optimization/customHostnames")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|items| items.iter())
        .filter_map(|(hostname, host_state)| {
            host_state
                .get("id")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
                .map(|id| (hostname.clone(), id.to_string(), host_state.clone()))
        })
        .collect::<Vec<_>>();
    if tracked.is_empty() {
        return Ok(());
    }
    let remote_custom = api.list_custom_hostnames(zone_id, None).await?;
    let mut changed = false;
    for (hostname, custom_id, mut host_state) in tracked {
        match remote_custom
            .iter()
            .find(|remote| remote.get("id").and_then(Value::as_str) == Some(custom_id.as_str()))
        {
            Some(remote)
                if managed_custom_hostname_matches(
                    remote,
                    &hostname,
                    &host_state,
                    Some(default_origin),
                ) =>
            {
                changed |= update_custom_hostname_activation(&mut host_state, remote);
            }
            Some(remote) => {
                changed |= update_custom_hostname_activation(&mut host_state, remote);
                let conflict_changed = host_state.get("status").and_then(Value::as_str)
                    != Some("conflict")
                    || host_state.get("messageCode").and_then(Value::as_str)
                        != Some("customHostnameOwnershipConflict")
                    || host_state.get("message").and_then(Value::as_str)
                        != Some("Custom Hostname is not owned by fn-knock");
                let object = ensure_object(&mut host_state);
                object.insert("status".to_string(), json!("conflict"));
                object.insert(
                    "messageCode".to_string(),
                    json!("customHostnameOwnershipConflict"),
                );
                object.insert(
                    "message".to_string(),
                    json!("Custom Hostname is not owned by fn-knock"),
                );
                changed |= conflict_changed;
            }
            None => {
                let object = ensure_object(&mut host_state);
                changed |= object.get("hostnameStatus").and_then(Value::as_str) != Some("deleted")
                    || object
                        .get("sslStatus")
                        .is_some_and(|value| !value.is_null());
                object.insert("hostnameStatus".to_string(), json!("deleted"));
                object.insert("sslStatus".to_string(), Value::Null);
            }
        }
        set_host_state(ownership, &hostname, host_state);
    }
    if changed {
        save_managed_state(state, ownership).await?;
    }
    Ok(())
}

fn update_custom_hostname_activation(host_state: &mut Value, remote: &Value) -> bool {
    let hostname_status = remote
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("pending");
    let ssl_status = remote
        .pointer("/ssl/status")
        .and_then(Value::as_str)
        .unwrap_or("pending");
    let changed = host_state.get("hostnameStatus").and_then(Value::as_str) != Some(hostname_status)
        || host_state.get("sslStatus").and_then(Value::as_str) != Some(ssl_status);
    let object = ensure_object(host_state);
    object.insert("hostnameStatus".to_string(), json!(hostname_status));
    object.insert("sslStatus".to_string(), json!(ssl_status));
    changed
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
        .filter(|(_, state)| custom_hostname_can_validate_candidates(state))
        .map(|(hostname, _)| hostname.clone())
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
            if let (Some(_), Some(kind)) = (owned_origin, owned_kind) {
                (
                    remote_value
                        .clone()
                        .unwrap_or_else(|| json!({ "origin": remote })),
                    kind,
                    current
                        .as_ref()
                        .and_then(|value| value.get("previousOrigin"))
                        .and_then(Value::as_str),
                )
            } else {
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
            }
        }
        Some(remote) if owned_origin == Some(remote) => {
            if let Some(kind) = owned_kind {
                (
                    api.update_fallback_origin(zone_id, desired_origin).await?,
                    kind,
                    current
                        .as_ref()
                        .and_then(|value| value.get("previousOrigin"))
                        .and_then(Value::as_str),
                )
            } else if takeover {
                (
                    api.update_fallback_origin(zone_id, desired_origin).await?,
                    "adopted",
                    Some(remote),
                )
            } else {
                return Err(CloudflareApiError {
                    status: Some(StatusCode::CONFLICT),
                    message: "A different Cloudflare for SaaS fallback origin exists; preview and explicitly confirm takeover"
                        .to_string(),
                });
            }
        }
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
        let definitive = ownership
            .pointer("/optimization/capabilityProbe")
            .is_some_and(capability_probe_is_definitively_unsupported);
        if definitive {
            return Ok(CapabilityProbeResult::Unsupported);
        }
        // Older releases classified any candidate route failure as a product
        // capability failure and persisted `unsupported` without a reason
        // code. That state is safe to retry; definitive entitlement failures
        // always carry a reasonCode and remain disabled.
        if let Some(optimization) = ownership
            .pointer_mut("/optimization")
            .and_then(Value::as_object_mut)
        {
            optimization.remove("capabilityProbe");
        }
        save_managed_state(state, ownership).await?;
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
                    disable_unsupported_optimization(
                        state,
                        managed,
                        ownership,
                        &error.to_string(),
                        Some(CLOUDFLARE_SAAS_REQUIRED_ERROR_CODE),
                    )
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
                disable_unsupported_optimization(
                    state,
                    managed,
                    ownership,
                    &error.to_string(),
                    Some(CLOUDFLARE_SAAS_REQUIRED_ERROR_CODE),
                )
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
            // The Custom Hostname and certificate are already active here.
            // A failed request to one candidate IP is a route-level result,
            // not evidence that Cloudflare for SaaS is unsupported. Retain the
            // isolated hostname so the user can retry this scan or apply a
            // different candidate without reprovisioning the capability probe.
            let failed_probe = capability_probe_failure_state(&probe_state, &error);
            ensure_nested_object(ownership, &["optimization"])
                .insert("capabilityProbe".to_string(), failed_probe);
            save_managed_state(state, ownership).await?;
            Err(local_error(format!(
                "Preferred edge candidate failed capability validation: {error}"
            )))
        }
    }
}

fn capability_probe_failure_state(probe_state: &Value, error: &str) -> Value {
    let mut failed = probe_state.clone();
    record_preferred_edge_probe_failure(&mut failed, error);
    failed
}

fn capability_probe_is_definitively_unsupported(probe: &Value) -> bool {
    probe.get("status").and_then(Value::as_str) == Some("unsupported")
        && probe.get("reasonCode").and_then(Value::as_str)
            == Some(CLOUDFLARE_SAAS_REQUIRED_ERROR_CODE)
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
    reason_code: Option<&str>,
) -> Result<(), CloudflareApiError> {
    let mut capability_probe = json!({
        "status": "unsupported",
        "message": reason,
        "testedAt": time_utils::now_iso(),
    });
    if let Some(reason_code) = reason_code {
        ensure_object(&mut capability_probe).insert("reasonCode".to_string(), json!(reason_code));
    }
    ensure_nested_object(ownership, &["optimization"])
        .insert("capabilityProbe".to_string(), capability_probe);
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
                &tracked_exact_dns_snapshot(
                    &hostname,
                    record_id,
                    &value,
                    ownership,
                    Some(&edge_hostname),
                ),
                &instance_id,
            )
            .await?;
        }
        let mut next = value;
        if next.get("hostnameStatus").is_none()
            && let Some(status) = custom_hostname_activation_status(&next).map(str::to_string)
        {
            ensure_object(&mut next).insert("hostnameStatus".to_string(), json!(status));
        }
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
                    if ownership
                        .pointer("/optimization/recoveredOrigins")
                        .and_then(Value::as_object)
                        .is_some_and(|items| items.contains_key(previous))
                    {
                        ignore_not_found(api.delete_fallback_origin(zone_id).await)?;
                    } else {
                        api.update_fallback_origin(zone_id, previous).await?;
                    }
                }
            }
            _ => {}
        }
    }
    for recovered_origin in ownership
        .pointer("/optimization/recoveredOrigins")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|items| items.values())
    {
        if recovered_origin.get("id").and_then(Value::as_str).is_some() {
            delete_dns_if_owned(
                api,
                zone_id,
                recovered_origin,
                &managed_instance_id(managed),
            )
            .await?;
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
    let default_custom_origin = ownership
        .pointer("/optimization/originDns/name")
        .and_then(Value::as_str)
        .map(str::to_string);
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
                &tracked_exact_dns_snapshot(&hostname, id, &host, ownership, None),
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
            match api.get_custom_hostname(zone_id, id).await {
                Ok(remote)
                    if managed_custom_hostname_matches(
                        &remote,
                        &hostname,
                        &host,
                        default_custom_origin.as_deref(),
                    ) =>
                {
                    ignore_not_found(api.delete_custom_hostname(zone_id, id).await)?;
                }
                Ok(_) => {
                    return Err(CloudflareApiError {
                        status: Some(StatusCode::CONFLICT),
                        message: format!(
                            "Custom Hostname {hostname} changed outside fn-knock; refusing automatic deletion"
                        ),
                    });
                }
                Err(error) if error.status == Some(StatusCode::NOT_FOUND) => {}
                Err(error) => return Err(error),
            }
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

async fn reconcile_optimization_host_membership(
    state: &AppState,
    api: &CloudflareApi,
    zone_id: &str,
    ownership: &mut Value,
    config: &Value,
    instance_id: &str,
) -> Result<Vec<String>, CloudflareApiError> {
    let configured = configured_hosts(config);
    let settings = load_domain_settings(state).await?;
    let (managed_hosts, _) = partition_optimization_hosts(configured.clone(), &settings);

    // An explicit external-hostname choice is a request to relinquish, not to
    // delete unconditionally. Retry that safe path before applying the stricter
    // cleanup policy used for hostnames removed from the application config.
    for hostname in &settings.external_hostnames {
        let cleanup_error = if ownership
            .pointer(&format!(
                "/optimization/customHostnames/{}",
                json_pointer_escape(hostname)
            ))
            .is_some()
        {
            relinquish_optimization_host(state, api, zone_id, ownership, hostname, instance_id)
                .await
                .err()
        } else {
            None
        };
        if let Some(error) = cleanup_error {
            tracing::warn!(%error, %hostname, "external optimization hostname cleanup remains pending");
        }
    }

    let mut configured_set = configured.into_iter().collect::<HashSet<_>>();
    configured_set.extend(settings.external_hostnames.iter().cloned());
    cleanup_removed_hosts(state, api, zone_id, ownership, &configured_set, instance_id).await?;
    Ok(managed_hosts)
}

fn tracked_exact_dns_snapshot(
    hostname: &str,
    id: &str,
    host: &Value,
    ownership: &Value,
    legacy_edge_hostname: Option<&str>,
) -> Value {
    let target_path = if host.get("exactDnsTarget").and_then(Value::as_str) == Some("origin") {
        "/optimization/originDns/name"
    } else {
        "/optimization/edgeDns/name"
    };
    let content = ownership
        .pointer(target_path)
        .cloned()
        .or_else(|| legacy_edge_hostname.map(|value| json!(value)))
        .unwrap_or(Value::Null);
    json!({
        "id": id,
        "name": hostname,
        "type": "CNAME",
        "content": content,
        "proxied": false,
    })
}

fn host_has_tracked_remote_resources(host: &Value) -> bool {
    host.get("id").and_then(Value::as_str).is_some()
        || host.get("exactDnsId").and_then(Value::as_str).is_some()
        || host
            .get("validationDns")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .any(|record| record.get("id").and_then(Value::as_str).is_some())
}

async fn forget_optimization_host_state(
    state: &AppState,
    ownership: &mut Value,
    hostname: &str,
) -> Result<(), CloudflareApiError> {
    if let Some(items) = ownership
        .pointer_mut("/optimization/customHostnames")
        .and_then(Value::as_object_mut)
    {
        items.remove(hostname);
    }
    save_managed_state(state, ownership).await
}

async fn relinquish_optimization_host(
    state: &AppState,
    api: &CloudflareApi,
    zone_id: &str,
    ownership: &mut Value,
    hostname: &str,
    instance_id: &str,
) -> Result<(), CloudflareApiError> {
    let Some(host) = ownership
        .pointer(&format!(
            "/optimization/customHostnames/{}",
            json_pointer_escape(hostname)
        ))
        .cloned()
    else {
        return Ok(());
    };
    let default_custom_origin = ownership
        .pointer("/optimization/originDns/name")
        .and_then(Value::as_str)
        .map(str::to_string);

    if let Some(id) = host.get("exactDnsId").and_then(Value::as_str) {
        let owned = tracked_exact_dns_snapshot(hostname, id, &host, ownership, None);
        if let Err(error) = delete_dns_if_owned(api, zone_id, &owned, instance_id).await {
            if error.status == Some(StatusCode::CONFLICT) {
                tracing::warn!(%error, %hostname, "retaining externally changed exact DNS while relinquishing optimization hostname");
            } else {
                return Err(error);
            }
        }
    }
    for record in host
        .get("validationDns")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if record.get("id").and_then(Value::as_str).is_none() {
            continue;
        }
        if let Err(error) = delete_dns_if_owned(api, zone_id, record, instance_id).await {
            if error.status == Some(StatusCode::CONFLICT) {
                tracing::warn!(%error, %hostname, "retaining externally changed validation DNS while relinquishing optimization hostname");
            } else {
                return Err(error);
            }
        }
    }
    if let Some(id) = host.get("id").and_then(Value::as_str) {
        match api.get_custom_hostname(zone_id, id).await {
            Ok(remote)
                if managed_custom_hostname_matches(
                    &remote,
                    hostname,
                    &host,
                    default_custom_origin.as_deref(),
                ) =>
            {
                ignore_not_found(api.delete_custom_hostname(zone_id, id).await)?;
            }
            Ok(_) => {
                tracing::warn!(%hostname, "retaining externally changed Custom Hostname while relinquishing ownership");
            }
            Err(error) if error.status == Some(StatusCode::NOT_FOUND) => {}
            Err(error) => return Err(error),
        }
    }
    forget_optimization_host_state(state, ownership, hostname).await
}

async fn run_scan(
    state: &AppState,
    job_id: Option<&str>,
) -> Result<OptimizationScanResult, CloudflareApiError> {
    let settings = load_source_settings(state).await?;
    let source_fingerprint = source_settings_fingerprint(&settings);
    let prefixes = load_cloudflare_prefixes(state).await;
    let ownership = load_managed_state(state).await;
    let current_ip = ownership
        .pointer("/optimization/selected/ip")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<Ipv4Addr>().ok())
        .filter(|ip| candidate_ip_is_cloudflare(*ip, &prefixes));
    let (mut seeds, source_warnings, resolver_diagnostics, mut resolution_path) =
        load_candidate_seeds(&settings, &prefixes).await;
    if let Some(ip) = current_ip {
        if seeds.is_empty() {
            resolution_path = "current-candidate".to_string();
        }
        merge_current_candidate_seed(&mut seeds, ip);
    }
    if seeds.is_empty() {
        let mut runtime = load_runtime(state).await;
        let runtime_object = ensure_object(&mut runtime);
        runtime_object.insert(
            "lastSourceWarnings".to_string(),
            json!(source_warnings.clone()),
        );
        runtime_object.insert(
            "lastResolverDiagnostics".to_string(),
            json!(resolver_diagnostics.clone()),
        );
        runtime_object.insert(
            "lastResolutionPath".to_string(),
            json!(resolution_path.clone()),
        );
        let _ = save_runtime(state, &runtime).await;
        if let Some(job_id) = job_id {
            update_job(
                state,
                job_id,
                json!({
                    "sourceWarnings": source_warnings,
                    "resolverDiagnostics": resolver_diagnostics,
                    "resolutionPath": resolution_path,
                    "candidateSourceCount": 0,
                    "sourceFingerprint": source_fingerprint,
                }),
            )
            .await;
        }
        return Err(local_error(CANDIDATE_RESOLUTION_UNAVAILABLE_SCAN_ERROR));
    }
    let vantage = probe_local_vantage(state).await;
    let business_hostname = scan_validation_hostname(&ownership)
        .ok_or_else(|| scan_validation_hostname_error(&ownership))?;
    if let Some(job_id) = job_id {
        update_job(
            state,
            job_id,
            json!({
                "vantage": vantage,
                "sourceWarnings": source_warnings,
                "resolverDiagnostics": resolver_diagnostics,
                "resolutionPath": resolution_path,
                "candidateSourceCount": seeds.len(),
                "businessValidationHostname": business_hostname,
                "sourceFingerprint": source_fingerprint,
            }),
        )
        .await;
    }
    let total = seeds.len().max(1);
    let mut join_set = JoinSet::new();
    let mut results = Vec::new();
    let mut processed_count = 0usize;
    for chunk in seeds.chunks(PROBE_CONCURRENCY) {
        if let Some(job_id) = job_id
            && is_job_cancelled(state, job_id).await
        {
            return Ok(OptimizationScanResult {
                candidates: Vec::new(),
                vantage,
                source_warnings,
                resolver_diagnostics,
                resolution_path,
                source_fingerprint,
            });
        }
        for seed in chunk.iter().cloned() {
            join_set.spawn(async move {
                let metrics = probe_latency_metrics(seed.ip).await;
                (seed, metrics)
            });
        }
        while let Some(result) = join_set.join_next().await {
            if let Ok((seed, Some(metrics))) = result
                && metrics.loss_ratio <= 1.0 / 3.0
            {
                results.push(OptimizationCandidate {
                    ip: seed.ip.to_string(),
                    median_latency_ms: metrics.median_latency_ms,
                    jitter_ms: metrics.jitter_ms,
                    loss_ratio: metrics.loss_ratio,
                    download_mbps: 0.0,
                    score: f64::MAX,
                    verified_at: Some(time_utils::now_iso()),
                    source_types: seed.source_types,
                    source_hostnames: seed.source_hostnames,
                    colo: metrics.colo,
                    cf_ray: metrics.cf_ray,
                    business_hostname: Some(business_hostname.clone()),
                    business_status: None,
                    business_colo: None,
                    business_cf_ray: None,
                    business_validated: false,
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
    if let Some(current_ip) = current_ip.map(|ip| ip.to_string())
        && let Some(position) = results
            .iter()
            .position(|candidate| candidate.ip == current_ip)
        && position >= DOWNLOAD_SHORTLIST
    {
        let current = results.remove(position);
        results.truncate(DOWNLOAD_SHORTLIST.saturating_sub(1));
        results.push(current);
    } else {
        results.truncate(DOWNLOAD_SHORTLIST);
    }
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
        let validation_hostname = business_hostname.clone();
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
            let probe = probe_custom_hostname_details(&validation_hostname, ip)
                .await
                .ok()?;
            candidate.business_hostname = Some(validation_hostname);
            candidate.business_status = Some(probe.status);
            candidate.business_colo = probe.colo;
            candidate.business_cf_ray = probe.cf_ray;
            candidate.business_validated = true;
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
            return Ok(OptimizationScanResult {
                candidates: Vec::new(),
                vantage,
                source_warnings,
                resolver_diagnostics,
                resolution_path,
                source_fingerprint,
            });
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
    Ok(OptimizationScanResult {
        candidates: results,
        vantage,
        source_warnings,
        resolver_diagnostics,
        resolution_path,
        source_fingerprint,
    })
}

async fn load_candidate_seeds(
    settings: &OptimizationSourceSettings,
    prefixes: &[Ipv4Net],
) -> (
    Vec<CandidateSeed>,
    Vec<String>,
    Vec<ResolverDiagnostic>,
    String,
) {
    let selected_builtins = settings
        .builtin_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let mut hostname_sources = BUILTIN_CANDIDATE_SOURCES
        .iter()
        .filter(|source| selected_builtins.contains(source.id))
        .map(|source| {
            (
                source.hostname.to_string(),
                "builtin".to_string(),
                source.id.to_string(),
            )
        })
        .collect::<Vec<_>>();
    hostname_sources.extend(
        settings
            .custom_hostnames
            .iter()
            .cloned()
            .map(|hostname| (hostname.clone(), "custom".to_string(), hostname)),
    );

    let mut resolved = Vec::new();
    let doh_client = build_doh_client();
    for chunk in hostname_sources.chunks(8) {
        let mut tasks = JoinSet::new();
        for (index, (hostname, source_type, source_id)) in chunk.iter().cloned().enumerate() {
            let client = doh_client.clone();
            let prefixes = prefixes.to_vec();
            tasks.spawn(async move {
                let result = match client {
                    Ok(client) => resolve_candidate_hostname(&client, &hostname, &prefixes).await,
                    Err(failure) => CandidateResolution::failed_for_all_providers(failure),
                };
                (index, hostname, source_type, source_id, result)
            });
        }
        let mut chunk_results = Vec::new();
        while let Some(task) = tasks.join_next().await {
            if let Ok(value) = task {
                chunk_results.push(value);
            }
        }
        chunk_results.sort_by_key(|value| value.0);
        resolved.extend(chunk_results);
    }

    let mut seeds = Vec::new();
    let mut indexes = HashMap::new();
    let mut warnings = Vec::new();
    let mut resolver_attempts = Vec::new();
    let mut doh_candidates_available = false;
    for (_, hostname, source_type, source_id, result) in resolved {
        let all_failed_summary = result.all_failed_summary();
        resolver_attempts.extend(result.attempts);
        let ips = result.ips;
        if ips.is_empty() {
            if let Some(summary) = all_failed_summary {
                warnings.push(format!("{hostname}: {summary}"));
            } else {
                warnings.push(format!(
                    "{hostname} ({source_id}) did not resolve to a verified Cloudflare IPv4 address"
                ));
            }
            continue;
        }
        doh_candidates_available = true;
        for ip in ips {
            merge_candidate_seed(&mut seeds, &mut indexes, ip, &source_type, Some(&hostname));
        }
    }
    if settings.official_ranges {
        for ip in sample_candidate_ips(prefixes) {
            merge_candidate_seed(&mut seeds, &mut indexes, ip, "official-range", None);
            if seeds.len() >= MAX_CANDIDATES {
                break;
            }
        }
    }
    seeds.truncate(MAX_CANDIDATES);
    let resolution_path =
        initial_resolution_path(doh_candidates_available, settings.official_ranges);
    (
        seeds,
        warnings,
        aggregate_resolver_diagnostics(&resolver_attempts),
        resolution_path.to_string(),
    )
}

fn candidate_ip_is_cloudflare(ip: Ipv4Addr, prefixes: &[Ipv4Net]) -> bool {
    prefixes.iter().any(|prefix| prefix.contains(&ip))
}

fn merge_candidate_seed(
    seeds: &mut Vec<CandidateSeed>,
    indexes: &mut HashMap<Ipv4Addr, usize>,
    ip: Ipv4Addr,
    source_type: &str,
    source_hostname: Option<&str>,
) {
    if let Some(index) = indexes.get(&ip).copied() {
        let seed = &mut seeds[index];
        if !seed.source_types.iter().any(|value| value == source_type) {
            seed.source_types.push(source_type.to_string());
        }
        if let Some(hostname) = source_hostname
            && !seed.source_hostnames.iter().any(|value| value == hostname)
        {
            seed.source_hostnames.push(hostname.to_string());
        }
        return;
    }
    if seeds.len() >= MAX_CANDIDATES {
        return;
    }
    indexes.insert(ip, seeds.len());
    seeds.push(CandidateSeed {
        ip,
        source_types: vec![source_type.to_string()],
        source_hostnames: source_hostname
            .map(|value| vec![value.to_string()])
            .unwrap_or_default(),
    });
}

fn merge_current_candidate_seed(seeds: &mut Vec<CandidateSeed>, ip: Ipv4Addr) {
    if let Some(seed) = seeds.iter_mut().find(|seed| seed.ip == ip) {
        if !seed.source_types.iter().any(|value| value == "current") {
            seed.source_types.push("current".to_string());
        }
        return;
    }
    if seeds.len() >= MAX_CANDIDATES {
        seeds.pop();
    }
    seeds.push(CandidateSeed {
        ip,
        source_types: vec!["current".to_string()],
        source_hostnames: Vec::new(),
    });
}

async fn probe_local_vantage(state: &AppState) -> Value {
    let measured_at = time_utils::now_iso();
    let response = state
        .fallback_client
        .get("https://www.cloudflare.com/cdn-cgi/trace")
        .timeout(Duration::from_secs(8))
        .send()
        .await;
    let Ok(response) = response else {
        return json!({
            "id": "local-server",
            "label": "fn-knock server",
            "publicIp": Value::Null,
            "defaultColo": Value::Null,
            "measuredAt": measured_at,
        });
    };
    let text = response.text().await.unwrap_or_default();
    let trace = parse_trace(&text);
    json!({
        "id": "local-server",
        "label": "fn-knock server",
        "publicIp": trace.get("ip").cloned().unwrap_or_default(),
        "defaultColo": trace.get("colo").cloned().unwrap_or_default(),
        "measuredAt": measured_at,
    })
}

fn scan_validation_hostname_error(ownership: &Value) -> CloudflareApiError {
    let capability_probe = ownership.pointer("/optimization/capabilityProbe");
    if capability_probe
        .and_then(|probe| probe.get("reasonCode"))
        .and_then(Value::as_str)
        == Some(CLOUDFLARE_SAAS_REQUIRED_ERROR_CODE)
    {
        return local_error(CLOUDFLARE_SAAS_REQUIRED_SCAN_ERROR);
    }

    let business_hostname_conflict = ownership
        .pointer("/optimization/customHostnames")
        .and_then(Value::as_object)
        .is_some_and(|items| {
            items
                .values()
                .any(|state| state.get("status").and_then(Value::as_str) == Some("conflict"))
        });
    if business_hostname_conflict {
        return local_error(CLOUDFLARE_RESOURCE_CONFLICT_SCAN_ERROR);
    }

    let capability_pending = capability_probe
        .and_then(|probe| probe.get("status"))
        .and_then(Value::as_str)
        .is_some_and(|status| matches!(status, "pending" | "awaiting-candidate"));
    let business_hostname_pending = ownership
        .pointer("/optimization/customHostnames")
        .and_then(Value::as_object)
        .is_some_and(|items| {
            items.values().any(|state| {
                matches!(
                    state.get("status").and_then(Value::as_str),
                    Some("queued" | "pending" | "active" | "ready")
                ) && !scan_business_hostname_is_ready(state)
            })
        });

    if capability_pending || business_hostname_pending {
        local_error(CLOUDFLARE_SAAS_VALIDATION_PENDING_SCAN_ERROR)
    } else {
        local_error(OPTIMIZATION_NOT_READY_SCAN_ERROR)
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
    if records.len() == 1
        && dns_record_owned_for_update(record, owned_id, instance_id, record_type, content, proxied)
    {
        operations.push(preview_operation(logical_id, "dns", "update", name, true));
    } else {
        let single_record = records.len() == 1;
        conflicts.push(json!({
            "id": logical_id,
            "kind": "dns",
            "target": name,
            "messageCode": if single_record { "optimizationDnsConflict" } else { "multipleOptimizationDnsConflict" },
            "message": if single_record {
                "An unowned DNS record already uses the optimization hostname"
            } else {
                "Multiple DNS records already use the optimization hostname"
            },
            "takeoverAllowed": single_record,
            "details": dns_conflict_details(
                &records,
                instance_id,
                record_type,
                content.unwrap_or(""),
                proxied,
            ),
        }));
    }
    Ok(())
}

fn recoverable_fn_knock_custom_hostname_from_snapshot(
    custom: &Value,
    exact_records: &[Value],
    origin_records: &[Value],
    recovery_origin: Option<&str>,
    root: &str,
    current_instance_id: &str,
    recovered_origin: Option<&Value>,
) -> Option<RecoverableCustomHostname> {
    let origin_hostname = custom
        .get("custom_origin_server")
        .and_then(Value::as_str)?
        .trim()
        .trim_end_matches('.')
        .to_ascii_lowercase();
    let expected_recovery_origin = recovery_origin?
        .trim()
        .trim_end_matches('.')
        .to_ascii_lowercase();
    if origin_hostname != expected_recovery_origin {
        return None;
    }
    let legacy_instance_id = fn_knock_origin_instance(&origin_hostname, root)?;
    let hostname = custom.get("hostname").and_then(Value::as_str)?;
    let expected_edge = format!("fnknock-edge-{legacy_instance_id}.{root}");
    let exact_dns = exact_records.iter().find(|record| {
        record
            .get("name")
            .and_then(Value::as_str)
            .is_some_and(|value| value.eq_ignore_ascii_case(hostname))
            && record.get("type").and_then(Value::as_str) == Some("CNAME")
            && record
                .get("content")
                .and_then(Value::as_str)
                .is_some_and(|value| {
                    value.eq_ignore_ascii_case(&expected_edge)
                        || value.eq_ignore_ascii_case(&origin_hostname)
                })
            && record.get("proxied").and_then(Value::as_bool) == Some(false)
            && is_managed_dns(record, &legacy_instance_id)
    })?;
    let origin_dns = origin_records.iter().find(|record| {
        let tunnel_target = record
            .get("content")
            .and_then(Value::as_str)
            .map(|value| value.trim().trim_end_matches('.'));
        record
            .get("name")
            .and_then(Value::as_str)
            .is_some_and(|value| value.eq_ignore_ascii_case(&origin_hostname))
            && record.get("type").and_then(Value::as_str) == Some("CNAME")
            && tunnel_target.is_some_and(|value| {
                value
                    .strip_suffix(".cfargotunnel.com")
                    .is_some_and(|id| uuid::Uuid::parse_str(id).is_ok())
            })
            && record.get("proxied").and_then(Value::as_bool) == Some(true)
            && (is_managed_dns(record, &legacy_instance_id)
                || recovered_origin.is_some_and(|saved| {
                    saved.get("recoveredFromInstance").and_then(Value::as_str)
                        == Some(legacy_instance_id.as_str())
                        && saved.get("id").and_then(Value::as_str)
                            == record.get("id").and_then(Value::as_str)
                        && saved.get("name").and_then(Value::as_str)
                            == record.get("name").and_then(Value::as_str)
                        && saved.get("type").and_then(Value::as_str)
                            == record.get("type").and_then(Value::as_str)
                        && saved.get("content").and_then(Value::as_str)
                            == record.get("content").and_then(Value::as_str)
                        && saved.get("proxied").and_then(Value::as_bool)
                            == record.get("proxied").and_then(Value::as_bool)
                        && is_managed_dns(record, current_instance_id)
                }))
    })?;
    Some(RecoverableCustomHostname {
        legacy_instance_id,
        origin_hostname,
        origin_dns: origin_dns.clone(),
        exact_dns: exact_dns.clone(),
    })
}

#[allow(clippy::too_many_arguments)]
async fn inspect_recoverable_fn_knock_custom_hostname(
    api: &CloudflareApi,
    zone_id: &str,
    root: &str,
    custom: &Value,
    exact_records: &[Value],
    recovery_origin: Option<&str>,
    current_instance_id: &str,
    recovered_origin: Option<&Value>,
) -> Result<(Option<RecoverableCustomHostname>, Vec<Value>), CloudflareApiError> {
    let Some(origin_hostname) = custom
        .get("custom_origin_server")
        .and_then(Value::as_str)
        .filter(|value| fn_knock_origin_instance(value, root).is_some())
    else {
        return Ok((None, Vec::new()));
    };
    let origin_records = api.list_dns_records(zone_id, Some(origin_hostname)).await?;
    let recoverable = recoverable_fn_knock_custom_hostname_from_snapshot(
        custom,
        exact_records,
        &origin_records,
        recovery_origin,
        root,
        current_instance_id,
        recovered_origin,
    );
    Ok((recoverable, origin_records))
}

async fn adopt_recoverable_fn_knock_origin(
    state: &AppState,
    api: &CloudflareApi,
    zone_id: &str,
    ownership: &mut Value,
    recoverable: &RecoverableCustomHostname,
    origin_target: &str,
    instance_id: &str,
) -> Result<(), CloudflareApiError> {
    let origin_dns = upsert_managed_dns(
        api,
        ManagedDnsRequest {
            zone_id,
            name: &recoverable.origin_hostname,
            record_type: "CNAME",
            content: origin_target,
            proxied: true,
            owned_id: recoverable.origin_dns.get("id").and_then(Value::as_str),
            takeover: true,
            instance_id,
        },
    )
    .await?;
    let mut recovered_origin = origin_dns;
    ensure_object(&mut recovered_origin).insert(
        "recoveredFromInstance".to_string(),
        json!(recoverable.legacy_instance_id),
    );
    ensure_nested_object(ownership, &["optimization", "recoveredOrigins"])
        .insert(recoverable.origin_hostname.clone(), recovered_origin);
    save_managed_state(state, ownership).await
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
    fn source_settings_include_builtins_and_normalize_custom_hostnames() {
        let defaults = OptimizationSourceSettings::default();
        assert!(defaults.official_ranges);
        assert_eq!(defaults.builtin_ids.len(), BUILTIN_CANDIDATE_SOURCES.len());

        let normalized = normalize_source_settings(OptimizationSourceSettings {
            official_ranges: true,
            builtin_ids: vec![
                "sweden-government".to_string(),
                "sweden-government".to_string(),
                "us-fbi".to_string(),
                "removed-source".to_string(),
            ],
            custom_hostnames: vec![
                " WWW.Example.org. ".to_string(),
                "www.example.org".to_string(),
            ],
        })
        .expect("settings should normalize");
        assert_eq!(normalized.builtin_ids, vec!["sweden-government"]);
        assert_eq!(normalized.custom_hostnames, vec!["www.example.org"]);
    }

    #[test]
    fn domain_settings_normalize_and_deduplicate_external_hostnames() {
        let normalized = normalize_domain_settings(OptimizationDomainSettings {
            external_hostnames: vec![
                " App.Example.com. ".to_string(),
                "app.example.com".to_string(),
                "other.example.com".to_string(),
            ],
        })
        .expect("domain settings should normalize");
        assert_eq!(
            normalized.external_hostnames,
            vec!["app.example.com", "other.example.com"]
        );
        assert!(
            normalize_domain_settings(OptimizationDomainSettings {
                external_hostnames: vec!["https://app.example.com".to_string()],
            })
            .is_err()
        );
    }

    #[test]
    fn external_hostname_partition_preserves_configured_order() {
        let settings = OptimizationDomainSettings {
            external_hostnames: vec![
                "external.example.com".to_string(),
                "stale.example.com".to_string(),
            ],
        };
        let (managed, external) = partition_optimization_hosts(
            vec![
                "auth.example.com".to_string(),
                "external.example.com".to_string(),
                "app.example.com".to_string(),
            ],
            &settings,
        );
        assert_eq!(managed, vec!["auth.example.com", "app.example.com"]);
        assert_eq!(external, vec!["external.example.com"]);
    }

    #[test]
    fn dns_conflict_details_distinguish_instance_ownership() {
        let records = vec![
            json!({
                "type": "CNAME",
                "content": "current.example.com",
                "proxied": false,
                "comment": "Managed by fn-knock (instance-a)",
            }),
            json!({
                "type": "A",
                "content": "192.0.2.1",
                "proxied": true,
                "tags": ["fn-knock-instance:instance-b"],
            }),
            json!({
                "type": "TXT",
                "content": "external",
                "proxied": null,
            }),
        ];
        let details = dns_conflict_details(
            &records,
            "instance-a",
            "CNAME",
            "desired.example.com",
            false,
        );
        assert_eq!(details["records"][0]["ownerKind"], "current-instance");
        assert_eq!(
            details["records"][1]["ownerKind"],
            "other-fn-knock-instance"
        );
        assert_eq!(details["records"][2]["ownerKind"], "external");
        assert_eq!(details["desired"]["content"], "desired.example.com");
    }

    #[test]
    fn exact_dns_cleanup_uses_the_tracked_origin_or_edge_target() {
        let ownership = json!({
            "optimization": {
                "originDns": { "name": "origin.example.com" },
                "edgeDns": { "name": "edge.example.com" },
            }
        });
        let origin = tracked_exact_dns_snapshot(
            "app.example.com",
            "dns-1",
            &json!({ "exactDnsTarget": "origin" }),
            &ownership,
            None,
        );
        let edge = tracked_exact_dns_snapshot(
            "app.example.com",
            "dns-2",
            &json!({ "exactDnsTarget": "edge" }),
            &ownership,
            None,
        );
        assert_eq!(origin["content"], "origin.example.com");
        assert_eq!(edge["content"], "edge.example.com");
    }

    #[test]
    fn source_settings_reject_urls_ips_and_an_empty_source_set() {
        for value in [
            "https://www.example.org",
            "28.0.2.55",
            "*.example.org",
            "example.org/path",
        ] {
            assert!(normalize_candidate_hostname(value).is_err(), "{value}");
        }
        assert!(
            normalize_source_settings(OptimizationSourceSettings {
                official_ranges: false,
                builtin_ids: Vec::new(),
                custom_hostnames: Vec::new(),
            })
            .is_err()
        );
    }

    #[test]
    fn fake_ip_is_rejected_even_when_a_candidate_hostname_returns_it() {
        let prefixes = parse_prefixes(&CLOUDFLARE_IPV4_FALLBACK.join("\n"));
        assert!(!candidate_ip_is_cloudflare(
            "28.0.2.55".parse().expect("valid fake IP"),
            &prefixes
        ));
        assert!(candidate_ip_is_cloudflare(
            "104.18.26.94".parse().expect("valid Cloudflare IP"),
            &prefixes
        ));
    }

    #[test]
    fn candidate_sources_merge_without_losing_provenance() {
        let ip = "104.18.26.94".parse().expect("valid IP");
        let mut seeds = Vec::new();
        let mut indexes = HashMap::new();
        merge_candidate_seed(&mut seeds, &mut indexes, ip, "builtin", Some("www.gov.se"));
        merge_candidate_seed(&mut seeds, &mut indexes, ip, "official-range", None);
        assert_eq!(seeds.len(), 1);
        assert_eq!(seeds[0].source_hostnames, vec!["www.gov.se"]);
        assert_eq!(seeds[0].source_types, vec!["builtin", "official-range"]);
    }

    #[test]
    fn extracts_real_pop_from_cloudflare_ray_instead_of_geoip() {
        assert_eq!(cf_ray_colo("a261079199891d1c-SIN").as_deref(), Some("SIN"));
        assert_eq!(cf_ray_colo("bad"), None);
        assert_eq!(cf_ray_colo("ray-too-long"), None);
        assert_eq!(
            bounded_cf_ray(&reqwest::header::HeaderValue::from_static(
                "a261079199891d1c-SIN",
            ))
            .as_deref(),
            Some("a261079199891d1c-SIN")
        );
        assert_eq!(
            bounded_cf_ray(&reqwest::header::HeaderValue::from_static("   ")),
            None
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
    fn automatic_switch_requires_a_full_fifteen_percent_lead() {
        assert!(score_is_15_percent_better(85.0, 100.0));
        assert!(!score_is_15_percent_better(85.01, 100.0));
        assert!(!score_is_15_percent_better(f64::NAN, 100.0));
        assert!(!score_is_15_percent_better(10.0, 0.0));
    }

    #[test]
    fn automatic_first_round_uses_the_freshly_measured_current_candidate() {
        let candidate = |ip: &str, score: f64| OptimizationCandidate {
            ip: ip.to_string(),
            median_latency_ms: score,
            jitter_ms: 0.0,
            loss_ratio: 0.0,
            download_mbps: 100.0,
            score,
            verified_at: Some(time_utils::now_iso()),
            source_types: Vec::new(),
            source_hostnames: Vec::new(),
            colo: Some("SIN".to_string()),
            cf_ray: None,
            business_hostname: Some("app.example.com".to_string()),
            business_status: Some(200),
            business_colo: Some("SIN".to_string()),
            business_cf_ray: None,
            business_validated: true,
        };
        let mut ownership = json!({
            "optimization": {
                "selected": { "ip": "104.16.1.1", "score": 1.0 }
            }
        });
        let mut runtime = json!({});
        apply_automatic_scan_result(
            &mut ownership,
            &mut runtime,
            &[
                candidate("104.16.2.2", 80.0),
                candidate("104.16.1.1", 100.0),
            ],
        );
        assert_eq!(
            runtime.pointer("/pendingCandidate/candidate/ip"),
            Some(&json!("104.16.2.2"))
        );
    }

    #[test]
    fn current_candidate_is_kept_inside_the_global_seed_limit() {
        let mut seeds = (0..MAX_CANDIDATES)
            .map(|index| CandidateSeed {
                ip: Ipv4Addr::new(104, 16, (index / 256) as u8, index as u8),
                source_types: vec!["official-range".to_string()],
                source_hostnames: Vec::new(),
            })
            .collect::<Vec<_>>();
        let current = Ipv4Addr::new(104, 17, 1, 1);
        merge_current_candidate_seed(&mut seeds, current);
        assert_eq!(seeds.len(), MAX_CANDIDATES);
        assert!(
            seeds
                .iter()
                .any(|seed| seed.ip == current && seed.source_types == vec!["current"])
        );
    }

    #[test]
    fn completed_scans_expire_after_ten_minutes() {
        let completed = 1_000_000;
        assert!(scan_is_fresh(completed, completed));
        assert!(scan_is_fresh(completed, completed + SCAN_APPLY_TTL_MS));
        assert!(!scan_is_fresh(completed, completed + SCAN_APPLY_TTL_MS + 1));
        assert!(!scan_is_fresh(completed, completed - 1));
        assert!(!scan_is_fresh(0, completed));
    }

    #[test]
    fn source_fingerprint_changes_with_the_effective_configuration() {
        let defaults = OptimizationSourceSettings::default();
        let mut changed = defaults.clone();
        changed.official_ranges = false;
        assert_eq!(
            source_settings_fingerprint(&defaults),
            source_settings_fingerprint(&defaults.clone())
        );
        assert_ne!(
            source_settings_fingerprint(&defaults),
            source_settings_fingerprint(&changed)
        );
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
    fn scan_errors_distinguish_saas_setup_validation_and_readiness() {
        let saas_required = scan_validation_hostname_error(&json!({
            "optimization": {
                "capabilityProbe": {
                    "status": "unsupported",
                    "reasonCode": CLOUDFLARE_SAAS_REQUIRED_ERROR_CODE,
                }
            }
        }));
        assert_eq!(
            optimization_scan_error_code(&saas_required),
            Some(CLOUDFLARE_SAAS_REQUIRED_ERROR_CODE)
        );

        let validation_pending = scan_validation_hostname_error(&json!({
            "optimization": {
                "capabilityProbe": {
                    "status": "pending",
                    "hostnameStatus": "active",
                    "sslStatus": "pending_validation",
                }
            }
        }));
        assert_eq!(
            optimization_scan_error_code(&validation_pending),
            Some(CLOUDFLARE_SAAS_VALIDATION_PENDING_ERROR_CODE)
        );

        let ownership_conflict = scan_validation_hostname_error(&json!({
            "optimization": {
                "capabilityProbe": { "status": "compatible" },
                "customHostnames": {
                    "auth.example.com": { "status": "conflict" }
                }
            }
        }));
        assert_eq!(
            optimization_scan_error_code(&ownership_conflict),
            Some(CLOUDFLARE_RESOURCE_CONFLICT_ERROR_CODE)
        );

        let compatible_probe_without_live_hostname = scan_validation_hostname_error(&json!({
            "optimization": {
                "capabilityProbe": { "status": "compatible" }
            }
        }));
        assert_eq!(
            optimization_scan_error_code(&compatible_probe_without_live_hostname),
            Some(OPTIMIZATION_NOT_READY_ERROR_CODE)
        );

        let not_ready = scan_validation_hostname_error(&json!({}));
        assert_eq!(
            optimization_scan_error_code(&not_ready),
            Some(OPTIMIZATION_NOT_READY_ERROR_CODE)
        );

        let resolution_unavailable = local_error(CANDIDATE_RESOLUTION_UNAVAILABLE_SCAN_ERROR);
        assert_eq!(
            optimization_scan_error_code(&resolution_unavailable),
            Some(CANDIDATE_RESOLUTION_UNAVAILABLE_ERROR_CODE)
        );

        let unrelated = local_error("latency probe failed");
        assert_eq!(optimization_scan_error_code(&unrelated), None);
    }

    #[test]
    fn recovers_only_a_fully_verified_previous_fn_knock_lineage() {
        let custom = json!({
            "id": "custom-id",
            "hostname": "auth.tu.example.com",
            "custom_origin_server": "fnknock-origin-7f531e6dd1e4.tu.example.com",
            "status": "active",
            "ssl": { "status": "active" },
        });
        let exact = json!([{
            "id": "exact-id",
            "name": "auth.tu.example.com",
            "type": "CNAME",
            "content": "fnknock-edge-7f531e6dd1e4.tu.example.com",
            "proxied": false,
            "comment": "Managed by fn-knock (7f531e6dd1e4)",
            "tags": [],
        }]);
        let origin = json!([{
            "id": "origin-id",
            "name": "fnknock-origin-7f531e6dd1e4.tu.example.com",
            "type": "CNAME",
            "content": "b8e3c226-e512-4232-a5a1-3fbdc590e880.cfargotunnel.com",
            "proxied": true,
            "comment": "Managed by fn-knock (7f531e6dd1e4)",
            "tags": [],
        }]);
        let recovered = recoverable_fn_knock_custom_hostname_from_snapshot(
            &custom,
            exact.as_array().unwrap(),
            origin.as_array().unwrap(),
            Some("fnknock-origin-7f531e6dd1e4.tu.example.com"),
            "tu.example.com",
            "f63f7fcb2f0f",
            None,
        )
        .expect("verified previous fn-knock lineage should be recoverable");
        assert_eq!(recovered.legacy_instance_id, "7f531e6dd1e4");
        assert_eq!(recovered.exact_dns["id"], json!("exact-id"));
        assert_eq!(recovered.origin_dns["id"], json!("origin-id"));

        assert!(
            recoverable_fn_knock_custom_hostname_from_snapshot(
                &custom,
                exact.as_array().unwrap(),
                origin.as_array().unwrap(),
                Some("fnknock-origin-another000000.tu.example.com"),
                "tu.example.com",
                "f63f7fcb2f0f",
                None,
            )
            .is_none()
        );
        let unrelated_exact = json!([{
            "id": "exact-id",
            "name": "auth.tu.example.com",
            "type": "CNAME",
            "content": "fnknock-edge-7f531e6dd1e4.tu.example.com",
            "proxied": false,
            "comment": "managed manually",
            "tags": [],
        }]);
        assert!(
            recoverable_fn_knock_custom_hostname_from_snapshot(
                &custom,
                unrelated_exact.as_array().unwrap(),
                origin.as_array().unwrap(),
                Some("fnknock-origin-7f531e6dd1e4.tu.example.com"),
                "tu.example.com",
                "f63f7fcb2f0f",
                None,
            )
            .is_none()
        );

        let recovered_origin = json!({
            "id": "origin-id",
            "name": "fnknock-origin-7f531e6dd1e4.tu.example.com",
            "type": "CNAME",
            "content": "eda45cde-5a2b-4a6e-9f0f-52ca0c75254f.cfargotunnel.com",
            "proxied": true,
            "comment": "Managed by fn-knock (f63f7fcb2f0f)",
            "tags": [],
            "recoveredFromInstance": "7f531e6dd1e4",
        });
        let current_origin = json!([{
            "id": "origin-id",
            "name": "fnknock-origin-7f531e6dd1e4.tu.example.com",
            "type": "CNAME",
            "content": "eda45cde-5a2b-4a6e-9f0f-52ca0c75254f.cfargotunnel.com",
            "proxied": true,
            "comment": "Managed by fn-knock (f63f7fcb2f0f)",
            "tags": [],
        }]);
        assert!(
            recoverable_fn_knock_custom_hostname_from_snapshot(
                &custom,
                exact.as_array().unwrap(),
                current_origin.as_array().unwrap(),
                Some("fnknock-origin-7f531e6dd1e4.tu.example.com"),
                "tu.example.com",
                "f63f7fcb2f0f",
                Some(&recovered_origin),
            )
            .is_some()
        );
        let mut changed_origin = current_origin.clone();
        changed_origin[0]["content"] =
            json!("b8e3c226-e512-4232-a5a1-3fbdc590e880.cfargotunnel.com");
        assert!(
            recoverable_fn_knock_custom_hostname_from_snapshot(
                &custom,
                exact.as_array().unwrap(),
                changed_origin.as_array().unwrap(),
                Some("fnknock-origin-7f531e6dd1e4.tu.example.com"),
                "tu.example.com",
                "f63f7fcb2f0f",
                Some(&recovered_origin),
            )
            .is_none()
        );
    }

    #[test]
    fn managed_custom_hostname_rejects_id_hostname_and_origin_drift() {
        let owned = json!({
            "id": "custom-id",
            "customOriginServer": "fnknock-origin-old.tu.example.com",
        });
        let remote = json!({
            "id": "custom-id",
            "hostname": "auth.tu.example.com",
            "custom_origin_server": "fnknock-origin-old.tu.example.com",
        });
        assert!(managed_custom_hostname_matches(
            &remote,
            "auth.tu.example.com",
            &owned,
            Some("fnknock-origin-current.tu.example.com"),
        ));

        for (field, value) in [
            ("id", "different-id"),
            ("hostname", "other.tu.example.com"),
            (
                "custom_origin_server",
                "fnknock-origin-other.tu.example.com",
            ),
        ] {
            let mut drifted = remote.clone();
            drifted[field] = json!(value);
            assert!(!managed_custom_hostname_matches(
                &drifted,
                "auth.tu.example.com",
                &owned,
                Some("fnknock-origin-current.tu.example.com"),
            ));
        }

        let legacy_owned = json!({ "id": "custom-id" });
        assert!(managed_custom_hostname_matches(
            &json!({
                "id": "custom-id",
                "hostname": "auth.tu.example.com",
                "custom_origin_server": "fnknock-origin-current.tu.example.com",
            }),
            "auth.tu.example.com",
            &legacy_owned,
            Some("fnknock-origin-current.tu.example.com"),
        ));
        assert!(!managed_custom_hostname_matches(
            &remote,
            "auth.tu.example.com",
            &legacy_owned,
            None,
        ));
    }

    #[test]
    fn scan_validation_requires_both_hostname_and_certificate_readiness() {
        let pending_business_hostname = json!({
            "optimization": {
                "customHostnames": {
                    "pending.example.com": {
                        "status": "pending",
                        "sslStatus": "active",
                    }
                }
            }
        });
        assert_eq!(scan_validation_hostname(&pending_business_hostname), None);

        let ready_business_hostname = json!({
            "optimization": {
                "customHostnames": {
                    "ready.example.com": {
                        "id": "custom-ready",
                        "status": "ready",
                        "sslStatus": "active",
                        "exactDnsId": "dns-ready",
                    }
                }
            }
        });
        assert_eq!(
            scan_validation_hostname(&ready_business_hostname).as_deref(),
            Some("ready.example.com")
        );

        let pending_capability_hostname = json!({
            "optimization": {
                "capabilityProbe": {
                    "hostname": "probe.example.com",
                    "status": "pending",
                    "hostnameStatus": "pending",
                    "sslStatus": "active",
                    "activationDns": { "id": "dns-probe" },
                }
            }
        });
        assert_eq!(scan_validation_hostname(&pending_capability_hostname), None);

        let ready_capability_hostname = json!({
            "optimization": {
                "capabilityProbe": {
                    "hostname": "probe.example.com",
                    "status": "pending",
                    "hostnameStatus": "active",
                    "sslStatus": "active",
                    "activationDns": { "id": "dns-probe" },
                }
            }
        });
        assert_eq!(
            scan_validation_hostname(&ready_capability_hostname).as_deref(),
            Some("probe.example.com")
        );

        let capability_without_activation_dns = json!({
            "optimization": {
                "capabilityProbe": {
                    "hostname": "probe.example.com",
                    "status": "awaiting-candidate",
                    "hostnameStatus": "active",
                    "sslStatus": "active",
                }
            }
        });
        assert_eq!(
            scan_validation_hostname(&capability_without_activation_dns),
            None
        );

        let cleaned_capability_hostname = json!({
            "optimization": {
                "capabilityProbe": {
                    "hostname": "deleted-probe.example.com",
                    "status": "compatible",
                }
            }
        });
        assert_eq!(scan_validation_hostname(&cleaned_capability_hostname), None);

        let partially_reconciled = json!({
            "optimization": {
                "customHostnames": {
                    "ready.example.com": {
                        "status": "optimized",
                        "sslStatus": "active",
                        "exactDnsId": "dns-ready",
                    },
                    "conflict.example.com": { "status": "conflict" },
                }
            }
        });
        assert_eq!(scan_validation_hostname(&partially_reconciled), None);
        assert_eq!(
            optimization_scan_error_code(&scan_validation_hostname_error(&partially_reconciled)),
            Some(CLOUDFLARE_RESOURCE_CONFLICT_ERROR_CODE)
        );

        let failed_route_with_active_hostname = json!({
            "optimization": {
                "customHostnames": {
                    "retry.example.com": {
                        "id": "custom-retry",
                        "status": "probe-failed",
                        "hostnameStatus": "active",
                        "sslStatus": "active",
                        "message": "Cloudflare edge returned HTTP 530",
                    }
                }
            }
        });
        assert_eq!(
            scan_validation_hostname(&failed_route_with_active_hostname).as_deref(),
            Some("retry.example.com")
        );
        assert_eq!(
            active_probe_hostnames(&failed_route_with_active_hostname),
            vec!["retry.example.com"]
        );

        let legacy_fallback_without_exact_dns = json!({
            "optimization": {
                "customHostnames": {
                    "fallback.example.com": {
                        "id": "custom-fallback",
                        "status": "fallback",
                        "sslStatus": "active",
                    }
                }
            }
        });
        assert_eq!(
            scan_validation_hostname(&legacy_fallback_without_exact_dns).as_deref(),
            Some("fallback.example.com")
        );

        let conflict_with_active_hostname = json!({
            "optimization": {
                "customHostnames": {
                    "conflict.example.com": {
                        "id": "custom-conflict",
                        "status": "conflict",
                        "hostnameStatus": "active",
                        "sslStatus": "active",
                    }
                }
            }
        });
        assert_eq!(
            scan_validation_hostname(&conflict_with_active_hostname),
            None
        );
        assert!(active_probe_hostnames(&conflict_with_active_hostname).is_empty());
    }

    #[test]
    fn cloudflare_route_rejections_preserve_the_actionable_cause() {
        assert_eq!(
            cloudflare_route_rejection_message(
                403,
                "cloudflare error 1000: dns points to prohibited ip"
            )
            .as_deref(),
            Some("Cloudflare Error 1000: DNS points to a prohibited Cloudflare IP")
        );
        assert_eq!(
            cloudflare_route_rejection_message(530, "error 1016").as_deref(),
            Some("Cloudflare Error 1016: origin DNS resolution failed")
        );
        assert_eq!(
            cloudflare_route_rejection_message(522, "gateway unavailable").as_deref(),
            Some("Cloudflare edge returned HTTP 522")
        );
        assert_eq!(cloudflare_route_rejection_message(200, "ok"), None);
    }

    #[test]
    fn control_plane_refresh_does_not_republish_a_suppressed_route() {
        let mut host_state = json!({
            "id": "custom-fallback",
            "status": "fallback",
            "hostnameStatus": "pending",
            "sslStatus": "pending_validation",
        });
        let changed = update_custom_hostname_activation(
            &mut host_state,
            &json!({
                "status": "active",
                "ssl": { "status": "active" },
            }),
        );

        assert!(changed);
        assert_eq!(
            host_state.get("status").and_then(Value::as_str),
            Some("fallback")
        );
        assert_eq!(
            host_state.get("hostnameStatus").and_then(Value::as_str),
            Some("active")
        );
        assert_eq!(
            host_state.get("sslStatus").and_then(Value::as_str),
            Some("active")
        );
        assert!(host_state.get("exactDnsId").is_none());
        assert!(custom_hostname_can_validate_candidates(&host_state));
    }

    #[test]
    fn capability_route_failure_remains_retryable() {
        let failed = capability_probe_failure_state(
            &json!({
                "id": "capability-id",
                "hostname": "probe.example.com",
                "status": "pending",
                "hostnameStatus": "active",
                "sslStatus": "active",
                "activationDns": { "id": "activation-id" },
            }),
            "Cloudflare edge returned HTTP 530",
        );

        assert_eq!(
            failed.get("status").and_then(Value::as_str),
            Some("probe-failed")
        );
        assert_eq!(
            failed.get("messageCode").and_then(Value::as_str),
            Some("preferredEdgeProbeFailed")
        );
        assert!(failed.get("reasonCode").is_none());
        assert!(capability_probe_hostname_is_ready(&failed));
        assert!(!capability_probe_is_definitively_unsupported(&json!({
            "status": "unsupported",
            "message": "Cloudflare edge returned HTTP 530",
        })));
        assert!(capability_probe_is_definitively_unsupported(&json!({
            "status": "unsupported",
            "reasonCode": CLOUDFLARE_SAAS_REQUIRED_ERROR_CODE,
        })));
    }

    #[test]
    fn failed_edge_route_state_is_safe_for_origin_fallback_and_rescan() {
        let mut host_state = json!({
            "id": "custom-id",
            "status": "optimized",
            "hostnameStatus": "active",
            "sslStatus": "active",
            "exactDnsId": "edge-record",
            "exactDnsTarget": "edge",
        });
        assert!(exact_route_is_optimized(&host_state));

        set_exact_dns_route(&mut host_state, &json!({ "id": "origin-record" }), "origin");
        record_preferred_edge_probe_failure(&mut host_state, "Cloudflare edge returned HTTP 522");

        assert!(!exact_route_is_optimized(&host_state));
        assert_eq!(
            host_state.get("exactDnsTarget").and_then(Value::as_str),
            Some("origin")
        );
        assert_eq!(
            host_state.get("status").and_then(Value::as_str),
            Some("probe-failed")
        );
        assert!(custom_hostname_can_validate_candidates(&host_state));
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
