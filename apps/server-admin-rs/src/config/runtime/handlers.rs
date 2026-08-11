use super::*;

#[utoipa::path(
    get,
    path = "/api/admin/config/captcha",
    tag = "config",
    operation_id = "get_api_admin_config_captcha",
    responses((status = 200, description = "CAPTCHA configuration"))
)]
pub(super) async fn get_captcha(State(state): State<AppState>) -> Response {
    match load_captcha_settings(&state).await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => {
            let translator = Translator::from_state(&state).await;
            tracing::warn!(%error, "failed to load captcha config");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                runtime_config_route_text(&translator, "loadCaptchaFailed"),
            )
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/config/captcha",
    tag = "config",
    operation_id = "post_api_admin_config_captcha",
    request_body = serde_json::Value,
    responses((status = 200, description = "Updated CAPTCHA configuration"))
)]
pub(super) async fn update_captcha(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    if body.get("provider").and_then(Value::as_str) == Some("turnstile") {
        let site_key = body
            .pointer("/turnstile/site_key")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        let secret_key = body
            .pointer("/turnstile/secret_key")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        if site_key.is_empty() || secret_key.is_empty() {
            return response::error(
                StatusCode::BAD_REQUEST,
                admin_text(&translator, "captcha.turnstileKeysRequired"),
            );
        }
    }

    // Validation and the read-modify-write update must observe the same
    // snapshot so concurrent partial updates cannot lose each other's fields.
    let _update_guard = state.security.captcha_settings_update_lock.lock().await;
    let current = match load_captcha_settings(&state).await {
        Ok(current) => current,
        Err(error) => {
            tracing::warn!(%error, "failed to load captcha config before validation");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                runtime_config_route_text(&translator, "loadCaptchaFailed"),
            );
        }
    };
    if let Err(message_key) = validate_pow_captcha_patch(&current, &body) {
        return response::error(
            StatusCode::BAD_REQUEST,
            admin_text(&translator, message_key),
        );
    }

    match update_captcha_settings(&state, &body).await {
        Ok(data) => {
            common_auth_locations::schedule_common_auth_locations_rebuild(
                state.clone(),
                "captcha-settings",
            );
            response::ok(data).into_response()
        }
        Err(error) => {
            tracing::warn!(%error, "failed to save captcha config");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                runtime_config_route_text(&translator, "saveCaptchaFailed"),
            )
        }
    }
}

pub(super) fn validate_pow_captcha_patch(
    current: &Value,
    patch: &Value,
) -> Result<(), &'static str> {
    if patch.get("pow").is_some_and(|value| !value.is_object())
        || patch
            .pointer("/pow/uncommon_location")
            .is_some_and(|value| !value.is_object())
    {
        return Err("captcha.powDifficultyInvalid");
    }
    let current_base = current
        .pointer("/pow/base_max_number")
        .and_then(Value::as_i64)
        .unwrap_or(POW_DEFAULT_BASE_MAX_NUMBER);
    let current_uncommon = current
        .pointer("/pow/uncommon_location/max_number")
        .and_then(Value::as_i64)
        .unwrap_or(POW_DEFAULT_UNCOMMON_MAX_NUMBER);
    let base = validate_optional_pow_number(patch.pointer("/pow/base_max_number"), current_base)?;
    let uncommon = validate_optional_pow_number(
        patch.pointer("/pow/uncommon_location/max_number"),
        current_uncommon,
    )?;
    if let Some(enabled) = patch.pointer("/pow/uncommon_location/enabled")
        && !enabled.is_boolean()
    {
        return Err("captcha.powEnabledBooleanRequired");
    }
    if uncommon < base {
        return Err("captcha.powUncommonDifficultyTooLow");
    }
    Ok(())
}

fn validate_optional_pow_number(value: Option<&Value>, fallback: i64) -> Result<i64, &'static str> {
    let Some(value) = value else {
        return Ok(fallback);
    };
    let Some(value) = value.as_i64() else {
        return Err("captcha.powDifficultyInvalid");
    };
    if !(POW_MIN_MAX_NUMBER..=POW_MAX_MAX_NUMBER).contains(&value)
        || value % POW_MAX_NUMBER_STEP != 0
    {
        return Err("captcha.powDifficultyInvalid");
    }
    Ok(value)
}

