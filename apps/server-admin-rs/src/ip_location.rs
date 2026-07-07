use std::{collections::HashSet, env, time::Duration};

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tokio::time::MissedTickBehavior;
use url::Url;

use crate::{http_utils, i18n::Translator, response, state::AppState, time_utils};

const IP_LOCATION_BATCH_LIMIT: usize = 20;
const LOOKUP_SUCCESS_CACHE_TTL_SECONDS: usize = 7 * 24 * 60 * 60;
const LOOKUP_FAILED_STATE_TTL_SECONDS: usize = 300;
const MAX_ATTEMPTS: i64 = 5;
const QUEUE_POLL_INTERVAL: Duration = Duration::from_secs(1);
const QUEUE_BATCH_SIZE: usize = 3;
const DEFAULT_IP_LOOKUP_URL: &str = "https://ipaddress.fnknock.cn/api/v1";
const IP_LOCATION_API_SETTINGS_KEY: &str = "fn_knock:ip-location-api:settings";
const USER_AGENT: &str = "fn-knock-server-admin/1.0";

#[derive(Deserialize)]
struct BatchBody {
    ips: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct IpLocationResult {
    ip: String,
    #[serde(rename = "normalizedIp")]
    normalized_ip: String,
    version: String,
    continent: String,
    country: String,
    province: String,
    city: String,
    district: String,
    isp: String,
    #[serde(rename = "countryCode")]
    country_code: String,
    raw: String,
    #[serde(rename = "sourceRaw")]
    source_raw: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct IpLocationState {
    status: String,
    attempts: i64,
    #[serde(rename = "maxAttempts")]
    max_attempts: i64,
    #[serde(rename = "updatedAt")]
    updated_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "nextAttemptAt")]
    next_attempt_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<IpLocationResult>,
}

#[derive(Clone, Debug, Serialize)]
struct IpLocationSnapshot {
    ip: String,
    #[serde(rename = "normalizedIp")]
    normalized_ip: String,
    status: String,
    attempts: i64,
    #[serde(rename = "maxAttempts")]
    max_attempts: i64,
    location: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<IpLocationResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(rename = "updatedAt")]
    updated_at: i64,
}

enum LookupOutcome {
    Success(IpLocationResult),
    Failure(String),
}

pub fn ip_location_routes() -> Router<AppState> {
    Router::new().route("/api/admin/ip-location/batch", post(batch))
}

pub fn start_ip_location_worker(state: AppState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(QUEUE_POLL_INTERVAL);
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            if let Err(error) = process_queue(&state).await {
                tracing::warn!(%error, "IP location queue tick failed");
            }
        }
    });
}

pub async fn ensure_ip_locations_enqueued(
    state: &AppState,
    ips: Vec<String>,
) -> redis::RedisResult<()> {
    for ip in ips {
        let _ = ensure_enqueued(state, &ip).await?;
    }
    Ok(())
}

pub async fn ensure_ip_location_enqueued(state: &AppState, ip: &str) -> redis::RedisResult<Value> {
    Ok(serde_json::to_value(ensure_enqueued(state, ip).await?).unwrap_or_else(|_| json!({})))
}

pub async fn register_usage(
    state: &AppState,
    ip: &str,
    references: Vec<String>,
) -> anyhow::Result<String> {
    let normalized_ip = http_utils::normalize_ip(ip);
    if normalized_ip.is_empty() {
        return Ok(String::new());
    }

    let references = {
        let mut seen = HashSet::new();
        references
            .into_iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty() && seen.insert(value.clone()))
            .collect::<Vec<_>>()
    };
    if !references.is_empty() {
        state
            .redis
            .add_ip_location_references(
                &normalized_ip,
                &references,
                LOOKUP_SUCCESS_CACHE_TTL_SECONDS,
            )
            .await?;
    }

    if let Some(cached) = state.redis.get_ip_location_cache(&normalized_ip).await?
        && let Ok(result) = serde_json::from_value::<IpLocationResult>(cached)
    {
        if !references.is_empty() {
            sync_tracked_references(state, &normalized_ip, &result, &references).await?;
        }
        return Ok(result.raw);
    }

    let _ = ensure_enqueued(state, &normalized_ip).await?;
    Ok(String::new())
}

