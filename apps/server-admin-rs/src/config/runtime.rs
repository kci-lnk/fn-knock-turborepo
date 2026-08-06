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
    auto_https, common_auth_locations, gateway_settings,
    i18n::Translator,
    proxy_config::{self, build_gateway_auth_config},
    response, runtime_profile,
    state::AppState,
    store as app_store, system_assets,
    terminal_paths::normalize_terminal_default_cwd,
    time_utils, waf, whitelist,
};

mod fnos_connect_waf;
mod fnos_network;
mod handlers;
mod migrations;
mod runtime_apply;
mod smart_connect;
mod store;
mod utils;

pub(crate) use fnos_connect_waf::normalize_fnos_connect_waf;
use fnos_connect_waf::{
    get_fnos_connect_waf, start_fnos_connect_waf_reconciler, update_fnos_connect_waf,
};
use fnos_network::*;
use handlers::*;
use migrations::*;
pub(crate) use runtime_apply::*;
pub(crate) use smart_connect::*;
pub(crate) use store::*;
use utils::*;

#[cfg(test)]
mod tests;

const CAPTCHA_SETTINGS_KEY: &str = "fn_knock:captcha:settings";
const LEGACY_CAPTCHA_SETTINGS_KEY: &str = "fn_knock:config:captcha";
pub(crate) const POW_MIN_MAX_NUMBER: i64 = 10_000;
pub(crate) const POW_MAX_MAX_NUMBER: i64 = 1_000_000;
pub(crate) const POW_MAX_NUMBER_STEP: i64 = 10_000;
pub(crate) const POW_DEFAULT_BASE_MAX_NUMBER: i64 = 100_000;
pub(crate) const POW_DEFAULT_UNCOMMON_MAX_NUMBER: i64 = 300_000;
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
        .route("/api/admin/config/run_type", post(update_run_type))
        .route(
            "/api/admin/config/protocol_mapping_feature",
            get(get_protocol_mapping_feature).post(update_protocol_mapping_feature),
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

pub fn terminal_feature_routes() -> Router<AppState> {
    Router::new().route(
        "/api/admin/config/terminal_feature",
        get(get_terminal_feature).post(update_terminal_feature),
    )
}

pub fn smart_connect_config_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/admin/config/smart_connect/details",
            get(get_smart_connect_details),
        )
        .route(
            "/api/admin/config/smart_connect",
            post(update_smart_connect),
        )
}

pub fn firewall_runtime_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/admin/config/auto_manage_firewall",
            post(update_auto_manage_firewall),
        )
        .route(
            "/api/admin/config/firewall_additional_ports",
            get(get_firewall_additional_ports).post(update_firewall_additional_ports),
        )
        .route("/api/admin/firewall/reset", post(reset_firewall))
        .route("/api/admin/firewall/clear", post(clear_firewall))
}

pub fn fnos_connect_waf_routes() -> Router<AppState> {
    Router::new().route(
        "/api/admin/config/fnos_connect_waf",
        get(get_fnos_connect_waf).post(update_fnos_connect_waf),
    )
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
    if let Some(localized) =
        proxy_config::localize_stream_mapping_runtime_error(translator, message)
    {
        return localized;
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
    let mut config = match state.store.get_config().await {
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

    if let Err(error) = waf::sync_waf_config_to_gateway(&state, &config).await {
        tracing::warn!(%error, "failed to sync WAF config on boot");
    }

    if let Err(error) = sync_smart_connect_on_boot(&state, &config).await {
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

    start_fnos_connect_waf_reconciler(state);
}

pub(crate) async fn migrate_and_constrain_config_after_import(
    state: &AppState,
) -> Result<Value, String> {
    let mut config = state
        .store
        .get_config()
        .await
        .map_err(|error| error.to_string())?;
    apply_boot_config_migrations(state, &mut config)
        .await
        .map_err(|error| error.to_string())?;
    apply_runtime_constraints_on_boot(state, &mut config)
        .await
        .map_err(|error| error.to_string())?;
    Ok(config)
}

pub(crate) async fn sync_smart_connect_after_import(
    state: &AppState,
    config: &Value,
) -> Result<(), String> {
    sync_smart_connect_on_boot(state, config).await
}

pub(crate) async fn sync_fnos_port_icon_hijack_after_import(
    state: &AppState,
    config: &Value,
) -> Result<(), String> {
    let value = normalize_fnos_port_icon_hijack(config.get("fnos_port_icon_hijack"));
    state
        .go_backend
        .set_fnos_port_icon_hijack_config(&value)
        .await
        .map_err(|error| error.to_string())
        .and_then(|value| ensure_go_success(value).map_err(|error| error.to_string()))
}

pub(crate) async fn sync_fnos_network_tuning_after_import(
    state: &AppState,
    previous_config: &Value,
    next_config: &Value,
    translator: &Translator,
) -> Result<(), String> {
    fnos_network::sync_fnos_network_tuning_after_import(
        state,
        previous_config,
        next_config,
        translator,
    )
    .await
}
