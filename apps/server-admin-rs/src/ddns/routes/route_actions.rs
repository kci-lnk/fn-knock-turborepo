use super::*;

#[utoipa::path(delete, path = "/api/admin/ddns/logs", tag = "ddns", operation_id = "delete_api_admin_ddns_logs", responses((status = 200, description = "Cleared DDNS logs")))]
pub(super) async fn clear_logs(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.storage.store.clear_log_buffer(DDNS_LOGS).await {
        Ok(()) => response::success_empty().into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to clear DDNS logs");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ddns_text(&translator, "logsClearFailed", &[]),
            )
        }
    }
}

#[utoipa::path(get, path = "/api/admin/ddns/poll", tag = "ddns", operation_id = "get_api_admin_ddns_poll", responses((status = 200, description = "DDNS log poll")))]
pub(super) async fn poll(
    State(state): State<AppState>,
    Query(query): Query<PollQuery>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let logs = match state
        .storage
        .store
        .poll_log_buffer(DDNS_LOGS, query.cursor.as_deref())
        .await
    {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, "failed to poll DDNS logs");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ddns_text(&translator, "pollFailed", &[]),
            );
        }
    };
    let status = match build_ddns_status(&state, &translator).await {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, "failed to build DDNS poll status");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ddns_text(&translator, "pollFailed", &[]),
            );
        }
    };
    response::ok(json!({
        "cursor": logs.get("cursor").cloned().unwrap_or(json!(0)),
        "reset": logs.get("reset").cloned().unwrap_or(json!(false)),
        "logs": parse_log_entries(logs.get("items").and_then(Value::as_array).cloned().unwrap_or_default().into_iter().filter_map(|item| item.as_str().map(str::to_string)).collect()),
        "status": status
    }))
    .into_response()
}
