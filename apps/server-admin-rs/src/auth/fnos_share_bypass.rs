use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};

use axum::http::{HeaderMap, HeaderValue, Uri, header};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::{sync::Mutex as AsyncMutex, time::sleep};
use url::Url;

use crate::{cookies, runtime_config, state::AppState, time_utils};

const SHARE_ENTRY_ID_LENGTH: usize = 18;
const FNOS_SHARE_NEED_PASSWORD_CODE: i64 = 3_000_008;
const CACHE_KEY_PREFIX: &str = "fn_knock:fnos-share:validation:";
const SESSION_KEY_PREFIX: &str = "fn_knock:fnos-share:session:";
const LOCK_KEY_PREFIX: &str = "fn_knock:lock:fnos-share:validation:";
const FNOS_DETECTION_PATH: &str = "/locales/zh-CN/os.json";
const FNOS_DETECTION_SUCCESS_CACHE_TTL_MS: i64 = 30_000;
const FNOS_DETECTION_FAILURE_CACHE_TTL_MS: i64 = 3_000;
const FNOS_DETECTION_MAX_CACHE_ENTRIES: usize = 1_024;
const FNOS_DETECTION_TIMEOUT_MS: u64 = 1_000;
const FNOS_DETECTION_MIN_MATCHED_APP_KEYS: usize = 4;
const MAX_FNOS_SHARE_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const FNOS_DETECTION_APP_KEYS: &[&str] = &[
    "account",
    "appStore",
    "docker",
    "fileManager",
    "mediaCenter",
    "photos",
    "recycleBin",
    "resourceManager",
    "setting",
    "system",
    "vm",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FnosSharePreflightDecision {
    pub handled: bool,
    pub redirect_location: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FnosShareAuthorizationResult {
    pub authorized: bool,
    pub set_cookies: Vec<String>,
    pub response_headers: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
struct SharePolicy {
    enabled: bool,
    upstream_timeout_ms: u64,
    validation_cache_ttl_seconds: usize,
    validation_lock_ttl_seconds: usize,
    session_ttl_seconds: i64,
}

#[derive(Debug, Clone)]
struct ResolvedFnosShareConfig {
    policy: SharePolicy,
    backend: Option<FnosBackend>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FnosBackend {
    base_url: Url,
    host_header: String,
    identity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ShareValidationCacheRecord {
    version: i64,
    valid: bool,
    #[serde(rename = "validationState")]
    validation_state: String,
    #[serde(rename = "shareId")]
    share_id: String,
    #[serde(rename = "backendId", default)]
    backend_id: String,
    #[serde(rename = "cleanPath")]
    clean_path: String,
    token: Option<String>,
    name: Option<String>,
    #[serde(rename = "type")]
    kind: Option<i64>,
    #[serde(rename = "checkedAt")]
    checked_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ShareSessionRecord {
    #[serde(default)]
    version: i64,
    #[serde(rename = "shareId")]
    share_id: String,
    #[serde(rename = "backendId", default)]
    backend_id: String,
    #[serde(rename = "cleanPath")]
    clean_path: String,
    token: Option<String>,
    name: Option<String>,
    #[serde(rename = "type")]
    kind: Option<i64>,
    #[serde(rename = "issuedAt", default)]
    issued_at: String,
    #[serde(rename = "lastSeenAt", default)]
    last_seen_at: String,
}

#[derive(Debug, Clone)]
struct ShareEntry {
    share_id: String,
    clean_path: String,
    is_clean: bool,
}

#[derive(Debug, Clone)]
struct ShareValidationFetchResult {
    cacheable: bool,
    data: ShareValidationCacheRecord,
}

#[derive(Debug, Clone)]
struct ProbeCacheRecord {
    is_fnos: bool,
    expires_at: i64,
}

static PROBE_CACHE: OnceLock<Mutex<HashMap<String, ProbeCacheRecord>>> = OnceLock::new();
static PROBE_LOCKS: OnceLock<Mutex<HashMap<String, Arc<AsyncMutex<()>>>>> = OnceLock::new();
static PROBE_CLIENT: OnceLock<Option<reqwest::Client>> = OnceLock::new();

pub(crate) async fn resolve_preflight(
    state: &AppState,
    headers: &HeaderMap,
    uri: &Uri,
    app_config: &Value,
    routed_upstream: Option<&str>,
    routed_upstream_host: Option<&str>,
    routed_upstream_route_id: Option<&str>,
) -> anyhow::Result<FnosSharePreflightDecision> {
    let Some((request_url, policy)) = resolve_enabled_share_request(headers, uri, app_config)
    else {
        return Ok(not_handled());
    };
    let config = get_resolved_config(
        policy,
        routed_upstream,
        routed_upstream_host,
        routed_upstream_route_id,
    )
    .await;
    let Some(backend) = config.backend.as_ref() else {
        return Ok(not_handled());
    };

    if let Some(share_entry) = extract_share_entry(&request_url) {
        let validation = validate_share_link(state, &share_entry.share_id, &config).await;
        if !validation.valid {
            return Ok(handled_redirect("/"));
        }
        if share_entry.is_clean {
            return Ok(handled());
        }
        return Ok(handled_redirect(&share_entry.clean_path));
    }

    let share_session_id = read_share_cookie(headers, cookies::FNOS_SHARE_SESSION_COOKIE_NAME);
    let current_session = match share_session_id.as_deref() {
        Some(session_id) => get_share_session(state, session_id).await?,
        None => None,
    };
    if current_session.is_some_and(|session| {
        session_matches_backend(&session, backend)
            && is_session_resource_path(&request_url, &session.clean_path, &session.share_id)
    }) {
        return Ok(handled());
    }

    Ok(not_handled())
}

pub(crate) async fn authorize(
    state: &AppState,
    headers: &HeaderMap,
    uri: &Uri,
    app_config: &Value,
    routed_upstream: Option<&str>,
    routed_upstream_host: Option<&str>,
    routed_upstream_route_id: Option<&str>,
) -> anyhow::Result<FnosShareAuthorizationResult> {
    let Some((request_url, policy)) = resolve_enabled_share_request(headers, uri, app_config)
    else {
        return Ok(share_unauthorized());
    };
    let share_session_id = read_share_cookie(headers, cookies::FNOS_SHARE_SESSION_COOKIE_NAME);
    let config = get_resolved_config(
        policy,
        routed_upstream,
        routed_upstream_host,
        routed_upstream_route_id,
    )
    .await;
    let Some(backend) = config.backend.as_ref() else {
        return Ok(share_unauthorized_with_cookie_clear(
            share_session_id.is_some(),
        ));
    };
    let current_session = match share_session_id.as_deref() {
        Some(session_id) => get_share_session(state, session_id).await?,
        None => None,
    };

    if let Some(share_entry) = extract_share_entry(&request_url) {
        let validation = validate_share_link(state, &share_entry.share_id, &config).await;
        if !validation.valid {
            return Ok(share_redirect_unauthorized("/", share_session_id.is_some()));
        }

        if let (Some(session_id), Some(session)) =
            (share_session_id.as_deref(), current_session.as_ref())
            && session.share_id == validation.share_id
            && session_matches_backend(session, backend)
        {
            save_share_session(
                state,
                session_id,
                session,
                config.policy.session_ttl_seconds,
            )
            .await?;
            return Ok(share_authorized(
                session_id,
                config.policy.session_ttl_seconds,
            ));
        }

        let session_id = hex::encode(rand::random::<[u8; 18]>());
        let now = time_utils::now_iso();
        let session = ShareSessionRecord {
            version: 2,
            share_id: validation.share_id.clone(),
            backend_id: backend.identity.clone(),
            clean_path: validation.clean_path.clone(),
            token: validation.token.clone(),
            name: validation.name.clone(),
            kind: validation.kind,
            issued_at: now.clone(),
            last_seen_at: now,
        };
        save_share_session(
            state,
            &session_id,
            &session,
            config.policy.session_ttl_seconds,
        )
        .await?;
        return Ok(share_authorized(
            &session_id,
            config.policy.session_ttl_seconds,
        ));
    }

    let Some(session) = current_session else {
        return Ok(share_redirect_unauthorized("/", share_session_id.is_some()));
    };
    let Some(session_id) = share_session_id.as_deref() else {
        return Ok(share_redirect_unauthorized("/", false));
    };

    if !session_matches_backend(&session, backend)
        || !is_session_resource_path(&request_url, &session.clean_path, &session.share_id)
    {
        return Ok(share_unauthorized_with_cookie_clear(true));
    }

    save_share_session(
        state,
        session_id,
        &session,
        config.policy.session_ttl_seconds,
    )
    .await?;
    Ok(share_authorized(
        session_id,
        config.policy.session_ttl_seconds,
    ))
}

fn session_matches_backend(session: &ShareSessionRecord, backend: &FnosBackend) -> bool {
    !session.backend_id.is_empty() && session.backend_id == backend.identity
}

fn not_handled() -> FnosSharePreflightDecision {
    FnosSharePreflightDecision {
        handled: false,
        redirect_location: None,
    }
}

fn share_unauthorized() -> FnosShareAuthorizationResult {
    FnosShareAuthorizationResult {
        authorized: false,
        set_cookies: Vec::new(),
        response_headers: Vec::new(),
    }
}

fn share_unauthorized_with_cookie_clear(clear_cookie: bool) -> FnosShareAuthorizationResult {
    FnosShareAuthorizationResult {
        authorized: false,
        set_cookies: clear_cookie
            .then(|| cookies::fnos_share_clear_cookie(None))
            .into_iter()
            .collect(),
        response_headers: Vec::new(),
    }
}

fn share_redirect_unauthorized(location: &str, clear_cookie: bool) -> FnosShareAuthorizationResult {
    let mut result = FnosShareAuthorizationResult {
        authorized: false,
        set_cookies: Vec::new(),
        response_headers: vec![(
            "X-Reauth-Redirect-Location".to_string(),
            location.to_string(),
        )],
    };
    if clear_cookie {
        result
            .set_cookies
            .push(cookies::fnos_share_clear_cookie(None));
    }
    result
}

fn share_authorized(session_id: &str, ttl_seconds: i64) -> FnosShareAuthorizationResult {
    FnosShareAuthorizationResult {
        authorized: true,
        set_cookies: vec![cookies::fnos_share_session_cookie(
            session_id,
            ttl_seconds,
            None,
        )],
        response_headers: vec![("X-Reauth-Access-Mode".to_string(), "fnos-share".to_string())],
    }
}

fn handled() -> FnosSharePreflightDecision {
    FnosSharePreflightDecision {
        handled: true,
        redirect_location: None,
    }
}

fn handled_redirect(location: &str) -> FnosSharePreflightDecision {
    FnosSharePreflightDecision {
        handled: true,
        redirect_location: Some(location.to_string()),
    }
}

async fn get_resolved_config(
    policy: SharePolicy,
    routed_upstream: Option<&str>,
    routed_upstream_host: Option<&str>,
    routed_upstream_route_id: Option<&str>,
) -> ResolvedFnosShareConfig {
    let backend = resolve_fnos_target(
        routed_upstream,
        routed_upstream_host,
        routed_upstream_route_id,
    )
    .await;
    if backend.is_none() {
        tracing::debug!("FNOS share bypass did not find a matching FNOS routed upstream");
    }
    ResolvedFnosShareConfig { policy, backend }
}

fn share_policy_from_config(config: &Value) -> SharePolicy {
    let normalized = runtime_config::normalize_fnos_share_bypass(config.get("fnos_share_bypass"));
    SharePolicy {
        enabled: normalized
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        upstream_timeout_ms: normalized
            .get("upstream_timeout_ms")
            .and_then(Value::as_i64)
            .unwrap_or(2500)
            .max(1) as u64,
        validation_cache_ttl_seconds: normalized
            .get("validation_cache_ttl_seconds")
            .and_then(Value::as_i64)
            .unwrap_or(30)
            .max(1) as usize,
        validation_lock_ttl_seconds: normalized
            .get("validation_lock_ttl_seconds")
            .and_then(Value::as_i64)
            .unwrap_or(5)
            .max(1) as usize,
        session_ttl_seconds: normalized
            .get("session_ttl_seconds")
            .and_then(Value::as_i64)
            .unwrap_or(300)
            .max(1),
    }
}

fn resolve_enabled_share_request(
    headers: &HeaderMap,
    uri: &Uri,
    app_config: &Value,
) -> Option<(Url, SharePolicy)> {
    let request_url = parse_request_url(&request_target(headers, uri))?;
    if !is_share_path(&request_url) {
        return None;
    }
    let policy = share_policy_from_config(app_config);
    policy.enabled.then_some((request_url, policy))
}

async fn resolve_fnos_target(
    routed_upstream: Option<&str>,
    routed_upstream_host: Option<&str>,
    routed_upstream_route_id: Option<&str>,
) -> Option<FnosBackend> {
    let candidate = resolve_routed_backend(
        routed_upstream,
        routed_upstream_host,
        routed_upstream_route_id,
    )?;
    match get_cached_fnos_target_probe(&candidate.identity) {
        Some(true) => return Some(candidate),
        Some(false) => return None,
        None => {}
    }

    let probe_lock = fnos_target_probe_lock(&candidate.identity);
    let _probe_guard = probe_lock.lock().await;
    match get_cached_fnos_target_probe(&candidate.identity) {
        Some(true) => return Some(candidate),
        Some(false) => return None,
        None => {}
    }

    let probe_result = probe_fnos_target(&candidate).await;
    release_fnos_target_probe_lock(&candidate.identity, &probe_lock);
    if probe_result == Some(true) {
        Some(candidate)
    } else {
        None
    }
}

fn release_fnos_target_probe_lock(identity: &str, probe_lock: &Arc<AsyncMutex<()>>) {
    let Some(locks) = PROBE_LOCKS.get() else {
        return;
    };
    let Ok(mut guard) = locks.lock() else {
        return;
    };
    if guard
        .get(identity)
        .is_some_and(|current| Arc::ptr_eq(current, probe_lock))
    {
        guard.remove(identity);
    }
}

fn fnos_target_probe_lock(origin: &str) -> Arc<AsyncMutex<()>> {
    let locks = PROBE_LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let Ok(mut guard) = locks.lock() else {
        return Arc::new(AsyncMutex::new(()));
    };
    guard
        .entry(origin.to_string())
        .or_insert_with(|| Arc::new(AsyncMutex::new(())))
        .clone()
}

fn resolve_routed_backend(
    routed_upstream: Option<&str>,
    routed_upstream_host: Option<&str>,
    routed_upstream_route_id: Option<&str>,
) -> Option<FnosBackend> {
    let target = routed_upstream?.trim();
    if target.is_empty() {
        return None;
    }
    let host_header = routed_upstream_host?.trim();
    if host_header.is_empty() || HeaderValue::try_from(host_header).is_err() {
        return None;
    }
    let route_id = routed_upstream_route_id?.trim();
    if route_id.is_empty() || route_id.len() > 128 {
        return None;
    }
    let base_url = to_upstream_base_url(target)?;
    let normalized_host = host_header.to_ascii_lowercase();
    let identity = crate::crypto_utils::sha256_hex_str(&format!(
        "{}\n{normalized_host}\n{route_id}",
        base_url.as_str()
    ));
    Some(FnosBackend {
        base_url,
        host_header: host_header.to_string(),
        identity,
    })
}

fn get_cached_fnos_target_probe(origin: &str) -> Option<bool> {
    let cache = PROBE_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = cache.lock().ok()?;
    let record = guard.get(origin)?;
    if record.expires_at <= time_utils::now_ms() {
        guard.remove(origin);
        return None;
    }
    Some(record.is_fnos)
}

fn set_cached_fnos_target_probe(origin: &str, is_fnos: bool) {
    let ttl_ms = if is_fnos {
        FNOS_DETECTION_SUCCESS_CACHE_TTL_MS
    } else {
        FNOS_DETECTION_FAILURE_CACHE_TTL_MS
    };
    let cache = PROBE_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut guard) = cache.lock() {
        let now = time_utils::now_ms();
        guard.retain(|_, record| record.expires_at > now);
        if guard.len() >= FNOS_DETECTION_MAX_CACHE_ENTRIES
            && let Some(oldest) = guard
                .iter()
                .min_by_key(|(_, record)| record.expires_at)
                .map(|(key, _)| key.clone())
        {
            guard.remove(&oldest);
        }
        guard.insert(
            origin.to_string(),
            ProbeCacheRecord {
                is_fnos,
                expires_at: now + ttl_ms,
            },
        );
    }
}

async fn probe_fnos_target(backend: &FnosBackend) -> Option<bool> {
    let target = join_upstream_path(&backend.base_url, FNOS_DETECTION_PATH)?;
    let client = PROBE_CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .danger_accept_invalid_certs(true)
                .timeout(Duration::from_millis(FNOS_DETECTION_TIMEOUT_MS))
                .build()
                .ok()
        })
        .as_ref()?;
    let response = match client
        .get(target)
        .header(reqwest::header::HOST, &backend.host_header)
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
    {
        Ok(response) => response,
        Err(error) => {
            tracing::debug!(%error, backend_id = %backend.identity, "FNOS target probe failed");
            set_cached_fnos_target_probe(&backend.identity, false);
            return Some(false);
        }
    };
    if response.status() != reqwest::StatusCode::OK {
        set_cached_fnos_target_probe(&backend.identity, false);
        return Some(false);
    }
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !content_type.contains("application/json") {
        set_cached_fnos_target_probe(&backend.identity, false);
        return Some(false);
    }
    let payload = match crate::http_body::read_response_json_limited::<Value>(
        response,
        MAX_FNOS_SHARE_RESPONSE_BYTES,
    )
    .await
    {
        Ok(payload) => payload,
        Err(_) => {
            set_cached_fnos_target_probe(&backend.identity, false);
            return Some(false);
        }
    };
    let is_fnos = is_fnos_locale_payload(&payload);
    set_cached_fnos_target_probe(&backend.identity, is_fnos);
    Some(is_fnos)
}

async fn validate_share_link(
    state: &AppState,
    share_id: &str,
    config: &ResolvedFnosShareConfig,
) -> ShareValidationCacheRecord {
    let Some(backend) = config.backend.as_ref() else {
        return unknown_validation(share_id, "");
    };
    let cache_key = validation_cache_key(&backend.identity, share_id);
    if let Ok(Some(cached)) = get_cached_validation(state, &cache_key).await {
        return cached;
    }

    let lock_key = validation_lock_key(&backend.identity, share_id);
    let lock_token = hex::encode(rand::random::<[u8; 12]>());
    let acquired = state
        .store
        .set_key_if_not_exists_with_ttl(
            &lock_key,
            &lock_token,
            config.policy.validation_lock_ttl_seconds,
        )
        .await
        .unwrap_or(false);
    if !acquired {
        let timeout_ms = (config.policy.validation_lock_ttl_seconds as u64 * 1000)
            .min(config.policy.upstream_timeout_ms + 500);
        if let Ok(Some(waited)) = wait_for_cached_validation(state, &cache_key, timeout_ms).await {
            return waited;
        }
        let fallback = fetch_validation(share_id, config).await;
        if fallback.cacheable {
            let _ = cache_validation(state, &cache_key, &fallback.data, config).await;
        }
        return fallback.data;
    }

    let fresh = fetch_validation(share_id, config).await;
    if fresh.cacheable {
        let _ = cache_validation(state, &cache_key, &fresh.data, config).await;
    }
    let _ = state
        .store
        .delete_key_if_value(&lock_key, &lock_token)
        .await;
    fresh.data
}

async fn fetch_validation(
    share_id: &str,
    config: &ResolvedFnosShareConfig,
) -> ShareValidationFetchResult {
    let Some(backend) = config.backend.as_ref() else {
        return ShareValidationFetchResult {
            cacheable: false,
            data: unknown_validation(share_id, ""),
        };
    };
    let clean_path = format!("/s/{share_id}");
    let fallback = unknown_validation(share_id, &backend.identity);
    let Some(target_url) = join_upstream_path(&backend.base_url, &clean_path) else {
        return ShareValidationFetchResult {
            cacheable: false,
            data: fallback,
        };
    };
    let client = match reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(Duration::from_millis(config.policy.upstream_timeout_ms))
        .build()
    {
        Ok(client) => client,
        Err(_) => {
            return ShareValidationFetchResult {
                cacheable: false,
                data: fallback,
            };
        }
    };
    let response = match client
        .get(target_url)
        .header(reqwest::header::HOST, &backend.host_header)
        .header(reqwest::header::ACCEPT, "text/html,application/xhtml+xml")
        .send()
        .await
    {
        Ok(response) => response,
        Err(error) => {
            tracing::warn!(%error, %share_id, "FNOS share validation failed");
            return ShareValidationFetchResult {
                cacheable: false,
                data: fallback,
            };
        }
    };
    let html =
        crate::http_body::read_response_text_limited(response, MAX_FNOS_SHARE_RESPONSE_BYTES)
            .await
            .unwrap_or_default();
    match parse_share_data(&html, share_id, &backend.identity) {
        Some(data) => ShareValidationFetchResult {
            cacheable: true,
            data,
        },
        None => ShareValidationFetchResult {
            cacheable: false,
            data: fallback,
        },
    }
}

fn unknown_validation(share_id: &str, backend_id: &str) -> ShareValidationCacheRecord {
    ShareValidationCacheRecord {
        version: 2,
        valid: false,
        validation_state: "unknown".to_string(),
        share_id: share_id.to_string(),
        backend_id: backend_id.to_string(),
        clean_path: format!("/s/{share_id}"),
        token: None,
        name: None,
        kind: None,
        checked_at: time_utils::now_iso(),
    }
}

async fn cache_validation(
    state: &AppState,
    key: &str,
    value: &ShareValidationCacheRecord,
    config: &ResolvedFnosShareConfig,
) -> anyhow::Result<()> {
    state
        .store
        .set_json_value_ex(
            key,
            &serde_json::to_value(value).unwrap_or_else(|_| json!({})),
            config.policy.validation_cache_ttl_seconds,
        )
        .await?;
    Ok(())
}

async fn get_cached_validation(
    state: &AppState,
    key: &str,
) -> anyhow::Result<Option<ShareValidationCacheRecord>> {
    Ok(state
        .store
        .get_json_value(key)
        .await?
        .and_then(|value| serde_json::from_value::<ShareValidationCacheRecord>(value).ok())
        .map(normalize_share_validation))
}

async fn wait_for_cached_validation(
    state: &AppState,
    key: &str,
    timeout_ms: u64,
) -> anyhow::Result<Option<ShareValidationCacheRecord>> {
    let deadline = time_utils::now_ms() + timeout_ms.max(100) as i64;
    while time_utils::now_ms() < deadline {
        if let Some(cached) = get_cached_validation(state, key).await? {
            return Ok(Some(cached));
        }
        sleep(Duration::from_millis(80)).await;
    }
    Ok(None)
}

async fn get_share_session(
    state: &AppState,
    session_id: &str,
) -> anyhow::Result<Option<ShareSessionRecord>> {
    Ok(state
        .store
        .get_json_value(&share_session_key(session_id))
        .await?
        .and_then(|value| serde_json::from_value::<ShareSessionRecord>(value).ok())
        .map(normalize_share_session))
}

async fn save_share_session(
    state: &AppState,
    session_id: &str,
    session: &ShareSessionRecord,
    ttl_seconds: i64,
) -> anyhow::Result<()> {
    let mut next = session.clone();
    next.version = 2;
    if next.issued_at.trim().is_empty() {
        next.issued_at = time_utils::now_iso();
    }
    next.last_seen_at = time_utils::now_iso();
    state
        .store
        .set_json_value_ex(
            &share_session_key(session_id),
            &serde_json::to_value(next).unwrap_or_else(|_| json!({})),
            ttl_seconds.max(1) as usize,
        )
        .await?;
    Ok(())
}

fn normalize_share_session(value: ShareSessionRecord) -> ShareSessionRecord {
    ShareSessionRecord {
        version: 2,
        share_id: value.share_id,
        backend_id: value.backend_id,
        clean_path: value.clean_path,
        token: value.token.filter(|value| !value.trim().is_empty()),
        name: value.name.filter(|value| !value.trim().is_empty()),
        kind: value.kind,
        issued_at: value.issued_at,
        last_seen_at: value.last_seen_at,
    }
}

fn read_share_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    let cookie_header = headers.get(header::COOKIE)?.to_str().ok()?;
    for segment in cookie_header.split(';') {
        let (raw_key, raw_value) = match segment.split_once('=') {
            Some(pair) => pair,
            None => continue,
        };
        if raw_key.trim() != name {
            continue;
        }
        let value = raw_value.trim().trim_matches('"');
        if value.is_empty() {
            continue;
        }
        return Some(cookies::percent_decode(value));
    }
    None
}

fn parse_share_data(
    html: &str,
    share_id: &str,
    backend_id: &str,
) -> Option<ShareValidationCacheRecord> {
    let lower = html.to_ascii_lowercase();
    let mut offset = 0_usize;
    while let Some(relative_start) = lower[offset..].find("<script") {
        let start = offset + relative_start;
        let tag_end = start + lower[start..].find('>')?;
        let tag = &lower[start..=tag_end];
        let content_start = tag_end + 1;
        let close = content_start + lower[content_start..].find("</script>")?;
        offset = close + "</script>".len();
        if !(tag.contains("id=\"share-data\"") || tag.contains("id='share-data'")) {
            continue;
        }
        let payload = serde_json::from_str::<Value>(&html[content_start..close]).ok()?;
        let code = payload.get("code").and_then(Value::as_i64);
        let data = payload.get("data").unwrap_or(&Value::Null);
        let token = string_field(data, "token");
        let name = string_field(data, "name");
        let kind = data.get("type").and_then(Value::as_i64);
        let has_usable_token = code == Some(0) && token.is_some();
        let requires_password = code == Some(FNOS_SHARE_NEED_PASSWORD_CODE);
        return Some(normalize_share_validation(ShareValidationCacheRecord {
            version: 2,
            valid: has_usable_token || requires_password,
            validation_state: if has_usable_token {
                "valid"
            } else if requires_password {
                "password_required"
            } else {
                "invalid"
            }
            .to_string(),
            share_id: share_id.to_string(),
            backend_id: backend_id.to_string(),
            clean_path: format!("/s/{share_id}"),
            token,
            name,
            kind,
            checked_at: time_utils::now_iso(),
        }));
    }
    None
}

fn normalize_share_validation(value: ShareValidationCacheRecord) -> ShareValidationCacheRecord {
    let validation_state = match value.validation_state.as_str() {
        "valid" | "password_required" | "invalid" | "unknown" => value.validation_state,
        _ if value.valid => "valid".to_string(),
        _ => "invalid".to_string(),
    };
    ShareValidationCacheRecord {
        version: 2,
        valid: value.valid,
        validation_state,
        share_id: value.share_id,
        backend_id: value.backend_id,
        clean_path: value.clean_path,
        token: value.token.filter(|value| !value.trim().is_empty()),
        name: value.name.filter(|value| !value.trim().is_empty()),
        kind: value.kind,
        checked_at: value.checked_at,
    }
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn request_target(headers: &HeaderMap, uri: &Uri) -> String {
    headers
        .get("x-forwarded-path")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .or_else(|| uri.path_and_query().map(|value| value.as_str().to_string()))
        .unwrap_or_else(|| "/".to_string())
}

fn parse_request_url(raw_path: &str) -> Option<Url> {
    if !raw_path.starts_with('/') {
        return None;
    }
    let base = Url::parse("http://127.0.0.1").ok()?;
    Url::options().base_url(Some(&base)).parse(raw_path).ok()
}

fn extract_share_entry(request_url: &Url) -> Option<ShareEntry> {
    let path = request_url.path();
    let share_id = path.strip_prefix("/s/")?;
    if share_id.len() != SHARE_ENTRY_ID_LENGTH
        || !share_id
            .chars()
            .all(|value| value.is_ascii_lowercase() || value.is_ascii_digit())
    {
        return None;
    }
    let clean_path = format!("/s/{share_id}");
    Some(ShareEntry {
        share_id: share_id.to_string(),
        is_clean: request_url.path() == clean_path && request_url.query().is_none(),
        clean_path,
    })
}

fn is_share_path(request_url: &Url) -> bool {
    request_url.path() == "/s" || request_url.path().starts_with("/s/")
}

fn is_session_resource_path(request_url: &Url, clean_path: &str, share_id: &str) -> bool {
    let pathname = request_url.path();
    let preview_path = format!("/s/preview/{share_id}");
    let thumb_path = format!("/s/thumb/{share_id}");
    pathname.starts_with(&format!("{clean_path}/"))
        || pathname.starts_with("/s/static/")
        || pathname.starts_with("/s/busstatic/")
        || pathname.starts_with("/s/download/")
        || pathname == preview_path
        || pathname.starts_with(&format!("{preview_path}/"))
        || pathname == thumb_path
        || pathname.starts_with(&format!("{thumb_path}/"))
}

fn to_upstream_base_url(target: &str) -> Option<Url> {
    let mut parsed = Url::parse(target.trim()).ok()?;
    match parsed.scheme() {
        "http" | "https" => {}
        "ws" => {
            parsed.set_scheme("http").ok()?;
        }
        "wss" => {
            parsed.set_scheme("https").ok()?;
        }
        _ => return None,
    }
    parsed.set_fragment(None);
    let mut base_path = parsed.path().trim_end_matches('/').to_string();
    base_path.push('/');
    parsed.set_path(&base_path);
    Some(parsed)
}

fn join_upstream_path(base_url: &Url, path: &str) -> Option<Url> {
    let base_query = base_url.query().map(ToString::to_string);
    let mut target = base_url.join(path.trim_start_matches('/')).ok()?;
    target.set_query(base_query.as_deref());
    Some(target)
}

fn is_fnos_locale_payload(value: &Value) -> bool {
    let Some(app) = value.get("app").and_then(Value::as_object) else {
        return false;
    };
    let Some(app_api_errors) = value.get("appApiErrors").and_then(Value::as_object) else {
        return false;
    };
    let matched = FNOS_DETECTION_APP_KEYS
        .iter()
        .filter(|key| {
            app.get(**key)
                .and_then(Value::as_str)
                .is_some_and(|value| !value.trim().is_empty())
        })
        .count();
    matched >= FNOS_DETECTION_MIN_MATCHED_APP_KEYS
        && app_api_errors
            .get("AuthFailed")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty())
}

fn validation_cache_key(backend_id: &str, share_id: &str) -> String {
    format!("{CACHE_KEY_PREFIX}{backend_id}:{share_id}")
}

fn validation_lock_key(backend_id: &str, share_id: &str) -> String {
    format!("{LOCK_KEY_PREFIX}{backend_id}:{share_id}")
}

fn share_session_key(session_id: &str) -> String {
    format!("{SESSION_KEY_PREFIX}{session_id}")
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderValue, header};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    use super::*;

    #[test]
    fn extracts_clean_share_entries_like_node() {
        let url = parse_request_url("/s/abc123abc123abc123").unwrap();
        let entry = extract_share_entry(&url).unwrap();
        assert_eq!(entry.share_id, "abc123abc123abc123");
        assert!(entry.is_clean);

        let dirty = parse_request_url("/s/abc123abc123abc123?x=1").unwrap();
        assert!(!extract_share_entry(&dirty).unwrap().is_clean);
    }

    #[test]
    fn parses_share_data_script_payload() {
        let parsed = parse_share_data(
            r#"<html><script id="share-data">{"code":0,"data":{"token":" t ","name":"n","type":1}}</script></html>"#,
            "abc123abc123abc123",
            "backend-a",
        )
        .unwrap();
        assert!(parsed.valid);
        assert_eq!(parsed.validation_state, "valid");
        assert_eq!(parsed.token.as_deref(), Some("t"));
    }

    #[test]
    fn recognizes_fnos_locale_payload() {
        let payload = json!({
            "app": {
                "account": "Account",
                "docker": "Docker",
                "fileManager": "Files",
                "photos": "Photos"
            },
            "appApiErrors": { "AuthFailed": "auth failed" }
        });
        assert!(is_fnos_locale_payload(&payload));
    }

    #[test]
    fn ordinary_and_disabled_requests_never_enter_fnos_detection() {
        let headers = HeaderMap::new();
        let enabled = json!({
            "fnos_share_bypass": { "enabled": true },
            "host_mappings": [{
                "host": "offline.example.com",
                "target": "http://192.0.2.1:5666"
            }]
        });
        assert!(
            resolve_enabled_share_request(&headers, &Uri::from_static("/"), &enabled).is_none()
        );

        let disabled = json!({
            "fnos_share_bypass": { "enabled": false },
            "host_mappings": [{
                "host": "offline.example.com",
                "target": "http://192.0.2.1:5666"
            }]
        });
        assert!(
            resolve_enabled_share_request(
                &headers,
                &Uri::from_static("/s/abc123abc123abc123"),
                &disabled,
            )
            .is_none()
        );
    }

    #[tokio::test]
    async fn ordinary_preflight_with_offline_mappings_returns_without_probe_delay() {
        let directory = tempfile::tempdir().expect("temporary auth database");
        let mut settings = {
            let _environment = crate::test_support::EnvGuard::new(&[]);
            crate::settings::Settings::from_env()
        };
        settings.data_dir = directory.path().join("data");
        settings.gateway_config_dir = directory.path().join("gateway");
        settings.sqlite_path = directory.path().join("fn-knock.sqlite3");
        settings.legacy_redis_url = String::new();
        settings.internal_rpc_token = "fnos-share-fast-path-test".to_string();
        let state = AppState::new(settings).await.expect("auth test state");
        let config = json!({
            "fnos_share_bypass": { "enabled": true },
            "host_mappings": [{
                "host": "offline.example.com",
                "target": "http://192.0.2.1:5666"
            }]
        });

        let decision = tokio::time::timeout(
            Duration::from_millis(100),
            resolve_preflight(
                &state,
                &HeaderMap::new(),
                &Uri::from_static("/"),
                &config,
                None,
                None,
                None,
            ),
        )
        .await
        .expect("ordinary preflight must not wait for FNOS detection")
        .expect("ordinary preflight");
        assert!(!decision.handled);

        let share_decision = tokio::time::timeout(
            Duration::from_millis(100),
            resolve_preflight(
                &state,
                &HeaderMap::new(),
                &Uri::from_static("/s/abc123abc123abc123"),
                &config,
                None,
                None,
                None,
            ),
        )
        .await
        .expect("missing trusted route metadata must not trigger configuration probing")
        .expect("share preflight without routed backend");
        assert!(!share_decision.handled);
    }

    #[test]
    fn routed_backend_requires_target_host_and_route_identity() {
        let resolved = resolve_routed_backend(
            Some(" http://10.0.0.9:8000/base "),
            Some("NAS.EXAMPLE.COM"),
            Some("route-generation-a"),
        )
        .unwrap();
        assert_eq!(resolved.base_url.as_str(), "http://10.0.0.9:8000/base/");
        assert_eq!(resolved.host_header, "NAS.EXAMPLE.COM");

        assert!(
            resolve_routed_backend(
                Some("http://10.0.0.9:8000"),
                None,
                Some("route-generation-a")
            )
            .is_none(),
            "missing routed Host metadata must fail closed"
        );
        assert!(
            resolve_routed_backend(None, None, None).is_none(),
            "missing routed target must not fall back to configuration guessing"
        );
        assert!(
            resolve_routed_backend(Some("http://10.0.0.9:8000"), Some("NAS.EXAMPLE.COM"), None)
                .is_none(),
            "missing route lifecycle identity must fail closed"
        );
        assert!(
            resolve_routed_backend(
                Some("http://192.0.2.10:7997"),
                Some("NAS.EXAMPLE.COM"),
                Some("route-generation-auth-port"),
            )
            .is_some(),
            "a remote FNOS target may legitimately use the auth service port number"
        );
    }

    #[tokio::test]
    async fn target_probe_preserves_base_path_and_actual_host() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = [0_u8; 4096];
            let read = socket.read(&mut buffer).await.unwrap();
            let request = String::from_utf8_lossy(&buffer[..read]).to_ascii_lowercase();
            assert!(request.starts_with("get /fnos/locales/zh-cn/os.json "));
            assert!(request.contains("\r\nhost: nas.example.com\r\n"));
            let body = r#"{"app":{"account":"a","docker":"d","fileManager":"f","photos":"p"},"appApiErrors":{"AuthFailed":"failed"}}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        });
        let backend = resolve_routed_backend(
            Some(&format!("http://{address}/fnos")),
            Some("nas.example.com"),
            Some("route-generation-a"),
        )
        .unwrap();

        assert_eq!(probe_fnos_target(&backend).await, Some(true));
        server.await.unwrap();
    }

    #[test]
    fn validation_and_sessions_are_scoped_to_the_backend() {
        assert_ne!(
            validation_cache_key("backend-a", "abc123abc123abc123"),
            validation_cache_key("backend-b", "abc123abc123abc123")
        );
        assert_ne!(
            validation_lock_key("backend-a", "abc123abc123abc123"),
            validation_lock_key("backend-b", "abc123abc123abc123")
        );

        let backend = resolve_routed_backend(
            Some("http://10.0.0.8:5666"),
            Some("nas.example.com"),
            Some("route-generation-a"),
        )
        .unwrap();
        let readded_backend = resolve_routed_backend(
            Some("http://10.0.0.8:5666"),
            Some("nas.example.com"),
            Some("route-generation-b"),
        )
        .unwrap();
        assert_ne!(
            backend.identity, readded_backend.identity,
            "re-added mapping must not reuse the previous backend identity"
        );
        let mut session = ShareSessionRecord {
            version: 2,
            share_id: "abc123abc123abc123".to_string(),
            backend_id: backend.identity.clone(),
            clean_path: "/s/abc123abc123abc123".to_string(),
            token: None,
            name: None,
            kind: None,
            issued_at: "issued".to_string(),
            last_seen_at: "seen".to_string(),
        };
        assert!(session_matches_backend(&session, &backend));
        session.backend_id = "another-backend".to_string();
        assert!(!session_matches_backend(&session, &backend));
    }

    #[tokio::test]
    async fn target_probe_connection_failures_are_negative_cached() {
        let backend = resolve_routed_backend(
            Some("http://127.0.0.1:0"),
            Some("nas.example.com"),
            Some("route-generation-negative-cache"),
        )
        .unwrap();
        if let Some(cache) = PROBE_CACHE.get()
            && let Ok(mut guard) = cache.lock()
        {
            guard.remove(&backend.identity);
        }

        assert_eq!(probe_fnos_target(&backend).await, Some(false));
        assert_eq!(get_cached_fnos_target_probe(&backend.identity), Some(false));
    }

    #[test]
    fn share_authorization_results_match_node_headers_and_cookies() {
        let authorized = share_authorized("share-session", 300);
        assert!(authorized.authorized);
        assert_eq!(
            authorized.response_headers,
            vec![("X-Reauth-Access-Mode".to_string(), "fnos-share".to_string())]
        );
        let cookie = authorized.set_cookies.first().unwrap();
        assert!(cookie.contains("fn-knock-fnos-share-session=share-session"));
        assert!(cookie.contains("Path=/s"));
        assert!(cookie.contains("Max-Age=300"));

        let denied = share_redirect_unauthorized("/", true);
        assert!(!denied.authorized);
        assert_eq!(
            denied.response_headers,
            vec![("X-Reauth-Redirect-Location".to_string(), "/".to_string())]
        );
        assert!(denied.set_cookies.first().unwrap().contains("Max-Age=0"));
    }

    #[test]
    fn reads_share_cookie_like_node() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_str(
                "other=x; fn-knock-fnos-share-session=; fn-knock-fnos-share-session=\"share%201\"",
            )
            .unwrap(),
        );

        assert_eq!(
            read_share_cookie(&headers, cookies::FNOS_SHARE_SESSION_COOKIE_NAME).as_deref(),
            Some("share 1")
        );
    }

    #[test]
    fn normalizes_share_session_metadata_like_node() {
        let session = normalize_share_session(ShareSessionRecord {
            version: 0,
            share_id: "abc123abc123abc123".to_string(),
            backend_id: "backend-a".to_string(),
            clean_path: "/s/abc123abc123abc123".to_string(),
            token: Some(" token ".to_string()),
            name: Some(" ".to_string()),
            kind: Some(1),
            issued_at: "issued".to_string(),
            last_seen_at: "seen".to_string(),
        });

        assert_eq!(session.version, 2);
        assert_eq!(session.backend_id, "backend-a");
        assert_eq!(session.token.as_deref(), Some(" token "));
        assert_eq!(session.name, None);
        assert_eq!(session.kind, Some(1));
        assert_eq!(session.issued_at, "issued");
        assert_eq!(session.last_seen_at, "seen");
    }

    #[test]
    fn normalizes_cached_share_validation_metadata_like_node() {
        let validation = normalize_share_validation(ShareValidationCacheRecord {
            version: 0,
            valid: true,
            validation_state: "other".to_string(),
            share_id: "abc123abc123abc123".to_string(),
            backend_id: "backend-a".to_string(),
            clean_path: "/s/abc123abc123abc123".to_string(),
            token: Some(" token ".to_string()),
            name: Some(" ".to_string()),
            kind: Some(1),
            checked_at: "checked".to_string(),
        });

        assert_eq!(validation.version, 2);
        assert_eq!(validation.backend_id, "backend-a");
        assert_eq!(validation.validation_state, "valid");
        assert_eq!(validation.token.as_deref(), Some(" token "));
        assert_eq!(validation.name, None);
    }
}
