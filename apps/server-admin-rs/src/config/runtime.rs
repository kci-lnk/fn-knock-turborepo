use std::{collections::BTreeSet, fs, path::Path, process::Command};

use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::{Map, Value, json};
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    auto_https, common_auth_locations, gateway_settings,
    i18n::Translator,
    json_utils::ensure_object,
    net_utils,
    proxy_config::{self, build_gateway_auth_config},
    response, runtime_profile,
    state::AppState,
    store as app_store, system_assets, time_utils, waf, whitelist,
};

mod fnos_connect_waf;
mod fnos_network;
mod handlers;
mod migrations;
mod runtime_apply;
mod smart_connect;
mod store;
mod utils;

pub(crate) use fnos_connect_waf::start_fnos_connect_waf_reconciler;
pub(crate) use fnos_connect_waf::{fnos_connect_waf_routes, normalize_fnos_connect_waf};
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
const SMART_CONNECT_RUNTIME_KEY: &str = "fn_knock:smart-connect:runtime";
const LEGACY_REVERSE_PROXY_THROTTLE_PATCH_FLAG_KEY: &str =
    "fn_knock:patch:reverse-proxy-throttle:v1";
const REVERSE_PROXY_THROTTLE_DEFAULT_V2_PATCH_FLAG_KEY: &str =
    "fn_knock:patch:reverse-proxy-throttle-default:v2";
const LEGACY_EVENT_SYSTEM_RESOURCE_ALERTS_PATCH_FLAG_KEY: &str =
    "fn_knock:patch:event-system-resource-alerts:v1";
const GATEWAY_PORTAL_SHOW_WOL_DEFAULT_PATCH_FLAG_KEY: &str =
    "fn_knock:patch:gateway-portal-show-wol-default:v1";
const LEGACY_REDIRECTED_HTTP_PORTS: [i64; 2] = [80, 443];
const SMART_CONNECT_DNS_PORT: i64 = 53;
const SMART_CONNECT_LOCAL_TTL_SECONDS: u16 = 30;
const SMART_CONNECT_MANAGED_CONF_PATH: &str = "/etc/dnsmasq.d/fn-knock-smart-connect.conf";
const FNOS_NETWORK_TUNING_SYSCTL_PATH: &str = "/etc/sysctl.d/99-fn-knock-network.conf";
const GO_BACKEND_UNSUCCESSFUL_RESPONSE: &str = "Go backend returned an unsuccessful response";
const JS_MAX_SAFE_INTEGER_I64: i64 = 9_007_199_254_740_991;

pub(crate) fn sync_routes_config_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(sync_routes))
}

pub(crate) fn captcha_config_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_captcha))
        .routes(routes!(update_captcha))
}

pub(crate) fn run_type_config_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(update_run_type))
}

pub(crate) fn wol_feature_config_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_wol_feature))
        .routes(routes!(update_wol_feature))
}

pub(crate) fn protocol_mapping_feature_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_protocol_mapping_feature))
        .routes(routes!(update_protocol_mapping_feature))
}

pub(crate) fn auto_https_config_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_auto_https))
        .routes(routes!(update_auto_https))
}

pub(crate) fn default_route_config_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_default_route))
        .routes(routes!(update_default_route))
        .routes(routes!(update_default_tunnel))
}

#[allow(deprecated)] // Retain the legacy managed FRP runtime API during migration.
pub(crate) fn proxy_protocol_force_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_proxy_protocol_force))
        .routes(routes!(update_proxy_protocol_force))
}

pub(crate) fn run_mode_prompt_preferences_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_run_mode_prompt_preferences))
        .routes(routes!(update_run_mode_prompt_preferences))
}

pub(crate) fn fnos_port_icon_hijack_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_fnos_port_icon_hijack))
        .routes(routes!(update_fnos_port_icon_hijack))
}

pub(crate) fn fnos_network_tuning_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_fnos_network_tuning))
        .routes(routes!(update_fnos_network_tuning))
}

pub(crate) fn fnos_share_bypass_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_fnos_share_bypass))
        .routes(routes!(update_fnos_share_bypass))
}

pub fn smart_connect_config_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_smart_connect_details))
        .routes(routes!(update_smart_connect))
}

pub fn firewall_runtime_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(update_auto_manage_firewall))
        .routes(routes!(get_firewall_additional_ports))
        .routes(routes!(update_firewall_additional_ports))
        .routes(routes!(reset_firewall))
        .routes(routes!(clear_firewall))
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
    let message = proxy_config::stream_mapping_runtime_error_message(message).unwrap_or(message);
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

fn message_mentions_stream_port(message: &str, port: u64) -> bool {
    for marker in [
        format!("listen_port {port}"),
        format!("port {port}"),
        format!(":{port}"),
    ] {
        let mut remainder = message;
        while let Some((_, after)) = remainder.split_once(&marker) {
            if after
                .chars()
                .next()
                .is_none_or(|character| !character.is_ascii_digit())
            {
                return true;
            }
            remainder = after;
        }
    }
    false
}

