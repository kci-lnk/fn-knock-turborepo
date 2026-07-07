use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{BufRead, BufReader, Read},
    net::IpAddr,
    path::Path,
    process::Command,
    time::Duration,
};

use axum::{
    Json, Router,
    body::Bytes,
    extract::{Path as AxumPath, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use flate2::read::GzDecoder;
use ipnet::IpNet;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use tokio::time::{self as tokio_time, MissedTickBehavior};

use crate::{
    http_utils::{is_private_or_local_ip, normalize_ip},
    i18n::Translator,
    ip_location, response, runtime_profile, scanner,
    state::AppState,
    system_events, time_utils,
};

const RUNTIME_KEY: &str = "fn_knock:ssh_security:runtime";
const BLOCKS_INDEX_KEY: &str = "fn_knock:ssh_security:blocks:index";
const BLOCK_DATA_PREFIX: &str = "fn_knock:ssh_security:blocks:data:";
const FAILURES_PREFIX: &str = "fn_knock:ssh_security:failures:";
const PROCESSED_PREFIX: &str = "fn_knock:ssh_security:processed:";
const SSH_FIREWALL_CHAIN: &str = "FN-KNOCK-SSH";
const PROCESSED_TTL_SECONDS: i64 = 7 * 24 * 3600;
const STARTUP_BACKFILL_LOG_LIMIT: usize = 2000;
const SUCCESS_LOG_COALESCE_WINDOW_MS: i64 = 30 * 1000;
const SSH_SECURITY_TICK_SECONDS: u64 = 10;
const AUTH_LOG_CANDIDATES: &[&str] = &[
    "/var/log/auth.log",
    "/var/log/auth.log.1",
    "/var/log/auth.log.1.gz",
];

fn ssh_security_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.sshSecurity.{key}"))
}

fn ssh_security_text_params(
    translator: &Translator,
    key: &str,
    params: &[(&str, String)],
) -> String {
    translator.t_params(&format!("server.sshSecurity.{key}"), params)
}

fn ssh_security_route_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.sshSecurity.routes.{key}"))
}

fn ssh_security_route_text_params(
    translator: &Translator,
    key: &str,
    params: &[(&str, String)],
) -> String {
    translator.t_params(&format!("server.sshSecurity.routes.{key}"), params)
}

#[derive(Deserialize)]
struct ListQuery {
    page: Option<String>,
    limit: Option<String>,
    search: Option<String>,
    outcome: Option<String>,
}

pub fn ssh_security_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/admin/ssh-security/config",
            get(get_config).post(update_config),
        )
        .route("/api/admin/ssh-security/firewall/sync", post(sync_firewall))
        .route(
            "/api/admin/ssh-security/firewall/clear",
            post(clear_firewall),
        )
        .route("/api/admin/ssh-security/login-logs", get(login_logs))
        .route(
            "/api/admin/ssh-security/blocks",
            get(list_blocks).delete(delete_blocks),
        )
        .route(
            "/api/admin/ssh-security/blocks/{ip}",
            get(get_block).delete(delete_block),
        )
}

pub fn start_ssh_security_tasks(state: AppState) {
    tokio::spawn(async move {
        if let Err(error) = ssh_security_maintenance_tick(&state).await {
            tracing::warn!(%error, "SSH security boot sync failed");
        }

        let mut ticker = tokio_time::interval(Duration::from_secs(SSH_SECURITY_TICK_SECONDS));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            if let Err(error) = ssh_security_maintenance_tick(&state).await {
                tracing::debug!(%error, "SSH security maintenance tick failed");
            }
        }
    });
}

async fn get_config(State(state): State<AppState>) -> Response {
    match ssh_security_details(&state).await {
        Ok(details) => response::ok(details).into_response(),
        Err(error) => {
            let translator = Translator::from_state(&state).await;
            tracing::warn!(%error, "failed to load SSH security config");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ssh_security_route_text(&translator, "loadConfigFailed"),
            )
        }
    }
}

async fn update_config(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    let translator = Translator::from_state(&state).await;
    match update_ssh_security_config(&state, body, &translator).await {
        Ok(details) => response::ok(details).into_response(),
        Err(SshError::BadRequest(message)) => response::error(StatusCode::BAD_REQUEST, message),
        Err(SshError::Runtime(message)) => response::error(StatusCode::BAD_REQUEST, message),
        Err(SshError::Redis(error)) => {
            tracing::warn!(%error, "failed to update SSH security config");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ssh_security_route_text(&translator, "updateConfigFailed"),
            )
        }
    }
}

async fn sync_firewall(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match sync_firewall_blocks_now(&state, &translator).await {
        Ok(value) => {
            let ports = value
                .get("ports")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_i64)
                        .map(|port| port.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            let message = ssh_security_route_text_params(
                &translator,
                "syncFirewallSuccess",
                &[
                    (
                        "allowedCidrs",
                        value
                            .get("allowed_cidrs")
                            .and_then(Value::as_i64)
                            .unwrap_or(0)
                            .to_string(),
                    ),
                    ("ports", ports),
                    (
                        "synced",
                        value
                            .get("synced")
                            .and_then(Value::as_i64)
                            .unwrap_or(0)
                            .to_string(),
                    ),
                ],
            );
            Json(json!({ "success": true, "message": message, "data": value })).into_response()
        }
        Err(error) => {
            tracing::warn!(%error, "failed to sync SSH firewall");
            let message = error.to_string();
            response::error(
                StatusCode::BAD_GATEWAY,
                if message.trim().is_empty() {
                    ssh_security_route_text(&translator, "syncFirewallFailed")
                } else {
                    message
                },
            )
        }
    }
}

async fn clear_firewall(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    let availability = ssh_security_availability(&state, &translator);
    if !availability.available {
        return response::error(StatusCode::BAD_REQUEST, availability.reason);
    }
    let payload = json!({
        "chain_name": SSH_FIREWALL_CHAIN,
        "parent_chain": ["INPUT", "DOCKER-USER"]
    });
    match state.go_backend.clear_ssh_firewall(&payload).await {
        Ok(value) => {
            if let Err(error) = ensure_go_success(value, &translator, "clearSshPolicyFailed")
                .map_err(|error| {
                    tracing::warn!(%error, "go backend rejected SSH firewall clear");
                    error
                })
            {
                return response::error(
                    StatusCode::BAD_GATEWAY,
                    if error.to_string().trim().is_empty() {
                        ssh_security_route_text(&translator, "clearFirewallFailed")
                    } else {
                        error.to_string()
                    },
                );
            }
            let mut cleared = 0usize;
            match active_blocks(&state).await {
                Ok(records) => {
                    for record in records {
                        if let Some(ip) = record.get("ip").and_then(Value::as_str)
                            && mark_block_removed(&state, ip, "manual")
                                .await
                                .unwrap_or(false)
                        {
                            if let Err(error) = clear_failures(&state, ip).await {
                                tracing::warn!(%error, ip, "failed to clear SSH failures");
                            }
                            cleared += 1;
                        }
                    }
                }
                Err(error) => tracing::warn!(%error, "failed to mark SSH blocks as cleared"),
            }
            Json(json!({
                "success": true,
                "message": ssh_security_route_text(&translator, "clearFirewallSuccess"),
                "data": { "cleared_blocks": cleared }
            }))
            .into_response()
        }
        Err(error) => {
            tracing::warn!(%error, "go backend SSH firewall clear failed");
            response::error(
                StatusCode::BAD_GATEWAY,
                ssh_security_route_text(&translator, "clearFirewallFailed"),
            )
        }
    }
}

async fn login_logs(State(state): State<AppState>, Query(query): Query<ListQuery>) -> Response {
    let page = parse_positive(query.page.as_deref(), 1, i64::MAX);
    let limit = parse_positive(query.limit.as_deref(), 20, 100);
    let search = query.search.unwrap_or_default().trim().to_ascii_lowercase();
    let outcome = query.outcome.unwrap_or_default();
    let outcome = if outcome == "success" || outcome == "failure" {
        outcome
    } else {
        String::new()
    };
    let mut entries = query_recent_ssh_logs((page * limit * 5 + limit * 5).max(500) as usize);
    entries.retain(|entry| {
        if !outcome.is_empty()
            && entry.get("outcome").and_then(Value::as_str) != Some(outcome.as_str())
        {
            return false;
        }
        if search.is_empty() {
            return true;
        }
        ["ip", "username", "raw"].iter().any(|key| {
            entry
                .get(*key)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_ascii_lowercase()
                .contains(&search)
        })
    });
    entries.sort_by(|left, right| {
        iso_score(right.get("happened_at").and_then(Value::as_str))
            .cmp(&iso_score(left.get("happened_at").and_then(Value::as_str)))
    });
    entries = coalesce_success_login_logs(entries);
    let total = entries.len();
    let start = ((page - 1) * limit) as usize;
    let mut items = entries
        .into_iter()
        .skip(start)
        .take(limit as usize)
        .collect::<Vec<_>>();
    hydrate_ip_location_records(&state, &mut items, |entry| {
        entry
            .get("id")
            .and_then(Value::as_str)
            .map(|id| format!("ssh-login-log|{id}"))
    })
    .await;
    response::ok(json!({ "items": items, "total": total, "page": page, "limit": limit }))
        .into_response()
}

