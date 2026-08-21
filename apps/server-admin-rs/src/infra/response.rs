use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::Serialize;
use serde_json::{Value, json};

use crate::{
    app_version::APP_LOCAL_VERSION,
    go_backend::{
        GATEWAY_CONTROL_API_VERSION, GATEWAY_HEALTH_AUTH_BRIDGE, GATEWAY_HEALTH_DATAPLANE,
        GATEWAY_HEALTH_PROCESS,
    },
    runtime_profile,
    state::AppState,
};

#[derive(Serialize)]
pub struct ApiEnvelope<T: Serialize> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
}

pub fn ok<T: Serialize>(data: T) -> Json<ApiEnvelope<T>> {
    Json(ApiEnvelope {
        success: true,
        code: None,
        message: None,
        data: Some(data),
    })
}

pub fn success_message(message: impl Into<String>) -> Json<ApiEnvelope<Value>> {
    Json(ApiEnvelope {
        success: true,
        code: None,
        message: Some(message.into()),
        data: None,
    })
}

pub fn success_empty() -> Json<ApiEnvelope<Value>> {
    Json(ApiEnvelope {
        success: true,
        code: None,
        message: None,
        data: None,
    })
}

pub fn error(status: StatusCode, message: impl Into<String>) -> axum::response::Response {
    error_with_code(status, None, message)
}

pub fn error_with_code(
    status: StatusCode,
    code: Option<u16>,
    message: impl Into<String>,
) -> axum::response::Response {
    (
        status,
        Json(ApiEnvelope::<Value> {
            success: false,
            code,
            message: Some(message.into()),
            data: None,
        }),
    )
        .into_response()
}

#[utoipa::path(
    get,
    path = "/api/admin/healthz",
    tag = "health",
    responses(
        (status = 200, description = "Storage and gateway are healthy", body = serde_json::Value),
        (status = 503, description = "A required runtime component is unavailable", body = serde_json::Value)
    )
)]
pub async fn healthz(State(state): State<AppState>) -> axum::response::Response {
    let (storage_reachable, storage_error) = match state.storage.store.ping().await {
        Ok(()) => (true, Value::Null),
        Err(error) => (false, Value::String(error.to_string())),
    };
    let gateway_probe = state.gateway.client.get_server_info().await;
    let (gateway_reachable, gateway_version, gateway_error) = match gateway_probe {
        Ok(value) if value.get("success").and_then(Value::as_bool) == Some(true) => (
            true,
            value
                .pointer("/data/version")
                .and_then(Value::as_str)
                .map(|value| Value::String(value.to_string()))
                .unwrap_or(Value::Null),
            Value::Null,
        ),
        Ok(value) => (
            false,
            Value::Null,
            Value::String(
                value
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("Gateway admin probe failed")
                    .to_string(),
            ),
        ),
        Err(error) => (false, Value::Null, Value::String(error.to_string())),
    };
    let healthy = storage_reachable && gateway_reachable;
    let body = json!({
        "success": healthy,
        "data": {
            "node": {
                "alive": true,
                "pid": std::process::id(),
            },
            "storage": {
                "type": "sqlite",
                "reachable": storage_reachable,
                "error": storage_error,
            },
            "runtime_profile": runtime_profile(&state),
            "gateway_admin": {
                "reachable": gateway_reachable,
                "version": gateway_version,
                "error": gateway_error,
            },
        },
    });
    if healthy {
        Json(body).into_response()
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, Json(body)).into_response()
    }
}

/// Minimal, non-sensitive readiness endpoint consumed by the Windows desktop
/// shell and installer rollback check. Detailed errors remain in service logs.
pub async fn readyz(State(state): State<AppState>) -> axum::response::Response {
    let recovering = state.runtime_health.recovery_active();
    if recovering {
        let (storage, process, dataplane, auth_bridge, config_sync) = tokio::join!(
            state.runtime_health.component_ready("storage"),
            state.runtime_health.component_ready("gateway_process"),
            state.runtime_health.component_ready("gateway_dataplane"),
            state.runtime_health.component_ready("auth_bridge"),
            state.runtime_health.component_ready("config_sync"),
        );
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "ready": false,
                "version": APP_LOCAL_VERSION,
                "control_api_version": GATEWAY_CONTROL_API_VERSION,
                "components": {
                    "storage": storage,
                    "gateway_bundle": false,
                    "gateway_process": process,
                    "gateway_dataplane": dataplane,
                    "auth_bridge": auth_bridge,
                    "gateway_config_synced": config_sync,
                }
            })),
        )
            .into_response();
    }
    let (storage_ready, bundle, process, dataplane, auth_bridge) = tokio::join!(
        state.runtime_health.component_ready("storage"),
        state.gateway.client.verify_bundle_compatibility(),
        state.gateway.client.health_serving(GATEWAY_HEALTH_PROCESS),
        state
            .gateway
            .client
            .health_serving(GATEWAY_HEALTH_DATAPLANE),
        state
            .gateway
            .client
            .health_serving(GATEWAY_HEALTH_AUTH_BRIDGE),
    );
    let bundle_ready = bundle.is_ok();
    let process_ready = process.unwrap_or(false);
    let dataplane_ready = dataplane.unwrap_or(false);
    let auth_bridge_ready = auth_bridge.unwrap_or(false);
    let config_synced = state.gateway_config_synced();
    let ready = storage_ready
        && !recovering
        && bundle_ready
        && process_ready
        && dataplane_ready
        && auth_bridge_ready
        && config_synced;
    let body = json!({
        "ready": ready,
        "version": APP_LOCAL_VERSION,
        "control_api_version": GATEWAY_CONTROL_API_VERSION,
        "components": {
            "storage": storage_ready,
            "gateway_bundle": bundle_ready,
            "gateway_process": process_ready,
            "gateway_dataplane": dataplane_ready,
            "auth_bridge": auth_bridge_ready,
            "gateway_config_synced": config_synced,
        }
    });
    if ready {
        Json(body).into_response()
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, Json(body)).into_response()
    }
}

fn runtime_profile(state: &AppState) -> Value {
    serde_json::to_value(runtime_profile::get_runtime_profile(state)).unwrap_or_else(|_| json!({}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    #[tokio::test]
    async fn error_response_matches_node_envelope_shape() {
        let response = error(StatusCode::BAD_REQUEST, "Bad request");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let value: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(
            value,
            json!({
                "success": false,
                "message": "Bad request"
            })
        );
    }
}
