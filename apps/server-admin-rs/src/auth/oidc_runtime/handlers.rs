use super::*;

pub(super) async fn bind(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: Uri,
    Query(query): Query<BindQuery>,
) -> Response {
    let config = match state.redis.get_config().await {
        Ok(config) => config,
        Err(error) => {
            let translator = Translator::from_state(&state).await;
            tracing::warn!(%error, "failed to load config for OIDC bind");
            return bind_html_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &oidc_text(&translator, "bindFailedTitle"),
                &oidc_text(&translator, "loadConfigFailed"),
                DEFAULT_LOCALE,
                None,
            );
        }
    };
    let translator = translator_from_config(&config);
    let locale = locale_code(&config);
    let token = query.token.as_deref().map(str::trim).unwrap_or("");
    if token.is_empty() {
        return bind_html_response(
            StatusCode::BAD_REQUEST,
            &oidc_text(&translator, "inviteInvalid"),
            &oidc_text(&translator, "linkMissingToken"),
            &locale,
            None,
        );
    }

    let invite = match oidc_inspect_invite(&state, token).await {
        Ok(Some(invite)) => invite,
        Ok(None) => {
            return bind_html_response(
                StatusCode::NOT_FOUND,
                &oidc_text(&translator, "inviteExpired"),
                &oidc_text(&translator, "inviteMissingExpiredUsed"),
                &locale,
                None,
            );
        }
        Err(error) => {
            tracing::warn!(%error, "failed to inspect OIDC invite before bind");
            return bind_html_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &oidc_text(&translator, "bindFailedTitle"),
                &oidc_text(&translator, "bindStartFailed"),
                &locale,
                None,
            );
        }
    };
    let providers = invite
        .get("providers")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if providers.is_empty() {
        return bind_html_response(
            StatusCode::NOT_FOUND,
            &oidc_text(&translator, "noProvidersTitle"),
            &oidc_text(&translator, "noProvidersBody"),
            &locale,
            None,
        );
    }
    let selected_provider = query
        .provider_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| invite.get("provider_id").and_then(Value::as_str))
        .or_else(|| {
            (providers.len() == 1).then(|| providers[0].get("id").and_then(Value::as_str))?
        });
    let Some(provider_id) = selected_provider else {
        return bind_provider_selection_response(
            &uri,
            token,
            &invite,
            &providers,
            &translator,
            &locale,
        );
    };

    match build_authorization_url(
        &state,
        &headers,
        &uri,
        &config,
        &translator,
        provider_id,
        "bind",
        None,
        Some(token),
        false,
    )
    .await
    {
        Ok(result) => {
            let domain = resolve_cookie_domain(&config, &headers);
            let path = resolve_oidc_cookie_path(&config, &headers, uri.path());
            redirect_response(
                &result.authorization_url,
                vec![cookies::oidc_flow_cookie(
                    &result.flow_token,
                    result.max_age as i64,
                    domain.as_deref(),
                    &path,
                )],
            )
        }
        Err(error) => bind_html_response(
            StatusCode::BAD_REQUEST,
            &oidc_text(&translator, "bindFailedTitle"),
            &error,
            &locale,
            None,
        ),
    }
}

pub(super) async fn start(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: Uri,
    Json(body): Json<StartBody>,
) -> Response {
    let config = match state.redis.get_config().await {
        Ok(config) => config,
        Err(error) => {
            let translator = Translator::from_state(&state).await;
            tracing::warn!(%error, "failed to load config before OIDC start");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                oidc_text(&translator, "loadConfigFailed"),
            );
        }
    };
    let translator = translator_from_config(&config);
    let mode = match body.mode.as_deref().unwrap_or("login") {
        "login" | "bind" => body.mode.as_deref().unwrap_or("login"),
        _ => "login",
    };
    match build_authorization_url(
        &state,
        &headers,
        &uri,
        &config,
        &translator,
        &body.provider_id,
        mode,
        body.redirect_uri.as_deref(),
        body.invite_token.as_deref(),
        body.remember_me,
    )
    .await
    {
        Ok(result) => {
            let domain = resolve_cookie_domain(&config, &headers);
            let path = resolve_oidc_cookie_path(&config, &headers, uri.path());
            let cookie = cookies::oidc_flow_cookie(
                &result.flow_token,
                result.max_age as i64,
                domain.as_deref(),
                &path,
            );
            let mut response = Json(ApiEnvelope {
                success: true,
                code: None,
                message: None,
                data: Some(json!({ "authorization_url": result.authorization_url })),
            })
            .into_response();
            apply_no_store_headers(response.headers_mut());
            append_set_cookie(response.headers_mut(), &cookie);
            response
        }
        Err(error) => response::error(StatusCode::BAD_REQUEST, error),
    }
}

