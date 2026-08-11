use super::*;
use utoipa_axum::{router::OpenApiRouter, routes};

pub(super) fn openapi_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(status))
        .routes(routes!(overview))
        .routes(routes!(web_status))
        .routes(routes!(get_config))
        .routes(routes!(save_config))
        .routes(routes!(start_primary))
        .routes(routes!(stop_primary))
        .routes(routes!(get_logs))
        .routes(routes!(clear_logs))
        .routes(routes!(poll_primary))
        .routes(routes!(get_instances))
        .routes(routes!(create_instance))
        .routes(routes!(create_draft))
        .routes(routes!(get_instance))
        .routes(routes!(update_instance))
        .routes(routes!(delete_instance))
        .routes(routes!(start_instance))
        .routes(routes!(stop_instance))
        .routes(routes!(restart_instance))
        .routes(routes!(get_instance_logs))
        .routes(routes!(clear_instance_logs))
        .routes(routes!(poll_instance))
}

#[utoipa::path(get, path = "/api/admin/frpc/status", tag = "frpc", operation_id = "get_api_admin_frpc_status", responses((status = 200, description = "FRPC status")))]
pub(super) async fn status(State(state): State<AppState>) -> Response {
    frpc_response(
        &state,
        async {
            let overview = build_overview(&state).await?;
            let primary = overview
                .items
                .iter()
                .find(|item| item.id == overview.primary_instance_id);
            Ok(json!({
                "initialized": overview.initialized,
                "platform": overview.platform,
                "running": primary.map(|item| item.running).unwrap_or(false),
                "pid": primary.and_then(|item| item.pid),
                "desiredRunning": primary.map(|item| item.desired_running).unwrap_or(false),
                "supervisor": primary.map(|item| item.supervisor.clone()).unwrap_or_default(),
                "config_path": primary.map(|item| item.config_path.clone()).unwrap_or_default(),
                "defaults": overview.defaults,
                "total": overview.total,
                "running_count": overview.running_count,
            }))
        }
        .await,
    )
    .await
}

#[utoipa::path(get, path = "/api/admin/frpc/overview", tag = "frpc", operation_id = "get_api_admin_frpc_overview", responses((status = 200, description = "FRPC overview")))]
pub(super) async fn overview(
    State(state): State<AppState>,
    Query(query): Query<LimitQuery>,
) -> Response {
    frpc_response(
        &state,
        async {
            let logs = list_logs_inner(
                &state,
                FRPC_PRIMARY_INSTANCE_ID,
                parse_limit(query.limit.as_deref()),
            )
            .await?;
            Ok(json!({ "tcp": [], "logs": logs }))
        }
        .await,
    )
    .await
}

#[utoipa::path(get, path = "/api/admin/frpc/web-status", tag = "frpc", operation_id = "get_api_admin_frpc_web_status", responses((status = 200, description = "FRPC web status")))]
pub(super) async fn web_status() -> Response {
    response::ok(json!({ "tcp": [] })).into_response()
}

#[utoipa::path(get, path = "/api/admin/frpc/config", tag = "frpc", operation_id = "get_api_admin_frpc_config", responses((status = 200, description = "FRPC configuration")))]
pub(super) async fn get_config(State(state): State<AppState>) -> Response {
    frpc_response(
        &state,
        async {
            let content = read_config(&state, FRPC_PRIMARY_INSTANCE_ID).await?;
            Ok(json!({ "content": content }))
        }
        .await,
    )
    .await
}

#[utoipa::path(post, path = "/api/admin/frpc/config", tag = "frpc", operation_id = "post_api_admin_frpc_config", responses((status = 200, description = "Updated FRPC configuration")))]
pub(super) async fn save_config(
    State(state): State<AppState>,
    Json(body): Json<ConfigBody>,
) -> Response {
    frpc_response_empty(
        &state,
        save_config_inner(&state, FRPC_PRIMARY_INSTANCE_ID, body.content).await,
    )
    .await
}

#[utoipa::path(post, path = "/api/admin/frpc/start", tag = "frpc", operation_id = "post_api_admin_frpc_start", responses((status = 200, description = "Started primary FRPC instance")))]
pub(super) async fn start_primary(State(state): State<AppState>) -> Response {
    frpc_response(
        &state,
        async {
            let pid = start_instance_inner(&state, FRPC_PRIMARY_INSTANCE_ID).await?;
            Ok(json!({ "pid": pid }))
        }
        .await,
    )
    .await
}

