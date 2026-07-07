use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::Serialize;
use serde_json::{Value, json};

use crate::{runtime_profile, state::AppState};

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
    let code = status.as_u16();
    (
        status,
        Json(ApiEnvelope::<Value> {
            success: false,
            code: Some(code),
            message: Some(message.into()),
            data: None,
        }),
    )
        .into_response()
}

pub async fn healthz(State(state): State<AppState>) -> axum::response::Response {
    let (redis_reachable, redis_error) = match state.redis.ping().await {
        Ok(()) => (true, Value::Null),
        Err(error) => (false, Value::String(error.to_string())),
    };
    let gateway_probe = state.go_backend.get_server_info().await;
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
    let healthy = redis_reachable && gateway_reachable;
    let body = json!({
        "success": healthy,
        "data": {
            "node": {
                "alive": true,
                "pid": std::process::id(),
            },
            "redis": {
                "reachable": redis_reachable,
                "error": redis_error,
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

fn runtime_profile(state: &AppState) -> Value {
    serde_json::to_value(runtime_profile::get_runtime_profile(state)).unwrap_or_else(|_| json!({}))
}
