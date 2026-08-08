use axum::http::StatusCode;
use fn_knock_wol_protocol::{AckStatus, Command, MacAddress};
use serde::Serialize;
use serde_json::{Value, json};
use std::net::Ipv4Addr;

use crate::{events, state::AppState};

use super::{
    dispatch::{DispatchError, dispatch, dispatch_local},
    secrets::secret_store,
    status::{schedule_target_rechecks, status_view},
    store::{RelayRecord, TargetRecord, list_targets, load_relay, load_target},
};

pub(super) const WAKE_COOLDOWN_SECONDS: usize = 3;

#[derive(Debug)]
pub(crate) struct WolServiceError {
    pub(crate) status: StatusCode,
    pub(crate) message: String,
}

impl WolServiceError {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, message)
    }

    fn conflict(message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, message)
    }

    fn internal(action: &str, error: impl std::fmt::Display) -> Self {
        tracing::warn!(%error, action, "WoL service operation failed");
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to {action}"),
        )
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AuthTargetView {
    id: String,
    name: String,
    note: String,
    status: AuthTargetStatusView,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthTargetStatusView {
    state: String,
    checked_at: Option<String>,
}

pub(crate) async fn list_auth_targets(
    state: &AppState,
) -> Result<Vec<AuthTargetView>, WolServiceError> {
    let targets = list_targets(state)
        .await
        .map_err(|error| WolServiceError::internal("load Targets", error))?;
    let mut views = Vec::new();
    for target in targets.into_iter().filter(|target| target.enabled) {
        let status = status_view(state, &target.id)
            .await
            .map_err(|error| WolServiceError::internal("load Target status", error))?;
        views.push(AuthTargetView {
            id: target.id,
            name: target.name,
            note: target.note,
            status: AuthTargetStatusView {
                state: status.state,
                checked_at: status.checked_at,
            },
        });
    }
    Ok(views)
}

pub(crate) async fn wake_target(state: &AppState, id: &str) -> Result<Value, WolServiceError> {
    let target = load_target(state, id)
        .await
        .map_err(|error| WolServiceError::internal("load Target", error))?
        .ok_or_else(|| WolServiceError::new(StatusCode::NOT_FOUND, "Target was not found"))?;
    if !target.enabled {
        return Err(WolServiceError::conflict("Target is disabled"));
    }
    let mac = target
        .mac
        .parse::<MacAddress>()
        .map_err(|_| WolServiceError::bad_request("Target MAC address is invalid"))?;
    let relay = match target.relay_id.as_deref() {
        Some(relay_id) => {
            let relay = load_relay(state, relay_id)
                .await
                .map_err(|error| WolServiceError::internal("load Relay", error))?
                .ok_or_else(|| {
                    WolServiceError::new(StatusCode::NOT_FOUND, "Relay was not found")
                })?;
            if !relay.enabled {
                return Err(WolServiceError::conflict("Relay is disabled"));
            }
            Some(relay)
        }
        None => None,
    };
    let psk = match relay.as_ref() {
        Some(relay) => Some(
            secret_store(state)
                .read(&relay.id, relay.key_version)
                .map_err(|error| WolServiceError::internal("read Relay PSK", error))?
                .ok_or_else(|| WolServiceError::conflict("Relay PSK is not configured"))?,
        ),
        None => None,
    };
    let acquired = state
        .store
        .set_key_if_not_exists_with_ttl(
            &format!("fn_knock:wol:runtime:cooldown:{}", target.id),
            "1",
            WAKE_COOLDOWN_SECONDS,
        )
        .await
        .map_err(|error| WolServiceError::internal("acquire wake cooldown", error))?;
    if !acquired {
        return Err(WolServiceError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "Target was woken recently; wait before retrying",
        ));
    }

    let result = match (relay.as_ref(), psk.as_deref()) {
        (Some(relay), Some(psk)) => dispatch(relay, psk, Command::Wake, Some(mac)).await,
        (None, None) => {
            let broadcast_address = target
                .broadcast_address
                .as_deref()
                .map(str::parse::<Ipv4Addr>)
                .transpose()
                .map_err(|_| WolServiceError::bad_request("Target broadcast address is invalid"))?;
            dispatch_local(mac, broadcast_address).await
        }
        _ => unreachable!("Relay and PSK are resolved together"),
    };
    let result = match result {
        Ok(result) => {
            publish_wake_event(
                state,
                &target,
                relay.as_ref(),
                true,
                &result.request_id,
                result.attempts,
                result.latency_ms,
                "broadcasted",
            )
            .await;
            result
        }
        Err(error) => {
            publish_wake_event(
                state,
                &target,
                relay.as_ref(),
                false,
                error.request_id(),
                error.attempts(),
                error.latency_ms(),
                dispatch_failure_status(&error),
            )
            .await;
            return Err(dispatch_error(error));
        }
    };
    let mut value = serde_json::to_value(result).unwrap_or_else(|_| json!({}));
    if let Some(object) = value.as_object_mut() {
        object.insert("targetId".to_string(), json!(target.id));
    }
    schedule_target_rechecks(state.clone(), target.id);
    Ok(value)
}