#[axum::debug_handler(state = AppState)]
pub(super) async fn callback(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: Uri,
    Path(provider_id): Path<String>,
    Query(query): Query<CallbackQuery>,
) -> Response {
    let config = match state.redis.get_config().await {
        Ok(config) => config,
        Err(error) => {
            let translator = Translator::from_state(&state).await;
            tracing::warn!(%error, "failed to load config before OIDC callback");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                oidc_text(&translator, "loadConfigFailed"),
            );
        }
    };
    let translator = translator_from_config(&config);
    let code = query
        .code
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let state_token = query
        .state
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let flow_token = cookies::read_cookie(&headers, cookies::OIDC_FLOW_COOKIE_NAME);
    let clear_flow_cookie = || {
        state_token
            .filter(|state| oidc_flow_token_valid(state, flow_token.as_deref()))
            .map(|_| {
                let domain = resolve_cookie_domain(&config, &headers);
                let path = resolve_oidc_cookie_path(&config, &headers, uri.path());
                cookies::oidc_flow_clear_cookie(domain.as_deref(), &path)
            })
    };

    if let Some(error) = query.error.as_deref() {
        let auth_state = consume_callback_state_for_notice(
            &state,
            &provider_id,
            state_token,
            flow_token.as_deref(),
        )
        .await;
        return login_error_redirect_response(
            &state,
            &headers,
            &uri,
            &config,
            provider_error_message(error, &translator),
            &translator,
            auth_state
                .as_ref()
                .and_then(|value| value.get("redirect_uri"))
                .and_then(Value::as_str),
            auth_state.is_some(),
            clear_flow_cookie(),
        )
        .await;
    }

    let Some(code) = code else {
        let auth_state = consume_callback_state_for_notice(
            &state,
            &provider_id,
            state_token,
            flow_token.as_deref(),
        )
        .await;
        return login_error_redirect_response(
            &state,
            &headers,
            &uri,
            &config,
            oidc_text(&translator, "callbackMissingParams"),
            &translator,
            auth_state
                .as_ref()
                .and_then(|value| value.get("redirect_uri"))
                .and_then(Value::as_str),
            auth_state.is_some(),
            clear_flow_cookie(),
        )
        .await;
    };
    let Some(state_token) = state_token else {
        return login_error_redirect_response(
            &state,
            &headers,
            &uri,
            &config,
            oidc_text(&translator, "callbackMissingParams"),
            &translator,
            None,
            false,
            None,
        )
        .await;
    };

    let client_ip = client_ip_for_headers(&headers);
    let tracking_ip = normalize_auth_failure_tracking_ip(&client_ip);
    match state.redis.get_login_backoff_status(&tracking_ip).await {
        Ok(status) if status.blocked => {
            let auth_state = consume_callback_state_for_notice(
                &state,
                &provider_id,
                Some(state_token),
                flow_token.as_deref(),
            )
            .await;
            let message = status
                .retry_after
                .map(|retry_after| {
                    server_text_params(
                        &translator,
                        "tooManyAttemptsWithRetry",
                        &[("seconds", retry_after.max(1).to_string())],
                    )
                })
                .unwrap_or_else(|| server_text(&translator, "tooManyAttempts"));
            return login_error_redirect_response(
                &state,
                &headers,
                &uri,
                &config,
                message,
                &translator,
                auth_state
                    .as_ref()
                    .and_then(|value| value.get("redirect_uri"))
                    .and_then(Value::as_str),
                auth_state.is_some(),
                clear_flow_cookie(),
            )
            .await;
        }
        Ok(_) => {}
        Err(error) => tracing::warn!(%error, %tracking_ip, "failed to inspect OIDC backoff"),
    }

    match resolve_callback(
        &state,
        &headers,
        &uri,
        &config,
        &translator,
        &provider_id,
        code,
        state_token,
        flow_token.as_deref(),
    )
    .await
    {
        Ok(resolved) => {
            let redirect_to = resolved
                .state
                .get("redirect_uri")
                .and_then(Value::as_str)
                .unwrap_or("/");
            match create_oidc_session_response(
                &state,
                &headers,
                &config,
                &resolved,
                &translator,
                redirect_to,
                clear_flow_cookie(),
            )
            .await
            {
                Ok(response) => response,
                Err(error) => {
                    tracing::warn!(%error, "failed to create OIDC session");
                    login_error_redirect_response(
                        &state,
                        &headers,
                        &uri,
                        &config,
                        oidc_text(&translator, "loginFailedRetry"),
                        &translator,
                        Some(redirect_to),
                        true,
                        clear_flow_cookie(),
                    )
                    .await
                }
            }
        }
        Err(error) => {
            if error == oidc_text(&translator, "callbackStateExpired") {
                login_error_redirect_response(
                    &state,
                    &headers,
                    &uri,
                    &config,
                    error,
                    &translator,
                    None,
                    false,
                    clear_flow_cookie(),
                )
                .await
            } else if is_oidc_operation_aborted_error(&error) {
                login_error_redirect_response(
                    &state,
                    &headers,
                    &uri,
                    &config,
                    oidc_text(&translator, "operationAborted"),
                    &translator,
                    None,
                    true,
                    clear_flow_cookie(),
                )
                .await
            } else {
                let detail_message = error;
                let response_message = match state
                    .redis
                    .register_login_backoff_failure(&tracking_ip)
                    .await
                {
                    Ok(failure) => {
                        let retry_after = failure.retry_after.unwrap_or(1).max(1);
                        if let Err(event_error) = system_events::publish_auth_login_failure_event(
                            &state,
                            json!({
                                "ip": tracking_ip.clone(),
                                "attempts": failure.attempts,
                                "retry_after_seconds": retry_after,
                                "blocked_until": failure.blocked_until.map(time_utils::iso_from_ms),
                                "method": "OIDC",
                                "credential_name": provider_id.clone(),
                                "user_agent": user_agent(&headers),
                            }),
                        )
                        .await
                        {
                            tracing::warn!(%event_error, %tracking_ip, "failed to publish OIDC login failure event");
                        }
                        oidc_login_failed_retry_after_message(
                            &translator,
                            &detail_message,
                            retry_after,
                        )
                    }
                    Err(backoff_error) => {
                        tracing::warn!(%backoff_error, %tracking_ip, "failed to register OIDC login failure");
                        detail_message
                    }
                };
                login_error_redirect_response(
                    &state,
                    &headers,
                    &uri,
                    &config,
                    response_message,
                    &translator,
                    None,
                    true,
                    clear_flow_cookie(),
                )
                .await
            }
        }
    }
}
