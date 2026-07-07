use std::{collections::BTreeSet, net::IpAddr};

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use serde_json::{Map, Value, json};
use url::Url;

use crate::{
    i18n::Translator,
    proxy_config::{build_gateway_auth_config, build_host_rules_payload},
    response, scanner,
    state::AppState,
    time_utils, whitelist,
};

const GATEWAY_VISIBILITY_RUNTIME_KEY: &str = "fn_knock:gateway:visibility:runtime";
const GATEWAY_PROXY_HEADERS_RUNTIME_KEY: &str = "fn_knock:gateway:proxy-headers:runtime";
const GATEWAY_HOST_RESPONSE_RUNTIME_KEY: &str = "fn_knock:gateway:host-response:runtime";
const GATEWAY_PORTAL_TITLE_HOST_RULES_PATCH_FLAG_KEY: &str =
    "fn_knock:patch:gateway-portal-title-host-rules:v1";
const GATEWAY_PORTAL_ICON_HOST_RULES_PATCH_FLAG_KEY: &str =
    "fn_knock:patch:gateway-portal-icon-host-rules:v1";
const GO_BACKEND_UNSUCCESSFUL_RESPONSE: &str = "Go backend returned an unsuccessful response";

fn gateway_route_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.admin.gatewaySettingsRoutes.{key}"))
}

fn gateway_route_text_params(
    translator: &Translator,
    key: &str,
    params: &[(&str, String)],
) -> String {
    translator.t_params(&format!("server.admin.gatewaySettingsRoutes.{key}"), params)
}

fn localize_gateway_route_message(translator: &Translator, message: &str) -> String {
    match message.trim() {
        "Gateway payload must be an object" | "Gateway visibility payload must be an object" => {
            gateway_route_text(translator, "payloadObjectRequired")
        }
        "Failed to load config" => gateway_route_text(translator, "loadConfigFailed"),
        "Failed to load runtime" => gateway_route_text(translator, "loadRuntimeFailed"),
        GO_BACKEND_UNSUCCESSFUL_RESPONSE => {
            translator.t("server.admin.runtimeConfigRoutes.upstreamUnavailable")
        }
        _ => message.to_string(),
    }
}

pub fn gateway_settings_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/admin/config/gateway",
            get(get_gateway).post(update_gateway),
        )
        .route(
            "/api/admin/config/gateway/visibility",
            get(get_gateway_visibility).post(update_gateway_visibility),
        )
        .route(
            "/api/admin/config/gateway/proxy-headers",
            get(get_gateway_proxy_headers).post(update_gateway_proxy_headers),
        )
        .route(
            "/api/admin/config/gateway/host-response",
            get(get_gateway_host_response).post(update_gateway_host_response),
        )
}

pub(crate) async fn sync_gateway_settings_on_boot(state: AppState) {
    let config = match state.redis.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load config for gateway settings boot sync");
            return;
        }
    };

    if let Err(error) = sync_gateway_runtime(&state, &config).await {
        tracing::warn!(%error, "failed to sync gateway base runtime on boot");
    }

    let visibility_runtime = match state
        .redis
        .get_json_value(GATEWAY_VISIBILITY_RUNTIME_KEY)
        .await
    {
        Ok(Some(value)) => value,
        Ok(None) => default_gateway_visibility_runtime(),
        Err(error) => {
            tracing::warn!(%error, "failed to load gateway visibility runtime for boot sync");
            default_gateway_visibility_runtime()
        }
    };
    if let Err(error) = sync_gateway_visibility_runtime(&state, &visibility_runtime).await {
        tracing::warn!(%error, "failed to sync gateway visibility runtime on boot");
    }

    let proxy_headers_runtime = match state
        .redis
        .get_json_value(GATEWAY_PROXY_HEADERS_RUNTIME_KEY)
        .await
    {
        Ok(Some(value)) => value,
        Ok(None) => default_gateway_proxy_headers_runtime(),
        Err(error) => {
            tracing::warn!(%error, "failed to load gateway proxy headers runtime for boot sync");
            default_gateway_proxy_headers_runtime()
        }
    };
    if let Err(error) = sync_gateway_proxy_headers_runtime(&state, &proxy_headers_runtime).await {
        tracing::warn!(%error, "failed to sync gateway proxy headers runtime on boot");
    }

    let host_response_runtime = match state
        .redis
        .get_json_value(GATEWAY_HOST_RESPONSE_RUNTIME_KEY)
        .await
    {
        Ok(Some(value)) => value,
        Ok(None) => default_gateway_host_response_runtime(),
        Err(error) => {
            tracing::warn!(%error, "failed to load gateway host response runtime for boot sync");
            default_gateway_host_response_runtime()
        }
    };
    if let Err(error) =
        sync_gateway_host_response_runtime(&state, &config, &host_response_runtime).await
    {
        tracing::warn!(%error, "failed to sync gateway host response runtime on boot");
    }
}

