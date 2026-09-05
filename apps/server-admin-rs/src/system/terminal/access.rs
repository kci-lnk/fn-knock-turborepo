//! Feature policy is separate from target credentials and the retired terminal_feature config.
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{
    extract::FromRequestParts,
    http::{HeaderMap, request::Parts},
    response::Response,
};
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, OwnedRwLockReadGuard, RwLock};
use utoipa::ToSchema;
use uuid::Uuid;

use super::{
    domain::{TerminalError, TerminalErrorCode, TerminalResult},
    http::terminal_error,
};
use crate::{
    admin::panel::resolve_panel_auth_context,
    auth::{cookies, password},
    crypto_utils,
    state::AppState,
    store::AuthPasswordCredential,
    time_utils,
};

const SETTINGS_KEY: &str = "fn_knock:terminal:feature-settings-v2";
const GRANT_COOKIE: &str = "fn-knock-terminal-access";
const FALLBACK_TTL: i64 = 30 * 24 * 60 * 60;

#[derive(Default)]
pub(super) struct AccessRuntime {
    pub policy: Arc<RwLock<()>>,
    // Serialize password work and bound failed-attempt memory, including cookie-less clients.
    attempts: Mutex<HashMap<String, (u32, Instant)>>,
}

#[derive(Clone, Serialize, Deserialize)]
struct SettingsRecord {
    enabled: bool,
    password: Option<AuthPasswordCredential>,
    revision: String,
}

impl Default for SettingsRecord {
    fn default() -> Self {
        Self {
            enabled: true,
            password: None,
            revision: "initial".into(),
        }
    }
}

