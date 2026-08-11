use super::*;

#[utoipa::path(get, path = "/api/admin/terminal/status", tag = "terminal", operation_id = "get_api_admin_terminal_status", responses((status = 200, description = "Terminal runtime status")))]
pub(super) async fn status(State(state): State<AppState>) -> Response {
    if let Some(response) = terminal_unavailable_response(&state).await {
        return response;
    }
    match runtime_status(&state).await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => terminal_error(&state, error).await,
    }
}

#[utoipa::path(post, path = "/api/admin/terminal/tmux/install", tag = "terminal", operation_id = "post_api_admin_terminal_tmux_install", responses((status = 200, description = "tmux installation state")))]
pub(super) async fn install_tmux(State(state): State<AppState>) -> Response {
    if let Some(response) = terminal_unavailable_response(&state).await {
        return response;
    }
    match start_tmux_install(&state).await {
        Ok(mut data) => {
            let translator = Translator::from_state(&state).await;
            localize_tmux_install_state(&mut data, &translator);
            response::ok(data).into_response()
        }
        Err(error) => terminal_error(&state, error).await,
    }
}

#[utoipa::path(get, path = "/api/admin/terminal/sessions", tag = "terminal", operation_id = "get_api_admin_terminal_sessions", responses((status = 200, description = "Terminal sessions")))]
pub(super) async fn list_sessions(State(state): State<AppState>) -> Response {
    if let Some(response) = terminal_unavailable_response(&state).await {
        return response;
    }
    match terminal_list_sessions(&state).await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => terminal_error(&state, error).await,
    }
}

#[utoipa::path(
    get,
    path = "/api/admin/terminal/sessions/{id}",
    tag = "terminal",
    operation_id = "get_api_admin_terminal_sessions_by_id",
    params(("id" = String, Path, description = "Terminal session identifier")),
    responses((status = 200, description = "Terminal session"))
)]
pub(super) async fn get_session(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    if let Some(response) = terminal_unavailable_response(&state).await {
        return response;
    }
    match terminal_get_session(&state, &id).await {
        Ok(Some(data)) => response::ok(data).into_response(),
        Ok(None) => {
            let translator = Translator::from_state(&state).await;
            response::error(
                StatusCode::NOT_FOUND,
                terminal_text(&translator, "sessionNotFound", &[]),
            )
        }
        Err(error) => terminal_error(&state, error).await,
    }
}

#[utoipa::path(post, path = "/api/admin/terminal/sessions", tag = "terminal", operation_id = "post_api_admin_terminal_sessions", responses((status = 200, description = "Created terminal session")))]
pub(super) async fn create_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateSessionBody>,
) -> Response {
    if let Some(response) = terminal_unavailable_response(&state).await {
        return response;
    }
    let client_ip = detect_client_ip(&headers);
    match terminal_create_session(&state, body, &client_ip).await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => terminal_error(&state, error).await,
    }
}

#[utoipa::path(
    patch,
    path = "/api/admin/terminal/sessions/{id}",
    tag = "terminal",
    operation_id = "patch_api_admin_terminal_sessions_by_id",
    request_body = RenameSessionBody,
    params(("id" = String, Path, description = "Terminal session identifier")),
    responses((status = 200, description = "Renamed terminal session"))
)]
pub(super) async fn rename_session(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<RenameSessionBody>,
) -> Response {
    if let Some(response) = terminal_unavailable_response(&state).await {
        return response;
    }
    match terminal_rename_session(&state, &id, &body.title).await {
        Ok(Some(data)) => response::ok(data).into_response(),
        Ok(None) => {
            let translator = Translator::from_state(&state).await;
            response::error(
                StatusCode::NOT_FOUND,
                terminal_text(&translator, "sessionNotFound", &[]),
            )
        }
        Err(error) => terminal_error(&state, error).await,
    }
}