async fn batch(State(state): State<AppState>, Json(body): Json<BatchBody>) -> Response {
    let translator = Translator::from_state(&state).await;
    if body.ips.len() > IP_LOCATION_BATCH_LIMIT {
        return response::error(
            StatusCode::BAD_REQUEST,
            ip_location_route_text_params(
                &translator,
                "batchLimit",
                &[("max", IP_LOCATION_BATCH_LIMIT.to_string())],
            ),
        );
    }

    match ensure_enqueued_batch(&state, body.ips).await {
        Ok(items) => response::ok(json!({ "items": items })).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to enqueue IP location batch");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ip_location_route_text(&translator, "enqueueFailed"),
            )
        }
    }
}

fn ip_location_route_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.ipLocationRoutes.{key}"))
}

fn ip_location_route_text_params(
    translator: &Translator,
    key: &str,
    params: &[(&str, String)],
) -> String {
    translator.t_params(&format!("server.ipLocationRoutes.{key}"), params)
}

async fn ensure_enqueued_batch(
    state: &AppState,
    ips: Vec<String>,
) -> redis::RedisResult<Vec<IpLocationSnapshot>> {
    let mut seen = HashSet::new();
    let mut unique_ips = Vec::new();
    for ip in ips {
        let raw_ip = ip.trim();
        if raw_ip.is_empty() {
            continue;
        }
        let normalized_ip = http_utils::normalize_ip(raw_ip);
        let dedupe_key = if normalized_ip.is_empty() {
            format!("raw:{raw_ip}")
        } else {
            normalized_ip
        };
        if seen.insert(dedupe_key) {
            unique_ips.push(raw_ip.to_string());
        }
    }

    let mut snapshots = Vec::with_capacity(unique_ips.len());
    for ip in unique_ips {
        snapshots.push(ensure_enqueued(state, &ip).await?);
    }
    Ok(snapshots)
}

async fn ensure_enqueued(state: &AppState, ip: &str) -> redis::RedisResult<IpLocationSnapshot> {
    let normalized_ip = http_utils::normalize_ip(ip);
    if normalized_ip.is_empty() {
        return Ok(build_snapshot(
            ip,
            "",
            &IpLocationState {
                status: "skipped".to_string(),
                attempts: 0,
                max_attempts: MAX_ATTEMPTS,
                updated_at: time_utils::now_ms(),
                error: Some("invalid ip".to_string()),
                next_attempt_at: None,
                result: None,
            },
        ));
    }

    if let Some(cached) = state.redis.get_ip_location_cache(&normalized_ip).await? {
        if let Ok(result) = serde_json::from_value::<IpLocationResult>(cached) {
            let current = get_state(state, &normalized_ip).await?;
            let state_value = if current.status == "success" {
                current
            } else {
                build_success_state(result, current.attempts)
            };
            state
                .redis
                .set_ip_location_state(
                    &normalized_ip,
                    &serde_json::to_value(&state_value).unwrap_or_else(|_| json!({})),
                    LOOKUP_SUCCESS_CACHE_TTL_SECONDS,
                )
                .await?;
            return Ok(build_snapshot(ip, &normalized_ip, &state_value));
        }
    }

    if http_utils::is_private_or_local_ip(&normalized_ip) {
        let state_value = IpLocationState {
            status: "skipped".to_string(),
            attempts: 0,
            max_attempts: MAX_ATTEMPTS,
            updated_at: time_utils::now_ms(),
            error: None,
            next_attempt_at: None,
            result: None,
        };
        state
            .redis
            .set_ip_location_state(
                &normalized_ip,
                &serde_json::to_value(&state_value).unwrap_or_else(|_| json!({})),
                LOOKUP_FAILED_STATE_TTL_SECONDS,
            )
            .await?;
        return Ok(build_snapshot(ip, &normalized_ip, &state_value));
    }

    let current = get_state(state, &normalized_ip).await?;
    if matches!(
        current.status.as_str(),
        "success" | "queued" | "processing" | "failed"
    ) {
        return Ok(build_snapshot(ip, &normalized_ip, &current));
    }

    let now = time_utils::now_ms();
    let next_state = IpLocationState {
        status: "queued".to_string(),
        attempts: current.attempts,
        max_attempts: MAX_ATTEMPTS,
        updated_at: now,
        error: None,
        next_attempt_at: Some(now),
        result: None,
    };
    state
        .redis
        .enqueue_ip_location(
            &normalized_ip,
            &serde_json::to_value(&next_state).unwrap_or_else(|_| json!({})),
            now,
            LOOKUP_FAILED_STATE_TTL_SECONDS,
        )
        .await?;

    Ok(build_snapshot(ip, &normalized_ip, &next_state))
}