async fn list_blocks(State(state): State<AppState>, Query(query): Query<ListQuery>) -> Response {
    let translator = Translator::from_state(&state).await;
    let page = parse_positive(query.page.as_deref(), 1, i64::MAX);
    let limit = parse_positive(query.limit.as_deref(), 20, 100);
    let search = query.search.unwrap_or_default().trim().to_ascii_lowercase();
    match list_active_blocks(&state, page, limit, &search).await {
        Ok((mut items, total)) => {
            hydrate_ip_location_records(&state, &mut items, |record| {
                record
                    .get("ip")
                    .and_then(Value::as_str)
                    .map(|ip| format!("ssh-blocklist|{ip}"))
            })
            .await;
            response::ok(json!({ "items": items, "total": total, "page": page, "limit": limit }))
                .into_response()
        }
        Err(error) => {
            tracing::warn!(%error, "failed to list SSH security blocks");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ssh_security_route_text(&translator, "listBlocksFailed"),
            )
        }
    }
}

async fn get_block(State(state): State<AppState>, AxumPath(ip): AxumPath<String>) -> Response {
    let translator = Translator::from_state(&state).await;
    let normalized = normalize_ip(&ip);
    if normalized.is_empty() {
        return response::error(
            StatusCode::NOT_FOUND,
            ssh_security_route_text(&translator, "blockNotFound"),
        );
    }
    match load_block(&state, &normalized).await {
        Ok(Some(mut record)) if is_active_block(&record, time_utils::now_ms()) => {
            hydrate_ip_location_records(&state, std::slice::from_mut(&mut record), |record| {
                record
                    .get("ip")
                    .and_then(Value::as_str)
                    .map(|ip| format!("ssh-blocklist|{ip}"))
            })
            .await;
            response::ok(record).into_response()
        }
        Ok(_) => response::error(
            StatusCode::NOT_FOUND,
            ssh_security_route_text(&translator, "blockNotFound"),
        ),
        Err(error) => {
            tracing::warn!(%error, ip = normalized, "failed to load SSH block");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ssh_security_route_text(&translator, "loadBlockFailed"),
            )
        }
    }
}

async fn delete_block(State(state): State<AppState>, AxumPath(ip): AxumPath<String>) -> Response {
    let translator = Translator::from_state(&state).await;
    match remove_block(&state, &ip, "manual", &translator).await {
        Ok(true) => response::success_empty().into_response(),
        Ok(false) => response::error(
            StatusCode::NOT_FOUND,
            ssh_security_route_text(&translator, "blockNotFound"),
        ),
        Err(error) => {
            tracing::warn!(%error, "failed to remove SSH block");
            response::error(
                StatusCode::BAD_REQUEST,
                ssh_security_route_text(&translator, "removeBlockFailed"),
            )
        }
    }
}

async fn delete_blocks(State(state): State<AppState>, body: Bytes) -> Response {
    let translator = Translator::from_state(&state).await;
    let parsed = parse_json_body(&body);
    let raw_ips = parsed
        .get("ips")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(delete_ip_value_to_string)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if raw_ips.is_empty() {
        return response::error(
            StatusCode::BAD_REQUEST,
            ssh_security_route_text(&translator, "selectIps"),
        );
    }
    let mut removed = 0usize;
    let mut seen = HashSet::new();
    for raw_ip in raw_ips {
        let ip = normalize_ip(&raw_ip);
        if ip.is_empty() || !seen.insert(ip.clone()) {
            continue;
        }
        match remove_block(&state, &ip, "manual", &translator).await {
            Ok(true) => removed += 1,
            Ok(false) => {}
            Err(error) => tracing::warn!(%error, ip, "failed to remove SSH block"),
        }
    }
    response::ok(json!({ "removed": removed })).into_response()
}

#[derive(Debug)]
enum SshError {
    BadRequest(String),
    Runtime(String),
    Redis(redis::RedisError),
}

impl From<redis::RedisError> for SshError {
    fn from(value: redis::RedisError) -> Self {
        Self::Redis(value)
    }
}

async fn ssh_security_details(state: &AppState) -> Result<Value, redis::RedisError> {
    let translator = Translator::from_state(state).await;
    let config = load_config(state).await?;
    let runtime = load_runtime(state).await?;
    let active_block_count = active_blocks(state).await?.len();
    let ports = resolve_ssh_ports();
    let availability = ssh_security_availability(state, &translator);
    Ok(json!({
        "config": config,
        "summary": {
            "configured": config.get("configured_at").is_some_and(|value| !value.is_null()),
            "enabled": config.get("enabled").and_then(Value::as_bool).unwrap_or(false),
            "allowed_cidr_count": runtime.get("allowed_cidrs").and_then(Value::as_array).map(|items| items.len()).unwrap_or_default(),
            "active_block_count": active_block_count,
            "ssh_ports": ports,
            "log_source": availability.log_source,
            "available": availability.available,
            "unavailable_reason": availability.reason,
            "updated_at": config.get("updated_at").cloned().unwrap_or(Value::Null)
        }
    }))
}

async fn update_ssh_security_config(
    state: &AppState,
    body: Value,
    translator: &Translator,
) -> Result<Value, SshError> {
    let previous = load_config(state).await?;
    let (config, runtime) = compile_config_patch(state, &body, &previous, translator).await?;
    if config.get("enabled").and_then(Value::as_bool) == Some(true) {
        let availability = ssh_security_availability(state, translator);
        if !availability.available {
            return Err(SshError::Runtime(availability.reason));
        }
    }
    let mut all = state.redis.get_config().await?;
    if let Some(object) = all.as_object_mut() {
        object.insert("ssh_security".to_string(), config.clone());
    }
    state.redis.save_config(&all).await?;
    state.redis.set_json_value(RUNTIME_KEY, &runtime).await?;
    apply_ssh_security_config_once(state, &config, &runtime)
        .await
        .map_err(|error| SshError::Runtime(error.to_string()))?;
    ssh_security_details(state).await.map_err(SshError::Redis)
}

async fn load_config(state: &AppState) -> redis::RedisResult<Value> {
    let config = state.redis.get_config().await?;
    Ok(normalize_config(config.get("ssh_security").cloned()))
}

async fn load_runtime(state: &AppState) -> redis::RedisResult<Value> {
    Ok(normalize_runtime(
        state.redis.get_json_value(RUNTIME_KEY).await?,
    ))
}

pub(crate) fn normalize_config(value: Option<Value>) -> Value {
    let raw = value
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    json!({
        "enabled": raw.get("enabled").and_then(Value::as_bool).unwrap_or(false),
        "window_minutes": int_field(&raw, "window_minutes", 10, 1, 24 * 60),
        "failed_login_threshold": int_field(&raw, "failed_login_threshold", 5, 1, 1000),
        "block_duration_value": int_field(&raw, "block_duration_value", 1, 1, 365),
        "block_duration_unit": normalize_duration_unit(raw.get("block_duration_unit").and_then(Value::as_str)),
        "allowed_regions": normalize_allowed_regions(raw.get("allowed_regions")),
        "custom_cidrs": normalize_cidrs(raw.get("custom_cidrs")),
        "configured_at": normalize_timestamp(raw.get("configured_at")),
        "updated_at": normalize_timestamp(raw.get("updated_at"))
    })
}

fn normalize_runtime(value: Option<Value>) -> Value {
    let raw = value
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    json!({
        "enabled": raw.get("enabled").and_then(Value::as_bool).unwrap_or(false),
        "allowed_cidrs": normalize_cidrs(raw.get("allowed_cidrs")),
        "updated_at": normalize_timestamp(raw.get("updated_at"))
    })
}