#[allow(clippy::too_many_arguments)]
async fn publish_wake_event(
    state: &AppState,
    target: &TargetRecord,
    relay: Option<&RelayRecord>,
    success: bool,
    request_id: &str,
    attempts: u8,
    latency_ms: u64,
    status: &str,
) {
    let payload = json!({
        "success": success,
        "status": status,
        "target_id": target.id,
        "target_name": target.name,
        "delivery_mode": if relay.is_some() { "relay" } else { "local" },
        "relay_id": relay.map(|value| value.id.as_str()),
        "relay_name": relay.map(|value| value.name.as_str()),
        "request_id": request_id,
        "attempts": attempts,
        "latency_ms": latency_ms,
    });
    if let Err(error) = events::publish_wol_wake_completed_event(state, &target.id, payload).await {
        tracing::warn!(%error, request_id, "failed to publish WoL wake event");
    }
}

fn dispatch_error(error: DispatchError) -> WolServiceError {
    match error {
        DispatchError::Network { message, .. } => WolServiceError::new(
            StatusCode::BAD_GATEWAY,
            format!("Failed to send WoL request: {message}"),
        ),
        DispatchError::Timeout { .. } => WolServiceError::new(
            StatusCode::GATEWAY_TIMEOUT,
            "Relay acknowledgement timed out; broadcast status is unknown",
        ),
        DispatchError::Relay { status, .. } => WolServiceError::new(
            StatusCode::BAD_GATEWAY,
            match status {
                AckStatus::ClockSkew => {
                    "Relay rejected the request because its clock is out of sync"
                }
                AckStatus::InvalidTarget => "Relay rejected the target MAC address",
                AckStatus::BroadcastFailed => "Relay failed to send the local broadcast",
                AckStatus::InternalError => "Relay reported an internal error",
                AckStatus::Ok
                | AckStatus::TargetOnline
                | AckStatus::TargetOffline
                | AckStatus::TargetUnknown => "Relay returned an unexpected acknowledgement",
            },
        ),
    }
}

fn dispatch_failure_status(error: &DispatchError) -> &'static str {
    match error {
        DispatchError::Network { .. } => "network_error",
        DispatchError::Timeout { .. } => "ack_timeout",
        DispatchError::Relay { status, .. } => match status {
            AckStatus::ClockSkew => "clock_skew",
            AckStatus::InvalidTarget => "invalid_target",
            AckStatus::BroadcastFailed => "broadcast_failed",
            AckStatus::InternalError => "relay_error",
            AckStatus::Ok
            | AckStatus::TargetOnline
            | AckStatus::TargetOffline
            | AckStatus::TargetUnknown => "invalid_ack",
        },
    }
}
