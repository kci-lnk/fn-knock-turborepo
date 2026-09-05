//! Global Web Terminal feature switch, independent of SSH target credentials.
use std::sync::Arc;

use axum::{extract::FromRequestParts, http::request::Parts, response::Response};
use serde::{Deserialize, Serialize};
use tokio::sync::{OwnedRwLockReadGuard, RwLock};
use utoipa::ToSchema;
use uuid::Uuid;

use super::{
    domain::{TerminalError, TerminalErrorCode, TerminalResult},
    http::terminal_error,
};
use crate::state::AppState;

const SETTINGS_KEY: &str = "fn_knock:terminal:feature-settings-v2";

#[derive(Default)]
pub(super) struct AccessRuntime {
    pub policy: Arc<RwLock<()>>,
}

#[derive(Clone, Serialize, Deserialize, ToSchema)]
pub struct WebTerminalSettings {
    pub enabled: bool,
    pub revision: String,
}

impl Default for WebTerminalSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            revision: "initial".into(),
        }
    }
}

#[derive(Deserialize, ToSchema)]
pub struct WebTerminalSettingsInput {
    pub enabled: bool,
    pub revision: String,
}

fn internal(error: impl std::fmt::Display) -> TerminalError {
    tracing::warn!(%error, "terminal feature storage operation failed");
    TerminalError::internal("terminal feature operation failed")
}

pub async fn settings(state: &AppState) -> TerminalResult<WebTerminalSettings> {
    let value = state
        .storage
        .store
        .get_json_value(SETTINGS_KEY)
        .await
        .map_err(internal)?;
    // Unknown legacy password fields are deliberately ignored, including invalid
    // old hashes: they must never prevent use of an enabled terminal.
    let settings = match value {
        Some(value) => serde_json::from_value::<WebTerminalSettings>(value).map_err(internal)?,
        None => WebTerminalSettings::default(),
    };
    if settings.revision.is_empty() {
        return Err(TerminalError::internal(
            "terminal feature settings are invalid",
        ));
    }
    Ok(settings)
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
    let mut value = settings(state).await?;
    if input.revision != value.revision {
        return Err(TerminalError::new(
            TerminalErrorCode::Conflict,
            "terminal settings changed; refresh and retry",
        ));
    }
    if value.enabled != input.enabled {
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
    Ok(value)
}

/// Remove retired credentials while retaining the saved switch and revision.
/// Maintenance retries this after storage failures and also handles old backups.
pub(super) async fn cleanup_retired_password(state: &AppState) -> TerminalResult<()> {
    if let Some(raw) = state
        .storage
        .store
        .get_json_value(SETTINGS_KEY)
        .await
        .map_err(internal)?
        && raw.get("password").is_some()
    {
        let _guard = state.terminal.access.policy.write().await;
        let value = settings(state).await?;
        state
            .storage
            .store
            .set_json_value(
                SETTINGS_KEY,
                &serde_json::to_value(value).map_err(internal)?,
            )
            .await
            .map_err(internal)?;
    }
    let grants = state
        .storage
        .store
        .scan_keys("fn_knock:terminal:access-grant:", 200)
        .await
        .map_err(internal)?;
    state
        .storage
        .store
        .delete_keys(&grants)
        .await
        .map_err(internal)
}

/// Held until the handler returns, including output polls and connection creation.
pub(super) struct TerminalAccess {
    _guard: OwnedRwLockReadGuard<()>,
}
impl FromRequestParts<AppState> for TerminalAccess {
    type Rejection = Response;
    async fn from_request_parts(_parts: &mut Parts, state: &AppState) -> Result<Self, Response> {
        let guard = state.terminal.access.policy.clone().read_owned().await;
        if !settings(state).await.map_err(terminal_error)?.enabled {
            return Err(terminal_error(TerminalError::new(
                TerminalErrorCode::FeatureDisabled,
                "web terminal is disabled",
            )));
        }
        Ok(Self { _guard: guard })
    }
}

#[cfg(test)]
mod tests;