async fn compile_config_patch(
    state: &AppState,
    body: &Value,
    previous: &Value,
    translator: &Translator,
) -> Result<(Value, Value), SshError> {
    let raw = body.as_object().cloned().unwrap_or_default();
    let now = time_utils::now_iso();
    let enabled = raw
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or_else(|| {
            previous
                .get("enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        });
    let allowed_regions = resolve_allowed_regions(state, &raw, previous).await?;
    let custom_cidrs = if raw.contains_key("custom_cidrs") {
        validate_cidrs(raw.get("custom_cidrs"), translator)?;
        normalize_cidrs(raw.get("custom_cidrs"))
    } else {
        normalize_cidrs(previous.get("custom_cidrs"))
    };
    let config = json!({
        "enabled": enabled,
        "window_minutes": int_field_or_previous(&raw, previous, "window_minutes", 10, 1, 24 * 60),
        "failed_login_threshold": int_field_or_previous(&raw, previous, "failed_login_threshold", 5, 1, 1000),
        "block_duration_value": int_field_or_previous(&raw, previous, "block_duration_value", 1, 1, 365),
        "block_duration_unit": raw.get("block_duration_unit").and_then(Value::as_str).map(|value| normalize_duration_unit(Some(value))).unwrap_or_else(|| previous.get("block_duration_unit").and_then(Value::as_str).unwrap_or("day").to_string()),
        "allowed_regions": allowed_regions.selections,
        "custom_cidrs": custom_cidrs,
        "configured_at": previous.get("configured_at").cloned().filter(|value| !value.is_null()).unwrap_or_else(|| Value::String(now.clone())),
        "updated_at": now
    });
    let runtime = build_runtime_from_config(&config, allowed_regions.cidrs);
    Ok((config, runtime))
}

struct ResolvedAllowedRegions {
    selections: Value,
    cidrs: Vec<String>,
}

async fn resolve_allowed_regions(
    state: &AppState,
    raw: &Map<String, Value>,
    previous: &Value,
) -> Result<ResolvedAllowedRegions, SshError> {
    let source = if raw.contains_key("allowed_regions") {
        raw.get("allowed_regions")
    } else {
        previous.get("allowed_regions")
    };
    let Some(items) = source.and_then(Value::as_array) else {
        return Ok(ResolvedAllowedRegions {
            selections: json!([]),
            cidrs: Vec::new(),
        });
    };

    let mut seen = HashSet::new();
    let mut selections = Vec::new();
    let mut cidrs = Vec::new();
    for item in items {
        let province = item
            .get("province")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        if province.is_empty() {
            continue;
        }
        let query_city = item
            .get("query_city")
            .or_else(|| item.get("queryCity"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let key = format!("{province}::{}", query_city.unwrap_or(""));
        if !seen.insert(key) {
            continue;
        }
        let lookup = scanner::lookup_cidr_region(state, province, query_city)
            .await
            .map_err(SshError::BadRequest)?;
        selections.push(lookup.selection);
        cidrs.extend(lookup.cidrs);
    }
    cidrs = normalize_cidr_strings(cidrs);
    Ok(ResolvedAllowedRegions {
        selections: Value::Array(selections),
        cidrs,
    })
}

fn build_runtime_from_config(config: &Value, mut resolved_region_cidrs: Vec<String>) -> Value {
    let custom_cidrs = config
        .get("custom_cidrs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string);
    resolved_region_cidrs.extend(custom_cidrs);
    let allowed_cidrs = normalize_cidr_strings(resolved_region_cidrs);
    json!({
        "enabled": config.get("enabled").and_then(Value::as_bool).unwrap_or(false),
        "allowed_cidrs": if config.get("enabled").and_then(Value::as_bool).unwrap_or(false) {
            Value::Array(allowed_cidrs.into_iter().map(Value::String).collect())
        } else {
            json!([])
        },
        "updated_at": time_utils::now_iso()
    })
}

struct SshAvailability {
    available: bool,
    reason: String,
    log_source: &'static str,
}

struct FirewallPolicyResult {
    allowed_cidrs: usize,
    blocked_ips: usize,
    ports: Vec<i64>,
}

async fn ssh_security_maintenance_tick(state: &AppState) -> anyhow::Result<()> {
    let config = load_config(state).await?;
    let runtime = load_runtime(state).await?;
    apply_ssh_security_config_once(state, &config, &runtime).await
}

async fn apply_ssh_security_config_once(
    state: &AppState,
    config: &Value,
    runtime: &Value,
) -> anyhow::Result<()> {
    let translator = Translator::from_state(state).await;
    if config.get("enabled").and_then(Value::as_bool) != Some(true)
        || runtime.get("enabled").and_then(Value::as_bool) != Some(true)
    {
        disable_ssh_security(state, Some(runtime)).await?;
        return Ok(());
    }

    let availability = ssh_security_availability(state, &translator);
    if !availability.available {
        tracing::warn!(reason = %availability.reason, "skipped SSH security sync");
        disable_ssh_security(state, Some(runtime)).await?;
        return Ok(());
    }

    reconcile_expired_blocks(state).await?;
    let _ = sync_firewall_policy(state, Some(runtime), None, Vec::new(), &translator).await?;
    process_recent_ssh_entries(state, config, STARTUP_BACKFILL_LOG_LIMIT).await?;
    Ok(())
}

async fn disable_ssh_security(state: &AppState, runtime: Option<&Value>) -> anyhow::Result<()> {
    let payload = json!({
        "chain_name": SSH_FIREWALL_CHAIN,
        "parent_chain": ["INPUT", "DOCKER-USER"]
    });
    if let Err(error) = state.go_backend.clear_ssh_firewall(&payload).await {
        tracing::debug!(%error, "failed to clear disabled SSH firewall policy");
    }
    for record in active_blocks(state).await? {
        if let Some(ip) = record.get("ip").and_then(Value::as_str)
            && let Err(error) = mark_block_removed(state, ip, "disabled").await
        {
            tracing::warn!(%error, ip, "failed to mark SSH block disabled");
        }
    }
    if let Some(runtime) = runtime {
        let next = json!({
            "enabled": false,
            "allowed_cidrs": [],
            "updated_at": time_utils::now_iso(),
        });
        if runtime != &next {
            state.redis.set_json_value(RUNTIME_KEY, &next).await?;
        }
    }
    Ok(())
}

async fn sync_firewall_blocks_now(
    state: &AppState,
    translator: &Translator,
) -> anyhow::Result<Value> {
    let availability = ssh_security_availability(state, translator);
    if !availability.available {
        anyhow::bail!(availability.reason);
    }
    reconcile_expired_blocks(state).await?;
    let active = active_blocks(state).await?;
    let policy =
        sync_firewall_policy(state, None, Some(active.clone()), Vec::new(), translator).await?;
    let ports = policy.ports;
    let mut synced = 0usize;
    for record in active {
        let mut next = record.as_object().cloned().unwrap_or_default();
        next.insert("ports".to_string(), json!(ports));
        next.insert("applied".to_string(), Value::Bool(true));
        next.insert("removed_at".to_string(), Value::Null);
        next.insert("remove_reason".to_string(), Value::Null);
        save_block(state, &Value::Object(next)).await?;
        synced += 1;
    }
    Ok(json!({
        "cleared": synced,
        "synced": synced,
        "active_blocks": policy.blocked_ips,
        "allowed_cidrs": policy.allowed_cidrs,
        "ports": ports
    }))
}

async fn reconcile_expired_blocks(state: &AppState) -> redis::RedisResult<()> {
    for record in expired_active_blocks(state).await? {
        if let Some(ip) = record.get("ip").and_then(Value::as_str) {
            let _ = mark_block_removed(state, ip, "expired").await?;
        }
    }
    Ok(())
}

async fn expired_active_blocks(state: &AppState) -> redis::RedisResult<Vec<Value>> {
    let keys = state.redis.scan_keys(BLOCK_DATA_PREFIX, 100).await?;
    let mut records = Vec::new();
    let now = time_utils::now_ms();
    for key in keys {
        if let Some(record) = state
            .redis
            .get_json_value(&key)
            .await?
            .and_then(normalize_block_record)
            && record
                .get("applied")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            && iso_score(record.get("expires_at").and_then(Value::as_str)) <= now
        {
            records.push(record);
        }
    }
    Ok(records)
}

async fn sync_firewall_policy(
    state: &AppState,
    runtime: Option<&Value>,
    active_records: Option<Vec<Value>>,
    extra_blocked_ips: Vec<String>,
    translator: &Translator,
) -> anyhow::Result<FirewallPolicyResult> {
    let loaded_runtime;
    let runtime = match runtime {
        Some(runtime) => runtime,
        None => {
            loaded_runtime = load_runtime(state).await?;
            &loaded_runtime
        }
    };
    let active = match active_records {
        Some(records) => records,
        None => active_blocks(state).await?,
    };
    let mut blocked_ips = active
        .iter()
        .filter_map(|record| record.get("ip").and_then(Value::as_str))
        .map(str::to_string)
        .collect::<Vec<_>>();
    blocked_ips.extend(extra_blocked_ips);
    blocked_ips = normalize_ip_strings(blocked_ips);

    let allowed_cidrs = if runtime.get("enabled").and_then(Value::as_bool) == Some(true) {
        runtime
            .get("allowed_cidrs")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let allowed_cidrs = normalize_cidr_strings(allowed_cidrs);
    let ports = resolve_ssh_ports();
    if allowed_cidrs.is_empty() && blocked_ips.is_empty() {
        let payload = json!({
            "chain_name": SSH_FIREWALL_CHAIN,
            "parent_chain": ["INPUT", "DOCKER-USER"]
        });
        let value = state.go_backend.clear_ssh_firewall(&payload).await?;
        ensure_go_success(value, translator, "clearSshPolicyFailed")?;
        return Ok(FirewallPolicyResult {
            allowed_cidrs: 0,
            blocked_ips: 0,
            ports,
        });
    }

    let allowed_count = allowed_cidrs.len();
    let blocked_count = blocked_ips.len();
    let payload = json!({
        "chain_name": SSH_FIREWALL_CHAIN,
        "parent_chain": ["INPUT", "DOCKER-USER"],
        "ports": ports.clone(),
        "allowed_cidrs": allowed_cidrs,
        "blocked_ips": blocked_ips,
        "include_local_cidrs": true
    });
    let value = state.go_backend.sync_ssh_firewall(&payload).await?;
    ensure_go_success(value, translator, "syncSshPolicyFailed")?;
    Ok(FirewallPolicyResult {
        allowed_cidrs: allowed_count,
        blocked_ips: blocked_count,
        ports,
    })
}

async fn process_recent_ssh_entries(
    state: &AppState,
    config: &Value,
    limit: usize,
) -> anyhow::Result<()> {
    let window_ms = config
        .get("window_minutes")
        .and_then(Value::as_i64)
        .unwrap_or(10)
        .max(1)
        * 60
        * 1000;
    let cutoff = time_utils::now_ms() - window_ms;
    let mut entries = query_recent_ssh_logs(limit)
        .into_iter()
        .filter(|entry| iso_score(entry.get("happened_at").and_then(Value::as_str)) >= cutoff)
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        iso_score(left.get("happened_at").and_then(Value::as_str))
            .cmp(&iso_score(right.get("happened_at").and_then(Value::as_str)))
    });
    for entry in entries {
        if let Err(error) = handle_ssh_entry(state, config, &entry).await {
            tracing::warn!(%error, entry = %entry, "failed to handle SSH log entry");
        }
    }
    Ok(())
}

async fn handle_ssh_entry(state: &AppState, config: &Value, entry: &Value) -> anyhow::Result<()> {
    let id = entry.get("id").and_then(Value::as_str).unwrap_or("");
    if id.is_empty() || is_processed(state, id).await? {
        return Ok(());
    }
    if config.get("enabled").and_then(Value::as_bool) != Some(true) {
        mark_processed(state, id).await?;
        return Ok(());
    }
    let ip = normalize_ip(entry.get("ip").and_then(Value::as_str).unwrap_or(""));
    if ip.is_empty() || is_private_or_local_ip(&ip) {
        mark_processed(state, id).await?;
        return Ok(());
    }
    let ip_location = ip_location::register_usage(state, &ip, vec![format!("ssh-login-log|{id}")])
        .await
        .unwrap_or_default();
    let mut entry = entry.clone();
    if !ip_location.trim().is_empty()
        && let Some(object) = entry.as_object_mut()
    {
        object.insert("ipLocation".to_string(), Value::String(ip_location));
    }

    match entry.get("outcome").and_then(Value::as_str) {
        Some("failure") => handle_ssh_failure(state, config, &entry, id, &ip).await?,
        Some("success") => handle_ssh_success(state, config, &entry, id, &ip).await?,
        _ => {}
    }
    mark_processed(state, id).await?;
    Ok(())
}

async fn handle_ssh_failure(
    state: &AppState,
    config: &Value,
    entry: &Value,
    id: &str,
    ip: &str,
) -> anyhow::Result<()> {
    let window_minutes = config
        .get("window_minutes")
        .and_then(Value::as_i64)
        .unwrap_or(10)
        .max(1);
    let threshold = config
        .get("failed_login_threshold")
        .and_then(Value::as_i64)
        .unwrap_or(5)
        .max(1);
    let attempts = add_failure(state, ip, id, entry, window_minutes).await?;
    let mut event_payload = json!({
        "ip": ip,
        "username": entry.get("username").cloned().unwrap_or_else(|| json!("-")),
        "invalid_user": entry.get("invalid_user").and_then(Value::as_bool).unwrap_or(false),
        "auth_method": entry.get("auth_method").cloned().unwrap_or(Value::Null),
        "port": entry.get("port").cloned().unwrap_or(Value::Null),
        "attempts": attempts,
        "window_minutes": window_minutes,
        "threshold": threshold,
        "log_time": entry.get("happened_at").cloned().unwrap_or_else(|| json!(time_utils::now_iso()))
    });
    if let Some(location) = entry
        .get("ipLocation")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        && let Some(object) = event_payload.as_object_mut()
    {
        object.insert(
            "ip_location".to_string(),
            Value::String(location.to_string()),
        );
    }
    if let Err(error) = system_events::publish_ssh_login_failure_event(state, event_payload).await {
        tracing::debug!(%error, "failed to publish SSH login failure event");
    }
    if attempts < threshold || is_active_blocked(state, ip).await? {
        return Ok(());
    }
    create_ssh_block(state, config, entry, "failed_login_threshold", attempts).await
}

async fn handle_ssh_success(
    state: &AppState,
    config: &Value,
    entry: &Value,
    id: &str,
    ip: &str,
) -> anyhow::Result<()> {
    clear_failures(state, ip).await?;
    let mut event_payload = json!({
        "ip": ip,
        "username": entry.get("username").cloned().unwrap_or_else(|| json!("-")),
        "auth_method": entry.get("auth_method").cloned().unwrap_or(Value::Null),
        "port": entry.get("port").cloned().unwrap_or(Value::Null),
        "log_time": entry.get("happened_at").cloned().unwrap_or_else(|| json!(time_utils::now_iso()))
    });
    if let Some(location) = entry
        .get("ipLocation")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        && let Some(object) = event_payload.as_object_mut()
    {
        object.insert(
            "ip_location".to_string(),
            Value::String(location.to_string()),
        );
    }
    if let Err(error) = system_events::publish_ssh_login_success_event(state, event_payload).await {
        tracing::debug!(%error, "failed to publish SSH login success event");
    }
    let runtime = load_runtime(state).await?;
    if ip_allowed_by_runtime(&runtime, ip) || is_active_blocked(state, ip).await? {
        return Ok(());
    }
    create_ssh_block(state, config, entry, "cidr_not_allowed", 0).await?;
    mark_processed(state, id).await?;
    Ok(())
}

async fn create_ssh_block(
    state: &AppState,
    config: &Value,
    entry: &Value,
    reason: &str,
    failed_count: i64,
) -> anyhow::Result<()> {
    let ip = normalize_ip(entry.get("ip").and_then(Value::as_str).unwrap_or(""));
    if ip.is_empty() {
        return Ok(());
    }
    let block_seconds = ssh_block_duration_seconds(config);
    let blocked_at = time_utils::now_iso();
    let expires_at = millis_to_iso(time_utils::now_ms() + block_seconds * 1000);
    let translator = Translator::from_state(state).await;
    let ip_location = if let Some(location) = entry
        .get("ipLocation")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        location.to_string()
    } else {
        ip_location::register_usage(state, &ip, vec![format!("ssh-blocklist|{ip}")])
            .await
            .unwrap_or_default()
    };
    let policy = sync_firewall_policy(state, None, None, vec![ip.clone()], &translator).await?;
    let mut record = json!({
        "ip": ip,
        "ports": policy.ports,
        "blocked_at": blocked_at,
        "expires_at": expires_at,
        "reason": reason,
        "failed_count": failed_count,
        "window_minutes": config.get("window_minutes").and_then(Value::as_i64).unwrap_or(10),
        "threshold": config.get("failed_login_threshold").and_then(Value::as_i64).unwrap_or(5),
        "sample_user": entry.get("username").cloned().unwrap_or_else(|| json!("-")),
        "sample_auth_method": entry.get("auth_method").cloned().unwrap_or(Value::Null),
        "sample_log_time": entry.get("happened_at").cloned().unwrap_or_else(|| json!(time_utils::now_iso())),
        "applied": true,
        "removed_at": Value::Null,
        "remove_reason": Value::Null,
    });
    if !ip_location.trim().is_empty()
        && let Some(object) = record.as_object_mut()
    {
        object.insert("ipLocation".to_string(), Value::String(ip_location.clone()));
    }
    save_block(state, &record).await?;
    let mut payload = json!({
        "ip": record.get("ip").cloned().unwrap_or(Value::Null),
        "blocked_at": record.get("blocked_at").cloned().unwrap_or(Value::Null),
        "blocked_until": record.get("expires_at").cloned().unwrap_or(Value::Null),
        "block_seconds": block_seconds,
        "reason": reason,
        "failed_count": failed_count,
        "window_minutes": record.get("window_minutes").cloned().unwrap_or(Value::Null),
        "threshold": record.get("threshold").cloned().unwrap_or(Value::Null),
        "username": record.get("sample_user").cloned().unwrap_or(Value::Null),
    });
    if !ip_location.trim().is_empty()
        && let Some(object) = payload.as_object_mut()
    {
        object.insert("ip_location".to_string(), Value::String(ip_location));
    }
    if let Err(error) = system_events::publish_ssh_ip_blocked_event(state, payload).await {
        tracing::debug!(%error, "failed to publish SSH block event");
    }
    Ok(())
}

async fn is_processed(state: &AppState, id: &str) -> redis::RedisResult<bool> {
    Ok(state
        .redis
        .get_string_value(&format!("{PROCESSED_PREFIX}{id}"))
        .await?
        .is_some())
}

async fn mark_processed(state: &AppState, id: &str) -> redis::RedisResult<()> {
    state
        .redis
        .set_string_value_with_optional_ttl(
            &format!("{PROCESSED_PREFIX}{id}"),
            "1",
            Some(PROCESSED_TTL_SECONDS),
        )
        .await
}

async fn add_failure(
    state: &AppState,
    ip: &str,
    id: &str,
    entry: &Value,
    window_minutes: i64,
) -> redis::RedisResult<i64> {
    let score = iso_score(entry.get("happened_at").and_then(Value::as_str));
    let score = if score > 0 {
        score
    } else {
        time_utils::now_ms()
    };
    let window_ms = window_minutes.max(1) * 60 * 1000;
    state
        .redis
        .zadd_trim_count_expire(
            &format!("{FAILURES_PREFIX}{ip}"),
            id,
            score,
            score - window_ms,
            ((window_ms / 1000) + 3600) as usize,
        )
        .await
}

async fn clear_failures(state: &AppState, ip: &str) -> redis::RedisResult<()> {
    state
        .redis
        .delete_key(&format!("{FAILURES_PREFIX}{ip}"))
        .await
}

async fn is_active_blocked(state: &AppState, ip: &str) -> redis::RedisResult<bool> {
    Ok(load_block(state, ip)
        .await?
        .is_some_and(|record| is_active_block(&record, time_utils::now_ms())))
}

fn ip_allowed_by_runtime(runtime: &Value, ip: &str) -> bool {
    let cidrs = runtime
        .get("allowed_cidrs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter_map(|value| value.parse::<IpNet>().ok())
        .collect::<Vec<_>>();
    if cidrs.is_empty() {
        return true;
    }
    let Ok(ip) = ip.parse::<IpAddr>() else {
        return true;
    };
    cidrs.iter().any(|cidr| cidr.contains(&ip))
}

fn ssh_block_duration_seconds(config: &Value) -> i64 {
    let value = config
        .get("block_duration_value")
        .and_then(Value::as_i64)
        .unwrap_or(1)
        .clamp(1, 365);
    match config
        .get("block_duration_unit")
        .and_then(Value::as_str)
        .unwrap_or("day")
    {
        "minute" => value * 60,
        "hour" => value * 3600,
        _ => value * 24 * 3600,
    }
}

fn normalize_ip_strings(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = HashSet::new();
    values
        .into_iter()
        .filter_map(|value| {
            let ip = normalize_ip(&value);
            (!ip.is_empty() && seen.insert(ip.clone())).then_some(ip)
        })
        .collect()
}

fn coalesce_success_login_logs(entries: Vec<Value>) -> Vec<Value> {
    let mut result = Vec::new();
    let mut latest_by_key: HashMap<String, usize> = HashMap::new();

    for entry in entries {
        if entry.get("outcome").and_then(Value::as_str) != Some("success") {
            result.push(entry);
            continue;
        }

        let key = success_coalesce_key(&entry);
        let existing_index = latest_by_key.get(&key).copied();
        let should_start_new = existing_index.is_none_or(|index| {
            (entry_time_ms(&result[index]) - entry_time_ms(&entry)).abs()
                > SUCCESS_LOG_COALESCE_WINDOW_MS
        });

        if should_start_new {
            let mut next = entry;
            let repeat_count = positive_i64_from_value(next.get("repeat_count")).unwrap_or(1);
            let related_ports = entry_ports(&next);
            if let Some(object) = next.as_object_mut() {
                object.insert("repeat_count".to_string(), json!(repeat_count.max(1)));
                object.insert("related_ports".to_string(), json!(related_ports));
            }
            result.push(next);
            latest_by_key.insert(key, result.len() - 1);
            continue;
        }

        if let Some(index) = existing_index {
            let incoming_repeat = positive_i64_from_value(entry.get("repeat_count"))
                .unwrap_or(1)
                .max(1);
            let incoming_ports = entry_ports(&entry);
            let incoming_raw = entry
                .get("raw")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            if let Some(existing) = result.get_mut(index)
                && let Some(object) = existing.as_object_mut()
            {
                let repeat = positive_i64_from_value(object.get("repeat_count"))
                    .unwrap_or(1)
                    .max(1)
                    + incoming_repeat;
                object.insert("repeat_count".to_string(), json!(repeat));
                let mut merged_ports = object
                    .get("related_ports")
                    .and_then(Value::as_array)
                    .map(|values| merge_port_values(values.iter()))
                    .unwrap_or_default();
                merged_ports.extend(incoming_ports);
                merged_ports.sort_unstable();
                merged_ports.dedup();
                object.insert("related_ports".to_string(), json!(merged_ports));
                if !incoming_raw.is_empty() {
                    let existing_raw = object
                        .get("raw")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    if !existing_raw.contains(&incoming_raw) {
                        object.insert(
                            "raw".to_string(),
                            Value::String(if existing_raw.is_empty() {
                                incoming_raw
                            } else {
                                format!("{existing_raw}\n{incoming_raw}")
                            }),
                        );
                    }
                }
            }
        }
    }

    result
}

fn success_coalesce_key(entry: &Value) -> String {
    [
        entry.get("source").and_then(Value::as_str).unwrap_or(""),
        entry.get("outcome").and_then(Value::as_str).unwrap_or(""),
        entry.get("username").and_then(Value::as_str).unwrap_or(""),
        entry.get("ip").and_then(Value::as_str).unwrap_or(""),
        entry
            .get("auth_method")
            .and_then(Value::as_str)
            .unwrap_or(""),
    ]
    .join("|")
}

fn entry_time_ms(entry: &Value) -> i64 {
    iso_score(entry.get("happened_at").and_then(Value::as_str))
}

fn entry_ports(entry: &Value) -> Vec<i64> {
    let related = entry
        .get("related_ports")
        .and_then(Value::as_array)
        .into_iter()
        .flatten();
    let port = entry.get("port").into_iter();
    merge_port_values(related.chain(port))
}

fn merge_port_values<'a>(values: impl IntoIterator<Item = &'a Value>) -> Vec<i64> {
    let mut ports = values
        .into_iter()
        .filter_map(parse_i64_from_json_like_node)
        .filter(|port| *port > 0 && *port <= 65535)
        .collect::<Vec<_>>();
    ports.sort_unstable();
    ports.dedup();
    ports
}

async fn hydrate_ip_location_records<F>(state: &AppState, items: &mut [Value], mut reference: F)
where
    F: FnMut(&Value) -> Option<String>,
{
    for item in items {
        let ip = item
            .get("ip")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if ip.is_empty() {
            continue;
        }
        let refs = reference(item).into_iter().collect::<Vec<_>>();
        match ip_location::register_usage(state, &ip, refs).await {
            Ok(location) if !location.trim().is_empty() => {
                if let Some(object) = item.as_object_mut() {
                    object.insert("ipLocation".to_string(), Value::String(location));
                }
            }
            Ok(_) => {}
            Err(error) => tracing::debug!(%error, ip, "failed to hydrate SSH IP location"),
        }
    }
}

fn ensure_go_success(
    value: Value,
    translator: &Translator,
    fallback_key: &str,
) -> anyhow::Result<()> {
    if value
        .get("success")
        .and_then(Value::as_bool)
        .unwrap_or(true)
    {
        return Ok(());
    }
    anyhow::bail!(
        "{}",
        value
            .get("message")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| ssh_security_text(translator, fallback_key))
    )
}

fn ssh_security_availability(state: &AppState, translator: &Translator) -> SshAvailability {
    let log_source = detect_log_source();
    let target = runtime_profile::deployment_target(state);
    if target == "openwrt" {
        return SshAvailability {
            available: false,
            reason: ssh_security_text(translator, "openWrtUnsupported"),
            log_source,
        };
    }
    if !host_firewall_available(state) {
        let profile = runtime_profile::get_runtime_profile(state);
        return SshAvailability {
            available: false,
            reason: runtime_profile::capability_unavailable_message(
                "host_firewall_available",
                &profile,
                translator,
            ),
            log_source,
        };
    }
    if log_source == "unavailable" {
        return SshAvailability {
            available: false,
            reason: ssh_security_text(translator, "logSourceUnavailable"),
            log_source,
        };
    }
    SshAvailability {
        available: true,
        reason: String::new(),
        log_source,
    }
}

fn host_firewall_available(state: &AppState) -> bool {
    runtime_profile::host_firewall_available(state)
}

async fn load_block(state: &AppState, ip: &str) -> redis::RedisResult<Option<Value>> {
    let normalized = normalize_ip(ip);
    if normalized.is_empty() {
        return Ok(None);
    }
    Ok(state
        .redis
        .get_json_value(&format!("{BLOCK_DATA_PREFIX}{normalized}"))
        .await?
        .and_then(normalize_block_record))
}

async fn save_block(state: &AppState, record: &Value) -> redis::RedisResult<()> {
    let Some(record) = normalize_block_record(record.clone()) else {
        return Ok(());
    };
    let ip = record.get("ip").and_then(Value::as_str).unwrap_or_default();
    let ttl = block_ttl_seconds(&record);
    state
        .redis
        .set_json_value_ex(&format!("{BLOCK_DATA_PREFIX}{ip}"), &record, ttl)
        .await?;
    if record
        .get("applied")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        let score = iso_score(record.get("expires_at").and_then(Value::as_str));
        state
            .redis
            .zadd_string_member(BLOCKS_INDEX_KEY, ip, score)
            .await?;
    } else {
        state.redis.zrem_string_member(BLOCKS_INDEX_KEY, ip).await?;
    }
    Ok(())
}

async fn active_blocks(state: &AppState) -> redis::RedisResult<Vec<Value>> {
    let keys = state.redis.scan_keys(BLOCK_DATA_PREFIX, 100).await?;
    let mut records = Vec::new();
    let now = time_utils::now_ms();
    for key in keys {
        if let Some(record) = state
            .redis
            .get_json_value(&key)
            .await?
            .and_then(normalize_block_record)
        {
            if is_active_block(&record, now) {
                records.push(record);
            } else if let Some(ip) = record.get("ip").and_then(Value::as_str) {
                state.redis.zrem_string_member(BLOCKS_INDEX_KEY, ip).await?;
            }
        }
    }
    records.sort_by(|left, right| {
        iso_score(right.get("blocked_at").and_then(Value::as_str))
            .cmp(&iso_score(left.get("blocked_at").and_then(Value::as_str)))
    });
    Ok(records)
}

async fn list_active_blocks(
    state: &AppState,
    page: i64,
    limit: i64,
    search: &str,
) -> redis::RedisResult<(Vec<Value>, usize)> {
    let mut records = active_blocks(state).await?;
    if !search.is_empty() {
        records.retain(|record| {
            ["ip", "ipLocation", "sample_user"].iter().any(|key| {
                record
                    .get(*key)
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_ascii_lowercase()
                    .contains(search)
            })
        });
    }
    let total = records.len();
    let start = ((page - 1) * limit) as usize;
    Ok((
        records
            .into_iter()
            .skip(start)
            .take(limit as usize)
            .collect(),
        total,
    ))
}

async fn remove_block(
    state: &AppState,
    ip: &str,
    reason: &str,
    translator: &Translator,
) -> anyhow::Result<bool> {
    let Some(record) = load_block(state, ip).await? else {
        return Ok(false);
    };
    let record_ip = record
        .get("ip")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if record_ip.is_empty() {
        return Ok(false);
    }
    if record
        .get("applied")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        let active = active_blocks(state)
            .await?
            .into_iter()
            .filter(|item| item.get("ip").and_then(Value::as_str) != Some(record_ip.as_str()))
            .collect::<Vec<_>>();
        let _ = sync_firewall_policy(state, None, Some(active), Vec::new(), translator).await?;
    }
    let removed = mark_block_removed(state, &record_ip, reason).await?;
    if removed && reason == "manual" {
        clear_failures(state, &record_ip).await?;
    }
    Ok(removed)
}

async fn mark_block_removed(state: &AppState, ip: &str, reason: &str) -> redis::RedisResult<bool> {
    let Some(record) = load_block(state, ip).await? else {
        return Ok(false);
    };
    let mut next = record.as_object().cloned().unwrap_or_default();
    next.insert("applied".to_string(), Value::Bool(false));
    next.insert(
        "removed_at".to_string(),
        Value::String(time_utils::now_iso()),
    );
    next.insert(
        "remove_reason".to_string(),
        Value::String(reason.to_string()),
    );
    save_block(state, &Value::Object(next)).await?;
    Ok(true)
}

fn normalize_block_record(value: Value) -> Option<Value> {
    let raw = value.as_object()?;
    let ip = normalize_ip(raw.get("ip")?.as_str()?);
    if ip.is_empty() {
        return None;
    }
    let blocked_at = normalize_timestamp(raw.get("blocked_at"))?
        .as_str()?
        .to_string();
    let expires_at = normalize_timestamp(raw.get("expires_at"))?
        .as_str()?
        .to_string();
    let reason = match raw.get("reason").and_then(Value::as_str) {
        Some("cidr_not_allowed") => "cidr_not_allowed",
        _ => "failed_login_threshold",
    };
    let ports = raw
        .get("ports")
        .and_then(Value::as_array)
        .map(|items| merge_port_values(items.iter()))
        .unwrap_or_default();
    let mut record = Map::new();
    record.insert("ip".to_string(), Value::String(ip));
    if let Some(location) = raw
        .get("ipLocation")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        record.insert(
            "ipLocation".to_string(),
            Value::String(location.to_string()),
        );
    }
    if !ports.is_empty() {
        record.insert("ports".to_string(), json!(ports));
    }
    record.insert("blocked_at".to_string(), Value::String(blocked_at));
    record.insert("expires_at".to_string(), Value::String(expires_at));
    record.insert("reason".to_string(), Value::String(reason.to_string()));
    record.insert(
        "failed_count".to_string(),
        json!(
            raw.get("failed_count")
                .and_then(Value::as_i64)
                .unwrap_or_default()
                .max(0)
        ),
    );
    record.insert(
        "window_minutes".to_string(),
        json!(
            raw.get("window_minutes")
                .and_then(Value::as_i64)
                .unwrap_or_default()
                .max(0)
        ),
    );
    record.insert(
        "threshold".to_string(),
        json!(
            raw.get("threshold")
                .and_then(Value::as_i64)
                .unwrap_or_default()
                .max(0)
        ),
    );
    for key in ["sample_user", "sample_auth_method", "sample_log_time"] {
        if let Some(value) = raw
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            record.insert(key.to_string(), Value::String(value.to_string()));
        }
    }
    record.insert(
        "applied".to_string(),
        Value::Bool(raw.get("applied").and_then(Value::as_bool).unwrap_or(false)),
    );
    record.insert(
        "removed_at".to_string(),
        normalize_timestamp(raw.get("removed_at")).unwrap_or(Value::Null),
    );
    record.insert(
        "remove_reason".to_string(),
        match raw.get("remove_reason").and_then(Value::as_str) {
            Some("manual" | "expired" | "disabled") => Value::String(
                raw.get("remove_reason")
                    .and_then(Value::as_str)
                    .unwrap()
                    .to_string(),
            ),
            _ => Value::Null,
        },
    );
    Some(Value::Object(record))
}

