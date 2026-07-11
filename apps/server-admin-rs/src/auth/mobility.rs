use std::{
    collections::BTreeSet,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use serde_json::{Map, Value, json};
use tokio::time::{self, MissedTickBehavior};

use crate::{
    auth_mobility_keys::{
        session_mutation_lock_key as auth_mobility_session_mutation_lock_key,
        subject_hash as auth_mobility_subject_hash,
    },
    http_utils::normalize_ip,
    i18n::{DEFAULT_LOCALE, Translator},
    ip_location,
    state::AppState,
    store::{LoginSession, TotpCredential},
    system_events, time_utils, whitelist,
};

const MAX_SESSION_ACTIVE_IPS: usize = 32;
const DEFAULT_SESSION_TTL_SECONDS: i64 = 24 * 3600;
const DEFAULT_REMEMBER_ME_TTL_SECONDS: i64 = 365 * 24 * 3600;
const DEFAULT_POST_LOGIN_IP_GRANT_TTL_SECONDS: i64 = 3600;
const DEFAULT_SESSION_IP_MOBILITY_WINDOW_SECONDS: i64 = 20 * 60;
const MAX_AUTH_TTL_SECONDS: i64 = 5 * 365 * 24 * 3600;
const AUTH_MOBILITY_MAINTENANCE_INTERVAL_SECONDS: u64 = 60;
// Activity timestamps are housekeeping data, not authorization state. Coalescing
// identical touches keeps segmented media requests from rewriting the same
// session/binding/active-IP records for every segment while still refreshing at
// least twice inside the minimum (60 second) mobility window.
const AUTH_ACTIVITY_TOUCH_MIN_INTERVAL_SECONDS: i64 = 30;
const AUTH_MOBILITY_SESSION_LOCK_TTL_SECONDS: usize = 120;
const AUTH_MOBILITY_SESSION_LOCK_WAIT_SECONDS: u64 = 10;

struct AuthMobilitySessionMutationLease {
    state: AppState,
    key: String,
    lock_id: String,
    valid: Arc<AtomicBool>,
    heartbeat: Option<tokio::task::JoinHandle<()>>,
}

async fn acquire_auth_mobility_session_mutation_lease(
    state: &AppState,
    session_id: &str,
) -> anyhow::Result<Option<AuthMobilitySessionMutationLease>> {
    let key = auth_mobility_session_mutation_lock_key(session_id);
    let lock_id = uuid::Uuid::new_v4().to_string();
    let deadline = time::Instant::now()
        + std::time::Duration::from_secs(AUTH_MOBILITY_SESSION_LOCK_WAIT_SECONDS);
    loop {
        if state.shutdown.is_cancelled() {
            return Ok(None);
        }
        if state
            .store
            .set_json_value_nx_ex(
                &key,
                &json!({
                    "lockId": lock_id,
                    "sessionId": session_id,
                    "createdAt": time_utils::now_iso(),
                }),
                AUTH_MOBILITY_SESSION_LOCK_TTL_SECONDS,
            )
            .await?
        {
            let heartbeat_state = state.clone();
            let heartbeat_key = key.clone();
            let heartbeat_lock_id = lock_id.clone();
            let valid = Arc::new(AtomicBool::new(true));
            let heartbeat_valid = Arc::clone(&valid);
            let heartbeat = tokio::spawn(async move {
                let mut interval = time::interval(std::time::Duration::from_secs(
                    (AUTH_MOBILITY_SESSION_LOCK_TTL_SECONDS as u64 / 3).max(1),
                ));
                interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
                interval.tick().await;
                loop {
                    tokio::select! {
                        _ = heartbeat_state.shutdown.cancelled() => {
                            heartbeat_valid.store(false, Ordering::Release);
                            break;
                        }
                        _ = interval.tick() => {}
                    }
                    let heartbeat_payload = json!({
                        "lockId": heartbeat_lock_id,
                        "refreshedAt": time_utils::now_iso(),
                    });
                    let refreshed = tokio::select! {
                        _ = heartbeat_state.shutdown.cancelled() => {
                            heartbeat_valid.store(false, Ordering::Release);
                            break;
                        }
                        result = heartbeat_state.store.set_json_lock_if_owned_ex(
                            &heartbeat_key,
                            &heartbeat_lock_id,
                            &heartbeat_payload,
                            AUTH_MOBILITY_SESSION_LOCK_TTL_SECONDS,
                        ) => result,
                    };
                    if !matches!(refreshed, Ok(true)) {
                        heartbeat_valid.store(false, Ordering::Release);
                        break;
                    }
                }
            });
            return Ok(Some(AuthMobilitySessionMutationLease {
                state: state.clone(),
                key,
                lock_id,
                valid,
                heartbeat: Some(heartbeat),
            }));
        }
        if time::Instant::now() >= deadline {
            return Ok(None);
        }
        tokio::select! {
            _ = state.shutdown.cancelled() => return Ok(None),
            _ = time::sleep(std::time::Duration::from_millis(10)) => {}
        }
    }
}

impl AuthMobilitySessionMutationLease {
    async fn ensure_valid(&self) -> anyhow::Result<bool> {
        if !self.valid.load(Ordering::Acquire) {
            return Ok(false);
        }
        let refreshed = self
            .state
            .store
            .set_json_lock_if_owned_ex(
                &self.key,
                &self.lock_id,
                &json!({
                    "lockId": self.lock_id,
                    "refreshedAt": time_utils::now_iso(),
                }),
                AUTH_MOBILITY_SESSION_LOCK_TTL_SECONDS,
            )
            .await?;
        if !refreshed {
            self.valid.store(false, Ordering::Release);
        }
        Ok(refreshed)
    }

    async fn release(mut self) -> anyhow::Result<bool> {
        if let Some(heartbeat) = self.heartbeat.take() {
            heartbeat.abort();
        }
        self.state
            .store
            .delete_lock_if_owned(&self.key, &self.lock_id)
            .await
            .map_err(Into::into)
    }
}

impl Drop for AuthMobilitySessionMutationLease {
    fn drop(&mut self) {
        if let Some(heartbeat) = self.heartbeat.take() {
            heartbeat.abort();
        }
    }
}

#[derive(Clone, Debug)]
pub struct CreateLoginSessionInput {
    pub auth_method: String,
    pub auth_provider_name: Option<String>,
    pub credential_id: String,
    pub credential_name: String,
    pub totp_id: String,
    pub linked_totp_name: Option<String>,
    pub totp_credential: Option<TotpCredential>,
    pub client_ip: String,
    pub user_agent: String,
    pub remember_me: bool,
}

#[derive(Clone, Debug)]
pub struct CreatedLoginSession {
    pub session_id: String,
    pub ttl_seconds: i64,
    pub grant_type: String,
    pub expires_at: String,
    pub whitelist_record_id: Option<String>,
    pub post_login_ip_grant_mode: Option<String>,
    pub session_comment: Option<String>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct AuthMobilityRestoreIdentity<'a> {
    pub session_id: Option<&'a str>,
    pub fnos_token: Option<&'a str>,
    pub trim_media_token: Option<&'a str>,
    pub app_binding: Option<&'a str>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct AuthMobilityRestoreResult {
    pub success: bool,
    pub grant_type: Option<&'static str>,
}

#[derive(Clone, Debug)]
struct AuthCredentialSettings {
    session_ttl_seconds: i64,
    remember_me_ttl_seconds: i64,
    post_login_ip_grant_mode: String,
    post_login_ip_grant_ttl_seconds: i64,
    session_ip_mobility_enabled: bool,
    session_ip_mobility_window_seconds: i64,
}

pub fn start_auth_mobility_tasks(state: AppState) {
    tokio::spawn(async move {
        let mut ticker = time::interval(std::time::Duration::from_secs(
            AUTH_MOBILITY_MAINTENANCE_INTERVAL_SECONDS,
        ));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        ticker.tick().await;
        loop {
            tokio::select! {
                _ = state.shutdown.cancelled() => break,
                _ = ticker.tick() => {}
            }
            tokio::select! {
                _ = state.shutdown.cancelled() => break,
                result = maintain_session_active_ips(&state) => {
                    if let Err(error) = result {
                        tracing::warn!(%error, "auth mobility active IP maintenance failed");
                    }
                }
            }
        }
    });
}

struct RecordSessionActiveIpArgs<'a> {
    state: &'a AppState,
    session_id: &'a str,
    session: Option<&'a LoginSession>,
    client_ip: &'a str,
    source: &'a str,
    ip_location: Option<&'a str>,
    whitelist_record_id: Option<&'a str>,
    settings: Option<&'a AuthCredentialSettings>,
    sync_reason: &'a str,
    schedule_sync: bool,
}