async fn process_queue(state: &AppState) -> anyhow::Result<()> {
    let due_ips = state
        .redis
        .due_ip_location_ips(time_utils::now_ms(), QUEUE_BATCH_SIZE)
        .await?;
    if due_ips.is_empty() {
        return Ok(());
    }

    for ip in due_ips {
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(error) = process_one(&state, &ip).await {
                tracing::warn!(%error, %ip, "failed to process IP location lookup");
            }
        });
    }
    Ok(())
}

async fn process_one(state: &AppState, ip: &str) -> anyhow::Result<()> {
    let lookup_timeout = lookup_timeout();
    let lock_ttl_seconds = (lookup_timeout.as_secs() + 5).max(10) as usize;
    let locked = state
        .redis
        .acquire_ip_location_lock(ip, time_utils::now_ms(), lock_ttl_seconds)
        .await?;
    if !locked {
        return Ok(());
    }

    let result = async {
        state.redis.remove_ip_location_queue_entry(ip).await?;
        let current = get_state(state, ip).await?;
        let attempts = current.attempts.max(0);
        if attempts >= MAX_ATTEMPTS {
            let failed = IpLocationState {
                status: "failed".to_string(),
                attempts,
                max_attempts: MAX_ATTEMPTS,
                updated_at: time_utils::now_ms(),
                error: current
                    .error
                    .clone()
                    .or_else(|| Some("max attempts reached".to_string())),
                next_attempt_at: None,
                result: None,
            };
            state
                .redis
                .set_ip_location_state(
                    ip,
                    &serde_json::to_value(failed).unwrap_or_else(|_| json!({})),
                    LOOKUP_FAILED_STATE_TTL_SECONDS,
                )
                .await?;
            return Ok::<(), anyhow::Error>(());
        }

        let processing = IpLocationState {
            status: "processing".to_string(),
            attempts,
            max_attempts: MAX_ATTEMPTS,
            updated_at: time_utils::now_ms(),
            error: None,
            next_attempt_at: None,
            result: None,
        };
        state
            .redis
            .set_ip_location_state(
                ip,
                &serde_json::to_value(processing).unwrap_or_else(|_| json!({})),
                LOOKUP_FAILED_STATE_TTL_SECONDS,
            )
            .await?;

        let next_attempt = attempts + 1;
        match lookup_remote(state, ip, lookup_timeout).await {
            LookupOutcome::Success(result) => {
                let success_state = build_success_state(result.clone(), next_attempt);
                state
                    .redis
                    .complete_ip_location_lookup(
                        ip,
                        &serde_json::to_value(&result).unwrap_or_else(|_| json!({})),
                        &serde_json::to_value(&success_state).unwrap_or_else(|_| json!({})),
                        LOOKUP_SUCCESS_CACHE_TTL_SECONDS,
                    )
                    .await?;
                sync_references(state, ip, &result).await?;
            }
            LookupOutcome::Failure(error) => {
                if next_attempt >= MAX_ATTEMPTS {
                    let failed = IpLocationState {
                        status: "failed".to_string(),
                        attempts: next_attempt,
                        max_attempts: MAX_ATTEMPTS,
                        updated_at: time_utils::now_ms(),
                        error: Some(error),
                        next_attempt_at: None,
                        result: None,
                    };
                    state
                        .redis
                        .set_ip_location_state(
                            ip,
                            &serde_json::to_value(failed).unwrap_or_else(|_| json!({})),
                            LOOKUP_FAILED_STATE_TTL_SECONDS,
                        )
                        .await?;
                } else {
                    let next_attempt_at = time_utils::now_ms() + retry_delay_ms(next_attempt);
                    let queued = IpLocationState {
                        status: "queued".to_string(),
                        attempts: next_attempt,
                        max_attempts: MAX_ATTEMPTS,
                        updated_at: time_utils::now_ms(),
                        error: Some(error),
                        next_attempt_at: Some(next_attempt_at),
                        result: None,
                    };
                    state
                        .redis
                        .enqueue_ip_location(
                            ip,
                            &serde_json::to_value(queued).unwrap_or_else(|_| json!({})),
                            next_attempt_at,
                            LOOKUP_FAILED_STATE_TTL_SECONDS,
                        )
                        .await?;
                }
            }
        }
        Ok(())
    }
    .await;

    let release_result = state.redis.release_ip_location_lock(ip).await;
    if let Err(error) = release_result {
        tracing::warn!(%error, %ip, "failed to release IP location lock");
    }
    result
}