fn is_active_block(record: &Value, now_ms: i64) -> bool {
    record
        .get("applied")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && iso_score(record.get("expires_at").and_then(Value::as_str)) > now_ms
}

fn block_ttl_seconds(record: &Value) -> usize {
    if !record
        .get("applied")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return 90 * 24 * 3600;
    }
    let expires_at = iso_score(record.get("expires_at").and_then(Value::as_str));
    let seconds_until_expiry = ((expires_at - time_utils::now_ms()).max(0) + 999) / 1000;
    (seconds_until_expiry + 90 * 24 * 3600).clamp(90 * 24 * 3600, (365 + 90) * 24 * 3600) as usize
}

fn query_recent_ssh_logs(limit: usize) -> Vec<Value> {
    if command_available("journalctl") {
        let output = Command::new("journalctl")
            .args([
                "-u",
                "ssh",
                "-u",
                "sshd",
                "-n",
                &limit.to_string(),
                "-o",
                "json",
            ])
            .output();
        if let Ok(output) = output
            && output.status.success()
        {
            let text = String::from_utf8_lossy(&output.stdout);
            let entries = text
                .lines()
                .filter_map(parse_journal_line)
                .collect::<Vec<_>>();
            if !entries.is_empty() {
                return entries;
            }
        }
    }
    query_auth_log(limit)
}