#[utoipa::path(
    get,
    path = "/api/admin/config/terminal_feature",
    tag = "config",
    operation_id = "get_api_admin_config_terminal_feature",
    responses((status = 200, description = "Terminal feature configuration"))
)]
pub(super) async fn get_terminal_feature(State(state): State<AppState>) -> Response {
    match load_config_section(&state, "terminal_feature", normalize_terminal_feature).await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => {
            let translator = Translator::from_state(&state).await;
            tracing::warn!(%error, "failed to load terminal feature config");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                runtime_config_route_text(&translator, "loadTerminalFeatureFailed"),
            )
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/config/terminal_feature",
    tag = "config",
    operation_id = "post_api_admin_config_terminal_feature",
    responses((status = 200, description = "Updated terminal feature configuration"))
)]
pub(super) async fn update_terminal_feature(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    if body.get("enabled").and_then(Value::as_bool) == Some(true)
        && !runtime_profile::terminal_available(&state)
    {
        return response::error(
            StatusCode::FORBIDDEN,
            capability_blocked_text(&state, "terminal_available", &translator),
        );
    }
    match update_config_section(
        &state,
        "terminal_feature",
        &body,
        normalize_terminal_feature,
    )
    .await
    {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to save terminal feature config");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                runtime_config_route_text(&translator, "saveTerminalFeatureFailed"),
            )
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/admin/config/wol_feature",
    tag = "config",
    operation_id = "get_api_admin_config_wol_feature",
    responses((status = 200, description = "Wake-on-LAN feature configuration"))
)]
pub(super) async fn get_wol_feature(State(state): State<AppState>) -> Response {
    match load_config_section(&state, "wol_feature", normalize_wol_feature).await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => {
            let translator = Translator::from_state(&state).await;
            tracing::warn!(%error, "failed to load WoL feature config");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                runtime_config_route_text(&translator, "loadWolFeatureFailed"),
            )
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/config/wol_feature",
    tag = "config",
    operation_id = "post_api_admin_config_wol_feature",
    request_body = serde_json::Value,
    responses((status = 200, description = "Updated Wake-on-LAN feature configuration"))
)]
pub(super) async fn update_wol_feature(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    if !body.is_object() || body.get("enabled").is_some_and(|value| !value.is_boolean()) {
        return response::error(
            StatusCode::BAD_REQUEST,
            runtime_config_route_text(&translator, "invalidWolFeature"),
        );
    }

    let _guard = state.wol.feature_update_lock.lock().await;
    let previous_config = match state.storage.store.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load config before WoL feature update");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                runtime_config_route_text(&translator, "loadWolFeatureFailed"),
            );
        }
    };
    let previously_enabled = crate::wol::feature_enabled(&previous_config);
    let mut updated_config = previous_config.clone();
    if !updated_config.is_object() {
        updated_config = app_store::default_config();
    }
    let mut next = normalize_wol_feature(updated_config.get("wol_feature"));
    merge_object(&mut next, &body);
    next = normalize_wol_feature(Some(&next));
    let next_enabled = next
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    ensure_object(&mut updated_config).insert("wol_feature".to_string(), next.clone());

    if let Err(error) = state.storage.store.save_config(&updated_config).await {
        tracing::warn!(%error, "failed to save WoL feature config");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            runtime_config_route_text(&translator, "saveWolFeatureFailed"),
        );
    }
    let stopped_before_gateway_sync = previously_enabled && !next_enabled;
    if stopped_before_gateway_sync {
        notify_wol_runtime_reload(&state);
    }
    if let Err(message) = gateway_settings::sync_gateway_runtime(&state, &updated_config).await {
        let rollback_saved = state
            .storage
            .store
            .save_config(&previous_config)
            .await
            .is_ok();
        if rollback_saved {
            let _ = gateway_settings::sync_gateway_runtime(&state, &previous_config).await;
            if stopped_before_gateway_sync {
                notify_wol_runtime_reload(&state);
            }
        }
        tracing::warn!(%message, "failed to sync WoL feature to Go gateway");
        return response::error(
            StatusCode::BAD_GATEWAY,
            runtime_config_route_text(&translator, "syncWolFeatureFailed"),
        );
    }

    if previously_enabled != next_enabled && !stopped_before_gateway_sync {
        notify_wol_runtime_reload(&state);
    }
    response::ok(next).into_response()
}

fn notify_wol_runtime_reload(state: &AppState) {
    state
        .wol
        .runtime_reload
        .send_modify(|generation| *generation = generation.wrapping_add(1));
}

#[utoipa::path(
    post,
    path = "/api/admin/config/run_type",
    tag = "config",
    operation_id = "post_api_admin_config_run_type",
    request_body = serde_json::Value,
    responses((status = 200, description = "Updated runtime mode"))
)]
pub(super) async fn update_run_type(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let Some(run_type) = normalize_run_type(body.get("run_type")) else {
        return response::error(
            StatusCode::BAD_REQUEST,
            runtime_config_route_text(&translator, "invalidRunType"),
        );
    };
    if run_type == 0 && !host_firewall_available(&state) {
        return response::error(
            StatusCode::FORBIDDEN,
            capability_blocked_text(&state, "direct_mode_available", &translator),
        );
    }
    let _protocol_mapping_guard = state.gateway.protocol_mapping_update_lock.lock().await;

    let previous_config = match state.storage.store.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load config before run_type update");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_text(&translator, "runType.switchFailed"),
            );
        }
    };
    let previous_protocol_mapping_feature = match load_protocol_mapping_feature(
        &state,
        Some(&previous_config),
    )
    .await
    {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, "failed to load protocol mapping feature before run_type update");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_text(&translator, "runType.switchFailed"),
            );
        }
    };
    let previous_run_type = previous_config
        .get("run_type")
        .and_then(Value::as_i64)
        .unwrap_or(3);
    let reverse_proxy_submode = body
        .get("reverse_proxy_submode")
        .and_then(Value::as_str)
        .filter(|value| *value == "path" || *value == "subdomain")
        .map(str::to_string)
        .or_else(|| {
            previous_config
                .get("reverse_proxy_submode")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| "path".to_string());

    let mut next_config = previous_config.clone();
    let object = ensure_config_object(&mut next_config);
    object.insert("run_type".to_string(), json!(run_type));
    object.insert(
        "reverse_proxy_submode".to_string(),
        Value::String(reverse_proxy_submode),
    );

    if let Err(error) = state.storage.store.save_config(&next_config).await {
        tracing::warn!(%error, "failed to save run_type config");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            admin_text(&translator, "runType.switchFailed"),
        );
    }
    if run_type != 3 {
        let disabled = json!({ "enabled": false });
        if let Err(error) = save_protocol_mapping_feature(&state, &disabled).await {
            tracing::warn!(%error, "failed to disable protocol mapping feature after run_type update");
            if let Err(rollback_error) = state.storage.store.save_config(&previous_config).await {
                tracing::warn!(%rollback_error, "failed to rollback run_type config");
            }
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_text(&translator, "runType.switchFailed"),
            );
        }
    }

    let runtime_result = async {
        sync_smart_connect(&state, &next_config).await?;
        apply_run_type_config(&state, &next_config, run_type).await
    }
    .await;

    match runtime_result {
        Ok(()) => {
            cleanup_auto_whitelist_after_direct_mode(&state, run_type).await;
            response::success_empty().into_response()
        }
        Err(error) => {
            tracing::warn!(%error, "failed to apply run_type runtime");
            rollback_config_protocol_feature_and_runtime(
                &state,
                &previous_config,
                &previous_protocol_mapping_feature,
                previous_run_type,
            )
            .await;
            response::error(
                StatusCode::BAD_GATEWAY,
                localize_runtime_config_error(&translator, &error),
            )
        }
    }
}