async fn lookup_remote(state: &AppState, ip: &str, timeout: Duration) -> LookupOutcome {
    let client = match reqwest::Client::builder().timeout(timeout).build() {
        Ok(client) => client,
        Err(error) => return LookupOutcome::Failure(error.to_string()),
    };
    let mut url = match ip_lookup_api_url(state).await {
        Ok(url) => url,
        Err(error) => return LookupOutcome::Failure(error),
    };
    url.query_pairs_mut().append_pair("ip", ip);

    let response = match client
        .get(url)
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .send()
        .await
    {
        Ok(response) => response,
        Err(error) => {
            if error.is_timeout() {
                return LookupOutcome::Failure("lookup timeout".to_string());
            }
            return LookupOutcome::Failure(error.to_string());
        }
    };
    let status = response.status();
    if !status.is_success() {
        return LookupOutcome::Failure(format!("http {}", status.as_u16()));
    }
    let payload = match response.json::<Value>().await {
        Ok(payload) => payload,
        Err(_) => return LookupOutcome::Failure("invalid lookup response".to_string()),
    };
    if payload.get("code").and_then(Value::as_i64) != Some(0)
        || payload.get("result").is_none_or(Value::is_null)
    {
        return LookupOutcome::Failure(
            payload
                .get("msg")
                .and_then(Value::as_str)
                .unwrap_or("invalid lookup response")
                .to_string(),
        );
    }

    match to_location_result(ip, &payload) {
        Some(result) => LookupOutcome::Success(result),
        None => LookupOutcome::Failure("empty lookup result".to_string()),
    }
}