fn parse_journal_line(line: &str) -> Option<Value> {
    let value = serde_json::from_str::<Value>(line).ok()?;
    let message = value.get("MESSAGE").and_then(Value::as_str)?;
    let micros = value
        .get("__REALTIME_TIMESTAMP")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or_default();
    let happened_at = if micros > 0 {
        millis_to_iso(micros / 1000)
    } else {
        time_utils::now_iso()
    };
    parse_ssh_message(message, &happened_at, "journal")
}

fn query_auth_log(limit: usize) -> Vec<Value> {
    let mut entries = Vec::new();
    for path in AUTH_LOG_CANDIDATES {
        let Ok(lines) = read_log_lines(path) else {
            continue;
        };
        for line in lines.into_iter().rev() {
            if entries.len() >= limit {
                break;
            }
            if !line.to_ascii_lowercase().contains("sshd") {
                continue;
            }
            if let Some((happened_at, message)) = parse_syslog_line(&line)
                && let Some(entry) = parse_ssh_message(&message, &happened_at, "auth.log")
            {
                entries.push(entry);
            }
        }
        if !entries.is_empty() {
            break;
        }
    }
    entries
}

fn read_log_lines(path: &str) -> std::io::Result<Vec<String>> {
    let file = fs::File::open(path)?;
    let reader: Box<dyn Read> = if path.ends_with(".gz") {
        Box::new(GzDecoder::new(file))
    } else {
        Box::new(file)
    };
    let mut lines = BufReader::new(reader)
        .lines()
        .map_while(Result::ok)
        .collect::<Vec<_>>();
    if lines.len() > 5000 {
        lines = lines.split_off(lines.len() - 5000);
    }
    Ok(lines)
}

