use super::*;

pub(super) async fn status(State(state): State<AppState>) -> Response {
    if let Some(response) = terminal_unavailable_response(&state).await {
        return response;
    }
    match runtime_status(&state).await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => terminal_error(&state, error).await,
    }
}

pub(super) async fn install_tmux(State(state): State<AppState>) -> Response {
    if let Some(response) = terminal_unavailable_response(&state).await {
        return response;
    }
    match start_tmux_install().await {
        Ok(mut data) => {
            let translator = Translator::from_state(&state).await;
            localize_tmux_install_state(&mut data, &translator);
            response::ok(data).into_response()
        }
        Err(error) => terminal_error(&state, error).await,
    }
}

pub(super) async fn list_sessions(State(state): State<AppState>) -> Response {
    if let Some(response) = terminal_unavailable_response(&state).await {
        return response;
    }
    match terminal_list_sessions(&state).await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => terminal_error(&state, error).await,
    }
}

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