fn stream_mapping_issue_identity(config: &Value, message: &str) -> (Value, Value, Value) {
    let mappings = config
        .get("stream_mappings")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let active = mappings
        .iter()
        .filter(|mapping| mapping.get("disabled").and_then(Value::as_bool) != Some(true))
        .collect::<Vec<_>>();
    let lowercase_message = message.to_ascii_lowercase();
    let candidates = active
        .iter()
        .copied()
        .filter(|mapping| {
            mapping
                .get("listen_port")
                .and_then(Value::as_u64)
                .is_some_and(|port| message_mentions_stream_port(&lowercase_message, port))
        })
        .collect::<Vec<_>>();
    let selected = candidates
        .iter()
        .copied()
        .find(|mapping| {
            mapping
                .get("protocol")
                .and_then(Value::as_str)
                .is_some_and(|protocol| lowercase_message.contains(&protocol.to_ascii_lowercase()))
        })
        .or_else(|| (candidates.len() == 1).then(|| candidates[0]))
        .or_else(|| (active.len() == 1).then(|| active[0]));
    let Some(mapping) = selected else {
        return (Value::Null, Value::Null, Value::Null);
    };
    (
        mapping
            .get("protocol")
            .and_then(Value::as_str)
            .map(str::to_ascii_lowercase)
            .map(Value::String)
            .unwrap_or(Value::Null),
        mapping
            .get("listen_port")
            .and_then(Value::as_u64)
            .map(Value::from)
            .unwrap_or(Value::Null),
        mapping
            .get("target")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|target| !target.is_empty())
            .map(|target| Value::String(target.to_string()))
            .unwrap_or(Value::Null),
    )
}

fn protocol_mapping_runtime_issue(config: &Value, error: &str) -> Option<Value> {
    let message = proxy_config::stream_mapping_runtime_error_message(error)?;
    let lowercase_message = message.to_ascii_lowercase();
    let local_port_loop = lowercase_message.contains("cannot target the same local");
    let listen_port_in_use = lowercase_message.contains("address already in use")
        || lowercase_message.contains("eaddrinuse")
        || lowercase_message.contains("only one usage of each socket address");
    if !local_port_loop
        && !listen_port_in_use
        && crate::transient_error::is_transient_runtime_error(message)
    {
        return None;
    }
    let code = if local_port_loop {
        "local_port_loop"
    } else if listen_port_in_use {
        "listen_port_in_use"
    } else {
        "runtime_sync_failed"
    };
    let (protocol, listen_port, target) = stream_mapping_issue_identity(config, message);
    Some(json!({
        "code": code,
        "message": message,
        "protocol": protocol,
        "listen_port": listen_port,
        "target": target,
    }))
}

async fn apply_run_type_config_on_boot_with<Apply, ApplyFuture>(
    state: &AppState,
    config: &Value,
    run_type: i64,
    mut apply: Apply,
) -> Result<(), String>
where
    Apply: FnMut() -> ApplyFuture,
    ApplyFuture: std::future::Future<Output = Result<(), String>>,
{
    let protocol_mapping_feature = load_protocol_mapping_feature(state, Some(config))
        .await
        .map_err(|error| error.to_string())?;
    let protocol_mapping_enabled = run_type == 3
        && protocol_mapping_feature
            .get("enabled")
            .and_then(Value::as_bool)
            == Some(true);

    match apply().await {
        Ok(()) => {
            if protocol_mapping_enabled && protocol_mapping_feature.get("runtime_issue").is_some() {
                let mut cleared = protocol_mapping_feature;
                ensure_object(&mut cleared).remove("runtime_issue");
                if let Err(error) = save_protocol_mapping_feature(state, &cleared).await {
                    tracing::warn!(%error, "failed to clear recovered protocol mapping boot issue");
                }
            }
            Ok(())
        }
        Err(error) if protocol_mapping_enabled => {
            let Some(issue) = protocol_mapping_runtime_issue(config, &error) else {
                return Err(error);
            };
            let mut disabled = protocol_mapping_feature;
            let disabled_object = ensure_object(&mut disabled);
            disabled_object.insert("enabled".to_string(), Value::Bool(false));
            disabled_object.insert("runtime_issue".to_string(), issue.clone());
            save_protocol_mapping_feature(state, &disabled)
                .await
                .map_err(|save_error| {
                    format!(
                        "failed to persist disabled protocol mapping state after startup error: {save_error}"
                    )
                })?;
            tracing::error!(
                issue = %issue,
                "protocol mappings failed during startup and were disabled for repair"
            );
            if let Err(retry_error) = apply().await {
                tracing::warn!(
                    %retry_error,
                    "runtime remained partially unavailable after disabling protocol mappings; continuing boot for administrator repair"
                );
            }
            Ok(())
        }
        Err(error) => Err(error),
    }
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

pub(crate) async fn sync_runtime_config_on_boot(state: AppState) -> Result<(), String> {
    let mut config = match state.storage.store.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load config for boot runtime sync");
            return Err(error.to_string());
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
    apply_run_type_config_on_boot_with(&state, &config, run_type, || {
        apply_run_type_config(&state, &config, run_type)
    })
    .await
    .inspect_err(|error| tracing::warn!(%error, "failed to sync run type config on boot"))?;

    let gateway_logging = normalize_gateway_logging(config.get("gateway_logging"));
    if let Err(error) = state
        .gateway
        .client
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
        .gateway
        .client
        .set_fnos_port_icon_hijack_config(&fnos_port_icon_hijack)
        .await
        .and_then(ensure_go_success)
    {
        tracing::warn!(%error, "failed to sync FnOS port icon hijack config on boot");
    }

    Ok(())
}

pub(crate) async fn migrate_and_constrain_config_after_import(
    state: &AppState,
) -> Result<Value, String> {
    let mut config = state
        .storage
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
        .gateway
        .client
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