#[derive(Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WebTerminalSettings {
    pub enabled: bool,
    pub password_configured: bool,
    pub revision: String,
}

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WebTerminalSettingsInput {
    pub enabled: bool,
    pub revision: String,
    /// Missing or empty means keep the existing password.
    pub password: Option<String>,
    #[serde(default)]
    pub clear_password: bool,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WebTerminalAccessStatus {
    pub enabled: bool,
    pub password_configured: bool,
    pub authorized: bool,
    pub revision: String,
}

#[derive(Deserialize, ToSchema)]
pub struct WebTerminalVerifyInput {
    pub password: String,
}

fn internal(error: impl std::fmt::Display) -> TerminalError {
    tracing::warn!(%error, "terminal access storage operation failed");
    TerminalError::internal("terminal access operation failed")
}

fn password_hash_error(error: anyhow::Error) -> TerminalError {
    if password::is_password_hash_busy(&error) {
        TerminalError::new(
            TerminalErrorCode::ResourceBusy,
            "password verification is busy; retry shortly",
        )
    } else {
        internal(error)
    }
}

async fn password_attempt(
    attempts: &mut HashMap<String, (u32, Instant)>,
    ip: &str,
    verification: impl std::future::Future<Output = TerminalResult<bool>>,
) -> TerminalResult<bool> {
    // Charge before awaiting so cancellation cannot grant a free attempt.
    // The caller holds the attempts lock throughout; a rejected pool admission
    // can safely refund exactly this attempt without racing another request.
    attempts
        .entry(ip.to_string())
        .or_insert((0, Instant::now()))
        .0 += 1;
    let result = verification.await;
    if result
        .as_ref()
        .is_err_and(|error| error.code == TerminalErrorCode::ResourceBusy)
    {
        let empty = attempts.get_mut(ip).is_some_and(|(count, _)| {
            *count = count.saturating_sub(1);
            *count == 0
        });
        if empty {
            attempts.remove(ip);
        }
    }
    result
}

async fn record(state: &AppState) -> TerminalResult<SettingsRecord> {
    let value = state
        .storage
        .store
        .get_json_value(SETTINGS_KEY)
        .await
        .map_err(internal)?;
    let record = match value {
        Some(value) => serde_json::from_value::<SettingsRecord>(value).map_err(internal)?,
        None => SettingsRecord::default(),
    };
    if record.revision.is_empty()
        || record
            .password
            .as_ref()
            .is_some_and(|value| !password::is_supported_auth_password_credential(value))
    {
        return Err(TerminalError::internal(
            "terminal access settings are invalid",
        ));
    }
    Ok(record)
}

fn public(record: SettingsRecord) -> WebTerminalSettings {
    WebTerminalSettings {
        enabled: record.enabled,
        password_configured: record.password.is_some(),
        revision: record.revision,
    }
}

pub async fn settings(state: &AppState) -> TerminalResult<WebTerminalSettings> {
    Ok(public(record(state).await?))
}

pub async fn update(
    state: &AppState,
    input: WebTerminalSettingsInput,
) -> TerminalResult<WebTerminalSettings> {
    // Once accepted, persistence and runtime cleanup have one application-owned
    // lifetime. Dropping the HTTP response must not interrupt a disable operation.
    let guard = state.terminal.access.policy.clone().write_owned().await;
    let task_state = state.clone();
    let (sender, receiver) = tokio::sync::oneshot::channel();
    state.spawn_background("terminal-feature-update", async move {
        let _guard = guard;
        let result = update_locked(&task_state, input).await;
        let _ = sender.send(result);
    });
    receiver.await.map_err(internal)?
}

async fn update_locked(
    state: &AppState,
    input: WebTerminalSettingsInput,
) -> TerminalResult<WebTerminalSettings> {
    let next_password = input.password.filter(|value| !value.is_empty());
    if input.clear_password && next_password.is_some() {
        return Err(TerminalError::invalid(
            "cannot set and clear the password together",
        ));
    }
    if let Some(value) = &next_password {
        password::validate_auth_password(value).map_err(TerminalError::invalid)?;
    }
    let mut value = record(state).await?;
    if input.revision != value.revision {
        return Err(TerminalError::new(
            TerminalErrorCode::Conflict,
            "terminal settings changed; refresh and retry",
        ));
    }
    let password_changed =
        next_password.is_some() || (input.clear_password && value.password.is_some());
    if let Some(secret) = next_password {
        value.password = Some(
            password::make_auth_password_credential("web-terminal-access", &secret, None)
                .await
                .map_err(password_hash_error)?,
        );
    } else if input.clear_password {
        value.password = None;
    }
    if password_changed || value.enabled != input.enabled {
        value.revision = Uuid::new_v4().to_string();
    }
    value.enabled = input.enabled;
    state
        .storage
        .store
        .set_json_value(
            SETTINGS_KEY,
            &serde_json::to_value(&value).map_err(internal)?,
        )
        .await
        .map_err(internal)?;
    if !value.enabled {
        state.terminal.shutdown_all().await;
    }
    Ok(public(value))
}

struct Identity {
    key: String,
    owner: Option<LoginGrantOwner>,
}

#[derive(Clone, Serialize, Deserialize)]
struct LoginGrantOwner {
    cookie_name: String,
    session_id: String,
}

#[derive(Serialize, Deserialize)]
struct AccessGrant {
    revision: String,
    owner: Option<LoginGrantOwner>,
}

async fn identity(state: &AppState, headers: &HeaderMap) -> TerminalResult<Option<Identity>> {
    let context = resolve_panel_auth_context(state, headers)
        .await
        .map_err(internal)?;
    let source = context.get("auth_source").and_then(|value| value.as_str());
    let cookie_name = match source {
        Some("panel_session") => Some(cookies::ADMIN_PANEL_SESSION_COOKIE_NAME),
        Some("reauth_session") => Some(cookies::SESSION_COOKIE_NAME),
        _ => None,
    };
    if let Some(name) = cookie_name {
        let token = cookies::read_cookie(headers, name)
            .ok_or_else(|| TerminalError::internal("missing login session"))?;
        return Ok(Some(Identity {
            key: grant_key(name, &token),
            owner: Some(LoginGrantOwner {
                cookie_name: name.into(),
                session_id: token,
            }),
        }));
    }
    Ok(cookies::read_cookie(headers, GRANT_COOKIE)
        .filter(|token| Uuid::parse_str(token).is_ok())
        .map(|token| Identity {
            key: grant_key("browser", &token),
            owner: None,
        }))
}

fn grant_key(source: &str, token: &str) -> String {
    format!(
        "fn_knock:terminal:access-grant:{}",
        crypto_utils::sha256_hex_str(&format!("{source}:{token}"))
    )
}

async fn authorized(
    state: &AppState,
    headers: &HeaderMap,
    value: &SettingsRecord,
) -> TerminalResult<bool> {
    if !value.enabled {
        return Ok(false);
    }
    if value.password.is_none() {
        return Ok(true);
    }
    let Some(identity) = identity(state, headers).await? else {
        return Ok(false);
    };
    let grant = state
        .storage
        .store
        .get_string_value(&identity.key)
        .await
        .map_err(internal)?;
    let granted = grant
        .map(|raw| serde_json::from_str::<AccessGrant>(&raw))
        .transpose()
        .map_err(internal)?
        .is_some_and(|grant| grant.revision == value.revision);
    if granted && identity.owner.is_none() {
        save_grant(state, &identity, &value.revision).await?;
    }
    Ok(granted)
}

pub async fn status(
    state: &AppState,
    headers: &HeaderMap,
) -> TerminalResult<WebTerminalAccessStatus> {
    let _guard = state.terminal.access.policy.read().await;
    let value = record(state).await?;
    Ok(WebTerminalAccessStatus {
        authorized: authorized(state, headers, &value).await?,
        enabled: value.enabled,
        password_configured: value.password.is_some(),
        revision: value.revision,
    })
}

pub async fn verify(
    state: &AppState,
    headers: &HeaderMap,
    input: WebTerminalVerifyInput,
) -> TerminalResult<Option<String>> {
    // Queue password work before taking a policy read lock so a burst of
    // verification requests cannot hold up disabling the feature.
    let mut attempts = state.terminal.access.attempts.lock().await;
    let _guard = state.terminal.access.policy.read().await;
    let value = record(state).await?;
    if !value.enabled {
        return Err(disabled());
    }
    let Some(credential) = value.password else {
        return Ok(None);
    };
    password::validate_auth_password(&input.password).map_err(|_| required())?;
    let ip =
        crate::http_utils::normalize_session_client_ip(&crate::http_utils::get_client_ip(headers));
    attempts.retain(|_, (_, started)| started.elapsed() < Duration::from_secs(60));
    if attempts.get(&ip).is_some_and(|(count, _)| *count >= 5)
        || (!attempts.contains_key(&ip) && attempts.len() >= 1024)
    {
        return Err(TerminalError::new(
            TerminalErrorCode::AccessRateLimited,
            "too many password attempts; retry in one minute",
        ));
    }
    let valid = password_attempt(&mut attempts, &ip, async {
        password::verify_auth_password(&input.password, &credential)
            .await
            .map_err(password_hash_error)
    })
    .await?;
    if !valid {
        return Err(required());
    }
    attempts.remove(&ip);
    let (identity, cookie) = match identity(state, headers).await? {
        Some(identity) if identity.owner.is_some() => (identity, None),
        _ => {
            // Never authorize a browser token supplied by the caller: rotate it
            // after verification to prevent session fixation.
            let token = Uuid::new_v4().to_string();
            (
                Identity {
                    key: grant_key("browser", &token),
                    owner: None,
                },
                Some(token),
            )
        }
    };
    save_grant(state, &identity, &value.revision).await?;
    Ok(cookie)
}

async fn save_grant(state: &AppState, identity: &Identity, revision: &str) -> TerminalResult<()> {
    let grant = AccessGrant {
        revision: revision.into(),
        owner: identity.owner.clone(),
    };
    // Login grants follow the parent's lifetime, including renewals elsewhere in
    // the admin UI. A separate TTL would prematurely expire an active login.
    state
        .storage
        .store
        .set_string_value_with_optional_ttl(
            &identity.key,
            &serde_json::to_string(&grant).map_err(internal)?,
            identity.owner.is_none().then_some(FALLBACK_TTL),
        )
        .await
        .map_err(internal)
}

pub(super) async fn expire_grants(state: &AppState) -> TerminalResult<()> {
    let _guard = state.terminal.access.policy.read().await;
    let settings = record(state).await?;
    let mut stale = Vec::new();
    for key in state
        .storage
        .store
        .scan_keys("fn_knock:terminal:access-grant:", 200)
        .await
        .map_err(internal)?
    {
        let Some(raw) = state
            .storage
            .store
            .get_string_value(&key)
            .await
            .map_err(internal)?
        else {
            continue;
        };
        let Ok(grant) = serde_json::from_str::<AccessGrant>(&raw) else {
            stale.push(key);
            continue;
        };
        if grant.revision != settings.revision {
            stale.push(key);
            continue;
        }
        let Some(owner) = grant.owner else {
            continue;
        };
        let expires = if owner.cookie_name == cookies::ADMIN_PANEL_SESSION_COOKIE_NAME {
            state
                .storage
                .store
                .docker_admin_session(&owner.session_id)
                .await
                .map_err(internal)?
                .map(|session| session.expires_at)
        } else {
            state
                .storage
                .store
                .get_session(&owner.session_id)
                .await
                .map_err(internal)?
                .and_then(|session| session.expires_at)
        };
        if expires
            .as_deref()
            .and_then(time_utils::parse_iso_ms)
            .is_none_or(|expires| expires <= time_utils::now_ms())
        {
            stale.push(key);
        }
    }
    state
        .storage
        .store
        .delete_keys(&stale)
        .await
        .map_err(internal)
}

pub fn browser_cookie(token: &str, secure: bool) -> String {
    // No Max-Age: this is deliberately a browser-session cookie.
    format!(
        "{GRANT_COOKIE}={token}; Path=/; HttpOnly; SameSite=Strict{}",
        if secure { "; Secure" } else { "" }
    )
}

fn disabled() -> TerminalError {
    TerminalError::new(
        TerminalErrorCode::FeatureDisabled,
        "web terminal is disabled",
    )
}
fn required() -> TerminalError {
    TerminalError::new(
        TerminalErrorCode::AccessPasswordRequired,
        "web terminal password verification required",
    )
}

/// Held until the handler returns, including output polls and connection creation.
pub(super) struct TerminalAccess {
    _guard: OwnedRwLockReadGuard<()>,
}
impl FromRequestParts<AppState> for TerminalAccess {
    type Rejection = Response;
    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Response> {
        let guard = state.terminal.access.policy.clone().read_owned().await;
        let value = record(state).await.map_err(terminal_error)?;
        if !value.enabled {
            return Err(terminal_error(disabled()));
        }
        if !authorized(state, &parts.headers, &value)
            .await
            .map_err(terminal_error)?
        {
            return Err(terminal_error(required()));
        }
        Ok(Self { _guard: guard })
    }
}

#[cfg(test)]
mod tests;
