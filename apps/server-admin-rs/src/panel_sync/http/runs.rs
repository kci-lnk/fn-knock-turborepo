use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};

use crate::{
    panel_sync::{model::*, repository::Repository, service},
    response,
    state::AppState,
};

use super::connections::service_error;

#[utoipa::path(post, path = "/api/admin/panel-sync/connections/{id}/sync", tag = "panel-sync", params(("id" = String, Path)), request_body = SyncRequest, responses((status = 202, body = SyncAccepted), (status = 409, body = serde_json::Value)))]
pub async fn sync(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<SyncRequest>,
) -> Response {
    match service::enqueue_manual(&state, &id, input).await {
        Ok(accepted) => (StatusCode::ACCEPTED, response::ok(accepted)).into_response(),
        Err(error) => service_error(error),
    }
}

#[utoipa::path(get, path = "/api/admin/panel-sync/connections/{id}/runs", tag = "panel-sync", params(("id" = String, Path)), responses((status = 200, body = [SyncRun])))]
pub async fn list(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let repository = Repository::new(&state);
    match repository.connection(&id).await {
        Ok(None) => response::error(StatusCode::NOT_FOUND, "面板连接不存在"),
        Err(error) => response::error(StatusCode::INTERNAL_SERVER_ERROR, error),
        Ok(Some(_)) => match repository.runs(&id).await {
            Ok(runs) => response::ok(runs).into_response(),
            Err(error) => response::error(StatusCode::INTERNAL_SERVER_ERROR, error),
        },
    }
}

#[utoipa::path(get, path = "/api/admin/panel-sync/runs/{run_id}", tag = "panel-sync", params(("run_id" = String, Path)), responses((status = 200, body = SyncRun), (status = 404, body = serde_json::Value)))]
pub async fn get(State(state): State<AppState>, Path(run_id): Path<String>) -> Response {
    match Repository::new(&state).run(&run_id).await {
        Ok(Some(run)) => response::ok(run).into_response(),
        Ok(None) => response::error(StatusCode::NOT_FOUND, "同步运行记录不存在"),
        Err(error) => response::error(StatusCode::INTERNAL_SERVER_ERROR, error),
    }
}