fn parse_ssh_message(message: &str, happened_at: &str, source: &str) -> Option<Value> {
    let message = message.trim();
    if message.is_empty() {
        return None;
    }
    let lower = message.to_ascii_lowercase();
    let (outcome, invalid_user, marker) = if lower.contains("accepted ") {
        ("success", false, " for ")
    } else if lower.contains("failed ") && lower.contains(" for invalid user ") {
        ("failure", true, " for invalid user ")
    } else if lower.contains("failed ") {
        ("failure", false, " for ")
    } else {
        return None;
    };
    let ip = extract_between(message, " from ", " port ").and_then(|value| {
        let ip = normalize_ip(value);
        if ip.is_empty() { None } else { Some(ip) }
    })?;
    let port = extract_after(message, " port ")
        .and_then(|value| value.split_whitespace().next())
        .and_then(|value| value.parse::<i64>().ok())
        .filter(|port| *port > 0 && *port <= 65535);
    let auth_method = message
        .split_whitespace()
        .nth(1)
        .map(str::to_string)
        .filter(|value| !value.is_empty());
    let username = extract_between(message, marker, " from ")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("-");
    let id = fingerprint(&[source, happened_at, outcome, username, &ip, message].join("|"));
    let mut entry = json!({
        "id": id,
        "happened_at": happened_at,
        "outcome": outcome,
        "username": username,
        "invalid_user": invalid_user,
        "ip": ip,
        "service": "sshd",
        "source": source,
        "raw": message
    });
    if let Some(port) = port {
        entry["port"] = json!(port);
    }
    if let Some(auth_method) = auth_method {
        entry["auth_method"] = json!(auth_method);
    }
    Some(entry)
}