pub(super) async fn cleanup_auto_whitelist_after_direct_mode(state: &AppState, run_type: i64) {
    if run_type != 0 {
        return;
    }
    match whitelist::remove_whitelist_records_by_source(state, "auto").await {
        Ok(removed) if removed > 0 => {
            whitelist::sync_reverse_proxy_trusted_ips(state).await;
        }
        Ok(_) => {}
        Err(error) => {
            tracing::warn!(
                %error,
                "failed to clear login IP grants after switching to direct mode"
            );
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/admin/config/protocol_mapping_feature",
    tag = "config",
    operation_id = "get_api_admin_config_protocol_mapping_feature",
    responses((status = 200, description = "Protocol mapping feature setting"))
)]
pub(super) async fn get_protocol_mapping_feature(State(state): State<AppState>) -> Response {
    let fallback_config = state.storage.store.get_config().await.ok();
    match load_protocol_mapping_feature(&state, fallback_config.as_ref()).await {
        Ok(config) => response::ok(config).into_response(),
        Err(error) => {
            let translator = Translator::from_state(&state).await;
            tracing::warn!(%error, "failed to load protocol mapping feature config");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                runtime_config_route_text(&translator, "loadProtocolMappingFeatureFailed"),
            )
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/config/protocol_mapping_feature",
    tag = "config",
    operation_id = "post_api_admin_config_protocol_mapping_feature",
    request_body = serde_json::Value,
    responses((status = 200, description = "Updated protocol mapping feature setting"))
)]
pub(super) async fn update_protocol_mapping_feature(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let valid_patch_shape = body.as_object().is_some_and(|object| {
        let enabled_is_valid = object.get("enabled").is_none_or(Value::is_boolean);
        let availability_is_valid = object.get("availability").is_none_or(|availability| {
            availability.is_null()
                || availability.as_object().is_some_and(|availability| {
                    availability.get("enabled").and_then(Value::as_bool) == Some(true)
                })
        });
        enabled_is_valid && availability_is_valid
    });
    if !valid_patch_shape {
        return response::error(
            StatusCode::BAD_REQUEST,
            admin_text(&translator, "protocolMapping.availabilityInvalid"),
        );
    }
    let _protocol_mapping_guard = state.gateway.protocol_mapping_update_lock.lock().await;
    let previous_config = match state.storage.store.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load config before protocol mapping update");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_text(&translator, "protocolMapping.updateFeatureFailed"),
            );
        }
    };
    let previous_settings =
        match load_protocol_mapping_feature(&state, Some(&previous_config)).await {
            Ok(settings) => settings,
            Err(error) => {
                tracing::warn!(%error, "failed to load protocol mapping feature before update");
                return response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    admin_text(&translator, "protocolMapping.updateFeatureFailed"),
                );
            }
        };
    let run_type = previous_config
        .get("run_type")
        .and_then(Value::as_i64)
        .unwrap_or(3);
    if body.get("enabled").and_then(Value::as_bool) == Some(true) && run_type != 3 {
        return response::error(
            StatusCode::BAD_REQUEST,
            admin_text(&translator, "protocolMapping.subdomainOnly"),
        );
    }

    let mut current = previous_settings.clone();
    merge_object(&mut current, &body);
    let next = match normalize_protocol_mapping_feature_strict(Some(&current)) {
        Ok(next) => next,
        Err(error) => {
            tracing::warn!(?error, "rejected invalid protocol mapping availability");
            return response::error(
                StatusCode::BAD_REQUEST,
                admin_text(&translator, "protocolMapping.availabilityInvalid"),
            );
        }
    };

    if let Err(error) = save_protocol_mapping_feature(&state, &next).await {
        tracing::warn!(%error, "failed to save protocol mapping feature key");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            admin_text(&translator, "protocolMapping.updateFeatureFailed"),
        );
    }

    match apply_run_type_config(&state, &previous_config, run_type).await {
        Ok(()) => response::ok(next).into_response(),
        Err(error) => {
            rollback_protocol_mapping_feature_and_runtime(
                &state,
                &previous_config,
                &previous_settings,
                run_type,
            )
            .await;
            response::error(
                StatusCode::BAD_GATEWAY,
                localize_runtime_config_error(&translator, &error),
            )
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/admin/config/smart_connect/details",
    tag = "config",
    operation_id = "get_api_admin_config_smart_connect_details",
    responses((status = 200, description = "Smart Connect details"))
)]
pub(super) async fn get_smart_connect_details(State(state): State<AppState>) -> Response {
    match load_smart_connect_details(&state).await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => {
            let translator = Translator::from_state(&state).await;
            tracing::warn!(%error, "failed to load smart connect details");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                runtime_config_route_text(&translator, "loadSmartConnectDetailsFailed"),
            )
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/config/smart_connect",
    tag = "config",
    operation_id = "post_api_admin_config_smart_connect",
    responses((status = 200, description = "Updated Smart Connect configuration"))
)]
pub(super) async fn update_smart_connect(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    if !host_firewall_available(&state) {
        return response::error(
            StatusCode::FORBIDDEN,
            capability_blocked_text(&state, "smart_connect_available", &translator),
        );
    }
    let _protocol_mapping_guard = state.gateway.protocol_mapping_update_lock.lock().await;

    let previous_config = match state.storage.store.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load config before smart connect update");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_text(&translator, "smartConnect.updateFailed"),
            );
        }
    };
    let run_type = previous_config
        .get("run_type")
        .and_then(Value::as_i64)
        .unwrap_or(3);
    let previous_protocol_mapping_feature = match load_protocol_mapping_feature(
        &state,
        Some(&previous_config),
    )
    .await
    {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, "failed to load protocol mapping feature before smart connect update");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_text(&translator, "smartConnect.updateFailed"),
            );
        }
    };
    if body.get("enabled").and_then(Value::as_bool) == Some(true) && run_type != 3 {
        return response::error(
            StatusCode::BAD_REQUEST,
            admin_text(&translator, "smartConnect.subdomainOnly"),
        );
    }

    let mut next_config = previous_config.clone();
    let mut smart = normalize_smart_connect_config(previous_config.get("smart_connect"));
    merge_object(&mut smart, &body);
    smart = normalize_smart_connect_config(Some(&smart));
    ensure_config_object(&mut next_config).insert("smart_connect".to_string(), smart);

    if let Err(error) = state.storage.store.save_config(&next_config).await {
        tracing::warn!(%error, "failed to save smart connect config");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            admin_text(&translator, "smartConnect.updateFailed"),
        );
    }

    match sync_smart_connect(&state, &next_config).await {
        Ok(details) => match apply_run_type_config(&state, &next_config, run_type).await {
            Ok(()) => response::ok(details).into_response(),
            Err(error) => {
                rollback_config_protocol_feature_and_runtime(
                    &state,
                    &previous_config,
                    &previous_protocol_mapping_feature,
                    run_type,
                )
                .await;
                response::error(
                    StatusCode::BAD_GATEWAY,
                    localize_runtime_config_error(&translator, &error),
                )
            }
        },
        Err(error) => {
            rollback_config_protocol_feature_and_runtime(
                &state,
                &previous_config,
                &previous_protocol_mapping_feature,
                run_type,
            )
            .await;
            response::error(
                StatusCode::BAD_GATEWAY,
                localize_runtime_config_error(&translator, &error),
            )
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/admin/config/fnos_network_tuning",
    tag = "config",
    operation_id = "get_api_admin_config_fnos_network_tuning",
    responses((status = 200, description = "fnOS network tuning status"))
)]
pub(super) async fn get_fnos_network_tuning(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match load_fnos_network_tuning_status(&state).await {
        Ok(data) => {
            response::ok(localize_fnos_network_tuning_status(data, &translator)).into_response()
        }
        Err(error) => {
            tracing::warn!(%error, "failed to load fnos network tuning status");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_text(&translator, "fnosNetworkTuning.updateFailed"),
            )
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/config/fnos_network_tuning",
    tag = "config",
    operation_id = "post_api_admin_config_fnos_network_tuning",
    responses((status = 200, description = "Updated fnOS network tuning configuration"))
)]
pub(super) async fn update_fnos_network_tuning(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let _update_guard = state.fnos_network_tuning_update_lock.lock().await;
    if let Some(reason_code) = fnos_network_tuning_blocked_reason_code(&state) {
        return response::error(
            StatusCode::FORBIDDEN,
            fnos_network_tuning_blocked_reason(&reason_code, &translator),
        );
    }
    match update_fnos_network_tuning_config(&state, &body, &translator).await {
        Ok(data) => {
            response::ok(localize_fnos_network_tuning_status(data, &translator)).into_response()
        }
        Err(error) => response::error(StatusCode::BAD_REQUEST, error),
    }
}