#[utoipa::path(
    delete,
    path = "/api/admin/terminal/sessions/{id}",
    tag = "terminal",
    operation_id = "delete_api_admin_terminal_sessions_by_id",
    params(("id" = String, Path, description = "Terminal session identifier")),
    responses((status = 200, description = "Deleted terminal session"))
)]
pub(super) async fn delete_session(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    if let Some(response) = terminal_unavailable_response(&state).await {
        return response;
    }
    match terminal_kill_session(&state, &id).await {
        Ok(()) => response::success_empty().into_response(),
        Err(error) => terminal_error(&state, error).await,
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/terminal/sessions/{id}/attachments",
    tag = "terminal",
    operation_id = "post_api_admin_terminal_sessions_by_id_attachments",
    params(("id" = String, Path, description = "Terminal session identifier")),
    responses((status = 200, description = "Created terminal attachment"))
)]
pub(super) async fn create_attachment(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Response {
    if let Some(response) = terminal_unavailable_response(&state).await {
        return response;
    }
    let client_ip = detect_client_ip(&headers);
    match terminal_create_attachment(&state, &id, &client_ip).await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => terminal_error(&state, error).await,
    }
}

#[utoipa::path(
    get,
    path = "/api/admin/terminal/attachments/{id}/poll",
    tag = "terminal",
    operation_id = "get_api_admin_terminal_attachments__id__poll",
    params(
        ("id" = String, Path, description = "Terminal attachment identifier"),
        ("cursor" = Option<String>, Query, description = "Byte cursor"),
        ("timeout_ms" = Option<f64>, Query, description = "Long-poll timeout in milliseconds")
    ),
    responses((status = 200, description = "Terminal attachment output"))
)]
pub(super) async fn poll_attachment(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Query(query): Query<PollQuery>,
) -> Response {
    if let Some(response) = terminal_unavailable_response(&state).await {
        return response;
    }
    match terminal_wait_for_output(
        &state,
        &id,
        parse_output_cursor_like_node(query.cursor.as_deref()),
        query.timeout_ms,
    )
    .await
    {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => terminal_error(&state, error).await,
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/terminal/attachments/{id}/input",
    tag = "terminal",
    operation_id = "post_api_admin_terminal_attachments_by_id_input",
    request_body = InputBody,
    params(("id" = String, Path, description = "Terminal attachment identifier")),
    responses((status = 200, description = "Terminal input accepted"))
)]
pub(super) async fn send_input(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<InputBody>,
) -> Response {
    if let Some(response) = terminal_unavailable_response(&state).await {
        return response;
    }
    match terminal_send_input(&state, &id, &body.data_base64).await {
        Ok(()) => response::success_empty().into_response(),
        Err(error) => terminal_error(&state, error).await,
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/terminal/attachments/{id}/resize",
    tag = "terminal",
    operation_id = "post_api_admin_terminal_attachments_by_id_resize",
    request_body = ResizeBody,
    params(("id" = String, Path, description = "Terminal attachment identifier")),
    responses((status = 200, description = "Resized terminal session"))
)]
pub(super) async fn resize_attachment(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<ResizeBody>,
) -> Response {
    if let Some(response) = terminal_unavailable_response(&state).await {
        return response;
    }
    match terminal_resize_attachment(&state, &id, body.cols, body.rows).await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => terminal_error(&state, error).await,
    }
}

#[utoipa::path(
    delete,
    path = "/api/admin/terminal/attachments/{id}",
    tag = "terminal",
    operation_id = "delete_api_admin_terminal_attachments_by_id",
    params(("id" = String, Path, description = "Terminal attachment identifier")),
    responses((status = 200, description = "Deleted terminal attachment"))
)]
pub(super) async fn delete_attachment(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    if let Some(response) = terminal_unavailable_response(&state).await {
        return response;
    }
    match terminal_detach_attachment(&state, &id).await {
        Ok(()) => response::success_empty().into_response(),
        Err(error) => terminal_error(&state, error).await,
    }
}

pub(super) async fn terminal_unavailable_response(state: &AppState) -> Option<Response> {
    if runtime_profile::terminal_available(state) {
        return None;
    }
    let translator = Translator::from_state(state).await;
    let profile = runtime_profile::get_runtime_profile(state);
    Some(response::error(
        StatusCode::FORBIDDEN,
        runtime_profile::capability_unavailable_message(
            "terminal_available",
            &profile,
            &translator,
        ),
    ))
}

pub(super) async fn terminal_error(state: &AppState, error: anyhow::Error) -> Response {
    let translator = Translator::from_state(state).await;
    response::error(
        StatusCode::INTERNAL_SERVER_ERROR,
        localize_terminal_error(&translator, &error.to_string()),
    )
}