struct PruneOptions<'a> {
    keep_ip: Option<&'a str>,
    schedule_sync: bool,
}

impl AuthCredentialSettings {
    fn from_config(config: &Value) -> Self {
        let raw = config
            .get("auth_credential_settings")
            .unwrap_or(&Value::Null);
        Self::from_raw_with_legacy(raw, legacy_auto_add_whitelist_on_login(config))
    }

    fn from_raw(raw: &Value) -> Self {
        Self::from_raw_with_legacy(raw, None)
    }

    fn from_raw_with_legacy(raw: &Value, legacy_auto_add_whitelist_on_login: Option<bool>) -> Self {
        let session_ttl_seconds = bounded_int_like_node(
            raw,
            "session_ttl_seconds",
            DEFAULT_SESSION_TTL_SECONDS,
            60,
            MAX_AUTH_TTL_SECONDS,
        );
        let remember_me_ttl_seconds = bounded_int_like_node(
            raw,
            "remember_me_ttl_seconds",
            DEFAULT_REMEMBER_ME_TTL_SECONDS,
            session_ttl_seconds,
            MAX_AUTH_TTL_SECONDS,
        );
        let post_login_ip_grant_mode =
            match raw.get("post_login_ip_grant_mode").and_then(Value::as_str) {
                Some("disabled") => "disabled",
                Some("custom") => "custom",
                Some("follow_session") => "follow_session",
                _ if legacy_auto_add_whitelist_on_login == Some(false) => "disabled",
                _ => "follow_session",
            }
            .to_string();
        let post_login_ip_grant_ttl_seconds = if post_login_ip_grant_mode == "custom" {
            bounded_int_like_node(
                raw,
                "post_login_ip_grant_ttl_seconds",
                DEFAULT_POST_LOGIN_IP_GRANT_TTL_SECONDS,
                60,
                MAX_AUTH_TTL_SECONDS,
            )
        } else {
            DEFAULT_POST_LOGIN_IP_GRANT_TTL_SECONDS
        };
        let session_ip_mobility_window_seconds = bounded_int_like_node(
            raw,
            "session_ip_mobility_window_seconds",
            DEFAULT_SESSION_IP_MOBILITY_WINDOW_SECONDS,
            60,
            24 * 3600,
        );
        Self {
            session_ttl_seconds,
            remember_me_ttl_seconds,
            post_login_ip_grant_mode,
            post_login_ip_grant_ttl_seconds,
            session_ip_mobility_enabled: raw
                .get("session_ip_mobility_enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            session_ip_mobility_window_seconds,
        }
    }
}

fn legacy_auto_add_whitelist_on_login(config: &Value) -> Option<bool> {
    config
        .pointer("/subdomain_mode/auto_add_whitelist_on_login")
        .and_then(Value::as_bool)
}

fn bounded_int_like_node(value: &Value, key: &str, fallback: i64, min: i64, max: i64) -> i64 {
    value
        .get(key)
        .and_then(parse_int_like_node)
        .unwrap_or(fallback)
        .clamp(min, max)
}

fn parse_int_like_node(value: &Value) -> Option<i64> {
    let raw = match value {
        Value::Null => return None,
        Value::Bool(true) => "true".to_string(),
        Value::Bool(false) => "false".to_string(),
        Value::Number(number) => number.to_string(),
        Value::String(value) => value.clone(),
        Value::Array(_) | Value::Object(_) => return None,
    };
    crate::node_compat::parse_i64_prefix_trim_start(&raw)
}

fn auto_ip_grant_comment(config: &Value) -> String {
    let locale = config
        .pointer("/locale/default_locale")
        .and_then(Value::as_str)
        .unwrap_or(DEFAULT_LOCALE);
    Translator::new(locale).t("auth.autoIpGrantComment")
}

fn normalize_auto_ip_grant_comment(value: Option<&str>, config: &Value) -> Option<String> {
    let trimmed = value?.trim();
    if trimmed.is_empty() {
        return None;
    }
    if is_auto_ip_grant_comment(trimmed) {
        Some(auto_ip_grant_comment(config))
    } else {
        Some(trimmed.to_string())
    }
}

pub(crate) fn is_auto_ip_grant_comment(value: &str) -> bool {
    matches!(
        value.trim(),
        "登录后自动授权"
            | "登入後自動授權"
            | "Automatically authorized after sign-in"
            | "로그인 후 자동 승인됨"
            | "ログイン後自動認証"
            | "server.auth.autoIpGrantComment"
    )
}

fn is_totp_subdomain_access_restricted(value: &Value) -> bool {
    value.get("mode").and_then(Value::as_str) == Some("custom")
}

fn is_follow_session_auto_grant(session: &LoginSession) -> bool {
    session.grant_type.as_deref() == Some("login_ip_grant")
        && session.post_login_ip_grant_mode.as_deref() == Some("follow_session")
}

fn binding_owner_session_id(value: &Value) -> Option<String> {
    value
        .get("ownerSessionId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn binding_whitelist_record_id(value: &Value) -> Option<String> {
    value
        .get("whitelistRecordId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn mobility_touch_is_fresh(last_seen_at: i64, now: i64, interval_seconds: i64) -> bool {
    last_seen_at > 0 && now >= last_seen_at && now - last_seen_at < interval_seconds.max(1)
}

fn mobility_binding_touch_is_fresh(
    binding: &Value,
    current_ip: &str,
    owner_session_id: &str,
    now: i64,
) -> bool {
    binding_owner_session_id(binding).as_deref() == Some(owner_session_id)
        && binding
            .get("currentIp")
            .and_then(Value::as_str)
            .is_some_and(|value| {
                normalized_or_trimmed_ip(value) == normalized_or_trimmed_ip(current_ip)
            })
        && mobility_touch_is_fresh(
            parse_iso_unix(binding.get("lastSeenAt").and_then(Value::as_str)).unwrap_or_default(),
            now,
            AUTH_ACTIVITY_TOUCH_MIN_INTERVAL_SECONDS,
        )
}

fn clear_binding_owner_session(value: &mut Value) {
    if let Some(object) = value.as_object_mut() {
        object.remove("ownerSessionId");
    }
}

fn set_binding_last_seen(value: &mut Value) {
    if let Some(object) = value.as_object_mut() {
        object.insert("lastSeenAt".to_string(), json!(time_utils::now_iso()));
    }
}

fn build_or_update_mobility_binding(
    existing: Option<Value>,
    subject_type: &str,
    subject_key: &str,
    current_ip: &str,
    expire_at: Option<i64>,
    owner_session_id: Option<&str>,
    whitelist_record_id: Option<String>,
) -> Value {
    let now_iso = time_utils::now_iso();
    let mut object: Map<String, Value> = existing
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    object.insert("version".to_string(), json!(1));
    object.insert(
        "subjectType".to_string(),
        Value::String(subject_type.to_string()),
    );
    object.insert(
        "subjectHash".to_string(),
        Value::String(auth_mobility_subject_hash(subject_type, subject_key)),
    );
    object.insert(
        "currentIp".to_string(),
        Value::String(current_ip.to_string()),
    );
    object.insert(
        "expireAt".to_string(),
        expire_at.map_or(Value::Null, |value| json!(value)),
    );
    object
        .entry("createdAt".to_string())
        .or_insert_with(|| Value::String(now_iso.clone()));
    object.insert("lastSeenAt".to_string(), Value::String(now_iso));
    if let Some(owner_session_id) = owner_session_id.filter(|value| !value.trim().is_empty()) {
        object.insert(
            "ownerSessionId".to_string(),
            Value::String(owner_session_id.to_string()),
        );
    } else {
        object.remove("ownerSessionId");
    }
    if let Some(whitelist_record_id) = whitelist_record_id.filter(|value| !value.trim().is_empty())
    {
        object.insert(
            "whitelistRecordId".to_string(),
            Value::String(whitelist_record_id),
        );
    } else {
        object.remove("whitelistRecordId");
    }
    Value::Object(object)
}

fn normalized_or_trimmed_ip(value: &str) -> String {
    let normalized = normalize_ip(value);
    if normalized.is_empty() {
        value.trim().to_string()
    } else {
        normalized
    }
}

pub(crate) fn session_ip_matches(session: &LoginSession, client_ip: &str) -> bool {
    let normalized_client_ip = normalized_or_trimmed_ip(client_ip);
    !normalized_client_ip.is_empty()
        && normalized_or_trimmed_ip(&session.ip) == normalized_client_ip
}

async fn cached_ip_location(state: &AppState, ip: &str) -> Option<String> {
    if ip.is_empty() {
        return None;
    }
    state
        .store
        .get_ip_location_cache(ip)
        .await
        .ok()
        .flatten()
        .and_then(|value| {
            value
                .get("raw")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
        })
}

mod active_ips;
mod cleanup;
mod events;
mod login;
mod restore;
mod trusted_sync;

pub use active_ips::effective_session_ips;
use active_ips::{
    active_ip_touch_is_fresh_for_session, initialize_login_session_mobility_metadata,
    maintain_session_active_ips, parse_active_ip_detail, prune_session_active_ips,
    record_browser_session_login, record_session_active_ip, register_login_session,
};
pub use cleanup::{
    clear_auto_ip_grants_for_auth_credential, clear_auto_ip_grants_for_totp_credential,
    destroy_session, destroy_sessions_for_auth_credential, destroy_sessions_for_auth_method,
    destroy_sessions_for_totp_credential, list_session_whitelist_record_ids,
    reconcile_session_ip_mobility_policy,
};
use events::*;
pub use login::create_login_session;
use restore::resolve_bootstrap_owner;
pub use restore::try_restore_access;
pub(crate) use trusted_sync::sync_browser_session_ip_with_session;
pub use trusted_sync::{sync_browser_session_ip, sync_trusted_request};

#[cfg(test)]
mod tests;