#[utoipa::path(
    get,
    path = "/api/admin/config/fnos_share_bypass",
    tag = "config",
    operation_id = "get_api_admin_config_fnos_share_bypass",
    responses((status = 200, description = "fnOS share bypass configuration"))
)]
pub(super) async fn get_fnos_share_bypass(State(state): State<AppState>) -> Response {
    match load_config_section(&state, "fnos_share_bypass", normalize_fnos_share_bypass).await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => {
            let translator = Translator::from_state(&state).await;
            tracing::warn!(%error, "failed to load fnos share bypass config");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                runtime_config_route_text(&translator, "loadFnosShareBypassFailed"),
            )
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/config/fnos_share_bypass",
    tag = "config",
    operation_id = "post_api_admin_config_fnos_share_bypass",
    responses((status = 200, description = "Updated fnOS share bypass configuration"))
)]
pub(super) async fn update_fnos_share_bypass(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match update_config_section(
        &state,
        "fnos_share_bypass",
        &body,
        normalize_fnos_share_bypass,
    )
    .await
    {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to save fnos share bypass config");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                runtime_config_route_text(&translator, "saveFnosShareBypassFailed"),
            )
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/admin/config/fnos_port_icon_hijack",
    tag = "config",
    operation_id = "get_api_admin_config_fnos_port_icon_hijack",
    responses((status = 200, description = "fnOS port icon hijack configuration"))
)]
pub(super) async fn get_fnos_port_icon_hijack(State(state): State<AppState>) -> Response {
    match load_config_section(
        &state,
        "fnos_port_icon_hijack",
        normalize_fnos_port_icon_hijack,
    )
    .await
    {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => {
            let translator = Translator::from_state(&state).await;
            tracing::warn!(%error, "failed to load fnos port icon hijack config");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                runtime_config_route_text(&translator, "loadFnosPortIconHijackFailed"),
            )
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/config/fnos_port_icon_hijack",
    tag = "config",
    operation_id = "post_api_admin_config_fnos_port_icon_hijack",
    responses((status = 200, description = "Updated fnOS port icon hijack configuration"))
)]
pub(super) async fn update_fnos_port_icon_hijack(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let previous_config = match state.storage.store.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load config before fnos port icon hijack update");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_text(&translator, "fnosPortIcon.syncFailed"),
            );
        }
    };

    let mut current = normalize_fnos_port_icon_hijack(previous_config.get("fnos_port_icon_hijack"));
    merge_object(&mut current, &body);
    if let Some(object) = current.as_object_mut() {
        object.insert(
            "updated_at".to_string(),
            Value::String(time_utils::now_iso()),
        );
    }
    let next = normalize_fnos_port_icon_hijack(Some(&current));
    let mut next_config = previous_config.clone();
    if !next_config.is_object() {
        next_config = app_store::default_config();
    }
    if let Some(object) = next_config.as_object_mut() {
        object.insert("fnos_port_icon_hijack".to_string(), next.clone());
    }

    if let Err(error) = state.storage.store.save_config(&next_config).await {
        tracing::warn!(%error, "failed to save fnos port icon hijack config");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            admin_text(&translator, "fnosPortIcon.syncFailed"),
        );
    }

    match state
        .gateway
        .client
        .set_fnos_port_icon_hijack_config(&next)
        .await
        .and_then(ensure_go_success)
    {
        Ok(()) => response::ok(next).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to sync fnos port icon hijack config to Go backend");
            if let Err(rollback_error) = state.storage.store.save_config(&previous_config).await {
                tracing::warn!(
                    %rollback_error,
                    "failed to rollback fnos port icon hijack config"
                );
            }
            response::error(
                StatusCode::BAD_GATEWAY,
                if error.to_string().trim().is_empty() {
                    admin_text(&translator, "fnosPortIcon.syncFailed")
                } else {
                    error.to_string()
                },
            )
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/admin/config/auto_https",
    tag = "config",
    operation_id = "get_api_admin_config_auto_https",
    responses((status = 200, description = "Auto HTTPS configuration"))
)]
pub(super) async fn get_auto_https(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match load_config_section(
        &state,
        "auto_https",
        auto_https::normalize_auto_https_config,
    )
    .await
    {
        Ok(config) => {
            let runtime = state.auto_https.runtime_state().await;
            let runtime = auto_https::localize_runtime_state(runtime, &translator);
            response::ok(merge_runtime(config, runtime)).into_response()
        }
        Err(error) => {
            tracing::warn!(%error, "failed to load auto HTTPS config");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                runtime_config_route_text(&translator, "loadAutoHttpsFailed"),
            )
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/config/auto_https",
    tag = "config",
    operation_id = "post_api_admin_config_auto_https",
    request_body = serde_json::Value,
    responses((status = 200, description = "Updated Auto HTTPS configuration"))
)]
pub(super) async fn update_auto_https(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let requested_enabled = body.get("enabled").and_then(Value::as_bool);
    if requested_enabled == Some(true) && !runtime_profile::auto_https_available(&state) {
        return response::error(
            StatusCode::FORBIDDEN,
            capability_blocked_text(&state, "auto_https_available", &translator),
        );
    }

    if requested_enabled == Some(true) {
        let runtime = state.auto_https.apply_config(true).await;
        let active = runtime.get("status").and_then(Value::as_str) == Some("active");
        let runtime = auto_https::localize_runtime_state(runtime, &translator);
        match update_config_section(
            &state,
            "auto_https",
            &json!({ "enabled": active }),
            auto_https::normalize_auto_https_config,
        )
        .await
        {
            Ok(config) => return response::ok(merge_runtime(config, runtime)).into_response(),
            Err(error) => {
                tracing::warn!(%error, "failed to save auto HTTPS config");
                return response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    runtime_config_route_text(&translator, "saveAutoHttpsFailed"),
                );
            }
        }
    }

    match update_config_section(
        &state,
        "auto_https",
        &body,
        auto_https::normalize_auto_https_config,
    )
    .await
    {
        Ok(config) => {
            let runtime = state
                .auto_https
                .apply_config(config["enabled"].as_bool().unwrap_or(false))
                .await;
            let runtime = auto_https::localize_runtime_state(runtime, &translator);
            response::ok(merge_runtime(config, runtime)).into_response()
        }
        Err(error) => {
            tracing::warn!(%error, "failed to save auto HTTPS config");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                runtime_config_route_text(&translator, "saveAutoHttpsFailed"),
            )
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/config/auto_manage_firewall",
    tag = "config",
    operation_id = "post_api_admin_config_auto_manage_firewall",
    request_body = serde_json::Value,
    responses((status = 200, description = "Updated automatic firewall management"))
)]
pub(super) async fn update_auto_manage_firewall(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    if !host_firewall_available(&state) {
        return response::error(
            StatusCode::FORBIDDEN,
            capability_blocked_text(&state, "host_firewall_available", &translator),
        );
    }
    let enabled = normalize_auto_manage_firewall(body.get("auto_manage_firewall"));
    match save_top_level_config_value(&state, "auto_manage_firewall", json!(enabled)).await {
        Ok(()) => response::ok(json!({ "auto_manage_firewall": enabled })).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to save auto manage firewall config");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                runtime_config_route_text(&translator, "saveAutoManageFirewallFailed"),
            )
        }
    }
}