async fn get_gateway(State(state): State<AppState>) -> Response {
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

async fn update_gateway(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
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

async fn get_gateway_visibility(State(state): State<AppState>) -> Response {
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

async fn update_gateway_visibility(
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

async fn get_gateway_proxy_headers(State(state): State<AppState>) -> Response {
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

async fn update_gateway_proxy_headers(
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

async fn get_gateway_host_response(State(state): State<AppState>) -> Response {
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

async fn update_gateway_host_response(
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

async fn build_gateway_settings_response(state: &AppState) -> anyhow::Result<Value> {
    let config = state.redis.get_config().await?;
    build_gateway_settings_response_from_config(state, config).await
}

async fn build_gateway_settings_response_from_config(
    state: &AppState,
    config: Value,
) -> anyhow::Result<Value> {
    let visibility_runtime = state
        .redis
        .get_json_value(GATEWAY_VISIBILITY_RUNTIME_KEY)
        .await?
        .unwrap_or_else(default_gateway_visibility_runtime);
    let proxy_headers_runtime = state
        .redis
        .get_json_value(GATEWAY_PROXY_HEADERS_RUNTIME_KEY)
        .await?
        .unwrap_or_else(default_gateway_proxy_headers_runtime);
    let host_response_runtime = state
        .redis
        .get_json_value(GATEWAY_HOST_RESPONSE_RUNTIME_KEY)
        .await?
        .unwrap_or_else(default_gateway_host_response_runtime);

    let subdomain = config
        .get("subdomain_mode")
        .cloned()
        .unwrap_or_else(default_subdomain_mode);
    let reverse_proxy_throttle = normalize_reverse_proxy_throttle(
        config
            .get("reverse_proxy_throttle")
            .unwrap_or(&default_reverse_proxy_throttle()),
    );
    let visibility_config = normalize_gateway_visibility(
        config
            .get("gateway_visibility")
            .unwrap_or(&default_gateway_visibility()),
    );
    let proxy_headers_config = normalize_disabled_hosts_config(
        config
            .get("gateway_proxy_headers")
            .unwrap_or(&default_disabled_hosts_config()),
    );
    let host_response_config = normalize_disabled_hosts_config(
        config
            .get("gateway_host_response")
            .unwrap_or(&default_disabled_hosts_config()),
    );
    let proxy_headers_config = sanitize_disabled_hosts_config(&config, &proxy_headers_config);
    let host_response_config = sanitize_disabled_hosts_config(&config, &host_response_config);
    let crawler_blocker = normalize_gateway_crawler_blocker(
        config
            .get("gateway_crawler_blocker")
            .unwrap_or(&default_gateway_crawler_blocker()),
    );
    let portal = normalize_gateway_portal(
        config
            .get("gateway_portal")
            .unwrap_or(&default_gateway_portal()),
    );
    let host_mappings = config
        .get("host_mappings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let visible_hosts = visible_host_mappings(&host_mappings);
    let proxy_header_items =
        build_gateway_proxy_header_items(&visible_hosts, &proxy_headers_config);
    let host_response_items =
        build_gateway_host_response_items(&visible_hosts, &host_response_config);

    Ok(json!({
        "auth_cache_ttl_seconds": subdomain
            .get("auth_cache_ttl_seconds")
            .and_then(number_floor)
            .unwrap_or(1),
        "auth_cache_unauthorized_ttl_seconds": subdomain
            .get("auth_cache_unauthorized_ttl_seconds")
            .and_then(number_floor)
            .unwrap_or(1),
        "reverse_proxy_throttle": reverse_proxy_throttle,
        "visibility": build_gateway_visibility_summary(&visibility_config, &visibility_runtime),
        "proxy_headers": build_gateway_proxy_headers_summary(&proxy_header_items, &proxy_headers_runtime),
        "host_response": build_gateway_host_response_summary(&host_response_items, &host_response_runtime),
        "crawler_blocker": crawler_blocker,
        "portal": portal,
    }))
}

async fn get_gateway_visibility_details(state: &AppState) -> anyhow::Result<Value> {
    let config = state.redis.get_config().await?;
    let visibility_config = normalize_gateway_visibility(
        config
            .get("gateway_visibility")
            .unwrap_or(&default_gateway_visibility()),
    );
    let runtime = state
        .redis
        .get_json_value(GATEWAY_VISIBILITY_RUNTIME_KEY)
        .await?
        .unwrap_or_else(default_gateway_visibility_runtime);
    Ok(json!({
        "config": visibility_config,
        "summary": build_gateway_visibility_summary(&visibility_config, &runtime),
    }))
}

async fn update_gateway_visibility_inner(state: &AppState, body: &Value) -> Result<Value, String> {
    let Some(object) = body.as_object() else {
        return Err("Gateway visibility payload must be an object".to_string());
    };
    let compiled = compile_gateway_visibility_config(state, object).await?;
    let mut next_config = state
        .redis
        .get_config()
        .await
        .map_err(|error| error.to_string())?;
    ensure_object(&mut next_config)
        .insert("gateway_visibility".to_string(), compiled.config.clone());

    state
        .redis
        .save_config(&next_config)
        .await
        .map_err(|error| error.to_string())?;
    state
        .redis
        .set_json_value(GATEWAY_VISIBILITY_RUNTIME_KEY, &compiled.runtime)
        .await
        .map_err(|error| error.to_string())?;
    sync_gateway_visibility_runtime(state, &compiled.runtime).await?;

    Ok(json!({
        "config": compiled.config,
        "summary": build_gateway_visibility_summary(&compiled.config, &compiled.runtime),
    }))
}

async fn get_gateway_proxy_headers_details(
    state: &AppState,
    translator: &Translator,
) -> anyhow::Result<Value> {
    let config = state.redis.get_config().await?;
    let proxy_config = normalize_disabled_hosts_config(
        config
            .get("gateway_proxy_headers")
            .unwrap_or(&default_disabled_hosts_config()),
    );
    let sanitized_config = sanitize_disabled_hosts_config(&config, &proxy_config);
    let host_mappings = config_host_mappings(&config);
    let visible_hosts = visible_host_mappings(&host_mappings);
    let items = build_gateway_proxy_header_items(&visible_hosts, &sanitized_config);
    let runtime = state
        .redis
        .get_json_value(GATEWAY_PROXY_HEADERS_RUNTIME_KEY)
        .await?
        .unwrap_or_else(default_gateway_proxy_headers_runtime);

    Ok(json!({
        "config": sanitized_config,
        "availability": build_proxy_headers_availability(&config, translator),
        "items": items,
        "summary": build_gateway_proxy_headers_summary(&items, &runtime),
    }))
}

async fn update_gateway_proxy_headers_inner(
    state: &AppState,
    previous_config: &Value,
    body: &Value,
) -> Result<Value, String> {
    let requested = disabled_hosts_config_from_body(body)?;
    let compiled = compile_gateway_proxy_headers_state(previous_config, &requested);
    let mut next_config = previous_config.clone();
    ensure_object(&mut next_config)
        .insert("gateway_proxy_headers".to_string(), compiled.config.clone());

    state
        .redis
        .save_config(&next_config)
        .await
        .map_err(|error| error.to_string())?;
    state
        .redis
        .set_json_value(GATEWAY_PROXY_HEADERS_RUNTIME_KEY, &compiled.runtime)
        .await
        .map_err(|error| error.to_string())?;
    sync_gateway_proxy_headers_runtime(state, &compiled.runtime).await?;
    let translator = Translator::from_state(state).await;
    get_gateway_proxy_headers_details(state, &translator)
        .await
        .map_err(|error| error.to_string())
}

async fn get_gateway_host_response_details(
    state: &AppState,
    translator: &Translator,
) -> anyhow::Result<Value> {
    let config = state.redis.get_config().await?;
    let host_response_config = normalize_disabled_hosts_config(
        config
            .get("gateway_host_response")
            .unwrap_or(&default_disabled_hosts_config()),
    );
    let sanitized_config = sanitize_disabled_hosts_config(&config, &host_response_config);
    let host_mappings = config_host_mappings(&config);
    let visible_hosts = visible_host_mappings(&host_mappings);
    let items = build_gateway_host_response_items(&visible_hosts, &sanitized_config);
    let runtime = state
        .redis
        .get_json_value(GATEWAY_HOST_RESPONSE_RUNTIME_KEY)
        .await?
        .unwrap_or_else(default_gateway_host_response_runtime);

    Ok(json!({
        "config": sanitized_config,
        "availability": build_host_response_availability(&config, translator),
        "items": items,
        "summary": build_gateway_host_response_summary(&items, &runtime),
    }))
}

async fn update_gateway_host_response_inner(
    state: &AppState,
    previous_config: &Value,
    body: &Value,
    translator: &Translator,
) -> Result<Value, String> {
    let requested = disabled_hosts_config_from_body(body)?;
    let compiled = compile_gateway_host_response_state(previous_config, &requested);
    let mut next_config = previous_config.clone();
    ensure_object(&mut next_config)
        .insert("gateway_host_response".to_string(), compiled.config.clone());

    state
        .redis
        .save_config(&next_config)
        .await
        .map_err(|error| error.to_string())?;
    state
        .redis
        .set_json_value(GATEWAY_HOST_RESPONSE_RUNTIME_KEY, &compiled.runtime)
        .await
        .map_err(|error| error.to_string())?;
    sync_gateway_host_response_runtime(state, previous_config, &compiled.runtime).await?;
    get_gateway_host_response_details(state, translator)
        .await
        .map_err(|error| error.to_string())
}

struct CompiledGatewayVisibility {
    config: Value,
    runtime: Value,
}

struct CompiledGatewayTargetRuntime {
    config: Value,
    runtime: Value,
}

async fn compile_gateway_visibility_config(
    state: &AppState,
    input: &Map<String, Value>,
) -> Result<CompiledGatewayVisibility, String> {
    let translator = Translator::from_state(state).await;
    let enabled = input
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let selections = dedupe_visibility_selection_inputs(input.get("selections"));
    let custom_cidrs =
        validate_gateway_custom_cidrs(string_list(input.get("custom_cidrs")), &translator)?;
    let mut stored_selections = Vec::new();
    let mut resolved_cidrs = Vec::new();

    for selection in selections {
        let lookup = scanner::lookup_cidr_region(
            state,
            &selection.province,
            selection.query_city.as_deref(),
        )
        .await?;
        stored_selections.push(lookup.selection);
        resolved_cidrs.extend(lookup.cidrs);
    }

    let merged_cidrs = normalize_cidr_lines(resolved_cidrs.into_iter().chain(custom_cidrs.clone()));
    if enabled && merged_cidrs.is_empty() {
        return Err(translator.t("server.gatewayVisibility.emptyEnabledConfig"));
    }
    let runtime_cidrs = if enabled { merged_cidrs } else { Vec::new() };

    Ok(CompiledGatewayVisibility {
        config: json!({
            "enabled": enabled,
            "selections": stored_selections,
            "custom_cidrs": custom_cidrs,
        }),
        runtime: json!({
            "enabled": enabled,
            "cidrs": runtime_cidrs,
            "updated_at": time_utils::now_iso(),
        }),
    })
}

#[derive(Debug, PartialEq, Eq)]
struct VisibilitySelectionInput {
    province: String,
    query_city: Option<String>,
}

fn dedupe_visibility_selection_inputs(value: Option<&Value>) -> Vec<VisibilitySelectionInput> {
    let mut result = Vec::new();
    let mut seen = BTreeSet::new();
    let Some(items) = value.and_then(Value::as_array) else {
        return result;
    };
    for item in items {
        let province = item
            .get("province")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        if province.is_empty() {
            continue;
        }
        let query_city = item
            .get("query_city")
            .or_else(|| item.get("queryCity"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string);
        let key = format!("{}::{}", province, query_city.as_deref().unwrap_or(""));
        if seen.insert(key) {
            result.push(VisibilitySelectionInput {
                province,
                query_city,
            });
        }
    }
    result
}

fn validate_gateway_custom_cidrs(
    values: Vec<Value>,
    translator: &Translator,
) -> Result<Vec<String>, String> {
    let cidrs = normalize_cidr_lines(values.into_iter().filter_map(|value| {
        value.as_str().map(|value| value.to_string()).or_else(|| {
            if value.is_null() {
                None
            } else {
                Some(value.to_string())
            }
        })
    }));
    let invalid = cidrs
        .iter()
        .filter(|cidr| !is_valid_cidr(cidr))
        .cloned()
        .collect::<Vec<_>>();
    if invalid.is_empty() {
        Ok(cidrs)
    } else {
        Err(translator.t_params(
            "server.gatewayVisibility.customCidrInvalid",
            &[("cidrs", invalid.join(", "))],
        ))
    }
}

fn normalize_cidr_lines(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for value in values {
        let cidr = value.trim();
        if cidr.is_empty() {
            continue;
        }
        if seen.insert(cidr.to_ascii_lowercase()) {
            result.push(cidr.to_string());
        }
    }
    result
}

fn is_valid_cidr(value: &str) -> bool {
    let normalized = value.trim();
    let Some((address, prefix_raw)) = normalized.split_once('/') else {
        return false;
    };
    if address.trim().is_empty()
        || prefix_raw.trim().is_empty()
        || prefix_raw.trim().chars().any(|ch| !ch.is_ascii_digit())
    {
        return false;
    }
    let Ok(prefix) = prefix_raw.trim().parse::<u16>() else {
        return false;
    };
    match address.trim().parse::<IpAddr>() {
        Ok(IpAddr::V4(_)) => prefix <= 32,
        Ok(IpAddr::V6(_)) => prefix <= 128,
        Err(_) => false,
    }
}

fn compile_gateway_proxy_headers_state(
    config: &Value,
    requested: &Value,
) -> CompiledGatewayTargetRuntime {
    let next_config = sanitize_disabled_hosts_config(config, requested);
    let host_mappings = config_host_mappings(config);
    let visible_hosts = visible_host_mappings(&host_mappings);
    let items = build_gateway_proxy_header_items(&visible_hosts, &next_config);
    let omit_targets = omitted_targets(&items, "send_proxy_headers");
    let enabled = is_any_subdomain_routing_mode(config);

    CompiledGatewayTargetRuntime {
        config: next_config,
        runtime: json!({
            "enabled": enabled,
            "omit_targets": if enabled { omit_targets } else { Vec::<String>::new() },
            "updated_at": time_utils::now_iso(),
        }),
    }
}

fn compile_gateway_host_response_state(
    config: &Value,
    requested: &Value,
) -> CompiledGatewayTargetRuntime {
    let next_config = sanitize_disabled_hosts_config(config, requested);
    let host_mappings = config_host_mappings(config);
    let visible_hosts = visible_host_mappings(&host_mappings);
    let items = build_gateway_host_response_items(&visible_hosts, &next_config);
    let omit_targets = omitted_targets(&items, "preserve_host");
    let enabled = is_any_subdomain_routing_mode(config);

    CompiledGatewayTargetRuntime {
        config: next_config,
        runtime: json!({
            "enabled": enabled,
            "omit_targets": if enabled { omit_targets } else { Vec::<String>::new() },
            "updated_at": time_utils::now_iso(),
        }),
    }
}

fn omitted_targets(items: &[Value], enabled_field: &str) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut targets = Vec::new();
    for item in items {
        if item.get(enabled_field).and_then(Value::as_bool) != Some(false) {
            continue;
        }
        let target = item
            .get("target")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        if !target.is_empty() && seen.insert(target.to_string()) {
            targets.push(target.to_string());
        }
    }
    targets
}

fn disabled_hosts_config_from_body(body: &Value) -> Result<Value, String> {
    let Some(object) = body.as_object() else {
        return Err("Gateway payload must be an object".to_string());
    };
    Ok(json!({
        "disabled_hosts": string_list(object.get("disabled_hosts")),
    }))
}

fn build_gateway_visibility_summary(config: &Value, runtime: &Value) -> Value {
    json!({
        "enabled": config.get("enabled").and_then(Value::as_bool).unwrap_or(false),
        "selection_count": config.get("selections").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
        "custom_cidr_count": config.get("custom_cidrs").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
        "cidr_count": runtime.get("cidrs").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
        "updated_at": runtime.get("updated_at").cloned().unwrap_or(Value::Null),
    })
}

fn build_gateway_proxy_headers_summary(items: &[Value], runtime: &Value) -> Value {
    json!({
        "total_count": items.len(),
        "disabled_count": items.iter().filter(|item| item.get("send_proxy_headers").and_then(Value::as_bool) == Some(false)).count(),
        "updated_at": runtime.get("updated_at").cloned().unwrap_or(Value::Null),
    })
}

fn build_gateway_host_response_summary(items: &[Value], runtime: &Value) -> Value {
    json!({
        "total_count": items.len(),
        "disabled_count": items.iter().filter(|item| item.get("preserve_host").and_then(Value::as_bool) == Some(false)).count(),
        "updated_at": runtime.get("updated_at").cloned().unwrap_or(Value::Null),
    })
}

fn build_proxy_headers_availability(config: &Value, translator: &Translator) -> Value {
    if is_any_subdomain_routing_mode(config) {
        return json!({ "available": true, "reason": "" });
    }
    json!({
        "available": false,
        "reason": translator.t_params(
            "server.gatewayProxyHeaders.unavailableReason",
            &[("mode", run_type_label(translator, config, "server.gatewayProxyHeaders.runTypes"))],
        ),
    })
}

fn build_host_response_availability(config: &Value, translator: &Translator) -> Value {
    if is_any_subdomain_routing_mode(config) {
        return json!({ "available": true, "reason": "" });
    }
    json!({
        "available": false,
        "reason": translator.t_params(
            "server.gatewayHostResponse.unavailableReason",
            &[("mode", run_type_label(translator, config, "server.gatewayHostResponse.runTypes"))],
        ),
    })
}

fn run_type_label(translator: &Translator, config: &Value, prefix: &str) -> String {
    match config.get("run_type").and_then(Value::as_i64).unwrap_or(3) {
        0 => translator.t(&format!("{prefix}.direct")),
        1 => translator.t(&format!("{prefix}.reverseProxy")),
        _ => translator.t(&format!("{prefix}.subdomain")),
    }
}

fn config_host_mappings(config: &Value) -> Vec<Value> {
    config
        .get("host_mappings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn sanitize_disabled_hosts_config(config: &Value, raw_config: &Value) -> Value {
    let visible_hosts = visible_host_mappings(&config_host_mappings(config))
        .iter()
        .filter_map(|mapping| mapping.get("host").and_then(Value::as_str))
        .map(normalize_host)
        .filter(|host| !host.is_empty())
        .collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    let disabled_hosts = raw_config
        .get("disabled_hosts")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(normalize_host)
                .filter(|host| {
                    !host.is_empty() && visible_hosts.contains(host) && seen.insert(host.clone())
                })
                .map(Value::String)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    json!({ "disabled_hosts": disabled_hosts })
}

fn is_any_subdomain_routing_mode(config: &Value) -> bool {
    config.get("run_type").and_then(Value::as_i64) == Some(3)
        || (config.get("run_type").and_then(Value::as_i64) == Some(1)
            && config
                .get("reverse_proxy_submode")
                .and_then(Value::as_str)
                .unwrap_or("path")
                == "subdomain")
}

async fn sync_gateway_visibility_runtime(state: &AppState, runtime: &Value) -> Result<(), String> {
    ensure_go_success(
        state
            .go_backend
            .set_gateway_visibility(&visibility_sync_payload(runtime))
            .await
            .map_err(|error| error.to_string())?,
    )
}

pub(crate) async fn sync_gateway_visibility_runtime_from_store(
    state: &AppState,
) -> Result<(), String> {
    let runtime = state
        .redis
        .get_json_value(GATEWAY_VISIBILITY_RUNTIME_KEY)
        .await
        .map_err(|error| error.to_string())?
        .unwrap_or_else(default_gateway_visibility_runtime);
    sync_gateway_visibility_runtime(state, &runtime).await
}

async fn sync_gateway_proxy_headers_runtime(
    state: &AppState,
    runtime: &Value,
) -> Result<(), String> {
    ensure_go_success(
        state
            .go_backend
            .set_forwarded_headers_config(&omit_targets_sync_payload(runtime))
            .await
            .map_err(|error| error.to_string())?,
    )
}

async fn sync_gateway_host_response_runtime(
    state: &AppState,
    config: &Value,
    runtime: &Value,
) -> Result<(), String> {
    ensure_go_success(
        state
            .go_backend
            .set_preserve_host_config(&omit_targets_sync_payload(runtime))
            .await
            .map_err(|error| error.to_string())?,
    )?;
    if is_any_subdomain_routing_mode(config) {
        let host_mappings = config_host_mappings(config);
        ensure_go_success(
            state
                .go_backend
                .set_host_rules(&build_host_rules_payload(&host_mappings))
                .await
                .map_err(|error| error.to_string())?,
        )?;
    }
    Ok(())
}

pub(crate) async fn sync_gateway_target_runtime_for_config(
    state: &AppState,
    config: &Value,
    save_config: bool,
) -> Result<(), String> {
    let proxy_source = config
        .get("gateway_proxy_headers")
        .cloned()
        .unwrap_or_else(default_disabled_hosts_config);
    let proxy_requested = normalize_disabled_hosts_config(&proxy_source);
    let proxy_compiled = compile_gateway_proxy_headers_state(config, &proxy_requested);

    let mut effective_config = config.clone();
    if save_config {
        ensure_object(&mut effective_config).insert(
            "gateway_proxy_headers".to_string(),
            proxy_compiled.config.clone(),
        );
    }

    let host_response_source = effective_config
        .get("gateway_host_response")
        .cloned()
        .unwrap_or_else(default_disabled_hosts_config);
    let host_response_requested = normalize_disabled_hosts_config(&host_response_source);
    let host_response_compiled =
        compile_gateway_host_response_state(&effective_config, &host_response_requested);

    if save_config {
        ensure_object(&mut effective_config).insert(
            "gateway_host_response".to_string(),
            host_response_compiled.config.clone(),
        );
        state
            .redis
            .save_config(&effective_config)
            .await
            .map_err(|error| error.to_string())?;
    }

    state
        .redis
        .set_json_value(GATEWAY_PROXY_HEADERS_RUNTIME_KEY, &proxy_compiled.runtime)
        .await
        .map_err(|error| error.to_string())?;
    sync_gateway_proxy_headers_runtime(state, &proxy_compiled.runtime).await?;

    state
        .redis
        .set_json_value(
            GATEWAY_HOST_RESPONSE_RUNTIME_KEY,
            &host_response_compiled.runtime,
        )
        .await
        .map_err(|error| error.to_string())?;
    sync_gateway_host_response_runtime(state, &effective_config, &host_response_compiled.runtime)
        .await
}

fn visibility_sync_payload(runtime: &Value) -> Value {
    let mut payload = Map::new();
    payload.insert(
        "enabled".to_string(),
        Value::Bool(
            runtime
                .get("enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        ),
    );
    payload.insert(
        "cidrs".to_string(),
        runtime
            .get("cidrs")
            .and_then(Value::as_array)
            .cloned()
            .map(Value::Array)
            .unwrap_or_else(|| Value::Array(Vec::new())),
    );
    if let Some(updated_at) = runtime.get("updated_at").and_then(Value::as_str) {
        payload.insert(
            "updated_at".to_string(),
            Value::String(updated_at.to_string()),
        );
    }
    Value::Object(payload)
}

fn omit_targets_sync_payload(runtime: &Value) -> Value {
    let mut payload = Map::new();
    payload.insert(
        "enabled".to_string(),
        Value::Bool(
            runtime
                .get("enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        ),
    );
    payload.insert(
        "omit_targets".to_string(),
        runtime
            .get("omit_targets")
            .and_then(Value::as_array)
            .cloned()
            .map(Value::Array)
            .unwrap_or_else(|| Value::Array(Vec::new())),
    );
    if let Some(updated_at) = runtime.get("updated_at").and_then(Value::as_str) {
        payload.insert(
            "updated_at".to_string(),
            Value::String(updated_at.to_string()),
        );
    }
    Value::Object(payload)
}

async fn rollback_gateway_visibility(
    state: &AppState,
    previous_config: &Value,
    previous_runtime: &Value,
    translator: &Translator,
) -> Option<String> {
    if let Err(error) = state.redis.save_config(previous_config).await {
        tracing::warn!(%error, "failed to rollback gateway visibility config");
        return Some(translator.t("server.admin.rollback.restoreVisibilityConfigFailed"));
    }
    if let Err(error) = state
        .redis
        .set_json_value(GATEWAY_VISIBILITY_RUNTIME_KEY, previous_runtime)
        .await
    {
        tracing::warn!(%error, "failed to rollback gateway visibility runtime");
        return Some(translator.t("server.admin.rollback.restoreVisibilityRuntimeFailed"));
    }
    if let Err(error) = sync_gateway_visibility_runtime(state, previous_runtime).await {
        tracing::warn!(%error, "failed to rollback gateway visibility runtime on go gateway");
        return Some(translator.t("server.admin.rollback.restoreGatewayVisibilityFailed"));
    }
    None
}

async fn rollback_gateway_proxy_headers(
    state: &AppState,
    previous_config: &Value,
    previous_runtime: &Value,
    translator: &Translator,
) -> Option<String> {
    if let Err(error) = state.redis.save_config(previous_config).await {
        tracing::warn!(%error, "failed to rollback gateway proxy headers config");
        return Some(translator.t("server.admin.rollback.restoreProxyHeadersConfigFailed"));
    }
    if let Err(error) = state
        .redis
        .set_json_value(GATEWAY_PROXY_HEADERS_RUNTIME_KEY, previous_runtime)
        .await
    {
        tracing::warn!(%error, "failed to rollback gateway proxy headers runtime");
        return Some(translator.t("server.admin.rollback.restoreProxyHeadersRuntimeFailed"));
    }
    if let Err(error) = sync_gateway_proxy_headers_runtime(state, previous_runtime).await {
        tracing::warn!(%error, "failed to rollback gateway proxy headers runtime on go gateway");
        return Some(translator.t("server.admin.rollback.restoreGatewayProxyHeadersRuntimeFailed"));
    }
    None
}

async fn rollback_gateway_host_response(
    state: &AppState,
    previous_config: &Value,
    previous_runtime: &Value,
    translator: &Translator,
) -> Option<String> {
    if let Err(error) = state.redis.save_config(previous_config).await {
        tracing::warn!(%error, "failed to rollback gateway host response config");
        return Some(translator.t("server.gatewayHostResponse.restoreConfigFailed"));
    }
    if let Err(error) = state
        .redis
        .set_json_value(GATEWAY_HOST_RESPONSE_RUNTIME_KEY, previous_runtime)
        .await
    {
        tracing::warn!(%error, "failed to rollback gateway host response runtime");
        return Some(translator.t("server.gatewayHostResponse.restoreRuntimeFailed"));
    }
    if let Err(error) =
        sync_gateway_host_response_runtime(state, previous_config, previous_runtime).await
    {
        tracing::warn!(%error, "failed to rollback gateway host response runtime on go gateway");
        return Some(translator.t("server.gatewayHostResponse.restoreGatewayRuntimeFailed"));
    }
    None
}

fn rollback_message(
    translator: &Translator,
    message: &str,
    rollback_error: Option<&str>,
    rolled_back_key: &str,
) -> String {
    if let Some(rollback_error) = rollback_error {
        return translator.t_params(
            "server.admin.rollback.failed",
            &[
                (
                    "message",
                    non_empty_message(message, rolled_back_key, translator),
                ),
                ("rollbackError", rollback_error.to_string()),
            ],
        );
    }
    if message.trim().is_empty() {
        translator.t(rolled_back_key)
    } else {
        localize_gateway_route_message(translator, message)
    }
}

fn non_empty_message(message: &str, fallback_key: &str, translator: &Translator) -> String {
    if message.trim().is_empty() {
        translator.t(fallback_key)
    } else {
        localize_gateway_route_message(translator, message)
    }
}

fn apply_gateway_patch(config: &mut Value, patch: &Map<String, Value>) {
    let object = ensure_object(config);

    if patch.contains_key("auth_cache_ttl_seconds")
        || patch.contains_key("auth_cache_unauthorized_ttl_seconds")
    {
        let mut subdomain = object
            .get("subdomain_mode")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        if let Some(value) = patch.get("auth_cache_ttl_seconds") {
            subdomain.insert(
                "auth_cache_ttl_seconds".to_string(),
                Value::Number(normalize_cache_ttl(Some(value), 1).into()),
            );
        }
        if let Some(value) = patch.get("auth_cache_unauthorized_ttl_seconds") {
            subdomain.insert(
                "auth_cache_unauthorized_ttl_seconds".to_string(),
                Value::Number(normalize_cache_ttl(Some(value), 1).into()),
            );
        }
        object.insert("subdomain_mode".to_string(), Value::Object(subdomain));
    }

    if let Some(value) = patch.get("reverse_proxy_throttle") {
        let previous = object
            .get("reverse_proxy_throttle")
            .cloned()
            .unwrap_or_else(default_reverse_proxy_throttle);
        object.insert(
            "reverse_proxy_throttle".to_string(),
            normalize_reverse_proxy_throttle(&merge_objects(&previous, value)),
        );
    }

    if let Some(value) = patch.get("portal") {
        let previous = object
            .get("gateway_portal")
            .cloned()
            .unwrap_or_else(default_gateway_portal);
        object.insert(
            "gateway_portal".to_string(),
            normalize_gateway_portal(&merge_objects(&previous, value)),
        );
    }

    if let Some(value) = patch.get("crawler_blocker") {
        let previous = object
            .get("gateway_crawler_blocker")
            .cloned()
            .unwrap_or_else(default_gateway_crawler_blocker);
        let mut merged = merge_objects(&previous, value);
        ensure_object(&mut merged).insert(
            "updated_at".to_string(),
            Value::String(time_utils::now_iso()),
        );
        object.insert(
            "gateway_crawler_blocker".to_string(),
            normalize_gateway_crawler_blocker(&merged),
        );
    }
}

async fn sync_gateway_runtime(state: &AppState, config: &Value) -> Result<(), String> {
    ensure_go_success(
        state
            .go_backend
            .set_auth_config(&build_gateway_auth_config(config))
            .await
            .map_err(|error| error.to_string())?,
    )?;
    let throttle = normalize_reverse_proxy_throttle(
        config
            .get("reverse_proxy_throttle")
            .unwrap_or(&default_reverse_proxy_throttle()),
    );
    ensure_go_success(
        state
            .go_backend
            .set_reverse_proxy_throttle(&throttle)
            .await
            .map_err(|error| error.to_string())?,
    )?;
    let crawler = normalize_gateway_crawler_blocker(
        config
            .get("gateway_crawler_blocker")
            .unwrap_or(&default_gateway_crawler_blocker()),
    );
    ensure_go_success(
        state
            .go_backend
            .set_crawler_blocker_config(&crawler)
            .await
            .map_err(|error| error.to_string())?,
    )?;
    let portal = normalize_gateway_portal(
        config
            .get("gateway_portal")
            .unwrap_or(&default_gateway_portal()),
    );
    ensure_go_success(
        state
            .go_backend
            .set_gateway_portal_config(&portal)
            .await
            .map_err(|error| error.to_string())?,
    )
}

async fn apply_gateway_portal_host_rules_patches_if_needed(
    state: &AppState,
    config: &Value,
) -> Result<(), String> {
    apply_gateway_portal_title_host_rules_patch_if_needed(state, config).await?;
    apply_gateway_portal_icon_host_rules_patch_if_needed(state, config).await?;
    Ok(())
}

async fn apply_gateway_portal_title_host_rules_patch_if_needed(
    state: &AppState,
    config: &Value,
) -> Result<bool, String> {
    if !is_gateway_portal_title_mode(config) {
        return Ok(false);
    }
    apply_gateway_portal_host_rules_patch_if_needed(
        state,
        config,
        GATEWAY_PORTAL_TITLE_HOST_RULES_PATCH_FLAG_KEY,
    )
    .await
}

async fn apply_gateway_portal_icon_host_rules_patch_if_needed(
    state: &AppState,
    config: &Value,
) -> Result<bool, String> {
    if !is_gateway_portal_app_icon_mode(config) {
        return Ok(false);
    }
    apply_gateway_portal_host_rules_patch_if_needed(
        state,
        config,
        GATEWAY_PORTAL_ICON_HOST_RULES_PATCH_FLAG_KEY,
    )
    .await
}

async fn apply_gateway_portal_host_rules_patch_if_needed(
    state: &AppState,
    config: &Value,
    flag_key: &str,
) -> Result<bool, String> {
    if !is_any_subdomain_routing_mode(config) {
        return Ok(false);
    }
    if state
        .redis
        .get_string_value(flag_key)
        .await
        .map_err(|error| error.to_string())?
        .as_deref()
        == Some("1")
    {
        return Ok(false);
    }

    let host_mappings = config_host_mappings(config);
    ensure_go_success(
        state
            .go_backend
            .set_host_rules(&build_host_rules_payload(&host_mappings))
            .await
            .map_err(|error| error.to_string())?,
    )?;

    if let Err(error) = state.redis.set_string_value(flag_key, "1").await {
        tracing::warn!(%error, %flag_key, "failed to mark gateway portal host-rules patch applied");
    }
    Ok(true)
}

async fn rollback_gateway_settings(state: &AppState, previous_config: &Value) {
    if let Err(error) = state.redis.save_config(previous_config).await {
        tracing::warn!(%error, "failed to rollback gateway settings config");
        return;
    }
    if let Err(error) = sync_gateway_runtime(state, previous_config).await {
        tracing::warn!(%error, "failed to rollback gateway settings runtime");
    }
}

fn ensure_go_success(value: Value) -> Result<(), String> {
    if value
        .get("success")
        .and_then(Value::as_bool)
        .unwrap_or(true)
    {
        return Ok(());
    }
    Err(value
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or(GO_BACKEND_UNSUCCESSFUL_RESPONSE)
        .to_string())
}

fn visible_host_mappings(mappings: &[Value]) -> Vec<Value> {
    mappings
        .iter()
        .filter(|mapping| !is_auth_service_mapping(mapping))
        .cloned()
        .collect()
}

fn is_auth_service_mapping(mapping: &Value) -> bool {
    mapping
        .get("target")
        .and_then(Value::as_str)
        .is_some_and(is_auth_service_target)
}

fn is_auth_service_target(target: &str) -> bool {
    is_http_proxy_target_url(target)
        && parse_target_port(target).is_some_and(|port| port == resolve_auth_service_port())
}

fn is_http_proxy_target_url(target: &str) -> bool {
    Url::parse(target.trim()).ok().is_some_and(|url| {
        matches!(url.scheme(), "http" | "https" | "ws" | "wss") && url.host_str().is_some()
    })
}

fn parse_target_port(target: &str) -> Option<i64> {
    let normalized = target.trim();
    if normalized.is_empty() {
        return None;
    }
    if let Ok(parsed) = Url::parse(normalized) {
        if let Some(port) = parsed.port() {
            return Some(i64::from(port));
        }
        return match parsed.scheme() {
            "https" | "wss" => Some(443),
            "http" | "ws" => Some(80),
            _ => None,
        };
    }
    let (_, tail) = normalized.rsplit_once(':')?;
    let digits = tail
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return None;
    }
    digits
        .parse::<i64>()
        .ok()
        .filter(|port| *port > 0 && *port <= 65535)
}

fn resolve_auth_service_port() -> i64 {
    std::env::var("AUTH_PORT")
        .ok()
        .and_then(|value| value.trim().parse::<i64>().ok())
        .filter(|port| *port > 0)
        .unwrap_or(7997)
}

fn build_gateway_proxy_header_items(hosts: &[Value], config: &Value) -> Vec<Value> {
    let disabled = disabled_hosts_set(config);
    hosts
        .iter()
        .map(|mapping| {
            let host = mapping.get("host").and_then(Value::as_str).unwrap_or("");
            json!({
                "host": host,
                "target": mapping.get("target").and_then(Value::as_str).unwrap_or("").trim(),
                "title": mapping.get("title").and_then(Value::as_str).unwrap_or("").trim(),
                "send_proxy_headers": !disabled.contains(&normalize_host(host)),
            })
        })
        .collect()
}

fn build_gateway_host_response_items(hosts: &[Value], config: &Value) -> Vec<Value> {
    let disabled = disabled_hosts_set(config);
    hosts
        .iter()
        .map(|mapping| {
            let host = mapping.get("host").and_then(Value::as_str).unwrap_or("");
            json!({
                "host": host,
                "target": mapping.get("target").and_then(Value::as_str).unwrap_or("").trim(),
                "title": mapping.get("title").and_then(Value::as_str).unwrap_or("").trim(),
                "preserve_host": !disabled.contains(&normalize_host(host)),
            })
        })
        .collect()
}

fn disabled_hosts_set(config: &Value) -> std::collections::HashSet<String> {
    config
        .get("disabled_hosts")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(normalize_host)
                .filter(|host| !host.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn normalize_gateway_visibility(value: &Value) -> Value {
    json!({
        "enabled": value.get("enabled").and_then(Value::as_bool).unwrap_or(false),
        "selections": value.get("selections").and_then(Value::as_array).cloned().unwrap_or_default(),
        "custom_cidrs": string_list(value.get("custom_cidrs")),
    })
}

fn normalize_disabled_hosts_config(value: &Value) -> Value {
    let mut seen = std::collections::HashSet::new();
    let disabled_hosts = value
        .get("disabled_hosts")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(normalize_host)
                .filter(|host| !host.is_empty() && seen.insert(host.clone()))
                .map(Value::String)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    json!({ "disabled_hosts": disabled_hosts })
}

fn normalize_reverse_proxy_throttle(value: &Value) -> Value {
    json!({
        "enabled": value.get("enabled").and_then(Value::as_bool).unwrap_or(true),
        "requests_per_second": positive_int(value.get("requests_per_second"), 100),
        "burst": positive_int(value.get("burst"), 200),
        "block_seconds": positive_int(value.get("block_seconds"), 30),
    })
}

fn normalize_gateway_crawler_blocker(value: &Value) -> Value {
    json!({
        "enabled": value.get("enabled").and_then(Value::as_bool).unwrap_or(false),
        "updated_at": value.get("updated_at").and_then(Value::as_str).map(|value| Value::String(value.to_string())).unwrap_or(Value::Null),
    })
}

fn normalize_gateway_portal(value: &Value) -> Value {
    json!({
        "enabled": value.get("enabled").and_then(Value::as_bool).unwrap_or(true),
        "display_style": if value.get("display_style").and_then(Value::as_str) == Some("domain") { "domain" } else { "title" },
        "show_app_icon": value.get("show_app_icon").and_then(Value::as_bool).unwrap_or(true),
        "icon_drag_mode": if value.get("icon_drag_mode").and_then(Value::as_str) == Some("free") { "free" } else { "corners" },
    })
}

fn is_gateway_portal_title_mode(config: &Value) -> bool {
    normalize_gateway_portal(
        config
            .get("gateway_portal")
            .unwrap_or(&default_gateway_portal()),
    )
    .get("display_style")
    .and_then(Value::as_str)
        != Some("domain")
}

fn is_gateway_portal_app_icon_mode(config: &Value) -> bool {
    normalize_gateway_portal(
        config
            .get("gateway_portal")
            .unwrap_or(&default_gateway_portal()),
    )
    .get("show_app_icon")
    .and_then(Value::as_bool)
        != Some(false)
}

fn merge_objects(previous: &Value, patch: &Value) -> Value {
    let mut merged = previous.as_object().cloned().unwrap_or_default();
    if let Some(patch) = patch.as_object() {
        for (key, value) in patch {
            merged.insert(key.clone(), value.clone());
        }
    }
    Value::Object(merged)
}

fn ensure_object(value: &mut Value) -> &mut Map<String, Value> {
    if !value.is_object() {
        *value = json!({});
    }
    value.as_object_mut().expect("value is object")
}

fn string_list(value: Option<&Value>) -> Vec<Value> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(|item| Value::String(item.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

fn normalize_host(value: &str) -> String {
    let without_scheme = value
        .trim()
        .to_ascii_lowercase()
        .split_once("://")
        .map(|(_, rest)| rest.to_string())
        .unwrap_or_else(|| value.trim().to_ascii_lowercase());
    without_scheme
        .split('/')
        .next()
        .unwrap_or("")
        .trim_end_matches('.')
        .to_string()
}

fn positive_int(value: Option<&Value>, fallback: i64) -> i64 {
    number_floor_value_or_parse(value)
        .filter(|value| *value > 0)
        .unwrap_or(fallback)
}

fn normalize_cache_ttl(value: Option<&Value>, fallback: i64) -> i64 {
    number_floor_value_or_parse(value)
        .filter(|value| *value >= 0)
        .unwrap_or(fallback)
}

fn number_floor_value_or_parse(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::String(raw) => parse_i64_prefix(raw.trim_start()),
        other => number_floor(other),
    }
}

fn number_floor(value: &Value) -> Option<i64> {
    if let Some(value) = value.as_i64() {
        return Some(value);
    }
    let value = value.as_f64()?;
    if value.is_finite() {
        Some(value.floor() as i64)
    } else {
        None
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
    has_digit
        .then(|| value[..end].parse::<i64>().ok())
        .flatten()
}

fn default_gateway_visibility_runtime() -> Value {
    json!({ "enabled": false, "cidrs": [], "updated_at": null })
}

fn default_gateway_proxy_headers_runtime() -> Value {
    json!({ "enabled": false, "omit_targets": [], "updated_at": null })
}

fn default_gateway_host_response_runtime() -> Value {
    json!({ "enabled": false, "omit_targets": [], "updated_at": null })
}

fn default_gateway_visibility() -> Value {
    json!({ "enabled": false, "selections": [], "custom_cidrs": [] })
}

fn default_disabled_hosts_config() -> Value {
    json!({ "disabled_hosts": [] })
}

fn default_reverse_proxy_throttle() -> Value {
    json!({
        "enabled": true,
        "requests_per_second": 100,
        "burst": 200,
        "block_seconds": 30,
    })
}

fn default_gateway_crawler_blocker() -> Value {
    json!({ "enabled": false, "updated_at": null })
}

fn default_gateway_portal() -> Value {
    json!({
        "enabled": true,
        "display_style": "title",
        "show_app_icon": true,
        "icon_drag_mode": "corners",
    })
}

fn default_subdomain_mode() -> Value {
    json!({
        "auth_cache_ttl_seconds": 1,
        "auth_cache_unauthorized_ttl_seconds": 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gateway_response_uses_node_defaults() {
        let config = json!({
            "subdomain_mode": {},
            "host_mappings": [
                { "host": "app.example.com", "target": "http://127.0.0.1:8080", "title": "App" },
                { "host": "auth.example.com", "target": "http://127.0.0.1:7997", "service_role": "auth" }
            ],
            "gateway_proxy_headers": { "disabled_hosts": ["app.example.com"] }
        });
        let host_mappings = config
            .get("host_mappings")
            .and_then(Value::as_array)
            .cloned()
            .unwrap();
        let visible = visible_host_mappings(&host_mappings);
        let proxy_items = build_gateway_proxy_header_items(
            &visible,
            config.get("gateway_proxy_headers").unwrap(),
        );
        assert_eq!(
            normalize_reverse_proxy_throttle(&json!({})),
            default_reverse_proxy_throttle()
        );
        assert_eq!(visible.len(), 1);
        assert_eq!(
            proxy_items[0]
                .get("send_proxy_headers")
                .and_then(Value::as_bool),
            Some(false)
        );
    }

    #[test]
    fn gateway_patch_merges_and_normalizes_sections() {
        let mut config = json!({
            "subdomain_mode": { "auth_cache_ttl_seconds": 1 },
            "reverse_proxy_throttle": { "enabled": true, "requests_per_second": 10, "burst": 20, "block_seconds": 30 },
            "gateway_portal": { "enabled": true, "display_style": "title", "show_app_icon": true, "icon_drag_mode": "corners" }
        });
        let patch = json!({
            "auth_cache_ttl_seconds": 8,
            "reverse_proxy_throttle": { "burst": 250 },
            "portal": { "display_style": "domain", "show_app_icon": false },
            "crawler_blocker": { "enabled": true }
        });
        apply_gateway_patch(&mut config, patch.as_object().unwrap());
        assert_eq!(
            config.pointer("/subdomain_mode/auth_cache_ttl_seconds"),
            Some(&Value::Number(8.into()))
        );
        assert_eq!(
            config.pointer("/reverse_proxy_throttle/requests_per_second"),
            Some(&Value::Number(10.into()))
        );
        assert_eq!(
            config.pointer("/reverse_proxy_throttle/burst"),
            Some(&Value::Number(250.into()))
        );
        assert_eq!(
            config
                .pointer("/gateway_portal/display_style")
                .and_then(Value::as_str),
            Some("domain")
        );
        assert_eq!(
            config
                .pointer("/gateway_crawler_blocker/enabled")
                .and_then(Value::as_bool),
            Some(true)
        );
        assert!(
            config
                .pointer("/gateway_crawler_blocker/updated_at")
                .and_then(Value::as_str)
                .is_some()
        );
    }

    #[test]
    fn gateway_number_normalizers_match_node_parse_int_for_strings() {
        assert_eq!(
            normalize_reverse_proxy_throttle(&json!({
                "requests_per_second": "12px",
                "burst": "1.9",
                "block_seconds": "  +30s"
            })),
            json!({
                "enabled": true,
                "requests_per_second": 12,
                "burst": 1,
                "block_seconds": 30
            })
        );
    }

    #[test]
    fn gateway_target_configs_filter_auth_targets_and_stale_hosts() {
        let config = json!({
            "run_type": 3,
            "host_mappings": [
                { "host": "app.example.com", "target": "http://127.0.0.1:8080", "title": "App" },
                { "host": "auth.example.com", "target": "http://127.0.0.1:7997", "title": "Auth" }
            ],
        });
        let requested = json!({
            "disabled_hosts": ["APP.EXAMPLE.COM", "missing.example.com", "auth.example.com"]
        });
        let compiled = compile_gateway_proxy_headers_state(&config, &requested);

        assert_eq!(
            compiled.config["disabled_hosts"],
            json!(["app.example.com"])
        );
        assert_eq!(
            compiled.runtime["omit_targets"],
            json!(["http://127.0.0.1:8080"])
        );
    }

    #[test]
    fn gateway_target_runtime_supports_reverse_proxy_subdomain_mode() {
        let config = json!({
            "run_type": 1,
            "reverse_proxy_submode": "subdomain",
            "host_mappings": [
                { "host": "app.example.com", "target": "http://127.0.0.1:8080", "title": "App" }
            ],
        });
        let compiled =
            compile_gateway_host_response_state(&config, &json!({ "disabled_hosts": [] }));

        assert_eq!(compiled.runtime["enabled"], Value::Bool(true));
        assert_eq!(compiled.runtime["omit_targets"], json!([]));
    }

    #[test]
    fn localizes_gateway_settings_route_errors() {
        let translator = Translator::new("zh-CN");

        assert_eq!(
            gateway_route_text(&translator, "loadGatewaySettingsFailed"),
            "加载网关设置失败"
        );
        assert_eq!(
            localize_gateway_route_message(&translator, "Gateway payload must be an object"),
            "网关请求内容必须是对象"
        );
        assert_eq!(
            localize_gateway_route_message(
                &translator,
                "Gateway visibility payload must be an object"
            ),
            "网关请求内容必须是对象"
        );
        assert_eq!(
            localize_gateway_route_message(&translator, GO_BACKEND_UNSUCCESSFUL_RESPONSE),
            "上游服务不可用"
        );
        assert_eq!(
            rollback_message(
                &translator,
                "Gateway payload must be an object",
                None,
                "server.admin.gatewayVisibility.updateFailedRolledBack",
            ),
            "网关请求内容必须是对象"
        );
        assert_eq!(
            gateway_route_text_params(
                &translator,
                "syncGatewaySettingsFailed",
                &[("message", "网关不可用".to_string())],
            ),
            "同步网关设置失败：网关不可用"
        );
    }

    #[test]
    fn gateway_visibility_cidr_validation_matches_node_shape() {
        let translator = Translator::new("en");
        let cidrs = validate_gateway_custom_cidrs(
            vec![
                Value::String(" 203.0.113.0/24 ".to_string()),
                Value::String("203.0.113.0/24".to_string()),
                Value::String("2001:db8::/32".to_string()),
            ],
            &translator,
        )
        .unwrap();
        assert_eq!(cidrs, vec!["203.0.113.0/24", "2001:db8::/32"]);

        let error = validate_gateway_custom_cidrs(
            vec![Value::String("10.0.0.0/33".to_string())],
            &translator,
        )
        .unwrap_err();
        assert!(error.contains("10.0.0.0/33"));
    }
}