#[utoipa::path(post, path = "/api/admin/frpc/stop", tag = "frpc", operation_id = "post_api_admin_frpc_stop", responses((status = 200, description = "Stopped primary FRPC instance")))]
pub(super) async fn stop_primary(State(state): State<AppState>) -> Response {
    frpc_response_empty(
        &state,
        stop_instance_inner(&state, FRPC_PRIMARY_INSTANCE_ID).await,
    )
    .await
}

#[utoipa::path(get, path = "/api/admin/frpc/logs", tag = "frpc", operation_id = "get_api_admin_frpc_logs", responses((status = 200, description = "Primary FRPC logs")))]
pub(super) async fn get_logs(
    State(state): State<AppState>,
    Query(query): Query<LimitQuery>,
) -> Response {
    frpc_response(
        &state,
        async {
            Ok(json!(
                list_logs_inner(
                    &state,
                    FRPC_PRIMARY_INSTANCE_ID,
                    parse_limit(query.limit.as_deref()),
                )
                .await?
            ))
        }
        .await,
    )
    .await
}

#[utoipa::path(delete, path = "/api/admin/frpc/logs", tag = "frpc", operation_id = "delete_api_admin_frpc_logs", responses((status = 200, description = "Cleared primary FRPC logs")))]
pub(super) async fn clear_logs(State(state): State<AppState>) -> Response {
    frpc_response_empty(
        &state,
        clear_logs_inner(&state, FRPC_PRIMARY_INSTANCE_ID).await,
    )
    .await
}

#[utoipa::path(get, path = "/api/admin/frpc/poll", tag = "frpc", operation_id = "get_api_admin_frpc_poll", responses((status = 200, description = "Primary FRPC poll result")))]
pub(super) async fn poll_primary(
    State(state): State<AppState>,
    Query(query): Query<PollQuery>,
) -> Response {
    frpc_response(
        &state,
        async {
            let mut data =
                poll_inner(&state, FRPC_PRIMARY_INSTANCE_ID, query.cursor.as_deref()).await?;
            let overview = build_overview(&state).await?;
            if let Some(status) = data.get_mut("status").and_then(Value::as_object_mut) {
                status.insert("tcp".to_string(), json!([]));
                status.insert("instances".to_string(), serde_json::to_value(overview)?);
            }
            Ok(data)
        }
        .await,
    )
    .await
}

#[utoipa::path(get, path = "/api/admin/frpc/instances", tag = "frpc", operation_id = "get_api_admin_frpc_instances", responses((status = 200, description = "FRPC instances")))]
pub(super) async fn get_instances(State(state): State<AppState>) -> Response {
    frpc_response(
        &state,
        async { Ok(serde_json::to_value(build_overview(&state).await?)?) }.await,
    )
    .await
}

#[utoipa::path(post, path = "/api/admin/frpc/instances/draft", tag = "frpc", operation_id = "post_api_admin_frpc_instances_draft", responses((status = 200, description = "Created FRPC draft")))]
pub(super) async fn create_draft(State(state): State<AppState>) -> Response {
    let _ = state;
    response::ok(json!({ "content": default_frpc_template() })).into_response()
}

#[utoipa::path(post, path = "/api/admin/frpc/instances", tag = "frpc", operation_id = "post_api_admin_frpc_instances", responses((status = 200, description = "Created FRPC instance")))]
pub(super) async fn create_instance(
    State(state): State<AppState>,
    Json(body): Json<InstanceBody>,
) -> Response {
    frpc_response(
        &state,
        async {
            Ok(serde_json::to_value(
                create_instance_inner(&state, body).await?,
            )?)
        }
        .await,
    )
    .await
}

#[utoipa::path(get, path = "/api/admin/frpc/instances/{id}", tag = "frpc", operation_id = "get_api_admin_frpc_instances__id_", params(("id" = String, Path, description = "FRPC instance identifier")), responses((status = 200, description = "FRPC instance")))]
pub(super) async fn get_instance(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Query(query): Query<LimitQuery>,
) -> Response {
    frpc_response(
        &state,
        async {
            let meta = get_meta_or_error(&state, &id).await?;
            let item = build_status(&state, &meta).await?;
            let content = read_config_for_meta(&meta).await?;
            let logs =
                list_logs_inner(&state, &meta.id, parse_limit(query.limit.as_deref())).await?;
            Ok(json!({ "item": item, "content": content, "logs": logs }))
        }
        .await,
    )
    .await
}