fn build_firewall_additional_ports_details(
    config: &Value,
    protocol_mapping_feature: &Value,
) -> Value {
    let run_type = config.get("run_type").and_then(Value::as_i64).unwrap_or(3);
    let protocol_mapping_enabled = run_type == 3
        && protocol_mapping_feature
            .get("enabled")
            .and_then(Value::as_bool)
            == Some(true);
    json!({
        "additionalPorts": normalize_firewall_additional_ports(
            config.get("firewall_additional_ports")
        ),
        "automaticPorts": automatic_exempt_port_numbers(
            config,
            protocol_mapping_enabled,
            run_type,
        ),
        "effectivePorts": exempt_port_numbers(
            config,
            protocol_mapping_enabled,
            run_type,
        ),
        "runType": run_type,
        "appliedNow": run_type != 1,
    })
}

#[utoipa::path(
    get,
    path = "/api/admin/config/firewall_additional_ports",
    tag = "config",
    operation_id = "get_api_admin_config_firewall_additional_ports",
    responses((status = 200, description = "Firewall additional port configuration"))
)]
pub(super) async fn get_firewall_additional_ports(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    if !host_firewall_available(&state) {
        return response::error(
            StatusCode::FORBIDDEN,
            capability_blocked_text(&state, "host_firewall_available", &translator),
        );
    }
    let config = match state.storage.store.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load firewall additional ports config");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_text(&translator, "firewallAdditionalPorts.loadFailed"),
            );
        }
    };
    match load_protocol_mapping_feature(&state, Some(&config)).await {
        Ok(feature) => {
            response::ok(build_firewall_additional_ports_details(&config, &feature)).into_response()
        }
        Err(error) => {
            tracing::warn!(%error, "failed to load protocol mapping feature for firewall ports");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_text(&translator, "firewallAdditionalPorts.loadFailed"),
            )
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/config/firewall_additional_ports",
    tag = "config",
    operation_id = "post_api_admin_config_firewall_additional_ports",
    request_body = serde_json::Value,
    responses((status = 200, description = "Updated firewall additional port configuration"))
)]
pub(super) async fn update_firewall_additional_ports(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    if !host_firewall_available(&state) {
        return response::error(
            StatusCode::FORBIDDEN,
            capability_blocked_text(&state, "host_firewall_available", &translator),
        );
    }
    let ports = match parse_firewall_additional_ports(&body) {
        Ok(ports) => ports,
        Err(key) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                admin_text(
                    &translator,
                    &format!("firewallAdditionalPorts.errors.{key}"),
                ),
            );
        }
    };
    update_firewall_additional_ports_transaction(&state, ports).await
}