fn parse_syslog_line(line: &str) -> Option<(String, String)> {
    let mut parts = line.split_whitespace();
    let month = parts.next()?;
    let day = parts.next()?.parse::<u8>().ok()?;
    let time_text = parts.next()?;
    let _host = parts.next()?;
    let message = parts.collect::<Vec<_>>().join(" ");
    let month = match month {
        "Jan" => 1,
        "Feb" => 2,
        "Mar" => 3,
        "Apr" => 4,
        "May" => 5,
        "Jun" => 6,
        "Jul" => 7,
        "Aug" => 8,
        "Sep" => 9,
        "Oct" => 10,
        "Nov" => 11,
        "Dec" => 12,
        _ => return None,
    };
    let mut t = time_text.split(':');
    let hour = t.next()?.parse::<u8>().ok()?;
    let minute = t.next()?.parse::<u8>().ok()?;
    let second = t.next()?.parse::<u8>().ok()?;
    let now = time::OffsetDateTime::now_utc();
    let date =
        time::Date::from_calendar_date(now.year(), time::Month::try_from(month).ok()?, day).ok()?;
    let time_value = time::Time::from_hms(hour, minute, second).ok()?;
    let mut happened_at = date.with_time(time_value).assume_utc();
    if happened_at > now + time::Duration::days(1) {
        happened_at = happened_at.replace_year(now.year() - 1).ok()?;
    }
    Some((
        happened_at
            .format(&time::format_description::well_known::Rfc3339)
            .ok()?,
        message,
    ))
}

fn detect_log_source() -> &'static str {
    if command_available("journalctl") {
        return "journal";
    }
    if AUTH_LOG_CANDIDATES
        .iter()
        .any(|path| Path::new(path).exists())
    {
        return "auth.log";
    }
    "unavailable"
}

fn command_available(command: &str) -> bool {
    Command::new(command)
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn resolve_ssh_ports() -> Vec<i64> {
    let path =
        std::env::var("SSHD_CONFIG_PATH").unwrap_or_else(|_| "/etc/ssh/sshd_config".to_string());
    let Ok(content) = fs::read_to_string(path) else {
        return vec![22];
    };
    let mut ports = content
        .lines()
        .filter_map(|line| {
            let line = line.split('#').next()?.trim();
            let mut parts = line.split_whitespace();
            if parts.next()?.eq_ignore_ascii_case("port") {
                parts.next()?.parse::<i64>().ok()
            } else {
                None
            }
        })
        .filter(|port| *port > 0 && *port <= 65535)
        .collect::<Vec<_>>();
    ports.sort_unstable();
    ports.dedup();
    if ports.is_empty() { vec![22] } else { ports }
}

fn normalize_allowed_regions(value: Option<&Value>) -> Value {
    let Some(items) = value.and_then(Value::as_array) else {
        return json!([]);
    };
    let mut seen = HashSet::new();
    let regions = items
        .iter()
        .filter_map(|item| {
            let province = item.get("province")?.as_str()?.trim();
            if province.is_empty() {
                return None;
            }
            let query_city = item
                .get("query_city")
                .or_else(|| item.get("queryCity"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty());
            let key = format!("{province}::{}", query_city.unwrap_or(""));
            if !seen.insert(key) {
                return None;
            }
            Some(json!({
                "province": province,
                "city": item.get("city").and_then(Value::as_str).unwrap_or(""),
                "label": item.get("label").and_then(Value::as_str).unwrap_or(province),
                "value": item.get("value").and_then(Value::as_str).unwrap_or(province),
                "query_city": query_city,
                "is_province_wide": item.get("is_province_wide").and_then(Value::as_bool).unwrap_or(query_city.is_none()),
                "is_municipality": item.get("is_municipality").and_then(Value::as_bool).unwrap_or(false)
            }))
        })
        .collect::<Vec<_>>();
    Value::Array(regions)
}

fn normalize_cidrs(value: Option<&Value>) -> Value {
    let Some(items) = value.and_then(Value::as_array) else {
        return json!([]);
    };
    Value::Array(
        normalize_cidr_strings(items.iter().filter_map(Value::as_str).map(str::to_string))
            .into_iter()
            .map(Value::String)
            .collect(),
    )
}

fn normalize_cidr_strings(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = HashSet::new();
    values
        .into_iter()
        .filter_map(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return None;
            }
            trimmed.parse::<IpNet>().ok()
        })
        .map(|cidr| cidr.to_string())
        .filter(|cidr| seen.insert(cidr.clone()))
        .collect()
}

