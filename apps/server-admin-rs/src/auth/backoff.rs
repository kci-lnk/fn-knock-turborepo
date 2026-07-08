use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::json;

use crate::{http_utils::normalize_ip, i18n::Translator, response, state::AppState};

fn backoff_route_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.backoffRoutes.{key}"))
}

#[derive(Deserialize)]
struct BackoffStatusQuery {
    ip: Option<String>,
}

#[derive(Deserialize)]
struct BackoffResetBody {
    ip: String,
}

pub fn backoff_routes() -> Router<AppState> {
    Router::new()
        .route("/api/admin/backoff/list", get(list))
        .route("/api/admin/backoff/status", get(status))
        .route("/api/admin/backoff/reset", post(reset))
}

async fn list(State(state): State<AppState>) -> Response {
    match state.store.list_blocked_login_backoffs().await {
        Ok(items) => response::ok(items).into_response(),
        Err(error) => {
            let translator = Translator::from_state(&state).await;
            tracing::warn!(%error, "failed to list login backoff records");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                backoff_route_text(&translator, "listFailed"),
            )
        }
    }
}

async fn status(
    State(state): State<AppState>,
    Query(query): Query<BackoffStatusQuery>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let Some(ip) = backoff_status_query_ip(query.ip.as_deref()) else {
        return response::error(
            StatusCode::BAD_REQUEST,
            backoff_route_text(&translator, "ipRequired"),
        );
    };
    match state.store.get_login_backoff_status(&ip).await {
        Ok(status) => response::ok(status).into_response(),
        Err(error) => {
            tracing::warn!(%error, %ip, "failed to inspect login backoff status");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                backoff_route_text(&translator, "statusFailed"),
            )
        }
    }
}

async fn reset(State(state): State<AppState>, Json(body): Json<BackoffResetBody>) -> Response {
    let translator = Translator::from_state(&state).await;
    let ip = normalize_auth_failure_tracking_ip(&body.ip);
    match state.store.reset_login_backoff(&ip).await {
        Ok(()) => response::ok(json!({})).into_response(),
        Err(error) => {
            tracing::warn!(%error, %ip, "failed to reset login backoff");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                backoff_route_text(&translator, "resetFailed"),
            )
        }
    }
}

fn backoff_status_query_ip(value: Option<&str>) -> Option<String> {
    let value = value?;
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

pub fn normalize_auth_failure_tracking_ip(value: &str) -> String {
    let normalized = normalize_ip(value);
    if !normalized.is_empty() {
        return normalized;
    }
    let raw = value.trim();
    if raw.is_empty() {
        "unknown".to_string()
    } else {
        raw.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_auth_failure_tracking_ip_like_node() {
        assert_eq!(
            normalize_auth_failure_tracking_ip("::ffff:192.168.1.2"),
            "192.168.1.2"
        );
        assert_eq!(normalize_auth_failure_tracking_ip("bad value"), "bad value");
        assert_eq!(normalize_auth_failure_tracking_ip(""), "unknown");
    }

    #[test]
    fn status_query_ip_matches_node_route_truthiness() {
        assert_eq!(backoff_status_query_ip(None), None);
        assert_eq!(backoff_status_query_ip(Some("")), None);
        assert_eq!(
            backoff_status_query_ip(Some("::ffff:192.168.1.2")),
            Some("::ffff:192.168.1.2".to_string())
        );
        assert_eq!(backoff_status_query_ip(Some(" ")), Some(" ".to_string()));
    }

    #[test]
    fn localizes_backoff_route_text() {
        let zh = Translator::new("zh-CN");
        assert_eq!(backoff_route_text(&zh, "resetFailed"), "重置登录退避失败");
    }
}