pub(super) async fn update_firewall_additional_ports_transaction(
    state: &AppState,
    ports: Vec<i64>,
) -> Response {
    update_firewall_additional_ports_transaction_with_reset(state, ports, &RuntimeFirewallReset)
        .await
}

pub(super) type FirewallResetFuture<'a> =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<Value, String>> + Send + 'a>>;

pub(super) trait FirewallResetOperation: Sync {
    fn reset<'a>(&'a self, state: &'a AppState, run_type: i64) -> FirewallResetFuture<'a>;
}

struct RuntimeFirewallReset;

impl FirewallResetOperation for RuntimeFirewallReset {
    fn reset<'a>(&'a self, state: &'a AppState, run_type: i64) -> FirewallResetFuture<'a> {
        Box::pin(reset_firewall_for_run_type(state, run_type))
    }
}

pub(super) async fn update_firewall_additional_ports_transaction_with_reset<R>(
    state: &AppState,
    ports: Vec<i64>,
    reset_firewall: &R,
) -> Response
where
    R: FirewallResetOperation + ?Sized,
{
    let translator = Translator::from_state(state).await;
    let _protocol_mapping_guard = state.gateway.protocol_mapping_update_lock.lock().await;
    let previous_config = match state.storage.store.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load config before firewall ports update");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_text(&translator, "firewallAdditionalPorts.loadFailed"),
            );
        }
    };
    let protocol_mapping_feature = match load_protocol_mapping_feature(
        state,
        Some(&previous_config),
    )
    .await
    {
        Ok(feature) => feature,
        Err(error) => {
            tracing::warn!(%error, "failed to load protocol mapping feature before firewall ports update");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_text(&translator, "firewallAdditionalPorts.loadFailed"),
            );
        }
    };
    let previous_run_type = previous_config
        .get("run_type")
        .and_then(Value::as_i64)
        .unwrap_or(3);
    let previous_ports =
        normalize_firewall_additional_ports(previous_config.get("firewall_additional_ports"));
    let next_config = match state
        .storage
        .store
        .set_config_top_level_value("firewall_additional_ports", json!(ports))
        .await
    {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to save firewall additional ports config");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_text(&translator, "firewallAdditionalPorts.saveFailed"),
            );
        }
    };

    match reset_firewall.reset(state, previous_run_type).await {
        Ok(_) => response::ok(build_firewall_additional_ports_details(
            &next_config,
            &protocol_mapping_feature,
        ))
        .into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to apply firewall additional ports");
            let localized_error = localize_runtime_config_error(&translator, &error);
            let rollback_result = match state
                .storage
                .store
                .set_config_top_level_value("firewall_additional_ports", json!(previous_ports))
                .await
            {
                Ok(_) => reset_firewall
                    .reset(state, previous_run_type)
                    .await
                    .map(|_| ()),
                Err(rollback_error) => Err(rollback_error.to_string()),
            };
            match rollback_result {
                Ok(()) => response::error(
                    StatusCode::BAD_GATEWAY,
                    admin_text_params(
                        &translator,
                        "firewallAdditionalPorts.updateFailedRolledBack",
                        &[("message", localized_error)],
                    ),
                ),
                Err(rollback_error) => {
                    tracing::warn!(%rollback_error, "failed to rollback firewall additional ports");
                    response::error(
                        StatusCode::BAD_GATEWAY,
                        admin_text_params(
                            &translator,
                            "firewallAdditionalPorts.updateFailedRollback",
                            &[
                                ("message", localized_error),
                                ("rollbackError", rollback_error),
                            ],
                        ),
                    )
                }
            }
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/firewall/reset",
    tag = "firewall",
    operation_id = "post_api_admin_firewall_reset",
    request_body = serde_json::Value,
    responses((status = 200, description = "Reset firewall"))
)]
pub(super) async fn reset_firewall(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    if !host_firewall_available(&state) {
        return response::error(
            StatusCode::FORBIDDEN,
            capability_blocked_text(&state, "host_firewall_available", &translator),
        );
    }
    let Some(run_type) = normalize_run_type(body.get("run_type")) else {
        return response::error(
            StatusCode::BAD_REQUEST,
            runtime_config_route_text(&translator, "invalidRunType"),
        );
    };
    if run_type == 0 && !host_firewall_available(&state) {
        return response::error(
            StatusCode::FORBIDDEN,
            capability_blocked_text(&state, "direct_mode_available", &translator),
        );
    }
    match reset_firewall_with_transaction_lock(&state, run_type).await {
        Ok(data) => Json(json!({
            "success": true,
            "data": data,
            "message": firewall_reset_success_message(&translator, &data, run_type),
        }))
        .into_response(),
        Err(error) => response::error(
            StatusCode::BAD_GATEWAY,
            localize_runtime_config_error(&translator, &error),
        ),
    }
}

pub(super) async fn reset_firewall_with_transaction_lock(
    state: &AppState,
    run_type: i64,
) -> Result<Value, String> {
    let _protocol_mapping_guard = state.gateway.protocol_mapping_update_lock.lock().await;
    reset_firewall_for_run_type(state, run_type).await
}

