use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};

use crate::panel_sync::{
    model::*,
    service::{self, ServiceError},
};
use crate::{response, state::AppState};

#[utoipa::path(get, path = "/api/admin/panel-sync/providers", tag = "panel-sync", responses((status = 200, body = [ProviderDescriptor])))]
pub async fn providers() -> Response {
    response::ok(service::providers()).into_response()
}

#[utoipa::path(get, path = "/api/admin/panel-sync/connections", tag = "panel-sync", responses((status = 200, body = [PanelConnection])))]
pub async fn list(State(state): State<AppState>) -> Response {
    result(service::connections(&state).await)
}

#[utoipa::path(post, path = "/api/admin/panel-sync/connections", tag = "panel-sync", request_body = ConnectionInput, responses((status = 200, body = PanelConnection), (status = 400, body = serde_json::Value)))]
pub async fn create(State(state): State<AppState>, Json(input): Json<ConnectionInput>) -> Response {
    result(service::create(&state, input).await)
}

#[utoipa::path(put, path = "/api/admin/panel-sync/connections/{id}", tag = "panel-sync", params(("id" = String, Path)), request_body = ConnectionUpdateInput, responses((status = 200, body = PanelConnection), (status = 404, body = serde_json::Value)))]
pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<ConnectionUpdateInput>,
) -> Response {
    result(service::update(&state, &id, input).await)
}

#[utoipa::path(delete, path = "/api/admin/panel-sync/connections/{id}", tag = "panel-sync", params(("id" = String, Path), ("cleanup_remote" = Option<bool>, Query), ("source_revision" = Option<String>, Query), ("plan_hash" = Option<String>, Query)), responses((status = 200, body = serde_json::Value), (status = 404, body = serde_json::Value)))]
pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(input): Query<DeleteConnectionRequest>,
) -> Response {
    let cleanup_remote = input.cleanup_remote;
    match service::delete(&state, &id, input).await {
        Ok(()) => {
            response::ok(json!({
                "detached": true,
                "remote_cleaned": cleanup_remote,
                "warning": if cleanup_remote { Value::Null } else { json!("连接已解除；远端内容未自动清理") }
            }))
            .into_response()
        }
        Err(error) => service_error(error),
    }
}

#[utoipa::path(post, path = "/api/admin/panel-sync/test", tag = "panel-sync", request_body = TestConnectionInput, responses((status = 200, body = ProbeResult), (status = 400, body = serde_json::Value)))]
pub async fn test(
    State(state): State<AppState>,
    Json(input): Json<TestConnectionInput>,
) -> Response {
    result(service::test(&state, input).await)
}

#[utoipa::path(post, path = "/api/admin/panel-sync/connections/{id}/preview", tag = "panel-sync", params(("id" = String, Path)), request_body = PreviewRequest, responses((status = 200, body = SyncPreview), (status = 400, body = serde_json::Value)))]
pub async fn preview(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<PreviewRequest>,
) -> Response {
    let _refresh_remote = input.refresh_remote.unwrap_or(true);
    match service::preview(&state, &id, input.cleanup_remote).await {
        Ok(plan) => response::ok(plan.preview).into_response(),
        Err(error) => service_error(error),
    }
}

fn result<T: serde::Serialize>(value: Result<T, ServiceError>) -> Response {
    match value {
        Ok(value) => response::ok(value).into_response(),
        Err(error) => service_error(error),
    }
}

pub(super) fn service_error(error: ServiceError) -> Response {
    let status = match error {
        ServiceError::NotFound => StatusCode::NOT_FOUND,
        ServiceError::Validation(_) => StatusCode::BAD_REQUEST,
        ServiceError::Conflict(_) => StatusCode::CONFLICT,
        ServiceError::Failed(_) => StatusCode::BAD_GATEWAY,
    };
    response::error(status, error.message())
}
