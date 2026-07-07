use std::{collections::BTreeSet, fs, net::Ipv4Addr, path::Path, process::Command};

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde_json::{Map, Value, json};

use crate::{
    auto_https, gateway_settings,
    i18n::Translator,
    proxy_config::{build_gateway_auth_config, build_host_rules_payload},
    redis_store, response, runtime_profile,
    state::AppState,
    system_assets,
    terminal_paths::normalize_terminal_default_cwd,
    time_utils, waf, whitelist,
};

const CAPTCHA_SETTINGS_KEY: &str = "fn_knock:captcha:settings";
const LEGACY_CAPTCHA_SETTINGS_KEY: &str = "fn_knock:config:captcha";
const PROTOCOL_MAPPING_FEATURE_KEY: &str = "fn_knock:protocol-mapping:feature";
const RUN_MODE_PROMPT_PREFERENCES_KEY: &str = "fn_knock:run-mode:prompt-preferences";
const WELCOME_GUIDE_STATUS_KEY: &str = "fn_knock:welcome-guide:status";
const SMART_CONNECT_RUNTIME_KEY: &str = "fn_knock:smart-connect:runtime";
const LEGACY_REVERSE_PROXY_THROTTLE_PATCH_FLAG_KEY: &str =
    "fn_knock:patch:reverse-proxy-throttle:v1";
const LEGACY_EVENT_SYSTEM_RESOURCE_ALERTS_PATCH_FLAG_KEY: &str =
    "fn_knock:patch:event-system-resource-alerts:v1";
const LEGACY_REDIRECTED_HTTP_PORTS: [i64; 2] = [80, 443];
const SMART_CONNECT_DNS_PORT: i64 = 53;
const SMART_CONNECT_LOCAL_TTL_SECONDS: u16 = 30;
const SMART_CONNECT_MANAGED_CONF_PATH: &str = "/etc/dnsmasq.d/fn-knock-smart-connect.conf";
const FNOS_NETWORK_TUNING_SYSCTL_PATH: &str = "/etc/sysctl.d/99-fn-knock-network.conf";
const GO_BACKEND_UNSUCCESSFUL_RESPONSE: &str = "Go backend returned an unsuccessful response";
const JS_MAX_SAFE_INTEGER_I64: i64 = 9_007_199_254_740_991;

pub fn runtime_config_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/admin/config/captcha",
            get(get_captcha).post(update_captcha),
        )
        .route(
            "/api/admin/config/terminal_feature",
            get(get_terminal_feature).post(update_terminal_feature),
        )
        .route("/api/admin/config/run_type", post(update_run_type))
        .route(
            "/api/admin/config/protocol_mapping_feature",
            get(get_protocol_mapping_feature).post(update_protocol_mapping_feature),
        )
        .route(
            "/api/admin/config/smart_connect/details",
            get(get_smart_connect_details),
        )
        .route(
            "/api/admin/config/smart_connect",
            post(update_smart_connect),
        )
        .route(
            "/api/admin/config/fnos_network_tuning",
            get(get_fnos_network_tuning).post(update_fnos_network_tuning),
        )
        .route(
            "/api/admin/config/proxy_protocol_force",
            get(get_proxy_protocol_force).post(update_proxy_protocol_force),
        )
        .route(
            "/api/admin/config/fnos_share_bypass",
            get(get_fnos_share_bypass).post(update_fnos_share_bypass),
        )
        .route(
            "/api/admin/config/fnos_port_icon_hijack",
            get(get_fnos_port_icon_hijack).post(update_fnos_port_icon_hijack),
        )
        .route(
            "/api/admin/config/auto_https",
            get(get_auto_https).post(update_auto_https),
        )
        .route(
            "/api/admin/config/auto_manage_firewall",
            post(update_auto_manage_firewall),
        )
        .route("/api/admin/firewall/reset", post(reset_firewall))
        .route("/api/admin/firewall/clear", post(clear_firewall))
        .route(
            "/api/admin/config/default_route",
            get(get_default_route).post(update_default_route),
        )
        .route(
            "/api/admin/config/default_tunnel",
            post(update_default_tunnel),
        )
        .route(
            "/api/admin/config/run_mode_prompt_preferences",
            get(get_run_mode_prompt_preferences).post(update_run_mode_prompt_preferences),
        )
        .route("/api/admin/config/welcome_guide", get(get_welcome_guide))
        .route(
            "/api/admin/config/welcome_guide/complete",
            post(complete_welcome_guide),
        )
        .route("/api/admin/sync-routes", post(sync_routes))
}

fn admin_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.admin.{key}"))
}

fn admin_text_params(translator: &Translator, key: &str, params: &[(&str, String)]) -> String {
    translator.t_params(&format!("server.admin.{key}"), params)
}

fn firewall_text_params(translator: &Translator, key: &str, params: &[(&str, String)]) -> String {
    translator.t_params(&format!("server.firewall.{key}"), params)
}

fn runtime_config_route_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.admin.runtimeConfigRoutes.{key}"))
}

fn localize_runtime_config_error(translator: &Translator, message: &str) -> String {
    if message.trim() == GO_BACKEND_UNSUCCESSFUL_RESPONSE {
        return runtime_config_route_text(translator, "upstreamUnavailable");
    }
    message.to_string()
}

fn smart_connect_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.smartConnect.{key}"))
}

fn smart_connect_text_params(
    translator: &Translator,
    key: &str,
    params: &[(&str, String)],
) -> String {
    translator.t_params(&format!("server.smartConnect.{key}"), params)
}

fn capability_blocked_text(state: &AppState, capability: &str, translator: &Translator) -> String {
    let profile = runtime_profile::get_runtime_profile(state);
    runtime_profile::capability_unavailable_message(capability, &profile, translator)
}

pub(crate) async fn sync_runtime_config_on_boot(state: AppState) {
    let mut config = match state.redis.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load config for boot runtime sync");
            return;
        }
    };
    match apply_boot_config_migrations(&state, &mut config).await {
        Ok(applied) if !applied.is_empty() => {
            tracing::info!(migrations = ?applied, "applied boot config migrations");
        }
        Ok(_) => {}
        Err(error) => tracing::warn!(%error, "failed to apply boot config migrations"),
    }
    match apply_runtime_constraints_on_boot(&state, &mut config).await {
        Ok(corrected) if !corrected.is_empty() => {
            tracing::info!(corrected = ?corrected, "applied runtime config constraints");
        }
        Ok(_) => {}
        Err(error) => tracing::warn!(%error, "failed to apply runtime config constraints"),
    }
    let run_type = config.get("run_type").and_then(Value::as_i64).unwrap_or(3);
    if let Err(error) = apply_run_type_config(&state, &config, run_type).await {
        tracing::warn!(%error, "failed to sync run type config on boot");
    }

    let gateway_logging = normalize_gateway_logging(config.get("gateway_logging"));
    if let Err(error) = state
        .go_backend
        .set_gateway_logging_config(&gateway_logging)
        .await
        .and_then(ensure_go_success)
    {
        tracing::warn!(%error, "failed to sync gateway logging config on boot");
    }

    if let Err(error) = waf::sync_waf_config_to_gateway(&state, config.get("waf")).await {
        tracing::warn!(%error, "failed to sync WAF config on boot");
    }

    if let Err(error) = sync_smart_connect(&state, &config).await {
        tracing::warn!(%error, "failed to sync smart connect on boot");
    }

    let fnos_port_icon_hijack =
        normalize_fnos_port_icon_hijack(config.get("fnos_port_icon_hijack"));
    if let Err(error) = state
        .go_backend
        .set_fnos_port_icon_hijack_config(&fnos_port_icon_hijack)
        .await
        .and_then(ensure_go_success)
    {
        tracing::warn!(%error, "failed to sync FnOS port icon hijack config on boot");
    }
}

async fn get_captcha(State(state): State<AppState>) -> Response {
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

async fn update_captcha(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
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

    match update_captcha_settings(&state, &body).await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to save captcha config");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                runtime_config_route_text(&translator, "saveCaptchaFailed"),
            )
        }
    }
}