#[utoipa::path(
    post,
    path = "/api/admin/firewall/clear",
    tag = "firewall",
    operation_id = "post_api_admin_firewall_clear",
    responses((status = 200, description = "Clear firewall"))
)]
pub(super) async fn clear_firewall(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    if !host_firewall_available(&state) {
        return response::error(
            StatusCode::FORBIDDEN,
            capability_blocked_text(&state, "host_firewall_available", &translator),
        );
    }
    match clear_firewall_with_transaction_lock(&state).await {
        Ok(data) => Json(json!({
            "success": true,
            "message": admin_text_params(
                &translator,
                "firewall.clearSuccess",
                &[("port", gateway_port().to_string())],
            ),
            "data": data,
        }))
        .into_response(),
        Err(error) => response::error(
            StatusCode::BAD_GATEWAY,
            localize_runtime_config_error(&translator, &error),
        ),
    }
}

pub(super) async fn clear_firewall_with_transaction_lock(
    state: &AppState,
) -> Result<Value, String> {
    let _protocol_mapping_guard = state.gateway.protocol_mapping_update_lock.lock().await;
    clear_legacy_gateway_redirects(state, gateway_port(), true).await?;
    let value = state
        .gateway
        .client
        .clean_iptables()
        .await
        .map_err(|error| error.to_string())?;
    ensure_go_success(value).map_err(|error| error.to_string())?;
    Ok(json!({ "gatewayPort": gateway_port() }))
}

#[utoipa::path(
    post,
    path = "/api/admin/sync-routes",
    tag = "config",
    operation_id = "post_api_admin_sync_routes",
    responses((status = 200, description = "Synchronized route configuration"))
)]
pub(super) async fn sync_routes(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    let config = match state.storage.store.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load config before route sync");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                runtime_config_route_text(&translator, "loadConfigFailed"),
            );
        }
    };
    let run_type = config.get("run_type").and_then(Value::as_i64).unwrap_or(3);
    if let Err(error) = apply_run_type_config(&state, &config, run_type).await {
        return response::error(
            StatusCode::BAD_GATEWAY,
            localize_runtime_config_error(&translator, &error),
        );
    }

    let gateway_logging = normalize_gateway_logging(config.get("gateway_logging"));
    if let Err(error) = state
        .gateway
        .client
        .set_gateway_logging_config(&gateway_logging)
        .await
        .and_then(ensure_go_success)
    {
        tracing::warn!(%error, "failed to sync gateway logging config");
        return response::error(
            StatusCode::BAD_GATEWAY,
            admin_text_params(
                &translator,
                "syncRoutes.partialFailedGatewayLogging",
                &[("gatewayLogging", "false".to_string())],
            ),
        );
    }

    let waf_config = match waf::sync_waf_config_to_gateway(&state, &config).await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to sync WAF config");
            return response::error(
                StatusCode::BAD_GATEWAY,
                admin_text_params(
                    &translator,
                    "syncRoutes.partialFailedGatewayLoggingWaf",
                    &[
                        ("gatewayLogging", "true".to_string()),
                        ("waf", "false".to_string()),
                    ],
                ),
            );
        }
    };

    let protocol_mapping_feature = match load_protocol_mapping_feature(&state, Some(&config)).await
    {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, "failed to load protocol mapping feature during route sync");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                runtime_config_route_text(&translator, "loadProtocolMappingFeatureFailed"),
            );
        }
    };
    let protocol_mapping_enabled = protocol_mapping_feature
        .get("enabled")
        .and_then(Value::as_bool)
        == Some(true);
    let data = json!({
        "synced_rules": if run_type == 1 && !is_reverse_proxy_subdomain_mode(&config) {
            config_array_len(&config, "proxy_mappings")
        } else {
            0
        },
        "synced_host_rules": if is_any_subdomain_routing_mode(&config) {
            config_array_len(&config, "host_mappings")
        } else {
            0
        },
        "synced_stream_rules": if run_type == 3 && protocol_mapping_enabled {
            config_array_len(&config, "stream_mappings")
        } else {
            0
        },
        "synced_gateway_logging": true,
        "synced_waf": true,
        "waf_bundle_id": waf_config.get("active_bundle_id").and_then(Value::as_str).unwrap_or(""),
    });
    Json(json!({
        "success": true,
        "data": data,
        "message": admin_text_params(
            &translator,
            "syncRoutes.success",
            &[
                ("rules", data.get("synced_rules").and_then(Value::as_i64).unwrap_or(0).to_string()),
                ("hostRules", data.get("synced_host_rules").and_then(Value::as_i64).unwrap_or(0).to_string()),
                ("streamRules", data.get("synced_stream_rules").and_then(Value::as_i64).unwrap_or(0).to_string()),
            ],
        ),
    }))
    .into_response()
}