async fn ip_lookup_api_url(state: &AppState) -> Result<Url, String> {
    let raw = state
        .redis
        .get_json_value(IP_LOCATION_API_SETTINGS_KEY)
        .await
        .map_err(|error| error.to_string())?;
    let settings = raw.as_ref();
    let mode = settings
        .and_then(|value| value.get("ip_lookup_mode"))
        .and_then(Value::as_str)
        .unwrap_or("online");
    let configured_url = settings
        .and_then(|value| value.get("ip_lookup_url"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let base_url = if mode == "custom" && !configured_url.trim().is_empty() {
        configured_url
    } else {
        DEFAULT_IP_LOOKUP_URL
    };
    build_ip_location_api_url(base_url, "ip/lookup")
}

fn build_ip_location_api_url(base_url: &str, path: &str) -> Result<Url, String> {
    let api_base = resolve_ip_location_api_base_url(base_url)?;
    Url::parse(&format!("{api_base}/{}", path.trim_start_matches('/')))
        .map_err(|error| error.to_string())
}

fn resolve_ip_location_api_base_url(value: &str) -> Result<String, String> {
    let normalized = value.trim().trim_end_matches('/');
    let mut url = Url::parse(normalized).map_err(|error| error.to_string())?;
    let path = url.path().trim_end_matches('/').to_string();
    if path.is_empty() {
        url.set_path("/api/v1");
    } else {
        url.set_path(&path);
    }
    url.set_query(None);
    url.set_fragment(None);
    Ok(url.as_str().trim_end_matches('/').to_string())
}

fn to_location_result(ip: &str, payload: &Value) -> Option<IpLocationResult> {
    let normalized_ip = http_utils::normalize_ip(ip);
    if normalized_ip.is_empty() {
        return None;
    }
    let result = payload.get("result")?;
    let version = if result.get("version").and_then(Value::as_str) == Some("ipv6") {
        "ipv6"
    } else {
        "ipv4"
    };
    let continent = string_field(result, "continent");
    let country = string_field(result, "country");
    let province = string_field(result, "province");
    let city = string_field(result, "city");
    let district = string_field(result, "district");
    let isp = string_field(result, "isp");
    let country_code = string_field(result, "country_code");
    let source_raw = string_field(result, "raw");
    let raw = format_raw(&country, &province, &city, &isp, &source_raw);
    if raw.is_empty() {
        return None;
    }
    Some(IpLocationResult {
        ip: ip.to_string(),
        normalized_ip,
        version: version.to_string(),
        continent,
        country,
        province,
        city,
        district,
        isp,
        country_code,
        raw,
        source_raw,
    })
}

fn format_raw(country: &str, province: &str, city: &str, isp: &str, source_raw: &str) -> String {
    if country.trim() == "中国" {
        if matches!(province.trim(), "台湾" | "香港" | "澳门") {
            return join_location_parts([province, city, isp]);
        }
        return join_location_parts([province.trim().if_empty(country), city, isp]);
    }
    let foreign = join_location_parts([country, province, city, isp]);
    if foreign.is_empty() {
        source_raw.trim().to_string()
    } else {
        foreign
    }
}

trait EmptyFallback {
    fn if_empty<'a>(&'a self, fallback: &'a str) -> &'a str;
}

impl EmptyFallback for str {
    fn if_empty<'a>(&'a self, fallback: &'a str) -> &'a str {
        if self.is_empty() { fallback } else { self }
    }
}

fn join_location_parts<const N: usize>(parts: [&str; N]) -> String {
    parts
        .into_iter()
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("|")
}

fn string_field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .map(js_string_or_empty_trim)
        .unwrap_or_default()
}

fn js_string_or_empty_trim(value: &Value) -> String {
    let text = match value {
        Value::Null => String::new(),
        Value::Bool(false) => String::new(),
        Value::Bool(true) => "true".to_string(),
        Value::Number(number) if number.as_f64() == Some(0.0) => String::new(),
        Value::Number(number) => number.to_string(),
        Value::String(value) if value.is_empty() => String::new(),
        Value::String(value) => value.clone(),
        Value::Array(items) => items
            .iter()
            .map(js_array_item_string)
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".to_string(),
    };
    text.trim().to_string()
}

fn js_array_item_string(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(value) => value.to_string(),
        Value::Number(number) => number.to_string(),
        Value::String(value) => value.clone(),
        Value::Array(items) => items
            .iter()
            .map(js_array_item_string)
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".to_string(),
    }
}

async fn get_state(state: &AppState, ip: &str) -> redis::RedisResult<IpLocationState> {
    let raw = state.redis.get_ip_location_state(ip).await?;
    Ok(raw
        .and_then(|value| serde_json::from_value::<IpLocationState>(value).ok())
        .unwrap_or_else(|| IpLocationState {
            status: "idle".to_string(),
            attempts: 0,
            max_attempts: MAX_ATTEMPTS,
            updated_at: 0,
            error: None,
            next_attempt_at: None,
            result: None,
        }))
}

fn build_success_state(result: IpLocationResult, attempts: i64) -> IpLocationState {
    IpLocationState {
        status: "success".to_string(),
        attempts,
        max_attempts: MAX_ATTEMPTS,
        updated_at: time_utils::now_ms(),
        error: None,
        next_attempt_at: None,
        result: Some(result),
    }
}

fn build_snapshot(ip: &str, normalized_ip: &str, state: &IpLocationState) -> IpLocationSnapshot {
    IpLocationSnapshot {
        ip: ip.to_string(),
        normalized_ip: normalized_ip.to_string(),
        status: state.status.clone(),
        attempts: state.attempts,
        max_attempts: state.max_attempts,
        location: state
            .result
            .as_ref()
            .map(|result| result.raw.clone())
            .unwrap_or_default(),
        result: state.result.clone(),
        error: state.error.clone(),
        updated_at: state.updated_at,
    }
}