async fn get_terminal_feature(State(state): State<AppState>) -> Response {
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

async fn update_terminal_feature(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Response {
    let translator = Translator::from_state(&state).await;
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

async fn update_run_type(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
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

    let previous_config = match state.redis.get_config().await {
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

    if let Err(error) = state.redis.save_config(&next_config).await {
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
            if let Err(rollback_error) = state.redis.save_config(&previous_config).await {
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

async fn cleanup_auto_whitelist_after_direct_mode(state: &AppState, run_type: i64) {
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

async fn get_protocol_mapping_feature(State(state): State<AppState>) -> Response {
    let fallback_config = state.redis.get_config().await.ok();
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

async fn update_protocol_mapping_feature(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let previous_config = match state.redis.get_config().await {
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
    let next = normalize_protocol_mapping_feature(Some(&current));
    let mut next_config = previous_config.clone();
    if next.get("enabled").and_then(Value::as_bool) == Some(false) {
        let object = ensure_config_object(&mut next_config);
        object.insert("stream_mappings".to_string(), Value::Array(Vec::new()));
    }

    if let Err(error) = save_protocol_mapping_feature(&state, &next).await {
        tracing::warn!(%error, "failed to save protocol mapping feature key");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            admin_text(&translator, "protocolMapping.updateFeatureFailed"),
        );
    }
    if next.get("enabled").and_then(Value::as_bool) == Some(false) {
        if let Err(error) = state.redis.save_config(&next_config).await {
            tracing::warn!(%error, "failed to clear stream mappings after protocol mapping disabled");
            if let Err(rollback_error) =
                save_protocol_mapping_feature(&state, &previous_settings).await
            {
                tracing::warn!(
                    %rollback_error,
                    "failed to rollback protocol mapping feature key"
                );
            }
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_text(&translator, "protocolMapping.updateFeatureFailed"),
            );
        }
    }

    match apply_run_type_config(&state, &next_config, run_type).await {
        Ok(()) => response::ok(next).into_response(),
        Err(error) => {
            rollback_config_protocol_feature_and_runtime(
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

async fn get_smart_connect_details(State(state): State<AppState>) -> Response {
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

async fn update_smart_connect(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    let translator = Translator::from_state(&state).await;
    if !host_firewall_available(&state) {
        return response::error(
            StatusCode::FORBIDDEN,
            capability_blocked_text(&state, "smart_connect_available", &translator),
        );
    }

    let previous_config = match state.redis.get_config().await {
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

    if let Err(error) = state.redis.save_config(&next_config).await {
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

async fn get_fnos_network_tuning(State(state): State<AppState>) -> Response {
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

async fn update_fnos_network_tuning(
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

async fn get_fnos_share_bypass(State(state): State<AppState>) -> Response {
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

async fn update_fnos_share_bypass(
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

async fn get_fnos_port_icon_hijack(State(state): State<AppState>) -> Response {
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

async fn update_fnos_port_icon_hijack(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let previous_config = match state.redis.get_config().await {
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
        next_config = redis_store::default_config();
    }
    if let Some(object) = next_config.as_object_mut() {
        object.insert("fnos_port_icon_hijack".to_string(), next.clone());
    }

    if let Err(error) = state.redis.save_config(&next_config).await {
        tracing::warn!(%error, "failed to save fnos port icon hijack config");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            admin_text(&translator, "fnosPortIcon.syncFailed"),
        );
    }

    match state
        .go_backend
        .set_fnos_port_icon_hijack_config(&next)
        .await
        .and_then(ensure_go_success)
    {
        Ok(()) => response::ok(next).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to sync fnos port icon hijack config to Go backend");
            if let Err(rollback_error) = state.redis.save_config(&previous_config).await {
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

async fn get_auto_https(State(state): State<AppState>) -> Response {
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

async fn update_auto_https(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    let translator = Translator::from_state(&state).await;
    let requested_enabled = body.get("enabled").and_then(Value::as_bool);
    if requested_enabled == Some(true) {
        let target = deployment_target(&state);
        if target == "docker" || target == "openwrt" {
            let key = if target == "openwrt" {
                "autoHttps.openWrtUnsupported"
            } else {
                "autoHttps.dockerUnsupported"
            };
            return response::error(StatusCode::FORBIDDEN, admin_text(&translator, key));
        }
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

async fn update_auto_manage_firewall(
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

async fn reset_firewall(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
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
    match reset_firewall_for_run_type(&state, run_type).await {
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

async fn clear_firewall(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    if !host_firewall_available(&state) {
        return response::error(
            StatusCode::FORBIDDEN,
            capability_blocked_text(&state, "host_firewall_available", &translator),
        );
    }
    let result = async {
        clear_legacy_gateway_redirects(&state, gateway_port(), true).await?;
        state
            .go_backend
            .clean_iptables()
            .await
            .map_err(|error| error.to_string())
            .and_then(|value| ensure_go_success(value).map_err(|error| error.to_string()))
    }
    .await;
    match result {
        Ok(()) => {
            let data = json!({ "gatewayPort": gateway_port() });
            Json(json!({
                "success": true,
                "data": data,
                "message": admin_text_params(
                    &translator,
                    "firewall.clearSuccess",
                    &[("port", gateway_port().to_string())],
                ),
            }))
            .into_response()
        }
        Err(error) => response::error(
            StatusCode::BAD_GATEWAY,
            localize_runtime_config_error(&translator, &error),
        ),
    }
}

async fn sync_routes(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    let config = match state.redis.get_config().await {
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
        .go_backend
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

    let waf_config = match waf::sync_waf_config_to_gateway(&state, config.get("waf")).await {
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

async fn get_default_route(State(state): State<AppState>) -> Response {
    match state.redis.get_config().await {
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

async fn update_default_route(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    let translator = Translator::from_state(&state).await;
    let path = body
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    match save_top_level_config_value(&state, "default_route", Value::String(path.clone())).await {
        Ok(()) => {
            if let Err(error) = state.go_backend.set_default_route(&path).await {
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

async fn update_default_tunnel(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
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

async fn get_proxy_protocol_force(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    let upstream_unavailable = runtime_config_route_text(&translator, "upstreamUnavailable");
    match state.go_backend.get_proxy_protocol_force().await {
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

async fn update_proxy_protocol_force(
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
    match state.go_backend.set_proxy_protocol_force(force).await {
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

async fn get_run_mode_prompt_preferences(State(state): State<AppState>) -> Response {
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

async fn update_run_mode_prompt_preferences(
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
        .redis
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

async fn get_welcome_guide(State(state): State<AppState>) -> Response {
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

async fn complete_welcome_guide(State(state): State<AppState>) -> Response {
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
        .redis
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

async fn apply_boot_config_migrations(
    state: &AppState,
    config: &mut Value,
) -> redis::RedisResult<Vec<&'static str>> {
    let mut applied = Vec::new();
    let mut config_changed = false;
    let mut mark_throttle_patch_done = false;
    let mut mark_resource_alerts_patch_done = false;

    if state
        .redis
        .get_string_value(LEGACY_REVERSE_PROXY_THROTTLE_PATCH_FLAG_KEY)
        .await?
        .as_deref()
        != Some("1")
    {
        if legacy_reverse_proxy_throttle_matches(config.get("reverse_proxy_throttle")) {
            let mut throttle = config
                .get("reverse_proxy_throttle")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            throttle.insert("requests_per_second".to_string(), json!(100));
            throttle.insert("burst".to_string(), json!(200));
            throttle.insert("block_seconds".to_string(), json!(30));
            ensure_config_object(config).insert(
                "reverse_proxy_throttle".to_string(),
                Value::Object(throttle),
            );
            config_changed = true;
            applied.push("legacy_reverse_proxy_throttle");
        }
        mark_throttle_patch_done = true;
    }

    if state
        .redis
        .get_string_value(LEGACY_EVENT_SYSTEM_RESOURCE_ALERTS_PATCH_FLAG_KEY)
        .await?
        .as_deref()
        != Some("1")
    {
        if legacy_resource_alert_rules_match(config) {
            set_event_resource_alert_enabled(config, "cpu_alert");
            set_event_resource_alert_enabled(config, "memory_alert");
            config_changed = true;
            applied.push("legacy_event_system_resource_alerts");
        }
        mark_resource_alerts_patch_done = true;
    }

    if config_changed {
        state.redis.save_config(config).await?;
    }
    if mark_throttle_patch_done {
        state
            .redis
            .set_string_value(LEGACY_REVERSE_PROXY_THROTTLE_PATCH_FLAG_KEY, "1")
            .await?;
    }
    if mark_resource_alerts_patch_done {
        state
            .redis
            .set_string_value(LEGACY_EVENT_SYSTEM_RESOURCE_ALERTS_PATCH_FLAG_KEY, "1")
            .await?;
    }
    Ok(applied)
}

async fn apply_runtime_constraints_on_boot(
    state: &AppState,
    config: &mut Value,
) -> redis::RedisResult<Vec<String>> {
    let mut corrected = Vec::new();
    let target = deployment_target(state);
    let host_runtime = host_runtime_available(state);
    let host_firewall = host_firewall_available(state);

    if !host_runtime && config.get("run_type").and_then(Value::as_i64) == Some(0) {
        ensure_config_object(config).insert("run_type".to_string(), json!(3));
        corrected.push("run_type=0 -> run_type=3".to_string());
    }

    let smart = normalize_smart_connect_config(config.get("smart_connect"));
    if !host_runtime && smart.get("enabled").and_then(Value::as_bool) == Some(true) {
        let mut next = smart;
        if let Some(object) = next.as_object_mut() {
            object.insert("enabled".to_string(), Value::Bool(false));
        }
        ensure_config_object(config).insert("smart_connect".to_string(), next);
        corrected.push("smart_connect.enabled -> false".to_string());
    }

    let terminal = normalize_terminal_feature(config.get("terminal_feature"));
    if matches!(target.as_str(), "docker" | "openwrt")
        && terminal.get("enabled").and_then(Value::as_bool) == Some(true)
    {
        let mut next = terminal;
        if let Some(object) = next.as_object_mut() {
            object.insert("enabled".to_string(), Value::Bool(false));
        }
        ensure_config_object(config).insert("terminal_feature".to_string(), next);
        corrected.push("terminal_feature.enabled -> false".to_string());
    }

    let auto_https = auto_https::normalize_auto_https_config(config.get("auto_https"));
    if matches!(target.as_str(), "docker" | "openwrt")
        && auto_https.get("enabled").and_then(Value::as_bool) == Some(true)
    {
        let mut next = auto_https;
        if let Some(object) = next.as_object_mut() {
            object.insert("enabled".to_string(), Value::Bool(false));
        }
        ensure_config_object(config).insert("auto_https".to_string(), next);
        corrected.push("auto_https.enabled -> false".to_string());
    }

    let ssh_security = crate::ssh_security::normalize_config(config.get("ssh_security").cloned());
    if (!host_firewall || target == "openwrt")
        && ssh_security.get("enabled").and_then(Value::as_bool) == Some(true)
    {
        let mut next = ssh_security;
        if let Some(object) = next.as_object_mut() {
            object.insert("enabled".to_string(), Value::Bool(false));
        }
        ensure_config_object(config).insert("ssh_security".to_string(), next);
        corrected.push("ssh_security.enabled -> false".to_string());
    }

    let auto_manage_firewall =
        config.get("auto_manage_firewall").and_then(Value::as_bool) != Some(false);
    if !host_firewall {
        if config.get("auto_manage_firewall").and_then(Value::as_bool) != Some(false) {
            corrected.push("auto_manage_firewall -> false".to_string());
        }
        ensure_config_object(config).insert("auto_manage_firewall".to_string(), Value::Bool(false));
    } else if config.get("auto_manage_firewall").and_then(Value::as_bool)
        != Some(auto_manage_firewall)
    {
        ensure_config_object(config).insert(
            "auto_manage_firewall".to_string(),
            json!(auto_manage_firewall),
        );
        corrected.push(format!("auto_manage_firewall -> {auto_manage_firewall}"));
    }

    if !corrected.is_empty() {
        state.redis.save_config(config).await?;
    }
    Ok(corrected)
}

fn legacy_reverse_proxy_throttle_matches(value: Option<&Value>) -> bool {
    int_field(value, "requests_per_second", 100, 1, 10_000) == 20
        && int_field(value, "burst", 200, 1, 100_000) == 50
        && int_field(value, "block_seconds", 30, 1, 86_400) == 30
}

fn legacy_resource_alert_rules_match(config: &Value) -> bool {
    let rules = config
        .pointer("/event_system/rules")
        .unwrap_or(&Value::Null);
    resource_rule_matches(rules.get("cpu_alert"), false, 85, 70, 15, 120)
        && resource_rule_matches(rules.get("memory_alert"), false, 90, 75, 15, 120)
}

fn resource_rule_matches(
    value: Option<&Value>,
    enabled: bool,
    threshold: i64,
    recover: i64,
    sample_interval: i64,
    sustain: i64,
) -> bool {
    bool_field(value, "enabled", true) == enabled
        && int_field(value, "threshold_percent", threshold, 1, 100) == threshold
        && int_field(value, "recover_percent", recover, 0, 100) == recover
        && int_field(value, "sample_interval_seconds", sample_interval, 1, 3600) == sample_interval
        && int_field(value, "sustain_seconds", sustain, 1, 86_400) == sustain
}

fn set_event_resource_alert_enabled(config: &mut Value, key: &str) {
    let object = ensure_config_object(config);
    let event_system = object
        .entry("event_system".to_string())
        .or_insert_with(|| json!({ "enabled": true, "retention_days": 30, "rules": {} }));
    if !event_system.is_object() {
        *event_system = json!({ "enabled": true, "retention_days": 30, "rules": {} });
    }
    let event_object = event_system
        .as_object_mut()
        .expect("event system is object");
    let rules = event_object
        .entry("rules".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !rules.is_object() {
        *rules = Value::Object(Map::new());
    }
    let rules_object = rules.as_object_mut().expect("rules is object");
    let rule = rules_object
        .entry(key.to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !rule.is_object() {
        *rule = Value::Object(Map::new());
    }
    rule.as_object_mut()
        .expect("rule is object")
        .insert("enabled".to_string(), Value::Bool(true));
}

async fn load_config_section(
    state: &AppState,
    key: &str,
    normalize: fn(Option<&Value>) -> Value,
) -> redis::RedisResult<Value> {
    let config = state.redis.get_config().await?;
    Ok(normalize(config.get(key)))
}

async fn update_config_section(
    state: &AppState,
    key: &str,
    patch: &Value,
    normalize: fn(Option<&Value>) -> Value,
) -> redis::RedisResult<Value> {
    let mut config = state.redis.get_config().await?;
    if !config.is_object() {
        config = redis_store::default_config();
    }
    let mut next = normalize(config.get(key));
    merge_object(&mut next, patch);
    next = normalize(Some(&next));
    if let Some(object) = config.as_object_mut() {
        object.insert(key.to_string(), next.clone());
    }
    state.redis.save_config(&config).await?;
    Ok(next)
}

async fn save_top_level_config_value(
    state: &AppState,
    key: &str,
    value: Value,
) -> redis::RedisResult<()> {
    let mut config = state.redis.get_config().await?;
    if !config.is_object() {
        config = redis_store::default_config();
    }
    if let Some(object) = config.as_object_mut() {
        object.insert(key.to_string(), value);
    }
    state.redis.save_config(&config).await
}

pub(crate) async fn load_protocol_mapping_feature(
    state: &AppState,
    fallback_config: Option<&Value>,
) -> redis::RedisResult<Value> {
    if let Some(value) = state
        .redis
        .get_json_value(PROTOCOL_MAPPING_FEATURE_KEY)
        .await?
    {
        return Ok(normalize_protocol_mapping_feature(Some(&value)));
    }
    Ok(normalize_protocol_mapping_feature(
        fallback_config.and_then(|config| config.get("protocol_mapping_feature")),
    ))
}

async fn save_protocol_mapping_feature(state: &AppState, value: &Value) -> redis::RedisResult<()> {
    let next = normalize_protocol_mapping_feature(Some(value));
    state
        .redis
        .set_json_value(PROTOCOL_MAPPING_FEATURE_KEY, &next)
        .await
}

async fn load_captcha_settings(state: &AppState) -> redis::RedisResult<Value> {
    let value = match state.redis.get_json_value(CAPTCHA_SETTINGS_KEY).await? {
        Some(value) => Some(value),
        None => {
            state
                .redis
                .get_json_value(LEGACY_CAPTCHA_SETTINGS_KEY)
                .await?
        }
    };
    Ok(normalize_captcha_settings(value.as_ref()))
}

async fn update_captcha_settings(state: &AppState, patch: &Value) -> redis::RedisResult<Value> {
    let current = load_captcha_settings(state).await?;
    let mut next = current.clone();
    merge_object(&mut next, patch);
    if let Some(patch_turnstile) = patch.get("turnstile").and_then(Value::as_object) {
        let mut turnstile = current
            .get("turnstile")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        for (key, value) in patch_turnstile {
            turnstile.insert(key.clone(), value.clone());
        }
        if let Some(object) = next.as_object_mut() {
            object.insert("turnstile".to_string(), Value::Object(turnstile));
        }
    }
    next = normalize_captcha_settings(Some(&next));
    state
        .redis
        .set_json_value(CAPTCHA_SETTINGS_KEY, &next)
        .await?;
    Ok(next)
}

async fn load_run_mode_prompt_preferences(state: &AppState) -> redis::RedisResult<Value> {
    Ok(normalize_run_mode_prompt_preferences(
        state
            .redis
            .get_json_value(RUN_MODE_PROMPT_PREFERENCES_KEY)
            .await?
            .as_ref(),
    ))
}

async fn load_welcome_guide_status(state: &AppState) -> redis::RedisResult<Value> {
    let raw = state
        .redis
        .get_string_value(WELCOME_GUIDE_STATUS_KEY)
        .await?;
    Ok(match raw.as_deref() {
        None => json!({ "completed": false, "completed_at": Value::Null }),
        Some("1") | Some("true") => json!({ "completed": true, "completed_at": Value::Null }),
        Some(value) => serde_json::from_str::<Value>(value)
            .ok()
            .map(|value| {
                json!({
                    "completed": value.get("completed").and_then(Value::as_bool).unwrap_or(false),
                    "completed_at": value
                        .get("completed_at")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(|value| Value::String(value.to_string()))
                        .unwrap_or(Value::Null),
                })
            })
            .unwrap_or_else(|| json!({ "completed": false, "completed_at": Value::Null })),
    })
}

fn merge_object(target: &mut Value, patch: &Value) {
    let Some(target) = target.as_object_mut() else {
        return;
    };
    if let Some(patch) = patch.as_object() {
        for (key, value) in patch {
            target.insert(key.clone(), value.clone());
        }
    }
}

fn merge_runtime(mut config: Value, runtime: Value) -> Value {
    if let Some(object) = config.as_object_mut() {
        object.insert("runtime".to_string(), runtime);
    }
    config
}

fn normalize_captcha_settings(value: Option<&Value>) -> Value {
    let provider = if value
        .and_then(|value| value.get("provider"))
        .and_then(Value::as_str)
        == Some("turnstile")
    {
        "turnstile"
    } else {
        "pow"
    };
    json!({
        "provider": provider,
        "widget_mode": "normal",
        "pow": {},
        "turnstile": {
            "site_key": value
                .and_then(|value| value.pointer("/turnstile/site_key"))
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or(""),
            "secret_key": value
                .and_then(|value| value.pointer("/turnstile/secret_key"))
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or(""),
        },
    })
}

pub(crate) fn normalize_terminal_feature(value: Option<&Value>) -> Value {
    json!({
        "enabled": bool_field(value, "enabled", false),
        "default_cwd": normalize_terminal_default_cwd(
            value
                .and_then(|value| value.get("default_cwd"))
                .and_then(Value::as_str)
        ),
        "max_sessions": int_field(value, "max_sessions", 3, 1, 12),
        "idle_timeout_seconds": int_field(value, "idle_timeout_seconds", 24 * 60 * 60, 60, 7 * 24 * 60 * 60),
        "resume_backend": "tmux",
        "allow_mobile_toolbar": bool_field(value, "allow_mobile_toolbar", true),
        "dangerously_run_as_current_user": bool_field(value, "dangerously_run_as_current_user", true),
    })
}

pub(crate) fn normalize_fnos_share_bypass(value: Option<&Value>) -> Value {
    json!({
        "enabled": bool_field(value, "enabled", false),
        "upstream_timeout_ms": int_field(value, "upstream_timeout_ms", 2500, 500, 15000),
        "validation_cache_ttl_seconds": int_field(value, "validation_cache_ttl_seconds", 30, 5, 300),
        "validation_lock_ttl_seconds": int_field(value, "validation_lock_ttl_seconds", 5, 1, 30),
        "session_ttl_seconds": int_field(value, "session_ttl_seconds", 300, 30, 3600),
    })
}

pub(crate) fn normalize_fnos_port_icon_hijack(value: Option<&Value>) -> Value {
    json!({
        "enabled": bool_field(value, "enabled", false),
        "updated_at": optional_string_field(value, "updated_at").map(Value::String).unwrap_or(Value::Null),
    })
}

fn normalize_auto_manage_firewall(value: Option<&Value>) -> bool {
    value.and_then(Value::as_bool) != Some(false)
}

fn normalize_run_mode_prompt_preferences(value: Option<&Value>) -> Value {
    json!({
        "directToReverseProxy": bool_field(value, "directToReverseProxy", false),
        "reverseProxyToDirect": bool_field(value, "reverseProxyToDirect", false),
        "switchToSubdomain": bool_field(value, "switchToSubdomain", false),
        "subdomainToReverseProxy": bool_field(value, "subdomainToReverseProxy", false),
    })
}

fn bool_field(value: Option<&Value>, key: &str, fallback: bool) -> bool {
    value
        .and_then(|value| value.get(key))
        .and_then(Value::as_bool)
        .unwrap_or(fallback)
}

fn bool_field_alias(
    value: Option<&Value>,
    snake_key: &str,
    camel_key: &str,
    fallback: bool,
) -> bool {
    value
        .and_then(|value| {
            value
                .get(snake_key)
                .and_then(Value::as_bool)
                .or_else(|| value.get(camel_key).and_then(Value::as_bool))
        })
        .unwrap_or(fallback)
}

fn optional_string_field(value: Option<&Value>, key: &str) -> Option<String> {
    value
        .and_then(|value| value.get(key))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn optional_string_field_alias(
    value: Option<&Value>,
    snake_key: &str,
    camel_key: &str,
) -> Option<String> {
    value
        .and_then(|value| {
            value
                .get(snake_key)
                .and_then(Value::as_str)
                .or_else(|| value.get(camel_key).and_then(Value::as_str))
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn int_field(value: Option<&Value>, key: &str, fallback: i64, min: i64, max: i64) -> i64 {
    value
        .and_then(|value| value.get(key))
        .and_then(parse_int_field_value)
        .unwrap_or(fallback)
        .clamp(min, max)
}

fn parse_int_field_value(value: &Value) -> Option<i64> {
    parse_i64_prefix(js_string_for_parse_int(value).trim_start())
}

fn js_string_for_parse_int(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.clone(),
        Value::Array(values) => values
            .iter()
            .map(js_array_item_string_for_parse_int)
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".to_string(),
    }
}

fn js_array_item_string_for_parse_int(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Array(_) => js_string_for_parse_int(value),
        Value::Object(_) => "[object Object]".to_string(),
        _ => js_string_for_parse_int(value),
    }
}

fn parse_i64_prefix(value: &str) -> Option<i64> {
    let mut chars = value.char_indices().peekable();
    if matches!(chars.peek(), Some((_, '+' | '-'))) {
        chars.next();
    }
    let mut end = 0;
    let mut has_digit = false;
    for (index, ch) in chars {
        if ch.is_ascii_digit() {
            has_digit = true;
            end = index + ch.len_utf8();
        } else {
            break;
        }
    }
    if !has_digit {
        return None;
    }
    value[..end].parse::<i64>().ok()
}

fn ensure_config_object(value: &mut Value) -> &mut Map<String, Value> {
    if !value.is_object() {
        *value = redis_store::default_config();
    }
    value.as_object_mut().expect("config is object")
}

fn normalize_run_type(value: Option<&Value>) -> Option<i64> {
    match value.and_then(Value::as_i64) {
        Some(0) => Some(0),
        Some(1) => Some(1),
        Some(3) => Some(3),
        _ => None,
    }
}

pub(crate) fn normalize_protocol_mapping_feature(value: Option<&Value>) -> Value {
    json!({ "enabled": bool_field(value, "enabled", false) })
}

fn normalize_smart_connect_config(value: Option<&Value>) -> Value {
    json!({
        "enabled": bool_field(value, "enabled", false),
        "selected_ipv4": optional_string_field(value, "selected_ipv4").unwrap_or_default(),
    })
}

fn default_smart_connect_runtime() -> Value {
    json!({
        "selected_ipv4": "",
        "synced_domains": [],
        "managed_rule_count": 0,
        "last_sync_at": Value::Null,
        "last_sync_error": Value::Null,
    })
}

fn normalize_smart_connect_runtime(value: Option<&Value>) -> Value {
    let raw = value.unwrap_or(&Value::Null);
    json!({
        "selected_ipv4": raw.get("selected_ipv4").and_then(Value::as_str).unwrap_or("").trim(),
        "synced_domains": string_array(raw.get("synced_domains")),
        "managed_rule_count": raw.get("managed_rule_count").and_then(Value::as_i64).unwrap_or(0).max(0),
        "last_sync_at": raw.get("last_sync_at").and_then(Value::as_str).map(|value| Value::String(value.trim().to_string())).unwrap_or(Value::Null),
        "last_sync_error": raw.get("last_sync_error").and_then(Value::as_str).map(|value| Value::String(value.trim().to_string())).unwrap_or(Value::Null),
    })
}

fn normalize_gateway_logging(value: Option<&Value>) -> Value {
    json!({
        "enabled": bool_field(value, "enabled", false),
        "max_days": int_field(value, "max_days", 7, 1, JS_MAX_SAFE_INTEGER_I64),
    })
}

pub(crate) fn normalize_fnos_network_tuning(value: Option<&Value>) -> Value {
    json!({
        "bbr_enabled": bool_field_alias(value, "bbr_enabled", "bbrEnabled", false),
        "mtu_probing_enabled": bool_field_alias(value, "mtu_probing_enabled", "mtuProbingEnabled", false),
        "previous_tcp_congestion_control": optional_string_field_alias(value, "previous_tcp_congestion_control", "previousTcpCongestionControl").map(Value::String).unwrap_or(Value::Null),
        "previous_default_qdisc": optional_string_field_alias(value, "previous_default_qdisc", "previousDefaultQdisc").map(Value::String).unwrap_or(Value::Null),
        "previous_tcp_mtu_probing": optional_string_field_alias(value, "previous_tcp_mtu_probing", "previousTcpMtuProbing").map(Value::String).unwrap_or(Value::Null),
        "updated_at": optional_string_field_alias(value, "updated_at", "updatedAt").map(Value::String).unwrap_or(Value::Null),
        "last_error": optional_string_field_alias(value, "last_error", "lastError").map(Value::String).unwrap_or(Value::Null),
    })
}

fn log_go_value_result(result: Result<Value, String>, operation: &'static str) {
    match result {
        Ok(value) => {
            if let Err(error) = ensure_go_success(value) {
                tracing::warn!(%error, operation, "go backend call failed during run type apply");
            }
        }
        Err(error) => {
            tracing::warn!(%error, operation, "go backend request failed during run type apply");
        }
    }
}

fn log_go_status_value_result(
    result: Result<(reqwest::StatusCode, Value), String>,
    operation: &'static str,
) {
    match result {
        Ok((status, value)) => {
            if !status.is_success() {
                let error = go_response_message(&value, &format!("go backend returned {status}"));
                tracing::warn!(%error, operation, "go backend call failed during run type apply");
                return;
            }
            if let Err(error) = ensure_go_success(value) {
                tracing::warn!(%error, operation, "go backend call failed during run type apply");
            }
        }
        Err(error) => {
            tracing::warn!(%error, operation, "go backend request failed during run type apply");
        }
    }
}

pub(crate) async fn apply_run_type_config(
    state: &AppState,
    config: &Value,
    run_type: i64,
) -> Result<(), String> {
    log_go_value_result(
        state
            .go_backend
            .set_auth_config(&build_gateway_auth_config(config))
            .await
            .map_err(|error| error.to_string()),
        "sync auth gateway config",
    );
    let default_throttle = json!({
        "enabled": true,
        "requests_per_second": 100,
        "burst": 200,
        "block_seconds": 30,
    });
    let throttle = config
        .get("reverse_proxy_throttle")
        .unwrap_or(&default_throttle);
    log_go_value_result(
        state
            .go_backend
            .set_reverse_proxy_throttle(throttle)
            .await
            .map_err(|error| error.to_string()),
        "sync reverse proxy throttle",
    );
    let default_crawler = json!({ "enabled": false });
    let crawler = config
        .get("gateway_crawler_blocker")
        .unwrap_or(&default_crawler);
    log_go_value_result(
        state
            .go_backend
            .set_crawler_blocker_config(crawler)
            .await
            .map_err(|error| error.to_string()),
        "sync crawler blocker config",
    );
    whitelist::sync_reverse_proxy_trusted_ips(state).await;
    if let Err(error) = gateway_settings::sync_gateway_visibility_runtime_from_store(state).await {
        tracing::warn!(
            %error,
            "failed to sync gateway visibility runtime during run type apply"
        );
    }
    if let Err(error) =
        gateway_settings::sync_gateway_target_runtime_for_config(state, config, false).await
    {
        tracing::warn!(
            %error,
            "failed to sync gateway target runtime during run type apply"
        );
    }

    let protocol_mapping_feature = load_protocol_mapping_feature(state, Some(config))
        .await
        .map_err(|error| error.to_string())?;
    let protocol_mapping_enabled = run_type == 3
        && protocol_mapping_feature
            .get("enabled")
            .and_then(Value::as_bool)
            == Some(true);

    if run_type == 1 {
        log_go_value_result(
            state
                .go_backend
                .set_proxy_protocol_force(true)
                .await
                .map_err(|error| error.to_string()),
            "enable proxy protocol force",
        );
        log_go_value_result(
            state
                .go_backend
                .flush_stream_rules()
                .await
                .map_err(|error| error.to_string()),
            "flush stream rules",
        );

        if is_reverse_proxy_subdomain_mode(config) {
            log_go_value_result(
                state
                    .go_backend
                    .flush_rules()
                    .await
                    .map_err(|error| error.to_string()),
                "flush path rules",
            );
            sync_host_rules(state, config).await;
            log_go_status_value_result(
                state
                    .go_backend
                    .set_default_route("/__select__")
                    .await
                    .map_err(|error| error.to_string()),
                "sync disabled default route",
            );
            return Ok(());
        }

        log_go_value_result(
            state
                .go_backend
                .flush_host_rules()
                .await
                .map_err(|error| error.to_string()),
            "flush host rules",
        );
        sync_path_rules(state, config).await;
        sync_default_route(state, config).await;
        return Ok(());
    }

    log_go_value_result(
        state
            .go_backend
            .set_proxy_protocol_force(false)
            .await
            .map_err(|error| error.to_string()),
        "disable proxy protocol force",
    );

    if run_type == 3 {
        log_go_value_result(
            state
                .go_backend
                .flush_rules()
                .await
                .map_err(|error| error.to_string()),
            "flush path rules",
        );
        sync_host_rules(state, config).await;
        if protocol_mapping_enabled {
            sync_stream_rules(state, config).await;
        } else {
            log_go_value_result(
                state
                    .go_backend
                    .flush_stream_rules()
                    .await
                    .map_err(|error| error.to_string()),
                "flush stream rules",
            );
        }
        sync_default_route(state, config).await;
        maybe_apply_host_firewall(state, config, run_type, protocol_mapping_enabled).await?;
        return Ok(());
    }

    log_go_value_result(
        state
            .go_backend
            .flush_host_rules()
            .await
            .map_err(|error| error.to_string()),
        "flush host rules",
    );
    log_go_value_result(
        state
            .go_backend
            .flush_stream_rules()
            .await
            .map_err(|error| error.to_string()),
        "flush stream rules",
    );
    sync_path_rules(state, config).await;
    sync_default_route(state, config).await;
    if run_type == 0 {
        sync_auth_entry_route(state).await;
    }
    maybe_apply_host_firewall(state, config, run_type, protocol_mapping_enabled).await
}

async fn sync_path_rules(state: &AppState, config: &Value) {
    log_go_value_result(
        state
            .go_backend
            .set_rules(
                config
                    .get("proxy_mappings")
                    .unwrap_or(&Value::Array(Vec::new())),
            )
            .await
            .map_err(|error| error.to_string()),
        "sync path rules",
    );
}

async fn sync_host_rules(state: &AppState, config: &Value) {
    let mappings = config
        .get("host_mappings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    log_go_value_result(
        state
            .go_backend
            .set_host_rules(&build_host_rules_payload(&mappings))
            .await
            .map_err(|error| error.to_string()),
        "sync host rules",
    );
}

async fn sync_stream_rules(state: &AppState, config: &Value) {
    log_go_value_result(
        state
            .go_backend
            .set_stream_rules(
                config
                    .get("stream_mappings")
                    .unwrap_or(&Value::Array(Vec::new())),
            )
            .await
            .map_err(|error| error.to_string()),
        "sync stream rules",
    );
}

async fn sync_auth_entry_route(state: &AppState) {
    log_go_value_result(
        state
            .go_backend
            .set_rules(&auth_entry_route_payload(state.settings.auth_port))
            .await
            .map_err(|error| error.to_string()),
        "sync auth entry route",
    );
    log_go_status_value_result(
        state
            .go_backend
            .set_default_route("/auth")
            .await
            .map_err(|error| error.to_string()),
        "sync auth default route",
    );
}

fn auth_entry_route_payload(auth_port: u16) -> Value {
    json!([{
        "path": "/auth",
        "target": format!("http://127.0.0.1:{auth_port}"),
        "rewrite_html": false,
        "use_auth": false,
        "use_root_mode": false,
        "strip_path": false,
    }])
}

async fn sync_default_route(state: &AppState, config: &Value) {
    let route = config
        .get("default_route")
        .and_then(Value::as_str)
        .unwrap_or("/__select__");
    log_go_status_value_result(
        state
            .go_backend
            .set_default_route(route)
            .await
            .map_err(|error| error.to_string()),
        "sync default route",
    );
}

async fn maybe_apply_host_firewall(
    state: &AppState,
    config: &Value,
    run_type: i64,
    protocol_mapping_enabled: bool,
) -> Result<(), String> {
    if !host_firewall_available(state) {
        return Ok(());
    }
    if run_type != 0 && !normalize_auto_manage_firewall(config.get("auto_manage_firewall")) {
        return Ok(());
    }
    if run_type == 1 {
        clear_legacy_gateway_redirects(state, gateway_port(), false).await?;
        log_go_value_result(
            state
                .go_backend
                .clean_iptables()
                .await
                .map_err(|error| error.to_string()),
            "clean iptables",
        );
        return Ok(());
    }
    let payload = json!({
        "chain_name": "FN-KNOCK-FW",
        "parent_chain": ["INPUT", "DOCKER-USER"],
        "exempt_ports": exempt_ports(config, protocol_mapping_enabled, run_type),
    });
    if run_type == 3 {
        log_go_value_result(
            state
                .go_backend
                .init_iptables(&payload)
                .await
                .map_err(|error| error.to_string()),
            "init default firewall",
        );
        clear_legacy_gateway_redirects(state, gateway_port(), false).await?;
        return Ok(());
    }

    clear_legacy_gateway_redirects(state, gateway_port(), false).await?;
    log_go_value_result(
        state
            .go_backend
            .init_iptables(&payload)
            .await
            .map_err(|error| error.to_string()),
        "init default firewall",
    );
    sync_active_whitelist_targets(state, false).await?;
    Ok(())
}

async fn rollback_config_protocol_feature_and_runtime(
    state: &AppState,
    previous_config: &Value,
    previous_protocol_mapping_feature: &Value,
    run_type: i64,
) {
    if let Err(error) = state.redis.save_config(previous_config).await {
        tracing::warn!(%error, "failed to rollback runtime config");
        return;
    }
    if let Err(error) =
        save_protocol_mapping_feature(state, previous_protocol_mapping_feature).await
    {
        tracing::warn!(%error, "failed to rollback protocol mapping feature");
        return;
    }
    if let Err(error) = sync_smart_connect(state, previous_config).await {
        tracing::warn!(%error, "failed to rollback smart connect runtime");
        return;
    }
    if let Err(error) = apply_run_type_config(state, previous_config, run_type).await {
        tracing::warn!(%error, "failed to rollback runtime state");
    }
}

async fn reset_firewall_for_run_type(state: &AppState, run_type: i64) -> Result<Value, String> {
    let config = state
        .redis
        .get_config()
        .await
        .map_err(|error| error.to_string())?;
    let protocol_mapping_feature = load_protocol_mapping_feature(state, Some(&config))
        .await
        .map_err(|error| error.to_string())?;
    let protocol_mapping_enabled = run_type == 3
        && protocol_mapping_feature
            .get("enabled")
            .and_then(Value::as_bool)
            == Some(true);
    clear_legacy_gateway_redirects(state, gateway_port(), true).await?;
    ensure_go_success(
        state
            .go_backend
            .clean_iptables()
            .await
            .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;

    if run_type != 1 {
        let payload = json!({
            "chain_name": "FN-KNOCK-FW",
            "parent_chain": ["INPUT", "DOCKER-USER"],
            "exempt_ports": exempt_ports(&config, protocol_mapping_enabled, run_type),
        });
        ensure_go_success(
            state
                .go_backend
                .init_iptables(&payload)
                .await
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
    }

    let whitelist_synced = if run_type == 0 {
        sync_active_whitelist_targets(state, true).await?
    } else {
        0
    };

    Ok(json!({
        "runType": run_type,
        "gatewayPort": gateway_port(),
        "exemptPorts": if run_type == 1 {
            Vec::<String>::new()
        } else {
            exempt_ports(&config, protocol_mapping_enabled, run_type)
        },
        "whitelistSynced": whitelist_synced,
    }))
}

async fn clear_legacy_gateway_redirects(
    state: &AppState,
    target_port: i64,
    strict: bool,
) -> Result<(), String> {
    let translator = Translator::from_state(state).await;
    for listen_port in LEGACY_REDIRECTED_HTTP_PORTS {
        let fallback = firewall_text_params(
            &translator,
            "clearLegacyTcpRedirectFailed",
            &[
                ("listenPort", listen_port.to_string()),
                ("targetPort", target_port.to_string()),
            ],
        );
        let result = match state
            .go_backend
            .clear_tcp_redirect(listen_port, target_port)
            .await
        {
            Ok((status, value)) => {
                ensure_go_success_with_acceptable_codes(status, value, &[404], &fallback)
            }
            Err(_) => Err(fallback),
        };
        if strict {
            result?;
        } else if let Err(error) = result {
            tracing::warn!(
                %error,
                listen_port,
                target_port,
                "failed to clear legacy TCP redirect"
            );
        }
    }
    Ok(())
}

async fn sync_active_whitelist_targets(state: &AppState, strict: bool) -> Result<usize, String> {
    let targets = state
        .redis
        .list_whitelist_active_concrete_targets()
        .await
        .map_err(|error| error.to_string())?;
    let mut concrete_targets = Vec::new();
    for target in targets {
        let value = target.target.trim();
        if !value.is_empty() {
            concrete_targets.push(value.to_string());
        }
    }

    let translator = Translator::from_state(state).await;
    for target in &concrete_targets {
        let fallback = firewall_text_params(
            &translator,
            "syncWhitelistTargetFailed",
            &[("target", target.to_string())],
        );
        let result = match state.go_backend.allow_ip(target).await {
            Ok(value) => ensure_go_success(value).map_err(|_| fallback.clone()),
            Err(_) => Err(fallback),
        };
        if strict {
            result?;
        } else if let Err(error) = result {
            tracing::warn!(%error, %target, "failed to sync whitelist target to Go backend");
        }
    }

    Ok(concrete_targets.len())
}

fn ensure_go_success_with_acceptable_codes(
    status: reqwest::StatusCode,
    value: Value,
    acceptable_codes: &[u16],
    fallback: &str,
) -> Result<(), String> {
    let code = if status.is_success() {
        go_response_code(&value).unwrap_or_else(|| status.as_u16())
    } else {
        status.as_u16()
    };
    if acceptable_codes.contains(&code) {
        return Ok(());
    }
    if status.is_success()
        && value
            .get("success")
            .and_then(Value::as_bool)
            .unwrap_or(true)
    {
        return Ok(());
    }
    Err(go_response_message(&value, fallback))
}

fn go_response_code(value: &Value) -> Option<u16> {
    value
        .get("code")
        .and_then(Value::as_u64)
        .and_then(|code| u16::try_from(code).ok())
}

fn firewall_reset_success_message(translator: &Translator, data: &Value, run_type: i64) -> String {
    let whitelist_message = if run_type == 0 {
        admin_text_params(
            translator,
            "firewall.whitelistSynced",
            &[(
                "count",
                data.get("whitelistSynced")
                    .and_then(Value::as_i64)
                    .unwrap_or(0)
                    .to_string(),
            )],
        )
    } else {
        String::new()
    };
    let exempt_ports_message = if run_type == 0 || run_type == 3 {
        let ports = data
            .get("exemptPorts")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        admin_text_params(translator, "firewall.exemptPorts", &[("ports", ports)])
    } else {
        String::new()
    };
    admin_text_params(
        translator,
        "firewall.resetSuccess",
        &[
            ("runType", admin_run_type_label(translator, run_type)),
            ("whitelistMessage", whitelist_message),
            ("exemptPortsMessage", exempt_ports_message),
        ],
    )
}

fn admin_run_type_label(translator: &Translator, run_type: i64) -> String {
    match run_type {
        0 => admin_text(translator, "runTypes.direct"),
        1 => admin_text(translator, "runTypes.reverseProxy"),
        3 => admin_text(translator, "runTypes.subdomain"),
        _ => run_type.to_string(),
    }
}

fn gateway_port() -> i64 {
    gateway_port_from_env(std::env::var("GO_REPROXY_PORT").ok())
}

fn gateway_port_from_env(value: Option<String>) -> i64 {
    let raw = value
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "7999".to_string());
    parse_i64_prefix(raw.trim_start())
        .filter(|port| *port > 0)
        .unwrap_or(7999)
}

fn exempt_ports(config: &Value, protocol_mapping_enabled: bool, run_type: i64) -> Vec<String> {
    let mut ports = BTreeSet::new();
    ports.insert(gateway_port().to_string());
    if run_type == 3 && protocol_mapping_enabled {
        if let Some(mappings) = config.get("stream_mappings").and_then(Value::as_array) {
            for mapping in mappings {
                if let Some(port) = mapping.get("listen_port").and_then(Value::as_i64)
                    && (1..=65535).contains(&port)
                {
                    ports.insert(port.to_string());
                }
            }
        }
    }
    let smart = normalize_smart_connect_config(config.get("smart_connect"));
    if run_type == 3
        && smart.get("enabled").and_then(Value::as_bool) == Some(true)
        && smart
            .get("selected_ipv4")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty())
    {
        ports.insert(SMART_CONNECT_DNS_PORT.to_string());
    }
    ports.into_iter().collect()
}

async fn load_smart_connect_details(state: &AppState) -> anyhow::Result<Value> {
    let translator = Translator::from_state(state).await;
    let config = state.redis.get_config().await?;
    let runtime = state
        .redis
        .get_json_value(SMART_CONNECT_RUNTIME_KEY)
        .await?
        .map(|value| normalize_smart_connect_runtime(Some(&value)))
        .unwrap_or_else(default_smart_connect_runtime);
    Ok(build_smart_connect_details(
        state,
        &config,
        runtime,
        &translator,
    ))
}

async fn sync_smart_connect(state: &AppState, config: &Value) -> Result<Value, String> {
    let translator = Translator::from_state(state).await;
    let smart = normalize_smart_connect_config(config.get("smart_connect"));
    let domains = list_smart_connect_domains(config);
    let available =
        host_firewall_available(state) && config.get("run_type").and_then(Value::as_i64) == Some(3);
    let enabled = smart.get("enabled").and_then(Value::as_bool) == Some(true);
    let selected_ipv4 = smart
        .get("selected_ipv4")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let now = time_utils::now_iso();

    let runtime_result = async {
        if !available || !enabled {
            clear_smart_connect_managed_config(&translator)?;
            return Ok(json!({
                "selected_ipv4": selected_ipv4,
                "synced_domains": [],
                "managed_rule_count": 0,
                "last_sync_at": now,
                "last_sync_error": Value::Null,
            }));
        }
        if selected_ipv4.is_empty() {
            return Err(smart_connect_text(&translator, "selectLocalIp"));
        }
        if !is_private_ipv4(&selected_ipv4) {
            return Err(smart_connect_text(&translator, "selectValidLocalIpv4"));
        }
        let dnsmasq = system_assets::build_dnsmasq_status_with_translator(&translator);
        if dnsmasq.get("installed").and_then(Value::as_bool) != Some(true) {
            return Err(smart_connect_text(&translator, "dnsmasqNotInstalled"));
        }
        if dnsmasq.get("initialized").and_then(Value::as_bool) != Some(true) {
            return Err(smart_connect_text(&translator, "dnsmasqNotInitialized"));
        }
        apply_smart_connect_managed_config(&selected_ipv4, &domains, &translator)?;
        Ok(json!({
            "selected_ipv4": selected_ipv4,
            "synced_domains": domains,
            "managed_rule_count": domains.len(),
            "last_sync_at": now,
            "last_sync_error": Value::Null,
        }))
    }
    .await;

    let runtime = match runtime_result {
        Ok(runtime) => runtime,
        Err(message) => {
            let runtime = json!({
                "selected_ipv4": selected_ipv4,
                "synced_domains": [],
                "managed_rule_count": 0,
                "last_sync_at": Value::Null,
                "last_sync_error": message,
            });
            let _ = state
                .redis
                .set_json_value(SMART_CONNECT_RUNTIME_KEY, &runtime)
                .await;
            return Err(message);
        }
    };

    state
        .redis
        .set_json_value(SMART_CONNECT_RUNTIME_KEY, &runtime)
        .await
        .map_err(|error| error.to_string())?;
    Ok(build_smart_connect_details(
        state,
        config,
        runtime,
        &translator,
    ))
}

pub(crate) fn schedule_smart_connect_sync_after_host_mappings_change(
    state: AppState,
    config: Value,
) {
    let smart = normalize_smart_connect_config(config.get("smart_connect"));
    if config.get("run_type").and_then(Value::as_i64) != Some(3)
        || smart.get("enabled").and_then(Value::as_bool) != Some(true)
    {
        return;
    }

    tokio::spawn(async move {
        let latest_config = match state.redis.get_config().await {
            Ok(config) => config,
            Err(error) => {
                tracing::warn!(
                    %error,
                    "failed to load config for smart connect background sync after host mappings change"
                );
                config
            }
        };
        if let Err(message) = sync_smart_connect(&state, &latest_config).await {
            tracing::warn!(
                %message,
                "failed to sync smart connect after host mappings change"
            );
        }
    });
}

fn build_smart_connect_details(
    state: &AppState,
    config: &Value,
    runtime: Value,
    translator: &Translator,
) -> Value {
    let smart = normalize_smart_connect_config(config.get("smart_connect"));
    let available =
        host_runtime_available(state) && config.get("run_type").and_then(Value::as_i64) == Some(3);
    let reason = if available {
        String::new()
    } else if !host_runtime_available(state) {
        capability_blocked_text(state, "smart_connect_available", translator)
    } else {
        let mode = smart_connect_run_type_label(
            translator,
            config.get("run_type").and_then(Value::as_i64).unwrap_or(3),
        );
        smart_connect_text_params(translator, "unavailableReason", &[("mode", mode)])
    };
    json!({
        "config": smart,
        "availability": {
            "available": available,
            "reason": reason,
        },
        "dnsmasq": merge_dnsmasq_runtime(
            system_assets::build_dnsmasq_status_with_translator(translator),
            runtime
        ),
        "domains": list_smart_connect_domains(config),
        "local_ip_options": list_private_ipv4_candidates(),
    })
}

fn merge_dnsmasq_runtime(mut status: Value, runtime: Value) -> Value {
    if let Some(object) = status.as_object_mut() {
        object.insert("runtime".to_string(), runtime);
    }
    status
}

fn smart_connect_run_type_label(translator: &Translator, run_type: i64) -> String {
    match run_type {
        0 => smart_connect_text(translator, "runTypes.direct"),
        1 => smart_connect_text(translator, "runTypes.reverseProxy"),
        3 => smart_connect_text(translator, "runTypes.subdomain"),
        _ => smart_connect_text(translator, "currentMode"),
    }
}

fn list_smart_connect_domains(config: &Value) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut auth_hosts = Vec::new();
    let mut app_hosts = Vec::new();
    if let Some(mappings) = config.get("host_mappings").and_then(Value::as_array) {
        for mapping in mappings {
            let host = normalize_host(mapping.get("host").and_then(Value::as_str).unwrap_or(""));
            if host.is_empty() || !seen.insert(host.clone()) {
                continue;
            }
            if mapping.get("service_role").and_then(Value::as_str) == Some("auth") {
                auth_hosts.push(host);
            } else {
                app_hosts.push(host);
            }
        }
    }
    auth_hosts.extend(app_hosts);
    auth_hosts
}

fn normalize_host(value: &str) -> String {
    let lower = value.trim().to_lowercase();
    let without_scheme = strip_alpha_scheme(&lower);
    without_scheme
        .split('/')
        .next()
        .unwrap_or("")
        .trim_end_matches('.')
        .to_string()
}

fn strip_alpha_scheme(value: &str) -> &str {
    let Some((scheme, rest)) = value.split_once("://") else {
        return value;
    };
    if !scheme.is_empty() && scheme.chars().all(|ch| ch.is_ascii_alphabetic()) {
        rest
    } else {
        value
    }
}

fn list_private_ipv4_candidates() -> Vec<Value> {
    let Ok(items) = get_if_addrs::get_if_addrs() else {
        return Vec::new();
    };
    let mut seen = BTreeSet::new();
    let mut output = Vec::new();
    for item in items {
        if item.is_loopback() || is_excluded_interface(&item.name) {
            continue;
        }
        let get_if_addrs::IfAddr::V4(v4) = item.addr else {
            continue;
        };
        let address = v4.ip.to_string();
        if !is_private_ipv4(&address) || !seen.insert(address.clone()) {
            continue;
        }
        let netmask = v4.netmask.to_string();
        let prefix = ipv4_netmask_to_prefix(v4.netmask);
        output.push(json!({
            "label": format!("{} ({})", address, item.name),
            "value": address,
            "interface": item.name,
            "netmask": netmask,
            "prefix": prefix,
        }));
    }
    output.sort_by(|left, right| {
        let left_key = format!(
            "{}\0{}",
            left.get("interface").and_then(Value::as_str).unwrap_or(""),
            left.get("value").and_then(Value::as_str).unwrap_or("")
        );
        let right_key = format!(
            "{}\0{}",
            right.get("interface").and_then(Value::as_str).unwrap_or(""),
            right.get("value").and_then(Value::as_str).unwrap_or("")
        );
        left_key.cmp(&right_key)
    });
    output
}

fn is_excluded_interface(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower == "lo"
        || lower.starts_with("docker")
        || lower.starts_with("br-")
        || lower.starts_with("veth")
        || lower.starts_with("tailscale")
        || lower.starts_with("zt")
        || lower.starts_with("tun")
        || lower.starts_with("tap")
        || lower.starts_with("wg")
}

fn ipv4_netmask_to_prefix(mask: Ipv4Addr) -> Option<u8> {
    let mask = u32::from(mask);
    let mut prefix = 0;
    let mut seen_zero = false;
    for bit in (0..32).rev() {
        let one = (mask & (1 << bit)) != 0;
        if one && seen_zero {
            return None;
        }
        if one {
            prefix += 1;
        } else {
            seen_zero = true;
        }
    }
    Some(prefix)
}

fn is_private_ipv4(value: &str) -> bool {
    let Ok(ip) = value.parse::<Ipv4Addr>() else {
        return false;
    };
    let [a, b, _, _] = ip.octets();
    a == 10 || (a == 172 && (16..=31).contains(&b)) || (a == 192 && b == 168)
}

fn apply_smart_connect_managed_config(
    selected_ipv4: &str,
    domains: &[String],
    translator: &Translator,
) -> Result<(), String> {
    let content = build_smart_connect_managed_config(selected_ipv4, domains);
    let path = Path::new(SMART_CONNECT_MANAGED_CONF_PATH);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let tmp = format!("{}.tmp", SMART_CONNECT_MANAGED_CONF_PATH);
    fs::write(&tmp, content).map_err(|error| error.to_string())?;
    fs::rename(&tmp, SMART_CONNECT_MANAGED_CONF_PATH).map_err(|error| error.to_string())?;
    restart_dnsmasq_service(translator)
}

fn clear_smart_connect_managed_config(translator: &Translator) -> Result<(), String> {
    if Path::new(SMART_CONNECT_MANAGED_CONF_PATH).exists() {
        fs::remove_file(SMART_CONNECT_MANAGED_CONF_PATH).map_err(|error| error.to_string())?;
        restart_dnsmasq_service(translator)?;
    }
    Ok(())
}

fn build_smart_connect_managed_config(selected_ipv4: &str, domains: &[String]) -> String {
    let normalized_ipv4 = selected_ipv4.trim();
    let mut normalized_domains = Vec::new();
    for domain in domains {
        let domain = domain.trim().to_lowercase();
        if !domain.is_empty() && !normalized_domains.contains(&domain) {
            normalized_domains.push(domain);
        }
    }
    let mut listen_addresses = vec!["127.0.0.1".to_string()];
    if !normalized_ipv4.is_empty() && !listen_addresses.iter().any(|item| item == normalized_ipv4) {
        listen_addresses.push(normalized_ipv4.to_string());
    }
    let mut lines = vec![
        "# Managed by fn-knock smart connect. Do not edit manually.".to_string(),
        format!("local-ttl={SMART_CONNECT_LOCAL_TTL_SECONDS}"),
        format!("listen-address={}", listen_addresses.join(",")),
        "bind-interfaces".to_string(),
    ];
    for domain in normalized_domains {
        lines.push(format!("address=/{domain}/{normalized_ipv4}"));
        lines.push(format!("local=/{domain}/"));
    }
    lines.push(String::new());
    lines.join("\n")
}

fn restart_dnsmasq_service(translator: &Translator) -> Result<(), String> {
    if Command::new("systemctl")
        .args(["restart", "dnsmasq"])
        .status()
        .is_ok_and(|status| status.success())
    {
        return Ok(());
    }
    if Command::new("service")
        .args(["dnsmasq", "restart"])
        .status()
        .is_ok_and(|status| status.success())
    {
        return Ok(());
    }
    Err(smart_connect_text(translator, "syncFailed"))
}

async fn load_fnos_network_tuning_status(state: &AppState) -> anyhow::Result<Value> {
    let config = state.redis.get_config().await?;
    let tuning = normalize_fnos_network_tuning(config.get("fnos_network_tuning"));
    Ok(build_fnos_network_tuning_status(state, tuning))
}

async fn update_fnos_network_tuning_config(
    state: &AppState,
    patch: &Value,
    translator: &Translator,
) -> Result<Value, String> {
    let patch = normalize_fnos_network_tuning_patch(patch, translator)?;
    let previous_config = state
        .redis
        .get_config()
        .await
        .map_err(|error| error.to_string())?;
    let previous = normalize_fnos_network_tuning(previous_config.get("fnos_network_tuning"));
    let before = read_fnos_kernel_state();
    let mut next = build_next_fnos_network_tuning_config(&previous, &patch, &before);
    let result = (|| {
        let transition_targets =
            apply_fnos_network_tuning_transition(&previous, &next, &patch, &before, translator)?;
        let verified_state = read_fnos_kernel_state();
        verify_fnos_network_tuning_state(
            &next,
            &patch,
            &verified_state,
            &transition_targets,
            translator,
        )?;
        Ok::<Value, String>(verified_state)
    })();

    let verified_state = match result {
        Ok(state) => state,
        Err(error) => {
            mark_fnos_network_tuning_failure(
                state,
                &previous_config,
                &previous,
                &before,
                &error,
                translator,
            )
            .await;
            return Err(error);
        }
    };

    clear_fnos_network_tuning_last_error(&mut next);
    let mut config = previous_config.clone();
    ensure_config_object(&mut config).insert("fnos_network_tuning".to_string(), next.clone());
    if let Err(error) = state.redis.save_config(&config).await {
        let message = error.to_string();
        mark_fnos_network_tuning_failure(
            state,
            &previous_config,
            &previous,
            &before,
            &message,
            translator,
        )
        .await;
        return Err(message);
    }
    if let Err(error) = write_fnos_network_tuning_sysctl_config(&next) {
        mark_fnos_network_tuning_failure(
            state,
            &previous_config,
            &previous,
            &before,
            &error,
            translator,
        )
        .await;
        return Err(error);
    }
    let saved_config = state
        .redis
        .get_config()
        .await
        .map_err(|error| error.to_string())?;
    let saved = normalize_fnos_network_tuning(saved_config.get("fnos_network_tuning"));
    Ok(build_fnos_network_tuning_status_with_state(
        state,
        saved,
        verified_state,
    ))
}

fn normalize_fnos_network_tuning_patch(
    patch: &Value,
    translator: &Translator,
) -> Result<Value, String> {
    let mut normalized = serde_json::Map::new();
    if let Some(value) = bool_patch_alias(patch, "bbr_enabled", "bbrEnabled") {
        normalized.insert("bbr_enabled".to_string(), Value::Bool(value));
    }
    if let Some(value) = bool_patch_alias(patch, "mtu_probing_enabled", "mtuProbingEnabled") {
        normalized.insert("mtu_probing_enabled".to_string(), Value::Bool(value));
    }
    if normalized.is_empty() {
        return Err(admin_text(
            translator,
            "fnosNetworkTuning.errors.emptyPatch",
        ));
    }
    Ok(Value::Object(normalized))
}

fn bool_patch_alias(patch: &Value, snake_key: &str, camel_key: &str) -> Option<bool> {
    patch
        .get(snake_key)
        .and_then(Value::as_bool)
        .or_else(|| patch.get(camel_key).and_then(Value::as_bool))
}

fn build_next_fnos_network_tuning_config(previous: &Value, patch: &Value, before: &Value) -> Value {
    let mut next = previous.clone();
    if let Some(object) = next.as_object_mut() {
        if let Some(value) = patch.get("bbr_enabled").and_then(Value::as_bool) {
            if value && previous.get("bbr_enabled").and_then(Value::as_bool) != Some(true) {
                object.insert(
                    "previous_tcp_congestion_control".to_string(),
                    before
                        .get("tcp_congestion_control")
                        .cloned()
                        .unwrap_or(Value::Null),
                );
                object.insert(
                    "previous_default_qdisc".to_string(),
                    before.get("default_qdisc").cloned().unwrap_or(Value::Null),
                );
            }
            object.insert("bbr_enabled".to_string(), Value::Bool(value));
        }
        if let Some(value) = patch.get("mtu_probing_enabled").and_then(Value::as_bool) {
            if value && previous.get("mtu_probing_enabled").and_then(Value::as_bool) != Some(true) {
                object.insert(
                    "previous_tcp_mtu_probing".to_string(),
                    before
                        .get("tcp_mtu_probing")
                        .cloned()
                        .unwrap_or(Value::Null),
                );
            }
            object.insert("mtu_probing_enabled".to_string(), Value::Bool(value));
        }
        object.insert(
            "updated_at".to_string(),
            Value::String(time_utils::now_iso()),
        );
        object.insert("last_error".to_string(), Value::Null);
    }
    normalize_fnos_network_tuning(Some(&next))
}

fn clear_fnos_network_tuning_last_error(config: &mut Value) {
    if let Some(object) = config.as_object_mut() {
        object.insert("last_error".to_string(), Value::Null);
    }
}

fn build_fnos_network_tuning_status(state: &AppState, config: Value) -> Value {
    let kernel_state = read_fnos_kernel_state();
    build_fnos_network_tuning_status_with_state(state, config, kernel_state)
}

fn build_fnos_network_tuning_status_with_state(
    state: &AppState,
    config: Value,
    kernel_state: Value,
) -> Value {
    let blocked_reason_code = fnos_network_tuning_blocked_reason_code(state);
    let available = fnos_network_tuning_available(blocked_reason_code.as_deref());
    let blocked_reason = blocked_reason_code
        .as_deref()
        .map(fnos_network_tuning_blocked_reason_fallback);
    json!({
        "available": available && blocked_reason_code.is_none(),
        "blocked_reason_code": blocked_reason_code.map(Value::String).unwrap_or(Value::Null),
        "blocked_reason": blocked_reason.map(Value::String).unwrap_or(Value::Null),
        "managed_config_path": fnos_network_tuning_sysctl_path().to_string_lossy(),
        "config": config.clone(),
        "state": kernel_state,
        "bbr": {
            "desired_enabled": config.get("bbr_enabled").and_then(Value::as_bool).unwrap_or(false),
            "active": kernel_state.get("bbr_active").and_then(Value::as_bool).unwrap_or(false),
            "supported": kernel_state.get("bbr_supported").and_then(Value::as_bool).unwrap_or(false),
            "module_loaded": kernel_state.get("bbr_module_loaded").and_then(Value::as_bool).unwrap_or(false),
            "current_congestion_control": kernel_state.get("tcp_congestion_control").cloned().unwrap_or(Value::Null),
            "current_default_qdisc": kernel_state.get("default_qdisc").cloned().unwrap_or(Value::Null),
            "available_congestion_control": kernel_state.get("tcp_available_congestion_control").cloned().unwrap_or_else(|| json!([])),
        },
        "mtu_probing": {
            "desired_enabled": config.get("mtu_probing_enabled").and_then(Value::as_bool).unwrap_or(false),
            "active": kernel_state.get("mtu_probing_active").and_then(Value::as_bool).unwrap_or(false),
            "current_value": kernel_state.get("tcp_mtu_probing").cloned().unwrap_or(Value::Null),
        },
        "last_error": config.get("last_error").cloned().unwrap_or(Value::Null),
    })
}

fn localize_fnos_network_tuning_status(mut status: Value, translator: &Translator) -> Value {
    let reason_code = status
        .get("blocked_reason_code")
        .and_then(Value::as_str)
        .map(str::to_string);
    if let (Some(object), Some(reason_code)) = (status.as_object_mut(), reason_code) {
        object.insert(
            "blocked_reason".to_string(),
            Value::String(fnos_network_tuning_blocked_reason(&reason_code, translator)),
        );
    }
    status
}

#[derive(Default)]
struct FnosNetworkTuningTransitionTargets {
    disabled_bbr_congestion_control: Option<String>,
    disabled_bbr_default_qdisc: Option<String>,
    disabled_tcp_mtu_probing: Option<String>,
}

fn verify_fnos_network_tuning_state(
    config: &Value,
    patch: &Value,
    state: &Value,
    targets: &FnosNetworkTuningTransitionTargets,
    translator: &Translator,
) -> Result<(), String> {
    if config.get("bbr_enabled").and_then(Value::as_bool) == Some(true)
        && state.get("bbr_active").and_then(Value::as_bool) != Some(true)
    {
        return Err(admin_text(
            translator,
            "fnosNetworkTuning.errors.bbrEnableVerificationFailed",
        ));
    }
    if patch.get("bbr_enabled").and_then(Value::as_bool) == Some(false) {
        let expected_congestion = targets
            .disabled_bbr_congestion_control
            .clone()
            .or_else(|| config_string(config, "previous_tcp_congestion_control"));
        let expected_qdisc = targets
            .disabled_bbr_default_qdisc
            .clone()
            .or_else(|| config_string(config, "previous_default_qdisc"));
        let current_congestion = state
            .get("tcp_congestion_control")
            .and_then(Value::as_str)
            .unwrap_or("");
        let current_qdisc = state
            .get("default_qdisc")
            .and_then(Value::as_str)
            .unwrap_or("");
        if let Some(expected) = expected_congestion.as_deref()
            && expected != current_congestion
        {
            return Err(admin_text(
                translator,
                "fnosNetworkTuning.errors.bbrRollbackCongestionFailed",
            ));
        }
        if let Some(expected) = expected_qdisc.as_deref()
            && expected != current_qdisc
        {
            return Err(admin_text(
                translator,
                "fnosNetworkTuning.errors.bbrRollbackQdiscFailed",
            ));
        }
        if expected_congestion.is_none() && current_congestion == "bbr" {
            return Err(admin_text(
                translator,
                "fnosNetworkTuning.errors.bbrRollbackStillBbrFailed",
            ));
        }
    }
    if config.get("mtu_probing_enabled").and_then(Value::as_bool) == Some(true)
        && state.get("tcp_mtu_probing").and_then(Value::as_str) != Some("1")
    {
        return Err(admin_text(
            translator,
            "fnosNetworkTuning.errors.mtuEnableVerificationFailed",
        ));
    }
    if patch.get("mtu_probing_enabled").and_then(Value::as_bool) == Some(false) {
        let expected_mtu = targets
            .disabled_tcp_mtu_probing
            .clone()
            .or_else(|| config_string(config, "previous_tcp_mtu_probing"))
            .unwrap_or_else(|| "0".to_string());
        if state.get("tcp_mtu_probing").and_then(Value::as_str) != Some(expected_mtu.as_str()) {
            return Err(admin_text(
                translator,
                "fnosNetworkTuning.errors.mtuRollbackFailed",
            ));
        }
    }
    Ok(())
}

fn read_fnos_kernel_state() -> Value {
    let congestion = read_sysctl("net.ipv4.tcp_congestion_control");
    let available = read_sysctl("net.ipv4.tcp_available_congestion_control")
        .map(|value| {
            value
                .split_whitespace()
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let qdisc = read_sysctl("net.core.default_qdisc");
    let mtu = read_sysctl("net.ipv4.tcp_mtu_probing");
    let bbr_module_loaded = read_bbr_module_loaded();
    let bbr_supported =
        available.iter().any(|value| value == "bbr") || congestion.as_deref() == Some("bbr");
    let bbr_active = congestion.as_deref() == Some("bbr") && qdisc.as_deref() == Some("fq");
    json!({
        "tcp_congestion_control": congestion.map(Value::String).unwrap_or(Value::Null),
        "tcp_available_congestion_control": available,
        "default_qdisc": qdisc.map(Value::String).unwrap_or(Value::Null),
        "tcp_mtu_probing": mtu.clone().map(Value::String).unwrap_or(Value::Null),
        "bbr_module_loaded": bbr_module_loaded,
        "bbr_supported": bbr_supported,
        "bbr_active": bbr_active,
        "mtu_probing_active": fnos_mtu_probing_active(mtu.as_deref()),
    })
}

fn fnos_mtu_probing_active(value: Option<&str>) -> bool {
    value == Some("1")
}

fn read_bbr_module_loaded() -> bool {
    fs::read_to_string("/proc/modules")
        .is_ok_and(|modules| bbr_module_loaded_from_proc_modules(&modules))
}

fn bbr_module_loaded_from_proc_modules(modules: &str) -> bool {
    modules
        .lines()
        .any(|line| line.split_whitespace().next() == Some("tcp_bbr"))
}

fn read_sysctl(key: &str) -> Option<String> {
    Command::new("sysctl")
        .args(["-n", key])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|value| !value.is_empty())
}

fn apply_fnos_network_tuning_transition(
    previous: &Value,
    next: &Value,
    patch: &Value,
    before_state: &Value,
    translator: &Translator,
) -> Result<FnosNetworkTuningTransitionTargets, String> {
    let mut targets = FnosNetworkTuningTransitionTargets::default();
    if patch.get("bbr_enabled").and_then(Value::as_bool) == Some(true) {
        ensure_bbr_supported(translator)?;
        write_sysctl("net.core.default_qdisc", "fq")?;
        write_sysctl("net.ipv4.tcp_congestion_control", "bbr")?;
    } else if patch.get("bbr_enabled").and_then(Value::as_bool) == Some(false) {
        let fallback = fnos_congestion_fallback(before_state);
        let previous_congestion =
            config_string(next, "previous_tcp_congestion_control").filter(|value| value != "bbr");
        targets.disabled_bbr_congestion_control = Some(write_sysctl_from_candidates(
            "net.ipv4.tcp_congestion_control",
            unique_fnos_network_candidates(vec![previous_congestion, Some(fallback)]),
            translator,
        )?);
        targets.disabled_bbr_default_qdisc = Some(write_sysctl_from_candidates(
            "net.core.default_qdisc",
            unique_fnos_network_candidates(vec![
                config_string(next, "previous_default_qdisc"),
                Some("pfifo_fast".to_string()),
            ]),
            translator,
        )?);
    } else if next.get("bbr_enabled").and_then(Value::as_bool) == Some(true)
        && previous.get("bbr_enabled").and_then(Value::as_bool) != Some(true)
    {
        ensure_bbr_supported(translator)?;
        write_sysctl("net.core.default_qdisc", "fq")?;
        write_sysctl("net.ipv4.tcp_congestion_control", "bbr")?;
    }

    if patch.get("mtu_probing_enabled").and_then(Value::as_bool) == Some(true) {
        write_sysctl("net.ipv4.tcp_mtu_probing", "1")?;
    } else if patch.get("mtu_probing_enabled").and_then(Value::as_bool) == Some(false) {
        targets.disabled_tcp_mtu_probing = Some(write_sysctl_from_candidates(
            "net.ipv4.tcp_mtu_probing",
            unique_fnos_network_candidates(vec![
                config_string(next, "previous_tcp_mtu_probing"),
                Some("0".to_string()),
            ]),
            translator,
        )?);
    }

    Ok(targets)
}

fn ensure_bbr_supported(translator: &Translator) -> Result<(), String> {
    let _ = Command::new("modprobe").arg("tcp_bbr").output();
    let state = read_fnos_kernel_state();
    if state.get("bbr_supported").and_then(Value::as_bool) == Some(true) {
        return Ok(());
    }
    Err(admin_text(
        translator,
        "fnosNetworkTuning.errors.bbrNotSupported",
    ))
}

fn unique_fnos_network_candidates(values: Vec<Option<String>>) -> Vec<String> {
    let mut candidates = Vec::new();
    for value in values {
        let Some(value) = value else {
            continue;
        };
        let trimmed = value.trim();
        if trimmed.is_empty() || candidates.iter().any(|candidate| candidate == trimmed) {
            continue;
        }
        candidates.push(trimmed.to_string());
    }
    candidates
}

fn write_sysctl_from_candidates(
    key: &str,
    candidates: Vec<String>,
    translator: &Translator,
) -> Result<String, String> {
    let mut last_error = None;
    for candidate in candidates {
        match write_sysctl(key, &candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or_else(|| {
        admin_text_params(
            translator,
            "fnosNetworkTuning.errors.setSysctlFailed",
            &[("key", key.to_string())],
        )
    }))
}

fn config_string(config: &Value, key: &str) -> Option<String> {
    config
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn kernel_state_string(state: &Value, key: &str) -> Option<String> {
    state
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn restore_fnos_network_tuning_runtime(
    previous: &Value,
    before_state: &Value,
    translator: &Translator,
) -> Result<(), String> {
    if previous.get("bbr_enabled").and_then(Value::as_bool) == Some(true) {
        ensure_bbr_supported(translator)?;
        write_sysctl("net.core.default_qdisc", "fq")?;
        write_sysctl("net.ipv4.tcp_congestion_control", "bbr")?;
    } else {
        if let Some(congestion) = kernel_state_string(before_state, "tcp_congestion_control") {
            write_sysctl("net.ipv4.tcp_congestion_control", &congestion)?;
        }
        if let Some(qdisc) = kernel_state_string(before_state, "default_qdisc") {
            write_sysctl("net.core.default_qdisc", &qdisc)?;
        }
    }

    if previous.get("mtu_probing_enabled").and_then(Value::as_bool) == Some(true) {
        write_sysctl("net.ipv4.tcp_mtu_probing", "1")?;
    } else if let Some(mtu) = kernel_state_string(before_state, "tcp_mtu_probing") {
        write_sysctl("net.ipv4.tcp_mtu_probing", &mtu)?;
    }

    Ok(())
}

async fn mark_fnos_network_tuning_failure(
    state: &AppState,
    previous_config: &Value,
    previous: &Value,
    before_state: &Value,
    message: &str,
    translator: &Translator,
) {
    let mut message = message.to_string();
    if let Err(error) = write_fnos_network_tuning_sysctl_config(previous)
        .and_then(|_| restore_fnos_network_tuning_runtime(previous, before_state, translator))
    {
        message = admin_text_params(
            translator,
            "fnosNetworkTuning.errors.rollbackFailed",
            &[("message", message), ("error", error)],
        );
    }

    let mut failed = previous.clone();
    if let Some(object) = failed.as_object_mut() {
        object.insert("last_error".to_string(), Value::String(message));
        object.insert(
            "updated_at".to_string(),
            Value::String(time_utils::now_iso()),
        );
    }
    let mut config = previous_config.clone();
    ensure_config_object(&mut config).insert("fnos_network_tuning".to_string(), failed);
    let _ = state.redis.save_config(&config).await;
}

fn fnos_congestion_fallback(state: &Value) -> String {
    state
        .get("tcp_available_congestion_control")
        .and_then(Value::as_array)
        .and_then(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .find(|value| *value == "cubic")
                .or_else(|| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .find(|value| !value.trim().is_empty() && *value != "bbr")
                })
        })
        .unwrap_or("cubic")
        .to_string()
}

fn write_sysctl(key: &str, value: &str) -> Result<(), String> {
    let output = Command::new("sysctl")
        .arg("-w")
        .arg(format!("{key}={value}"))
        .output()
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

fn write_fnos_network_tuning_sysctl_config(config: &Value) -> Result<(), String> {
    let path = fnos_network_tuning_sysctl_path();
    let mut lines = Vec::new();
    if config.get("bbr_enabled").and_then(Value::as_bool) == Some(true) {
        lines.push("net.core.default_qdisc=fq".to_string());
        lines.push("net.ipv4.tcp_congestion_control=bbr".to_string());
    }
    if config.get("mtu_probing_enabled").and_then(Value::as_bool) == Some(true) {
        lines.push("net.ipv4.tcp_mtu_probing=1".to_string());
    }
    if lines.is_empty() {
        let _ = fs::remove_file(&path);
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    lines.insert(
        0,
        "# Managed by fn-knock. Do not edit manually.".to_string(),
    );
    lines.push(String::new());
    let tmp = path.with_extension(format!(
        "{}.tmp",
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or("conf")
    ));
    fs::write(&tmp, lines.join("\n")).map_err(|error| error.to_string())?;
    fs::rename(&tmp, &path).map_err(|error| error.to_string())
}

fn fnos_network_tuning_sysctl_path() -> std::path::PathBuf {
    std::env::var("FN_KNOCK_NETWORK_SYSCTL_PATH")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(FNOS_NETWORK_TUNING_SYSCTL_PATH))
}

fn fnos_network_tuning_blocked_reason_code(state: &AppState) -> Option<String> {
    let profile = runtime_profile::get_runtime_profile(state);
    if profile.deployment_target != "fpk" {
        return Some("deployment".to_string());
    }
    if !profile.is_linux {
        return Some("platform".to_string());
    }
    if !profile.is_root_process {
        return Some("permission".to_string());
    }
    None
}

fn fnos_network_tuning_available(blocked_reason_code: Option<&str>) -> bool {
    blocked_reason_code.is_none()
}

fn fnos_network_tuning_blocked_reason(reason_code: &str, translator: &Translator) -> String {
    translator.t_with_fallback(
        &format!("server.admin.fnosNetworkTuning.blocked.{reason_code}"),
        &fnos_network_tuning_blocked_reason_fallback(reason_code),
    )
}

fn fnos_network_tuning_blocked_reason_fallback(reason_code: &str) -> String {
    match reason_code {
        "deployment" => "飞牛 FPK 网络优化仅支持 FPK 部署。",
        "platform" => "飞牛 FPK 网络优化需要 Linux 宿主环境。",
        "permission" => "飞牛 FPK 网络优化需要 root 权限。",
        _ => "飞牛 FPK 网络优化不可用。",
    }
    .to_string()
}

fn string_array(value: Option<&Value>) -> Vec<Value> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| Value::String(value.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

fn is_reverse_proxy_subdomain_mode(config: &Value) -> bool {
    config.get("run_type").and_then(Value::as_i64) == Some(1)
        && config
            .get("reverse_proxy_submode")
            .and_then(Value::as_str)
            .unwrap_or("path")
            == "subdomain"
}

fn is_any_subdomain_routing_mode(config: &Value) -> bool {
    config.get("run_type").and_then(Value::as_i64) == Some(3)
        || is_reverse_proxy_subdomain_mode(config)
}

fn config_array_len(config: &Value, key: &str) -> usize {
    config
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or_default()
}

fn proxy_protocol_force_payload(value: &Value, fallback: bool) -> Value {
    let force = value
        .pointer("/data/proxy_protocol_force")
        .and_then(Value::as_bool)
        .or_else(|| value.get("proxy_protocol_force").and_then(Value::as_bool))
        .unwrap_or(fallback);
    json!({ "proxy_protocol_force": force })
}

fn go_response_message(value: &Value, fallback: &str) -> String {
    value
        .get("message")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|message| !message.is_empty())
        .unwrap_or(fallback)
        .to_string()
}

fn host_runtime_available(state: &AppState) -> bool {
    runtime_profile::host_runtime_available(state)
}

fn ensure_go_success(value: Value) -> anyhow::Result<()> {
    if value
        .get("success")
        .and_then(Value::as_bool)
        .unwrap_or(true)
    {
        return Ok(());
    }
    anyhow::bail!(
        "{}",
        value
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or(GO_BACKEND_UNSUCCESSFUL_RESPONSE)
    )
}

fn host_firewall_available(state: &AppState) -> bool {
    runtime_profile::host_firewall_available(state)
}

fn deployment_target(state: &AppState) -> String {
    runtime_profile::deployment_target(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redis_json_keys_match_node_feature_section_store() {
        assert_eq!(CAPTCHA_SETTINGS_KEY, "fn_knock:captcha:settings");
        assert_eq!(LEGACY_CAPTCHA_SETTINGS_KEY, "fn_knock:config:captcha");
        assert_eq!(
            PROTOCOL_MAPPING_FEATURE_KEY,
            "fn_knock:protocol-mapping:feature"
        );
    }

    #[test]
    fn builds_proxy_protocol_force_payload_from_go_envelopes() {
        assert_eq!(
            proxy_protocol_force_payload(
                &json!({ "success": true, "data": { "proxy_protocol_force": true } }),
                false
            ),
            json!({ "proxy_protocol_force": true })
        );
        assert_eq!(
            proxy_protocol_force_payload(&json!({ "proxy_protocol_force": false }), true),
            json!({ "proxy_protocol_force": false })
        );
        assert_eq!(
            proxy_protocol_force_payload(&json!({ "success": true }), true),
            json!({ "proxy_protocol_force": true })
        );
    }

    #[test]
    fn normalizes_terminal_feature_like_node() {
        assert_eq!(
            normalize_terminal_feature(Some(&json!({
                "enabled": true,
                "default_cwd": "",
                "max_sessions": 20,
                "idle_timeout_seconds": 30,
                "allow_mobile_toolbar": false
            }))),
            json!({
                "enabled": true,
                "default_cwd": "~",
                "max_sessions": 12,
                "idle_timeout_seconds": 60,
                "resume_backend": "tmux",
                "allow_mobile_toolbar": false,
                "dangerously_run_as_current_user": true,
            })
        );
        assert_eq!(
            normalize_terminal_feature(Some(&json!({
                "default_cwd": "/usr/local/etc/fn-knock/"
            })))
            .get("default_cwd"),
            Some(&json!("~"))
        );
    }

    #[test]
    fn normalizes_gateway_logging_like_node_parse_int_without_upper_cap() {
        assert_eq!(
            normalize_gateway_logging(Some(&json!({
                "enabled": true,
                "max_days": "2x",
            }))),
            json!({ "enabled": true, "max_days": 2 })
        );
        assert_eq!(
            normalize_gateway_logging(Some(&json!({
                "max_days": 3.9,
            })))
            .get("max_days"),
            Some(&json!(3))
        );
        assert_eq!(
            normalize_gateway_logging(Some(&json!({
                "max_days": ["4x"],
            })))
            .get("max_days"),
            Some(&json!(4))
        );
        assert_eq!(
            normalize_gateway_logging(Some(&json!({
                "max_days": 999,
            })))
            .get("max_days"),
            Some(&json!(999))
        );
    }

    #[test]
    fn normalizes_captcha_settings() {
        assert_eq!(
            normalize_captcha_settings(Some(&json!({
                "provider": "turnstile",
                "turnstile": { "site_key": " site ", "secret_key": " secret " }
            }))),
            json!({
                "provider": "turnstile",
                "widget_mode": "normal",
                "pow": {},
                "turnstile": { "site_key": "site", "secret_key": "secret" }
            })
        );
    }

    #[test]
    fn normalizes_fnos_share_bypass_bounds() {
        assert_eq!(
            normalize_fnos_share_bypass(Some(&json!({
                "enabled": true,
                "upstream_timeout_ms": 100,
                "session_ttl_seconds": 99999
            }))),
            json!({
                "enabled": true,
                "upstream_timeout_ms": 500,
                "validation_cache_ttl_seconds": 30,
                "validation_lock_ttl_seconds": 5,
                "session_ttl_seconds": 3600,
            })
        );
    }

    #[test]
    fn normalizes_fnos_port_icon_hijack_like_node() {
        assert_eq!(
            normalize_fnos_port_icon_hijack(Some(&json!({
                "enabled": true,
                "updated_at": " 2026-07-05T01:02:03.000Z "
            }))),
            json!({
                "enabled": true,
                "updated_at": "2026-07-05T01:02:03.000Z"
            })
        );
        assert_eq!(
            normalize_fnos_port_icon_hijack(Some(&json!({
                "enabled": false,
                "updated_at": ""
            }))),
            json!({
                "enabled": false,
                "updated_at": Value::Null
            })
        );
    }

    #[test]
    fn normalizes_auto_manage_firewall_like_node() {
        assert!(normalize_auto_manage_firewall(Some(&json!(true))));
        assert!(!normalize_auto_manage_firewall(Some(&json!(false))));
        assert!(normalize_auto_manage_firewall(Some(&json!("false"))));
        assert!(normalize_auto_manage_firewall(None));
    }

    #[test]
    fn normalizes_runtime_mode_feature_configs_like_node() {
        assert_eq!(normalize_run_type(Some(&json!(0))), Some(0));
        assert_eq!(normalize_run_type(Some(&json!(1))), Some(1));
        assert_eq!(normalize_run_type(Some(&json!(3))), Some(3));
        assert_eq!(normalize_run_type(Some(&json!(2))), None);
        assert_eq!(
            normalize_protocol_mapping_feature(Some(&json!({ "enabled": true }))),
            json!({ "enabled": true })
        );
        assert_eq!(
            normalize_smart_connect_config(Some(&json!({
                "enabled": true,
                "selected_ipv4": " 192.168.1.20 "
            }))),
            json!({ "enabled": true, "selected_ipv4": "192.168.1.20" })
        );
    }

    #[test]
    fn smart_connect_domains_prioritize_auth_and_dedupe_hosts() {
        let config = json!({
            "host_mappings": [
                { "host": "app.example.com", "service_role": "app" },
                { "host": "https://AUTH.example.com/path", "service_role": "auth" },
                { "host": "app.example.com.", "service_role": "app" }
            ]
        });

        assert_eq!(
            list_smart_connect_domains(&config),
            vec![
                "auth.example.com".to_string(),
                "app.example.com".to_string()
            ]
        );
    }

    #[test]
    fn smart_connect_host_normalizer_strips_only_alpha_scheme_like_node() {
        assert_eq!(normalize_host("HTTP://Example.COM./path"), "example.com");
        assert_eq!(normalize_host("1://Example.COM/path"), "1:");
    }

    #[test]
    fn builds_smart_connect_managed_config_like_node() {
        let config = build_smart_connect_managed_config(
            " 192.168.1.20 ",
            &[
                "Beta.Example.com".to_string(),
                "alpha.example.com".to_string(),
                "beta.example.com".to_string(),
            ],
        );

        assert_eq!(
            config,
            [
                "# Managed by fn-knock smart connect. Do not edit manually.",
                "local-ttl=30",
                "listen-address=127.0.0.1,192.168.1.20",
                "bind-interfaces",
                "address=/beta.example.com/192.168.1.20",
                "local=/beta.example.com/",
                "address=/alpha.example.com/192.168.1.20",
                "local=/alpha.example.com/",
                "",
            ]
            .join("\n")
        );
    }

    #[test]
    fn gateway_port_matches_node_parse_int_fallback() {
        assert_eq!(gateway_port_from_env(None), 7999);
        assert_eq!(gateway_port_from_env(Some(String::new())), 7999);
        assert_eq!(gateway_port_from_env(Some("   ".to_string())), 7999);
        assert_eq!(gateway_port_from_env(Some(" 8000x ".to_string())), 8000);
        assert_eq!(gateway_port_from_env(Some("0x10".to_string())), 7999);
    }

    #[test]
    fn firewall_exempt_ports_include_stream_and_smart_connect_ports() {
        let config = json!({
            "smart_connect": { "enabled": true, "selected_ipv4": "192.168.1.20" },
            "stream_mappings": [
                { "listen_port": 2222 },
                { "listen_port": 70000 }
            ]
        });
        let ports = exempt_ports(&config, true, 3);

        assert!(ports.contains(&gateway_port().to_string()));
        assert!(ports.contains(&"2222".to_string()));
        assert!(ports.contains(&"53".to_string()));
        assert!(!ports.contains(&"70000".to_string()));
    }

    #[test]
    fn direct_mode_auth_entry_route_matches_node_payload() {
        assert_eq!(
            auth_entry_route_payload(7997),
            json!([{
                "path": "/auth",
                "target": "http://127.0.0.1:7997",
                "rewrite_html": false,
                "use_auth": false,
                "use_root_mode": false,
                "strip_path": false,
            }])
        );
    }

    #[test]
    fn normalizes_fnos_network_tuning_like_node() {
        assert_eq!(
            normalize_fnos_network_tuning(Some(&json!({
                "bbr_enabled": true,
                "mtu_probing_enabled": true,
                "previous_tcp_congestion_control": " cubic ",
                "previous_default_qdisc": "",
                "previous_tcp_mtu_probing": "0",
                "updated_at": " now ",
                "last_error": ""
            }))),
            json!({
                "bbr_enabled": true,
                "mtu_probing_enabled": true,
                "previous_tcp_congestion_control": "cubic",
                "previous_default_qdisc": Value::Null,
                "previous_tcp_mtu_probing": "0",
                "updated_at": "now",
                "last_error": Value::Null,
            })
        );
    }

    #[test]
    fn normalizes_fnos_network_tuning_from_camel_case_persisted_config() {
        assert_eq!(
            normalize_fnos_network_tuning(Some(&json!({
                "bbrEnabled": true,
                "mtuProbingEnabled": true,
                "previousTcpCongestionControl": " cubic ",
                "previousDefaultQdisc": " fq_codel ",
                "previousTcpMtuProbing": "0",
                "updatedAt": " now ",
                "lastError": " failed "
            }))),
            json!({
                "bbr_enabled": true,
                "mtu_probing_enabled": true,
                "previous_tcp_congestion_control": "cubic",
                "previous_default_qdisc": "fq_codel",
                "previous_tcp_mtu_probing": "0",
                "updated_at": "now",
                "last_error": "failed",
            })
        );
    }

    #[test]
    fn fnos_network_tuning_disable_keeps_previous_runtime_values() {
        let previous = normalize_fnos_network_tuning(Some(&json!({
            "bbr_enabled": true,
            "mtu_probing_enabled": true,
            "previous_tcp_congestion_control": "cubic",
            "previous_default_qdisc": "fq_codel",
            "previous_tcp_mtu_probing": "0",
        })));
        let before = json!({
            "tcp_congestion_control": "bbr",
            "default_qdisc": "fq",
            "tcp_mtu_probing": "1",
        });
        let next = build_next_fnos_network_tuning_config(
            &previous,
            &json!({
                "bbr_enabled": false,
                "mtu_probing_enabled": false,
            }),
            &before,
        );

        assert_eq!(
            next.get("bbr_enabled").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            next.get("mtu_probing_enabled").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            next.get("previous_tcp_congestion_control")
                .and_then(Value::as_str),
            Some("cubic")
        );
        assert_eq!(
            next.get("previous_default_qdisc").and_then(Value::as_str),
            Some("fq_codel")
        );
        assert_eq!(
            next.get("previous_tcp_mtu_probing").and_then(Value::as_str),
            Some("0")
        );
    }

    #[test]
    fn fnos_network_tuning_success_clears_previous_last_error_like_node() {
        let mut next = normalize_fnos_network_tuning(Some(&json!({
            "bbr_enabled": true,
            "mtu_probing_enabled": false,
            "last_error": "previous failure"
        })));

        clear_fnos_network_tuning_last_error(&mut next);

        assert_eq!(next.get("last_error"), Some(&Value::Null));
    }

    #[test]
    fn fnos_network_tuning_mtu_active_semantics_match_node() {
        assert!(fnos_mtu_probing_active(Some("1")));
        assert!(!fnos_mtu_probing_active(Some("0")));
        assert!(!fnos_mtu_probing_active(Some("2")));
        assert!(!fnos_mtu_probing_active(None));
    }

    #[test]
    fn fnos_network_tuning_module_loaded_reads_proc_modules_like_node() {
        assert!(bbr_module_loaded_from_proc_modules(
            "tcp_bbr 20480 0 - Live 0\nveth 32768 0 - Live 0\n"
        ));
        assert!(!bbr_module_loaded_from_proc_modules(
            "tcp_cubic 20480 1 - Live 0\ntcp_bbr_extra 20480 0 - Live 0\n"
        ));
    }

    #[test]
    fn fnos_network_tuning_available_depends_on_runtime_block_only() {
        assert!(fnos_network_tuning_available(None));
        assert!(!fnos_network_tuning_available(Some("deployment")));
        assert!(!fnos_network_tuning_available(Some("platform")));
        assert!(!fnos_network_tuning_available(Some("permission")));
    }

    #[test]
    fn fnos_network_tuning_patch_accepts_camel_case_aliases() {
        let translator = Translator::new("zh-CN");
        assert_eq!(
            normalize_fnos_network_tuning_patch(
                &json!({
                    "bbrEnabled": true,
                    "mtuProbingEnabled": false,
                }),
                &translator,
            )
            .expect("normalized patch"),
            json!({
                "bbr_enabled": true,
                "mtu_probing_enabled": false,
            })
        );
    }

    #[test]
    fn fnos_network_tuning_patch_rejects_empty_payload() {
        let translator = Translator::new("zh-CN");
        assert_eq!(
            normalize_fnos_network_tuning_patch(&json!({ "enabled": true }), &translator)
                .expect_err("empty patch should fail"),
            "请至少修改一个飞牛 FPK 网络优化选项"
        );
    }

    #[test]
    fn localizes_runtime_config_route_and_fnos_network_errors() {
        let zh = Translator::new("zh-CN");
        assert_eq!(
            runtime_config_route_text(&zh, "loadAutoHttpsFailed"),
            "加载自动 HTTPS 配置失败"
        );
        assert_eq!(
            localize_runtime_config_error(&zh, GO_BACKEND_UNSUCCESSFUL_RESPONSE),
            "上游服务不可用"
        );
        assert_eq!(
            admin_text_params(
                &zh,
                "fnosNetworkTuning.errors.setSysctlFailed",
                &[("key", "net.ipv4.tcp_mtu_probing".to_string())],
            ),
            "设置 net.ipv4.tcp_mtu_probing 失败"
        );
    }
}