#[utoipa::path(
    get,
    path = "/api/admin/config/default_route",
    tag = "config",
    operation_id = "get_api_admin_config_default_route",
    responses((status = 200, description = "Default route setting"))
)]
pub(super) async fn get_default_route(State(state): State<AppState>) -> Response {
    match state.storage.store.get_config().await {
        Ok(config) => response::ok(json!({
            "default_route": config
                .get("default_route")
                .and_then(Value::as_str)
                .unwrap_or("/__select__")
        }))
        .into_response(),
        Err(error) => {
            let translator = Translator::from_state(&state).await;
            tracing::warn!(%error, "failed to load default route");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                runtime_config_route_text(&translator, "loadDefaultRouteFailed"),
            )
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/config/default_route",
    tag = "config",
    operation_id = "post_api_admin_config_default_route",
    request_body = serde_json::Value,
    responses((status = 200, description = "Updated default route setting"))
)]
pub(super) async fn update_default_route(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let path = body
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    match save_top_level_config_value(&state, "default_route", Value::String(path.clone())).await {
        Ok(()) => {
            if let Err(error) = state.gateway.client.set_default_route(&path).await {
                tracing::warn!(%error, "failed to sync default route to Go backend");
            }
            response::success_empty().into_response()
        }
        Err(error) => {
            tracing::warn!(%error, "failed to save default route");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                runtime_config_route_text(&translator, "saveDefaultRouteFailed"),
            )
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/config/default_tunnel",
    tag = "config",
    operation_id = "post_api_admin_config_default_tunnel",
    request_body = serde_json::Value,
    responses((status = 200, description = "Updated default tunnel setting"))
)]
pub(super) async fn update_default_tunnel(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let tunnel = body.get("tunnel").and_then(Value::as_str).unwrap_or("");
    if tunnel != "frp" && tunnel != "cloudflared" {
        return response::error(
            StatusCode::BAD_REQUEST,
            runtime_config_route_text(&translator, "unsupportedTunnelType"),
        );
    }
    match save_top_level_config_value(&state, "default_tunnel", Value::String(tunnel.to_string()))
        .await
    {
        Ok(()) => response::success_empty().into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to save default tunnel");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                runtime_config_route_text(&translator, "saveDefaultTunnelFailed"),
            )
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/admin/config/proxy_protocol_force",
    tag = "config",
    operation_id = "get_api_admin_config_proxy_protocol_force",
    responses((status = 200, description = "Proxy protocol force setting"))
)]
pub(super) async fn get_proxy_protocol_force(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    let upstream_unavailable = runtime_config_route_text(&translator, "upstreamUnavailable");
    match state.gateway.client.get_proxy_protocol_force().await {
        Ok(value) => {
            if !value
                .get("success")
                .and_then(Value::as_bool)
                .unwrap_or(true)
            {
                return response::error(
                    StatusCode::BAD_GATEWAY,
                    go_response_message(&value, &upstream_unavailable),
                );
            }
            response::ok(proxy_protocol_force_payload(&value, false)).into_response()
        }
        Err(error) => {
            tracing::warn!(%error, "failed to load proxy protocol force config");
            response::error(StatusCode::BAD_GATEWAY, upstream_unavailable)
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/config/proxy_protocol_force",
    tag = "config",
    operation_id = "post_api_admin_config_proxy_protocol_force",
    request_body = serde_json::Value,
    responses((status = 200, description = "Updated proxy protocol force setting"))
)]
pub(super) async fn update_proxy_protocol_force(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let Some(force) = body.get("proxy_protocol_force").and_then(Value::as_bool) else {
        return response::error(
            StatusCode::BAD_REQUEST,
            runtime_config_route_text(&translator, "proxyProtocolForceBooleanRequired"),
        );
    };

    let upstream_unavailable = runtime_config_route_text(&translator, "upstreamUnavailable");
    match state.gateway.client.set_proxy_protocol_force(force).await {
        Ok(value) => {
            if !value
                .get("success")
                .and_then(Value::as_bool)
                .unwrap_or(true)
            {
                return response::error(
                    StatusCode::BAD_GATEWAY,
                    go_response_message(&value, &upstream_unavailable),
                );
            }
            response::ok(proxy_protocol_force_payload(&value, force)).into_response()
        }
        Err(error) => {
            tracing::warn!(%error, force, "failed to update proxy protocol force config");
            response::error(StatusCode::BAD_GATEWAY, upstream_unavailable)
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/admin/config/run_mode_prompt_preferences",
    tag = "config",
    operation_id = "get_api_admin_config_run_mode_prompt_preferences",
    responses((status = 200, description = "Run mode prompt preferences"))
)]
pub(super) async fn get_run_mode_prompt_preferences(State(state): State<AppState>) -> Response {
    match load_run_mode_prompt_preferences(&state).await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => {
            let translator = Translator::from_state(&state).await;
            tracing::warn!(%error, "failed to load run mode prompt preferences");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                runtime_config_route_text(&translator, "loadRunModePromptPreferencesFailed"),
            )
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/config/run_mode_prompt_preferences",
    tag = "config",
    operation_id = "post_api_admin_config_run_mode_prompt_preferences",
    request_body = serde_json::Value,
    responses((status = 200, description = "Updated run mode prompt preferences"))
)]
pub(super) async fn update_run_mode_prompt_preferences(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let mut current = match load_run_mode_prompt_preferences(&state).await {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, "failed to load run mode prompt preferences before update");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                runtime_config_route_text(&translator, "loadRunModePromptPreferencesFailed"),
            );
        }
    };
    merge_object(&mut current, &body);
    let next = normalize_run_mode_prompt_preferences(Some(&current));
    match state
        .storage
        .store
        .set_json_value(RUN_MODE_PROMPT_PREFERENCES_KEY, &next)
        .await
    {
        Ok(()) => response::ok(next).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to save run mode prompt preferences");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                runtime_config_route_text(&translator, "saveRunModePromptPreferencesFailed"),
            )
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/admin/config/welcome_guide",
    tag = "config",
    operation_id = "get_api_admin_config_welcome_guide",
    responses((status = 200, description = "Welcome guide status"))
)]
pub(super) async fn get_welcome_guide(State(state): State<AppState>) -> Response {
    match load_welcome_guide_status(&state).await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => {
            let translator = Translator::from_state(&state).await;
            tracing::warn!(%error, "failed to load welcome guide status");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                runtime_config_route_text(&translator, "loadWelcomeGuideFailed"),
            )
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/config/welcome_guide/complete",
    tag = "config",
    operation_id = "post_api_admin_config_welcome_guide_complete",
    responses((status = 200, description = "Completed welcome guide"))
)]
pub(super) async fn complete_welcome_guide(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    let current = match load_welcome_guide_status(&state).await {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, "failed to load welcome guide status before complete");
            json!({ "completed": false, "completed_at": Value::Null })
        }
    };
    let next = json!({
        "completed": true,
        "completed_at": current
            .get("completed_at")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(time_utils::now_iso),
    });
    match state
        .storage
        .store
        .set_json_value(WELCOME_GUIDE_STATUS_KEY, &next)
        .await
    {
        Ok(()) => response::ok(next).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to save welcome guide status");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                runtime_config_route_text(&translator, "saveWelcomeGuideFailed"),
            )
        }
    }
}
