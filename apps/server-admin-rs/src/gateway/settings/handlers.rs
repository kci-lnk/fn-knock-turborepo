use super::*;

pub(super) async fn get_gateway(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match build_gateway_settings_response(&state).await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to load gateway settings");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                gateway_route_text(&translator, "loadGatewaySettingsFailed"),
            )
        }
    }
}

pub(super) async fn update_gateway(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let Some(patch) = body.as_object() else {
        return response::error(
            StatusCode::BAD_REQUEST,
            gateway_route_text(&translator, "payloadObjectRequired"),
        );
    };

    let previous_config = match state.redis.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load config before gateway update");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                gateway_route_text(&translator, "loadConfigFailed"),
            );
        }
    };

    let mut updated_config = previous_config.clone();
    apply_gateway_patch(&mut updated_config, patch);

    if let Err(error) = state.redis.save_config(&updated_config).await {
        tracing::warn!(%error, "failed to save gateway settings");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            gateway_route_text(&translator, "saveGatewaySettingsFailed"),
        );
    }

    if let Err(message) = sync_gateway_runtime(&state, &updated_config).await {
        rollback_gateway_settings(&state, &previous_config).await;
        tracing::warn!(%message, "failed to sync gateway settings runtime");
        return response::error(
            StatusCode::BAD_GATEWAY,
            gateway_route_text_params(
                &translator,
                "syncGatewaySettingsFailed",
                &[("message", message)],
            ),
        );
    }
    whitelist::sync_reverse_proxy_trusted_ips(&state).await;

    if let Err(message) =
        apply_gateway_portal_host_rules_patches_if_needed(&state, &updated_config).await
    {
        rollback_gateway_settings(&state, &previous_config).await;
        tracing::warn!(%message, "failed to apply gateway portal host-rules patches");
        return response::error(
            StatusCode::BAD_GATEWAY,
            gateway_route_text_params(
                &translator,
                "syncGatewaySettingsFailed",
                &[("message", message)],
            ),
        );
    }

    match build_gateway_settings_response_from_config(&state, updated_config).await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to reload gateway settings after update");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                gateway_route_text(&translator, "responseReloadFailed"),
            )
        }
    }
}

pub(super) async fn get_gateway_visibility(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match get_gateway_visibility_details(&state).await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to load gateway visibility details");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                gateway_route_text(&translator, "loadGatewayVisibilityFailed"),
            )
        }
    }
}

pub(super) async fn update_gateway_visibility(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let previous_config = match state.redis.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load config before gateway visibility update");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                gateway_route_text(&translator, "loadConfigFailed"),
            );
        }
    };
    let previous_runtime = match state
        .redis
        .get_json_value(GATEWAY_VISIBILITY_RUNTIME_KEY)
        .await
    {
        Ok(runtime) => runtime.unwrap_or_else(default_gateway_visibility_runtime),
        Err(error) => {
            tracing::warn!(%error, "failed to load gateway visibility runtime before update");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                gateway_route_text(&translator, "loadRuntimeFailed"),
            );
        }
    };

    match update_gateway_visibility_inner(&state, &body).await {
        Ok(data) => response::ok(data).into_response(),
        Err(message) => {
            let rollback_error = rollback_gateway_visibility(
                &state,
                &previous_config,
                &previous_runtime,
                &translator,
            )
            .await;
            let message = rollback_message(
                &translator,
                &message,
                rollback_error.as_deref(),
                "server.admin.gatewayVisibility.updateFailedRolledBack",
            );
            response::error(StatusCode::BAD_GATEWAY, message)
        }
    }
}

pub(super) async fn get_gateway_proxy_headers(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match get_gateway_proxy_headers_details(&state, &translator).await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to load gateway proxy headers details");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                gateway_route_text(&translator, "loadGatewayProxyHeadersFailed"),
            )
        }
    }
}

pub(super) async fn update_gateway_proxy_headers(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let previous_config = match state.redis.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load config before gateway proxy headers update");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                gateway_route_text(&translator, "loadConfigFailed"),
            );
        }
    };
    if !is_any_subdomain_routing_mode(&previous_config) {
        return response::error(
            StatusCode::BAD_REQUEST,
            translator.t("server.admin.gatewayProxyHeaders.subdomainOnly"),
        );
    }
    let previous_runtime = match state
        .redis
        .get_json_value(GATEWAY_PROXY_HEADERS_RUNTIME_KEY)
        .await
    {
        Ok(runtime) => runtime.unwrap_or_else(default_gateway_proxy_headers_runtime),
        Err(error) => {
            tracing::warn!(%error, "failed to load gateway proxy headers runtime before update");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                gateway_route_text(&translator, "loadRuntimeFailed"),
            );
        }
    };

    match update_gateway_proxy_headers_inner(&state, &previous_config, &body).await {
        Ok(data) => response::ok(data).into_response(),
        Err(message) => {
            let rollback_error = rollback_gateway_proxy_headers(
                &state,
                &previous_config,
                &previous_runtime,
                &translator,
            )
            .await;
            let message = rollback_message(
                &translator,
                &message,
                rollback_error.as_deref(),
                "server.admin.gatewayProxyHeaders.updateFailedRolledBack",
            );
            response::error(StatusCode::BAD_GATEWAY, message)
        }
    }
}

pub(super) async fn get_gateway_host_response(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match get_gateway_host_response_details(&state, &translator).await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to load gateway host response details");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                gateway_route_text(&translator, "loadGatewayHostResponseFailed"),
            )
        }
    }
}

pub(super) async fn update_gateway_host_response(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let previous_config = match state.redis.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load config before gateway host response update");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                gateway_route_text(&translator, "loadConfigFailed"),
            );
        }
    };
    if !is_any_subdomain_routing_mode(&previous_config) {
        return response::error(
            StatusCode::BAD_REQUEST,
            translator.t("server.gatewayHostResponse.editSubdomainOnly"),
        );
    }
    let previous_runtime = match state
        .redis
        .get_json_value(GATEWAY_HOST_RESPONSE_RUNTIME_KEY)
        .await
    {
        Ok(runtime) => runtime.unwrap_or_else(default_gateway_host_response_runtime),
        Err(error) => {
            tracing::warn!(%error, "failed to load gateway host response runtime before update");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                gateway_route_text(&translator, "loadRuntimeFailed"),
            );
        }
    };

    match update_gateway_host_response_inner(&state, &previous_config, &body, &translator).await {
        Ok(data) => response::ok(data).into_response(),
        Err(message) => {
            let rollback_error = rollback_gateway_host_response(
                &state,
                &previous_config,
                &previous_runtime,
                &translator,
            )
            .await;
            let message = rollback_message(
                &translator,
                &message,
                rollback_error.as_deref(),
                "server.gatewayHostResponse.updateFailedRolledBack",
            );
            response::error(StatusCode::BAD_GATEWAY, message)
        }
    }
}