fn validate_cidrs(value: Option<&Value>, translator: &Translator) -> Result<(), SshError> {
    let Some(value) = value else {
        return Ok(());
    };
    let Some(items) = value.as_array() else {
        return Err(SshError::BadRequest(ssh_security_text(
            translator,
            "customCidrsMustBeArray",
        )));
    };
    let invalid = items
        .iter()
        .filter_map(|item| match item.as_str() {
            Some(value) if value.trim().is_empty() => None,
            Some(value) if value.trim().parse::<IpNet>().is_err() => Some(value.trim()),
            Some(_) => None,
            None => Some("<non-string>"),
        })
        .collect::<Vec<_>>();
    if invalid.is_empty() {
        Ok(())
    } else {
        Err(SshError::BadRequest(ssh_security_text_params(
            translator,
            "customCidrInvalid",
            &[("cidrs", invalid.join(", "))],
        )))
    }
}

fn normalize_duration_unit(value: Option<&str>) -> String {
    match value {
        Some("minute" | "hour" | "day") => value.unwrap().to_string(),
        _ => "day".to_string(),
    }
}

fn int_field(raw: &Map<String, Value>, key: &str, fallback: i64, min: i64, max: i64) -> i64 {
    raw.get(key)
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_str().and_then(|value| value.parse::<i64>().ok()))
        })
        .unwrap_or(fallback)
        .clamp(min, max)
}

fn int_field_or_previous(
    raw: &Map<String, Value>,
    previous: &Value,
    key: &str,
    fallback: i64,
    min: i64,
    max: i64,
) -> i64 {
    raw.get(key)
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_str().and_then(|value| value.parse::<i64>().ok()))
        })
        .or_else(|| previous.get(key).and_then(Value::as_i64))
        .unwrap_or(fallback)
        .clamp(min, max)
}

fn normalize_timestamp(value: Option<&Value>) -> Option<Value> {
    let value = value.and_then(Value::as_str)?;
    time_utils::parse_iso_ms(value).map(|_| Value::String(value.to_string()))
}

fn iso_score(value: Option<&str>) -> i64 {
    value.and_then(time_utils::parse_iso_ms).unwrap_or_default()
}

fn parse_positive(value: Option<&str>, fallback: i64, max: i64) -> i64 {
    value
        .and_then(|value| parse_i64_prefix(value.trim_start()))
        .filter(|value| *value > 0)
        .unwrap_or(fallback)
        .min(max)
}

fn parse_json_body(body: &Bytes) -> Value {
    if body.is_empty() {
        return json!({});
    }
    serde_json::from_slice(body).unwrap_or_else(|_| json!({}))
}

fn delete_ip_value_to_string(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value.clone(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::Array(items) => items
            .iter()
            .map(delete_ip_value_to_string)
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".to_string(),
    }
}

fn positive_i64_from_value(value: Option<&Value>) -> Option<i64> {
    parse_i64_from_json_like_node(value?).filter(|value| *value > 0)
}

fn parse_i64_from_json_like_node(value: &Value) -> Option<i64> {
    parse_i64_prefix(&delete_ip_value_to_string(value).trim_start())
}

fn parse_i64_prefix(value: &str) -> Option<i64> {
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
    if !has_digit {
        return None;
    }
    value[..end].parse::<i64>().ok()
}

fn extract_between<'a>(value: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let (_, rest) = value.split_once(start)?;
    let (part, _) = rest.split_once(end)?;
    Some(part)
}

fn extract_after<'a>(value: &'a str, marker: &str) -> Option<&'a str> {
    value.split_once(marker).map(|(_, rest)| rest)
}

fn fingerprint(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    hex::encode(hasher.finalize())[..24].to_string()
}

fn millis_to_iso(ms: i64) -> String {
    time::OffsetDateTime::from_unix_timestamp(ms.div_euclid(1000))
        .ok()
        .and_then(|time| {
            time.format(&time::format_description::well_known::Rfc3339)
                .ok()
        })
        .unwrap_or_else(time_utils::now_iso)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ssh_login_messages() {
        let success = parse_ssh_message(
            "sshd[1]: Accepted publickey for root from 1.2.3.4 port 456 ssh2",
            "2026-01-01T00:00:00Z",
            "auth.log",
        )
        .unwrap();
        assert_eq!(success["outcome"], json!("success"));
        assert_eq!(success["ip"], json!("1.2.3.4"));
        assert_eq!(success["port"], json!(456));

        let failure = parse_ssh_message(
            "sshd[1]: Failed password for invalid user admin from 5.6.7.8 port 22 ssh2",
            "2026-01-01T00:00:00Z",
            "auth.log",
        )
        .unwrap();
        assert_eq!(failure["outcome"], json!("failure"));
        assert_eq!(failure["invalid_user"], json!(true));
    }

    #[test]
    fn normalizes_ssh_config_defaults() {
        let config = normalize_config(None);
        assert_eq!(config["window_minutes"], json!(10));
        assert_eq!(config["block_duration_unit"], json!("day"));
    }

    #[test]
    fn localizes_ssh_security_route_success_messages() {
        let zh = Translator::new("zh-CN");
        assert_eq!(
            ssh_security_route_text_params(
                &zh,
                "syncFirewallSuccess",
                &[
                    ("allowedCidrs", "2".to_string()),
                    ("ports", "22, 2222".to_string()),
                    ("synced", "3".to_string())
                ],
            ),
            "已同步 2 条允许 CIDR 与 3 个 SSH 封锁 IP 到 22, 2222 端口"
        );
        assert_eq!(
            ssh_security_route_text(&zh, "clearFirewallSuccess"),
            "已清空 SSH 专用防火墙规则"
        );
    }

    #[test]
    fn active_block_requires_applied_and_future_expiry() {
        let record = json!({
            "ip": "1.2.3.4",
            "blocked_at": "2026-01-01T00:00:00Z",
            "expires_at": "2999-01-01T00:00:00Z",
            "applied": true,
            "ports": ["22", "2222x", 0, 22]
        });
        let normalized = normalize_block_record(record).unwrap();
        assert!(is_active_block(&normalized, time_utils::now_ms()));
        assert_eq!(normalized["ports"], json!([22, 2222]));
    }

    #[test]
    fn ssh_query_and_delete_parsers_match_node_edges() {
        assert_eq!(parse_positive(None, 1, 100), 1);
        assert_eq!(parse_positive(Some("2x"), 1, 100), 2);
        assert_eq!(parse_positive(Some("  +3.9"), 1, 100), 3);
        assert_eq!(parse_positive(Some("-1"), 1, 100), 1);
        assert_eq!(parse_positive(Some("999"), 1, 100), 100);

        assert_eq!(delete_ip_value_to_string(&Value::Null), "");
        assert_eq!(delete_ip_value_to_string(&json!(123)), "123");
        assert_eq!(
            delete_ip_value_to_string(&json!({"ip":"1.2.3.4"})),
            "[object Object]"
        );
        assert_eq!(
            delete_ip_value_to_string(&json!(["1.2.3.4", null, true])),
            "1.2.3.4,,true"
        );
    }

    #[test]
    fn coalesces_success_login_logs_like_node_window() {
        let first = json!({
            "id": "a",
            "happened_at": "2026-01-01T00:00:00Z",
            "outcome": "success",
            "username": "root",
            "ip": "1.2.3.4",
            "source": "auth.log",
            "auth_method": "publickey",
            "port": 22,
            "raw": "first"
        });
        let second = json!({
            "id": "b",
            "happened_at": "2026-01-01T00:00:20Z",
            "outcome": "success",
            "username": "root",
            "ip": "1.2.3.4",
            "source": "auth.log",
            "auth_method": "publickey",
            "port": "2222",
            "raw": "second"
        });
        let failure = json!({
            "id": "c",
            "happened_at": "2026-01-01T00:00:21Z",
            "outcome": "failure",
            "username": "root",
            "ip": "1.2.3.4",
            "source": "auth.log",
            "raw": "failure"
        });

        let entries = coalesce_success_login_logs(vec![first, second, failure]);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0]["repeat_count"], json!(2));
        assert_eq!(entries[0]["related_ports"], json!([22, 2222]));
        assert_eq!(entries[0]["raw"], json!("first\nsecond"));
        assert_eq!(entries[1]["outcome"], json!("failure"));
    }

    #[test]
    fn localizes_ssh_security_route_and_validation_text() {
        let zh = Translator::new("zh-CN");
        assert_eq!(
            ssh_security_route_text(&zh, "listBlocksFailed"),
            "获取 SSH 封锁列表失败"
        );
        let error = validate_cidrs(Some(&json!(["bad-cidr"])), &zh).unwrap_err();
        match error {
            SshError::BadRequest(message) => {
                assert_eq!(message, "自定义 CIDR 格式不正确：bad-cidr");
            }
            _ => panic!("expected bad request"),
        }
    }
}