async fn sync_references(
    state: &AppState,
    ip: &str,
    result: &IpLocationResult,
) -> anyhow::Result<()> {
    let refs = state.redis.ip_location_references(ip).await?;
    sync_tracked_references(state, ip, result, &refs).await
}

async fn sync_tracked_references(
    state: &AppState,
    ip: &str,
    result: &IpLocationResult,
    refs: &[String],
) -> anyhow::Result<()> {
    if refs.is_empty() {
        return Ok(());
    }

    let mut stale_refs = Vec::new();
    for reference in refs {
        if !sync_reference(state, reference, result).await? {
            stale_refs.push(reference.clone());
        }
    }
    if !stale_refs.is_empty() {
        state
            .redis
            .remove_ip_location_references(ip, &stale_refs)
            .await?;
    }
    Ok(())
}

async fn sync_reference(
    state: &AppState,
    reference: &str,
    result: &IpLocationResult,
) -> anyhow::Result<bool> {
    let Some((kind, id)) = reference.split_once('|') else {
        return Ok(false);
    };
    match kind {
        "session" | "scanner-blacklist" | "ssh-blocklist" => {
            let Some(key) = json_reference_key(kind, id) else {
                return Ok(false);
            };
            sync_json_ip_location(state, &key, result).await
        }
        "whitelist" => sync_hash_ip_location(state, "fn_knock:whitelist:records", id, result).await,
        "session-timeline" => sync_session_timeline(state, id, result).await,
        "system-event" => sync_system_event(state, id, result).await,
        _ => Ok(false),
    }
}

fn json_reference_key(kind: &str, id: &str) -> Option<String> {
    match kind {
        "session" => Some(format!("fn_knock:session:{id}")),
        "scanner-blacklist" => Some(format!("fn_knock:scanner:blacklist:data:{id}")),
        "ssh-blocklist" => Some(format!("fn_knock:ssh_security:blocks:data:{id}")),
        _ => None,
    }
}

async fn sync_hash_ip_location(
    state: &AppState,
    key: &str,
    field: &str,
    result: &IpLocationResult,
) -> anyhow::Result<bool> {
    let Some(mut record) = state.redis.hget_json_value(key, field).await? else {
        return Ok(false);
    };
    if !record_matches_ip(&record, &result.normalized_ip, "ip") {
        return Ok(false);
    }
    if record.get("ipLocation").and_then(Value::as_str) == Some(result.raw.as_str()) {
        return Ok(true);
    }
    if let Some(object) = record.as_object_mut() {
        object.insert("ipLocation".to_string(), Value::String(result.raw.clone()));
    } else {
        return Ok(false);
    }
    state.redis.hset_json_value(key, field, &record).await?;
    Ok(true)
}

async fn sync_json_ip_location(
    state: &AppState,
    key: &str,
    result: &IpLocationResult,
) -> anyhow::Result<bool> {
    let (Some(mut record), ttl) = state.redis.get_json_value_with_ttl(key).await? else {
        return Ok(false);
    };
    if !record_matches_ip(&record, &result.normalized_ip, "ip") {
        return Ok(false);
    }
    if record.get("ipLocation").and_then(Value::as_str) == Some(result.raw.as_str()) {
        return Ok(true);
    }
    if let Some(object) = record.as_object_mut() {
        object.insert("ipLocation".to_string(), Value::String(result.raw.clone()));
    } else {
        return Ok(false);
    }
    state
        .redis
        .set_json_value_preserve_ttl(key, &record, ttl)
        .await?;
    Ok(true)
}