#[utoipa::path(put, path = "/api/admin/frpc/instances/{id}", tag = "frpc", operation_id = "put_api_admin_frpc_instances_id", params(("id" = String, Path, description = "FRPC instance identifier")), responses((status = 200, description = "Updated FRPC instance")))]
pub(super) async fn update_instance(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<InstanceBody>,
) -> Response {
    frpc_response(
        &state,
        async {
            Ok(serde_json::to_value(
                update_instance_inner(&state, &id, body).await?,
            )?)
        }
        .await,
    )
    .await
}

#[utoipa::path(delete, path = "/api/admin/frpc/instances/{id}", tag = "frpc", operation_id = "delete_api_admin_frpc_instances_id", params(("id" = String, Path, description = "FRPC instance identifier")), responses((status = 200, description = "Deleted FRPC instance")))]
pub(super) async fn delete_instance(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    frpc_response_empty(&state, delete_instance_inner(&state, &id).await).await
}

#[utoipa::path(post, path = "/api/admin/frpc/instances/{id}/start", tag = "frpc", operation_id = "post_api_admin_frpc_instances_id_start", params(("id" = String, Path, description = "FRPC instance identifier")), responses((status = 200, description = "Started FRPC instance")))]
pub(super) async fn start_instance(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    frpc_response(
        &state,
        async {
            let pid = start_instance_inner(&state, &id).await?;
            Ok(json!({ "pid": pid }))
        }
        .await,
    )
    .await
}

#[utoipa::path(post, path = "/api/admin/frpc/instances/{id}/stop", tag = "frpc", operation_id = "post_api_admin_frpc_instances_id_stop", params(("id" = String, Path, description = "FRPC instance identifier")), responses((status = 200, description = "Stopped FRPC instance")))]
pub(super) async fn stop_instance(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    frpc_response_empty(&state, stop_instance_inner(&state, &id).await).await
}

#[utoipa::path(post, path = "/api/admin/frpc/instances/{id}/restart", tag = "frpc", operation_id = "post_api_admin_frpc_instances_id_restart", params(("id" = String, Path, description = "FRPC instance identifier")), responses((status = 200, description = "Restarted FRPC instance")))]
pub(super) async fn restart_instance(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    frpc_response(
        &state,
        async {
            let pid = restart_instance_inner(&state, &id).await?;
            Ok(json!({ "pid": pid }))
        }
        .await,
    )
    .await
}

#[utoipa::path(get, path = "/api/admin/frpc/instances/{id}/logs", tag = "frpc", operation_id = "get_api_admin_frpc_instances__id__logs", params(("id" = String, Path, description = "FRPC instance identifier")), responses((status = 200, description = "FRPC instance logs")))]
pub(super) async fn get_instance_logs(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Query(query): Query<LimitQuery>,
) -> Response {
    frpc_response(
        &state,
        async {
            Ok(json!(
                list_logs_inner(&state, &id, parse_limit(query.limit.as_deref())).await?
            ))
        }
        .await,
    )
    .await
}

#[utoipa::path(delete, path = "/api/admin/frpc/instances/{id}/logs", tag = "frpc", operation_id = "delete_api_admin_frpc_instances_id_logs", params(("id" = String, Path, description = "FRPC instance identifier")), responses((status = 200, description = "Cleared FRPC instance logs")))]
pub(super) async fn clear_instance_logs(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    frpc_response_empty(&state, clear_logs_inner(&state, &id).await).await
}

#[utoipa::path(get, path = "/api/admin/frpc/instances/{id}/poll", tag = "frpc", operation_id = "get_api_admin_frpc_instances__id__poll", params(("id" = String, Path, description = "FRPC instance identifier")), responses((status = 200, description = "FRPC instance poll result")))]
pub(super) async fn poll_instance(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Query(query): Query<PollQuery>,
) -> Response {
    frpc_response(
        &state,
        poll_inner(&state, &id, query.cursor.as_deref()).await,
    )
    .await
}

pub(super) async fn frpc_response(state: &AppState, result: FrpcResult<Value>) -> Response {
    let translator = Translator::from_state(state).await;
    match result {
        Ok(value) => response::ok(localize_frpc_response_value(value, &translator)).into_response(),
        Err(error) => response::error(
            error.status,
            localize_frpc_error(&translator, &error.message),
        ),
    }
}

pub(super) async fn frpc_response_empty(state: &AppState, result: FrpcResult<()>) -> Response {
    let translator = Translator::from_state(state).await;
    match result {
        Ok(()) => response::success_empty().into_response(),
        Err(error) => response::error(
            error.status,
            localize_frpc_error(&translator, &error.message),
        ),
    }
}