async fn sync_session_timeline(
    state: &AppState,
    session_id: &str,
    result: &IpLocationResult,
) -> anyhow::Result<bool> {
    let key = format!("fn_knock:auth_mobility:timeline:{session_id}");
    let (Some(record), ttl) = state.redis.get_json_value_with_ttl(&key).await? else {
        return Ok(false);
    };
    let Some(events) = record.as_array() else {
        return Ok(false);
    };

    let mut matched = false;
    let mut updated = false;
    let next_events = events
        .iter()
        .map(|event| {
            let mut next = event.clone();
            let Some(object) = next.as_object_mut() else {
                return next;
            };
            for (ip_key, location_key) in [("toIp", "toIpLocation"), ("fromIp", "fromIpLocation")] {
                let ip = object.get(ip_key).and_then(Value::as_str).unwrap_or("");
                if http_utils::normalize_ip(ip) != result.normalized_ip {
                    continue;
                }
                matched = true;
                if object.get(location_key).and_then(Value::as_str) != Some(result.raw.as_str()) {
                    object.insert(location_key.to_string(), Value::String(result.raw.clone()));
                    updated = true;
                }
            }
            next
        })
        .collect::<Vec<_>>();
    if !matched {
        return Ok(false);
    }
    if !updated {
        return Ok(true);
    }
    state
        .redis
        .set_json_value_preserve_ttl(&key, &Value::Array(next_events), ttl)
        .await?;
    Ok(true)
}

async fn sync_system_event(
    state: &AppState,
    event_id: &str,
    result: &IpLocationResult,
) -> anyhow::Result<bool> {
    let key = format!("fn_knock:events:data:{event_id}");
    let (Some(mut event), ttl) = state.redis.get_json_value_with_ttl(&key).await? else {
        return Ok(false);
    };
    let event_type = event
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let Some(payload) = event.get_mut("payload").and_then(Value::as_object_mut) else {
        return Ok(false);
    };
    let mut matched = false;
    let mut updated = false;
    for &(ip_key, location_key) in system_event_ip_fields(&event_type) {
        let ip = payload.get(ip_key).and_then(Value::as_str).unwrap_or("");
        if http_utils::normalize_ip(ip) != result.normalized_ip {
            continue;
        }
        matched = true;
        if payload
            .get(location_key)
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .is_empty()
        {
            payload.insert(location_key.to_string(), Value::String(result.raw.clone()));
            updated = true;
        }
    }
    if !matched {
        return Ok(false);
    }
    if !updated {
        return Ok(true);
    }
    state
        .redis
        .set_json_value_preserve_ttl(&key, &event, ttl)
        .await?;
    Ok(true)
}

fn system_event_ip_fields(event_type: &str) -> &'static [(&'static str, &'static str)] {
    match event_type {
        "FN_EVENT_AUTH_SESSION_IP_DRIFT" => {
            &[("from_ip", "from_ip_location"), ("to_ip", "to_ip_location")]
        }
        "FN_EVENT_AUTH_LOGIN_SUCCESS"
        | "FN_EVENT_AUTH_LOGOUT"
        | "FN_EVENT_AUTH_LOGIN_FAILURE"
        | "FN_EVENT_SECURITY_SCANNER_BLOCKED"
        | "FN_EVENT_GATEWAY_THROTTLE_BLOCKED"
        | "FN_EVENT_WAF_BLOCKED"
        | "FN_EVENT_SSH_LOGIN_SUCCESS"
        | "FN_EVENT_SSH_LOGIN_FAILURE"
        | "FN_EVENT_SSH_IP_BLOCKED" => &[("ip", "ip_location")],
        _ => &[],
    }
}

fn record_matches_ip(record: &Value, normalized_ip: &str, field: &str) -> bool {
    record
        .get(field)
        .and_then(Value::as_str)
        .is_some_and(|value| http_utils::normalize_ip(value) == normalized_ip)
}

fn retry_delay_ms(attempt: i64) -> i64 {
    if attempt <= 1 { 2000 } else { 5000 }
}

fn lookup_timeout() -> Duration {
    let millis = env::var("IP_LOOKUP_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(8000)
        .max(2000);
    Duration::from_millis(millis)
}

#[allow(dead_code)]
fn value_object(value: Value) -> Map<String, Value> {
    value.as_object().cloned().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_china_locations_like_node() {
        assert_eq!(
            format_raw("中国", "广东", "深圳", "电信", ""),
            "广东|深圳|电信"
        );
        assert_eq!(
            format_raw("中国", "香港", "香港", "PCCW", ""),
            "香港|香港|PCCW"
        );
        assert_eq!(
            format_raw("美国", "加州", "", "Google", ""),
            "美国|加州|Google"
        );
        assert_eq!(format_raw("", "", "", "", "raw"), "raw");
    }

    #[test]
    fn builds_location_result_from_lookup_payload() {
        let result = to_location_result(
            "8.8.8.8",
            &json!({
                "code": 0,
                "result": {
                    "version": "ipv4",
                    "continent": "北美洲",
                    "country": "美国",
                    "province": "加州",
                    "city": "",
                    "district": "",
                    "isp": "Google",
                    "country_code": "US",
                    "raw": "United States"
                }
            }),
        )
        .unwrap();

        assert_eq!(result.normalized_ip, "8.8.8.8");
        assert_eq!(result.country_code, "US");
        assert_eq!(result.raw, "美国|加州|Google");
    }

    #[test]
    fn string_fields_follow_node_string_value_or_empty_truthiness() {
        assert_eq!(js_string_or_empty_trim(&json!(null)), "");
        assert_eq!(js_string_or_empty_trim(&json!("")), "");
        assert_eq!(js_string_or_empty_trim(&json!(" value ")), "value");
        assert_eq!(js_string_or_empty_trim(&json!(0)), "");
        assert_eq!(js_string_or_empty_trim(&json!(12.5)), "12.5");
        assert_eq!(js_string_or_empty_trim(&json!(true)), "true");
        assert_eq!(js_string_or_empty_trim(&json!(false)), "");
        assert_eq!(js_string_or_empty_trim(&json!([])), "");
        assert_eq!(js_string_or_empty_trim(&json!([1, null, "x"])), "1,,x");
        assert_eq!(
            js_string_or_empty_trim(&json!({ "a": 1 })),
            "[object Object]"
        );
    }

    #[test]
    fn location_result_string_coercion_matches_node_lookup_payload() {
        let result = to_location_result(
            "8.8.4.4",
            &json!({
                "code": 0,
                "result": {
                    "version": "ipv4",
                    "continent": true,
                    "country": true,
                    "province": 12.5,
                    "city": false,
                    "district": null,
                    "isp": [1, 2],
                    "country_code": 0,
                    "raw": { "unexpected": true }
                }
            }),
        )
        .unwrap();

        assert_eq!(result.continent, "true");
        assert_eq!(result.country, "true");
        assert_eq!(result.province, "12.5");
        assert_eq!(result.city, "");
        assert_eq!(result.isp, "1,2");
        assert_eq!(result.country_code, "");
        assert_eq!(result.source_raw, "[object Object]");
        assert_eq!(result.raw, "true|12.5|1,2");
    }

    #[test]
    fn exposes_system_event_ip_field_mapping() {
        assert_eq!(
            system_event_ip_fields("FN_EVENT_AUTH_SESSION_IP_DRIFT"),
            &[("from_ip", "from_ip_location"), ("to_ip", "to_ip_location")]
        );
        assert_eq!(
            system_event_ip_fields("FN_EVENT_AUTH_LOGIN_FAILURE"),
            &[("ip", "ip_location")]
        );
        assert!(system_event_ip_fields("OTHER").is_empty());
    }

    #[test]
    fn maps_persistent_ip_location_reference_keys() {
        assert_eq!(
            json_reference_key("session", "abc").as_deref(),
            Some("fn_knock:session:abc")
        );
        assert_eq!(
            json_reference_key("scanner-blacklist", "203.0.113.10").as_deref(),
            Some("fn_knock:scanner:blacklist:data:203.0.113.10")
        );
        assert_eq!(
            json_reference_key("ssh-blocklist", "203.0.113.11").as_deref(),
            Some("fn_knock:ssh_security:blocks:data:203.0.113.11")
        );
        assert_eq!(json_reference_key("ssh-login-log", "entry-1"), None);
    }

    #[test]
    fn localizes_ip_location_route_messages() {
        let zh = Translator::new("zh-CN");
        assert_eq!(
            ip_location_route_text_params(&zh, "batchLimit", &[("max", "20".to_string())]),
            "单次最多查询 20 个 IP"
        );

        let en = Translator::new("en");
        assert_eq!(
            ip_location_route_text(&en, "enqueueFailed"),
            "Failed to enqueue IP location lookup"
        );
    }
}
