use std::{
    collections::{BTreeSet, HashMap},
    env, fs,
    future::Future,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    time::Duration,
};

mod config;
mod curl_transport;
mod domain_targets;
mod i18n;
mod interface_selector;
mod providers;
mod public_check;
mod route_actions;
mod settings;
mod store;
mod target;
mod tasks;

use config::*;
use domain_targets::*;
use i18n::*;
use interface_selector::*;
use providers::*;
use public_check::*;
use route_actions::*;
use route_actions::{__path_clear_logs, __path_poll};
use settings::*;
use store::*;
use target::*;
use tasks::*;

#[cfg(test)]
mod tests;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use get_if_addrs::{IfAddr, get_if_addrs};
use serde::Deserialize;
use serde_json::{Value, json};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tokio::time as tokio_time;
use tokio::{net::lookup_host, task::JoinSet};
use url::Url;
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;

use crate::{http_body, i18n::Translator, response, state::AppState, system_events, time_utils};

const PRIMARY_TARGET_ID: &str = "primary";
const PRIMARY_TARGET_NAME: &str = "主域名";

const DDNS_ENABLED: &str = "fn_knock:ddns:enabled";
const DDNS_SETTINGS: &str = "fn_knock:ddns:settings";
const DDNS_LEGACY_PROVIDER: &str = "fn_knock:ddns:provider";
const DDNS_LEGACY_CONFIG_PREFIX: &str = "fn_knock:ddns:config:";
const DDNS_LEGACY_LAST_IP: &str = "fn_knock:ddns:last_ip";
const DDNS_LEGACY_LAST_CHECK: &str = "fn_knock:ddns:last_check";
const DDNS_TARGET_IDS: &str = "fn_knock:ddns:v2:target_ids";
const DDNS_PRIMARY_TARGET_ID: &str = "fn_knock:ddns:v2:primary_target_id";
const DDNS_TARGET_PREFIX: &str = "fn_knock:ddns:v2:target:";
const DDNS_LOGS: &str = "fn_knock:ddns:logs";
const DDNS_LOG_TTL_SECONDS: usize = 7 * 24 * 3600;
const DDNS_LOG_MAX_LEN: usize = 1000;
const DDNS_UPDATE_LOCK_NAME: &str = "ddns-update";
const DDNS_UPDATE_LOCK_TTL_SECONDS: usize = 600;
const DDNS_STARTUP_CHECK_DELAY_SECONDS: u64 = 30;
const DDNS_INTERFACE_FAILOVER_RECHECK_DELAY_MILLIS: u64 = 1_500;
const DDNS_INTERFACE_PREFERRED_RECOVERY_CONFIRMATIONS: u8 = 3;
const DDNS_UPDATE_SCOPE_FIELD: &str = "update_scope";
const DDNS_NETWORK_INTERFACE_FIELD: &str = "network_interface";
const DDNS_IP_SOURCE_FIELD: &str = "ip_source";
const DDNS_INTERFACE_IPV4_INDEX_FIELD: &str = "interface_ipv4_index";
const DDNS_INTERFACE_IPV6_INDEX_FIELD: &str = "interface_ipv6_index";
const DDNS_INTERFACE_IPV4_SELECTOR_FIELD: &str = "interface_ipv4_selector";
const DDNS_INTERFACE_IPV6_SELECTOR_FIELD: &str = "interface_ipv6_selector";
const DDNS_ALLOW_PRIVATE_ADDRESSES_FIELD: &str = "allow_private_addresses";
const DDNS_STATIC_IPV4_FIELD: &str = "static_ipv4";
const DDNS_STATIC_IPV6_FIELD: &str = "static_ipv6";
const DDNS_SOURCE_DOMAIN_FIELD: &str = "source_domain";
const DDNS_EDGEONE_OVERSEAS_ACCESS_FIELD: &str = "edgeone_overseas_access";

const DEFAULT_PUBLIC_CHECK_IPV4: [&str; 2] = ["https://4.fnknock.cn", "http://ipv4.icanhazip.com"];
const DEFAULT_PUBLIC_CHECK_IPV6: [&str; 2] =
    ["https://6.fnknock.cn", "https://ipv6.icanhazip.com/"];
const DOCKER_HOST_INTERFACE_PREFIX: &str = "docker-host:";
const DEFAULT_DOCKER_HOST_IF_INET6_PATH: &str = "/host/proc/net/if_inet6";
const IP_DETECTION_TIMEOUT_MS: u64 = 7000;
const MAX_PUBLIC_CHECK_RESPONSE_BYTES: usize = 64 * 1024;
const RESPONSE_PREVIEW_MAX_LENGTH: usize = 240;

#[derive(Deserialize)]
struct ToggleBody {
    enabled: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SettingsBody {
    update_interval_minutes: Option<i64>,
    public_check_sources: Option<Value>,
    http_transport: Option<String>,
    public_dns_provider: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PublicCheckTestBody {
    public_check_sources: Value,
    http_transport: Option<String>,
    public_dns_provider: Option<String>,
    network_interface: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InterfaceSelectorPreviewBody {
    network_interface: String,
    family: String,
    selector: Value,
    current_address: Option<String>,
    #[serde(default)]
    allow_private_addresses: bool,
}

#[derive(Deserialize)]
struct ProviderBody {
    provider: String,
}

#[derive(Deserialize)]
struct ConfigBody {
    config: HashMap<String, String>,
}

#[derive(Deserialize)]
struct TargetBody {
    name: Option<String>,
    provider: String,
    enabled: Option<bool>,
    config: Option<HashMap<String, String>>,
}

#[derive(Deserialize)]
struct TargetEnabledBody {
    enabled: bool,
}

#[derive(Deserialize)]
struct LogsQuery {
    limit: Option<String>,
}

#[derive(Deserialize)]
struct PollQuery {
    cursor: Option<String>,
}

#[derive(Clone, Debug)]
struct DDNSTargetMeta {
    id: String,
    name: String,
    is_primary: bool,
    enabled: bool,
    provider: Option<String>,
    created_at: String,
    updated_at: String,
    sort_order: i64,
}

#[derive(Clone, Debug)]
struct DDNSTargetRecord {
    meta: DDNSTargetMeta,
    config: HashMap<String, String>,
    last_ip: Value,
    selection_anchor: Value,
    last_check: Value,
}

#[derive(Clone, Debug)]
struct ResolvedTargetIps {
    ipv4: Option<String>,
    ipv6: Option<String>,
    source: &'static str,
    source_label: String,
    warnings: Vec<String>,
    selection_logs: Vec<String>,
    interface_resolutions: HashMap<String, InterfaceAddressResolution>,
    update_scope: &'static str,
}

#[derive(Clone, Debug)]
struct DDNSProviderUpdateResult {
    success: bool,
    message: String,
}

pub fn ddns_status_routes() -> Router<AppState> {
    ddns_openapi_routes().into()
}

pub(crate) fn ddns_openapi_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_status))
        .routes(routes!(toggle))
        .routes(routes!(get_providers))
        .routes(routes!(get_settings))
        .routes(routes!(update_settings))
        .routes(routes!(test_public_check_sources))
        .routes(routes!(get_interfaces))
        .routes(routes!(resolve_interface_selector_preview))
        .routes(routes!(set_provider))
        .routes(routes!(get_config))
        .routes(routes!(save_config))
        .routes(routes!(get_targets))
        .routes(routes!(create_target))
        .routes(routes!(get_target))
        .routes(routes!(update_target))
        .routes(routes!(delete_target))
        .routes(routes!(set_target_enabled))
        .routes(routes!(test_primary_target))
        .routes(routes!(test_target))
        .routes(routes!(get_logs))
        .routes(routes!(clear_logs))
        .routes(routes!(poll))
}

pub fn start_ddns_tasks(state: AppState) {
    let startup_state = state.clone();
    state.spawn_background("ddns-startup-check", async move {
        tokio::select! {
            _ = startup_state.shutdown.cancelled() => return,
            _ = tokio_time::sleep(Duration::from_secs(DDNS_STARTUP_CHECK_DELAY_SECONDS)) => {}
        }
        tokio::select! {
            _ = startup_state.shutdown.cancelled() => {}
            result = run_automatic_ddns_check(&startup_state, "startup", false, false) => {
                if let Err(error) = result {
                    tracing::warn!(%error, "DDNS startup check failed");
                }
            }
        }
    });

    let scheduler_state = state.clone();
    state.spawn_background("ddns-scheduler", async move {
        loop {
            let interval_result = tokio::select! {
                _ = scheduler_state.shutdown.cancelled() => break,
                result = ddns_update_interval_minutes(&scheduler_state) => result,
            };
            let interval_minutes = match interval_result {
                Ok(value) => value,
                Err(error) => {
                    tracing::warn!(%error, "failed to load DDNS scheduler interval");
                    10
                }
            };
            tokio::select! {
                _ = scheduler_state.shutdown.cancelled() => break,
                _ = tokio_time::sleep(Duration::from_secs((interval_minutes.max(1) as u64) * 60)) => {}
                _ = scheduler_state.ddns_schedule_reload.notified() => {
                    continue;
                }
            }
            tokio::select! {
                _ = scheduler_state.shutdown.cancelled() => break,
                result = run_automatic_ddns_check(&scheduler_state, "cron", true, false) => {
                    if let Err(error) = result {
                        tracing::warn!(%error, "DDNS scheduled check failed");
                    }
                }
            }
        }
    });
}

pub(super) fn reload_ddns_schedule(state: &AppState) {
    state.ddns_schedule_reload.notify_one();
}

#[utoipa::path(get, path = "/api/admin/ddns/status", tag = "ddns", operation_id = "get_api_admin_ddns_status", responses((status = 200, description = "DDNS status")))]
async fn get_status(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match build_ddns_status(&state, &translator).await {
        Ok(status) => response::ok(status).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to build DDNS status");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ddns_text(&translator, "statusLoadFailed", &[]),
            )
        }
    }
}

#[utoipa::path(post, path = "/api/admin/ddns/toggle", tag = "ddns", operation_id = "post_api_admin_ddns_toggle", responses((status = 200, description = "Updated DDNS enabled state")))]
async fn toggle(State(state): State<AppState>, Json(body): Json<ToggleBody>) -> Response {
    let translator = Translator::from_state(&state).await;
    let was_enabled = match state.storage.store.get_string_value(DDNS_ENABLED).await {
        Ok(value) => value.as_deref() == Some("true"),
        Err(error) => {
            tracing::warn!(%error, "failed to read DDNS enabled state");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ddns_text(&translator, "toggleFailed", &[]),
            );
        }
    };
    match state
        .storage
        .store
        .set_string_value(DDNS_ENABLED, if body.enabled { "true" } else { "false" })
        .await
    {
        Ok(()) => {
            if body.enabled && !was_enabled {
                let run_state = state.clone();
                state.spawn_background("ddns-enable-check", async move {
                    tokio::select! {
                        _ = run_state.shutdown.cancelled() => {}
                        result = run_automatic_ddns_check(&run_state, "enable", true, false) => {
                            if let Err(error) = result {
                                tracing::warn!(%error, "DDNS enable check failed");
                            }
                        }
                    }
                });
            }
            response::success_empty().into_response()
        }
        Err(error) => {
            tracing::warn!(%error, "failed to toggle DDNS");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ddns_text(&translator, "toggleFailed", &[]),
            )
        }
    }
}

#[utoipa::path(get, path = "/api/admin/ddns/providers", tag = "ddns", operation_id = "get_api_admin_ddns_providers", responses((status = 200, description = "DDNS providers")))]
async fn get_providers(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    response::ok(provider_catalog(&translator)).into_response()
}

#[utoipa::path(get, path = "/api/admin/ddns/settings", tag = "ddns", operation_id = "get_api_admin_ddns_settings", responses((status = 200, description = "DDNS settings")))]
async fn get_settings(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.storage.store.get_string_value(DDNS_SETTINGS).await {
        Ok(raw) => response::ok(parse_settings(raw.as_deref())).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to load DDNS settings");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ddns_text(&translator, "settingsLoadFailed", &[]),
            )
        }
    }
}

#[utoipa::path(post, path = "/api/admin/ddns/settings", tag = "ddns", operation_id = "post_api_admin_ddns_settings", responses((status = 200, description = "Updated DDNS settings")))]
async fn update_settings(
    State(state): State<AppState>,
    Json(body): Json<SettingsBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let current = match state.storage.store.get_string_value(DDNS_SETTINGS).await {
        Ok(raw) => parse_settings(raw.as_deref()),
        Err(error) => {
            tracing::warn!(%error, "failed to load current DDNS settings");
            let message = error.to_string();
            return response::error(
                StatusCode::BAD_REQUEST,
                if message.is_empty() {
                    ddns_text(&translator, "settingsSaveFailed", &[])
                } else {
                    message
                },
            );
        }
    };
    let interval = match body.update_interval_minutes {
        Some(value) if (5..=1440).contains(&value) => value,
        Some(_) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                ddns_text(
                    &translator,
                    "intervalOutOfRange",
                    &[("min", "5".to_string()), ("max", "1440".to_string())],
                ),
            );
        }
        None => current
            .get("updateIntervalMinutes")
            .and_then(Value::as_i64)
            .unwrap_or(10),
    };
    let public_sources = match body.public_check_sources.as_ref() {
        Some(value) => match normalize_public_check_sources_strict(
            value,
            current.get("publicCheckSources").unwrap_or(&Value::Null),
            &translator,
        ) {
            Ok(value) => value,
            Err(message) => return response::error(StatusCode::BAD_REQUEST, message),
        },
        None => current
            .get("publicCheckSources")
            .cloned()
            .unwrap_or_else(default_public_check_sources),
    };
    let http_transport = merge_http_transport_update(body.http_transport.as_deref(), &current);
    let public_dns_provider =
        merge_public_dns_provider_update(body.public_dns_provider.as_deref(), &current);
    let stored = json!({
        "updateIntervalMinutes": interval,
        "publicCheckSources": public_sources,
        "httpTransport": normalize_http_transport(Some(&Value::String(http_transport.to_string()))),
        "publicDnsProvider": public_dns_provider
    });
    let serialized = serde_json::to_string(&stored).unwrap_or_default();
    match state
        .storage
        .store
        .set_string_value(DDNS_SETTINGS, &serialized)
        .await
    {
        Ok(()) => {
            reload_ddns_schedule(&state);
            response::ok(parse_settings(Some(serialized.as_str()))).into_response()
        }
        Err(error) => {
            tracing::warn!(%error, "failed to save DDNS settings");
            let message = error.to_string();
            response::error(
                StatusCode::BAD_REQUEST,
                if message.is_empty() {
                    ddns_text(&translator, "settingsSaveFailed", &[])
                } else {
                    message
                },
            )
        }
    }
}

#[utoipa::path(post, path = "/api/admin/ddns/public-check/test", tag = "ddns", operation_id = "post_api_admin_ddns_public_check_test", responses((status = 200, description = "DDNS public IP source test results")))]
async fn test_public_check_sources(
    State(state): State<AppState>,
    Json(body): Json<PublicCheckTestBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let network_interface = normalize_network_interface(body.network_interface.as_deref());
    if !network_interface.is_empty()
        && !list_ddns_network_interfaces().iter().any(|item| {
            item.get("name").and_then(Value::as_str) == Some(network_interface.as_str())
        })
    {
        return response::error(
            StatusCode::BAD_REQUEST,
            ddns_text(
                &translator,
                "interfaceNotFound",
                &[("name", network_interface)],
            ),
        );
    }

    let sources = match normalize_public_check_sources_strict(
        &body.public_check_sources,
        &json!({ "ipv4": [], "ipv6": [] }),
        &translator,
    ) {
        Ok(value) => value,
        Err(message) => return response::error(StatusCode::BAD_REQUEST, message),
    };
    let stored_settings = if body.http_transport.is_none() || body.public_dns_provider.is_none() {
        match state.storage.store.get_string_value(DDNS_SETTINGS).await {
            Ok(raw) => Some(parse_settings(raw.as_deref())),
            Err(error) => {
                tracing::warn!(%error, "failed to load DDNS settings for public check test");
                let message = error.to_string();
                return response::error(
                    StatusCode::BAD_REQUEST,
                    if message.is_empty() {
                        ddns_text(&translator, "publicCheckTestFailed", &[])
                    } else {
                        message
                    },
                );
            }
        }
    } else {
        None
    };
    let transport = if let Some(value) = body.http_transport.as_ref() {
        normalize_http_transport(Some(&Value::String(value.clone()))).to_string()
    } else {
        stored_settings
            .as_ref()
            .and_then(|settings| settings.get("httpTransport"))
            .and_then(Value::as_str)
            .unwrap_or("node")
            .to_string()
    };
    let public_dns_provider = if let Some(value) = body.public_dns_provider.as_ref() {
        normalize_public_dns_provider(Some(value.as_str())).to_string()
    } else {
        stored_settings
            .as_ref()
            .and_then(|settings| settings.get("publicDnsProvider"))
            .and_then(Value::as_str)
            .unwrap_or(DEFAULT_PUBLIC_DNS_PROVIDER)
            .to_string()
    };
    match test_public_check_sources_inner(
        &sources,
        &transport,
        &public_dns_provider,
        Some(network_interface.as_str()),
        &translator,
    )
    .await
    {
        Ok(results) => response::ok(json!({ "results": results })).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to test DDNS public check sources");
            response::error(
                StatusCode::BAD_REQUEST,
                localize_ddns_error(&translator, &error.to_string()),
            )
        }
    }
}

#[utoipa::path(get, path = "/api/admin/ddns/interfaces", tag = "ddns", operation_id = "get_api_admin_ddns_interfaces", responses((status = 200, description = "DDNS network interfaces")))]
async fn get_interfaces() -> Response {
    response::ok(list_ddns_network_interfaces()).into_response()
}

#[utoipa::path(post, path = "/api/admin/ddns/interfaces/resolve", tag = "ddns", operation_id = "post_api_admin_ddns_interfaces_resolve", responses((status = 200, description = "DDNS interface selector preview")))]
async fn resolve_interface_selector_preview(
    State(state): State<AppState>,
    Json(body): Json<InterfaceSelectorPreviewBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let interface = normalize_network_interface(Some(&body.network_interface));
    if !matches!(body.family.as_str(), "ipv4" | "ipv6") {
        return response::error(
            StatusCode::BAD_REQUEST,
            ddns_text(&translator, "interfaceSelectorFamilyInvalid", &[]),
        );
    }
    let Some(network) = list_ddns_network_interfaces()
        .into_iter()
        .find(|item| item.get("name").and_then(Value::as_str) == Some(interface.as_str()))
    else {
        return response::error(
            StatusCode::BAD_REQUEST,
            ddns_text(&translator, "interfaceNotFound", &[("name", interface)]),
        );
    };
    let selector = match parse_interface_selector_value(&body.selector, &body.family) {
        Ok(selector) => selector,
        Err(error) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                ddns_text(
                    &translator,
                    "interfaceSelectorInvalid",
                    &[("message", error.to_string())],
                ),
            );
        }
    };
    let selection = resolve_interface_selector_with_policy(
        &network,
        &body.family,
        &selector,
        body.current_address.as_deref(),
        body.allow_private_addresses,
    );
    let mut warnings = Vec::new();
    if selection.eligible.len() > 1 {
        warnings.push("multiple_matches");
    }
    if body.family == "ipv6"
        && selection.eligible.iter().any(|item| {
            item.get("temporary").is_none_or(Value::is_null)
                || item.get("deprecated").is_none_or(Value::is_null)
        })
    {
        warnings.push("status_unknown");
    }
    response::ok(json!({
        "selectedAddress": selection.selected,
        "matchedAddresses": selection.eligible,
        "rejectedAddresses": selection.rejected,
        "reason": selection.reason,
        "warnings": warnings,
        "selector": selector
    }))
    .into_response()
}

#[utoipa::path(post, path = "/api/admin/ddns/provider", tag = "ddns", operation_id = "post_api_admin_ddns_provider", responses((status = 200, description = "Updated primary DDNS provider")))]
async fn set_provider(State(state): State<AppState>, Json(body): Json<ProviderBody>) -> Response {
    match set_primary_provider(&state, &body.provider).await {
        Ok(()) => response::success_empty().into_response(),
        Err(error) => ddns_error_response_from_state(&state, error).await,
    }
}

#[utoipa::path(get, path = "/api/admin/ddns/config/{provider}", tag = "ddns", operation_id = "get_api_admin_ddns_config_provider", responses((status = 200, description = "DDNS provider configuration")))]
async fn get_config(State(state): State<AppState>, Path(provider): Path<String>) -> Response {
    match primary_target(&state).await {
        Ok(primary) if primary.meta.provider.as_deref() == Some(provider.as_str()) => {
            response::ok(primary.config).into_response()
        }
        Ok(_) => response::ok(json!({})).into_response(),
        Err(error) => ddns_error_response_from_state(&state, error).await,
    }
}

#[utoipa::path(post, path = "/api/admin/ddns/config/{provider}", tag = "ddns", operation_id = "post_api_admin_ddns_config_provider", responses((status = 200, description = "Saved DDNS provider configuration")))]
async fn save_config(
    State(state): State<AppState>,
    Path(provider): Path<String>,
    Json(body): Json<ConfigBody>,
) -> Response {
    match save_primary_config(&state, &provider, body.config).await {
        Ok(()) => response::success_empty().into_response(),
        Err(error) => ddns_error_response_from_state(&state, error).await,
    }
}

#[utoipa::path(get, path = "/api/admin/ddns/targets", tag = "ddns", operation_id = "get_api_admin_ddns_targets", responses((status = 200, description = "DDNS targets")))]
async fn get_targets(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match targets_overview(&state, &translator).await {
        Ok(value) => response::ok(value).into_response(),
        Err(error) => ddns_error_response_from_state(&state, error).await,
    }
}

#[utoipa::path(get, path = "/api/admin/ddns/targets/{id}", tag = "ddns", operation_id = "get_api_admin_ddns_targets_id", responses((status = 200, description = "DDNS target")))]
async fn get_target(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let translator = Translator::from_state(&state).await;
    match target_detail(&state, &id, &translator).await {
        Ok(Some(value)) => response::ok(value).into_response(),
        Ok(None) => response::error(
            StatusCode::NOT_FOUND,
            ddns_text(&translator, "targetNotFound", &[]),
        ),
        Err(error) => ddns_error_response_from_state(&state, error).await,
    }
}

#[utoipa::path(post, path = "/api/admin/ddns/targets", tag = "ddns", operation_id = "post_api_admin_ddns_targets", responses((status = 200, description = "Created DDNS target")))]
async fn create_target(State(state): State<AppState>, Json(body): Json<TargetBody>) -> Response {
    let translator = Translator::from_state(&state).await;
    match create_ddns_target(&state, body, &translator).await {
        Ok(value) => response::ok(value).into_response(),
        Err(error) => ddns_error_response_from_state(&state, error).await,
    }
}

#[utoipa::path(put, path = "/api/admin/ddns/targets/{id}", tag = "ddns", operation_id = "put_api_admin_ddns_targets_id", responses((status = 200, description = "Updated DDNS target")))]
async fn update_target(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<TargetBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match update_ddns_target(&state, &id, body, &translator).await {
        Ok(value) => response::ok(value).into_response(),
        Err(error) => ddns_error_response_from_state(&state, error).await,
    }
}

#[utoipa::path(delete, path = "/api/admin/ddns/targets/{id}", tag = "ddns", operation_id = "delete_api_admin_ddns_targets_id", responses((status = 200, description = "Deleted DDNS target")))]
async fn delete_target(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    match delete_ddns_target(&state, &id).await {
        Ok(()) => response::success_empty().into_response(),
        Err(error) => ddns_error_response_from_state(&state, error).await,
    }
}

#[utoipa::path(post, path = "/api/admin/ddns/targets/{id}/enabled", tag = "ddns", operation_id = "post_api_admin_ddns_targets_id_enabled", responses((status = 200, description = "Updated DDNS target enabled state")))]
async fn set_target_enabled(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<TargetEnabledBody>,
) -> Response {
    match set_ddns_target_enabled(&state, &id, body.enabled).await {
        Ok(()) => response::success_empty().into_response(),
        Err(error) => ddns_error_response_from_state(&state, error).await,
    }
}

#[utoipa::path(post, path = "/api/admin/ddns/test", tag = "ddns", operation_id = "post_api_admin_ddns_test", responses((status = 200, description = "DDNS primary target test result")))]
async fn test_primary_target(State(state): State<AppState>) -> Response {
    match primary_target(&state).await {
        Ok(target) => manual_test_target(&state, &target.meta.id).await,
        Err(error) => ddns_error_response_from_state(&state, error).await,
    }
}

#[utoipa::path(post, path = "/api/admin/ddns/targets/{id}/test", tag = "ddns", operation_id = "post_api_admin_ddns_targets_id_test", responses((status = 200, description = "DDNS target test result")))]
async fn test_target(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    manual_test_target(&state, &id).await
}

#[utoipa::path(get, path = "/api/admin/ddns/logs", tag = "ddns", operation_id = "get_api_admin_ddns_logs", responses((status = 200, description = "DDNS logs")))]
async fn get_logs(State(state): State<AppState>, Query(query): Query<LogsQuery>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state
        .storage
        .store
        .list_log_buffer(
            DDNS_LOGS,
            parse_ddns_log_limit(query.limit.as_deref()),
            DDNS_LOG_MAX_LEN,
        )
        .await
    {
        Ok(lines) => response::ok(parse_log_entries(lines)).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to load DDNS logs");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ddns_text(&translator, "logsLoadFailed", &[]),
            )
        }
    }
}

async fn manual_test_target(state: &AppState, id: &str) -> Response {
    let translator = Translator::from_state(state).await;
    let target = match find_target_or_err(state, id).await {
        Ok(target) => target,
        Err(error) => return ddns_error_response(&translator, error),
    };
    let Some(provider) = target.meta.provider.as_deref() else {
        return response::error(
            StatusCode::BAD_REQUEST,
            ddns_text(&translator, "selectProviderFirst", &[]),
        );
    };
    if let Some(message) = target_config_incomplete_message(&target, &translator) {
        return response::error(StatusCode::BAD_REQUEST, message);
    }

    if let Err(error) = append_target_log(
        state,
        "info",
        &target,
        &ddns_text(&translator, "manualTestStart", &[]),
        &translator,
    )
    .await
    {
        tracing::warn!(%error, "failed to append DDNS manual test start log");
    }

    let settings = match state.storage.store.get_string_value(DDNS_SETTINGS).await {
        Ok(raw) => parse_settings(raw.as_deref()),
        Err(error) => {
            tracing::warn!(%error, "failed to load DDNS settings for manual test");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ddns_text(&translator, "settingsLoadFailed", &[]),
            );
        }
    };
    let http_options = DDNSHttpClientOptions::from_settings_and_config(&settings, &target.config);

    let update_plan =
        match prepare_ddns_provider_update(&translator, provider, &target.config, &http_options)
            .await
        {
            Ok(plan) => plan,
            Err(error) => {
                let message = error.to_string();
                let _ = set_target_last_check(state, &target, "error", &message).await;
                let _ = append_target_log(
                    state,
                    "error",
                    &target,
                    &ddns_text(&translator, "testError", &[("message", message.clone())]),
                    &translator,
                )
                .await;
                return response::error(StatusCode::INTERNAL_SERVER_ERROR, message);
            }
        };

    if let Err(error) = ensure_target_auxiliary_state(
        state,
        &target,
        &http_options,
        true,
        Some(&ddns_text(&translator, "manualTestPrefix", &[])),
        &translator,
    )
    .await
    {
        let message = error.to_string();
        let _ = set_target_last_check(state, &target, "error", &message).await;
        let _ = append_target_log(
            state,
            "error",
            &target,
            &ddns_text(&translator, "testError", &[("message", message.clone())]),
            &translator,
        )
        .await;
        return response::error(StatusCode::INTERNAL_SERVER_ERROR, message);
    }

    match resolve_target_ips(&target, &settings, &translator).await {
        Ok(ips) => {
            let current_message = ddns_text(
                &translator,
                "currentTargetIp",
                &[
                    ("source", ips.source_label.clone()),
                    (
                        "ipv4",
                        ips.ipv4
                            .clone()
                            .unwrap_or_else(|| ddns_text(&translator, "none", &[])),
                    ),
                    (
                        "ipv6",
                        ips.ipv6
                            .clone()
                            .unwrap_or_else(|| ddns_text(&translator, "none", &[])),
                    ),
                ],
            );
            if let Err(error) =
                append_target_log(state, "info", &target, &current_message, &translator).await
            {
                tracing::warn!(%error, "failed to append DDNS detected IP log");
            }
            for warning in &ips.warnings {
                if let Err(error) =
                    append_target_log(state, "warn", &target, warning, &translator).await
                {
                    tracing::warn!(%error, "failed to append DDNS warning log");
                }
            }

            let (scoped_ipv4, scoped_ipv6) =
                apply_update_scope(ips.update_scope, ips.ipv4.clone(), ips.ipv6.clone());
            if scoped_ipv4.is_none() && scoped_ipv6.is_none() {
                let message =
                    target_ip_unavailable_message(&translator, ips.source, ips.update_scope);
                let _ = set_target_last_check(state, &target, "error", &message).await;
                let _ = append_target_log(
                    state,
                    "error",
                    &target,
                    &ddns_text(&translator, "testAborted", &[("message", message.clone())]),
                    &translator,
                )
                .await;
                return response::error(StatusCode::INTERNAL_SERVER_ERROR, message);
            }

            let previous_ipv4 = target.last_ip.get("ipv4").and_then(Value::as_str);
            let previous_ipv6 = target.last_ip.get("ipv6").and_then(Value::as_str);
            let result = match execute_ddns_provider_update(
                &translator,
                provider,
                &update_plan,
                &http_options,
                scoped_ipv4.as_deref(),
                scoped_ipv6.as_deref(),
            )
            .await
            {
                Ok(result) => result,
                Err(error) => {
                    let message = error.to_string();
                    let _ = set_target_last_check(state, &target, "error", &message).await;
                    let _ = append_target_log(
                        state,
                        "error",
                        &target,
                        &ddns_text(&translator, "testError", &[("message", message.clone())]),
                        &translator,
                    )
                    .await;
                    return response::error(StatusCode::INTERNAL_SERVER_ERROR, message);
                }
            };
            emit_ddns_update_completed_event(
                state,
                &target,
                "manual_test",
                provider,
                &result,
                ips.source,
                previous_ipv4,
                previous_ipv6,
                scoped_ipv4.as_deref(),
                scoped_ipv6.as_deref(),
                &translator,
            )
            .await;
            let result_message = manual_test_result_message(&translator, &result);
            if result.success {
                let _ = set_target_last_ip(
                    state,
                    &target,
                    scoped_ipv4.as_deref(),
                    scoped_ipv6.as_deref(),
                )
                .await;
                let _ = set_target_last_check(state, &target, "updated", &result_message).await;
                let _ =
                    append_target_log(state, "info", &target, &result_message, &translator).await;
            } else {
                let _ = set_target_last_check(state, &target, "error", &result_message).await;
                let _ =
                    append_target_log(state, "error", &target, &result_message, &translator).await;
            }
            let status = if result.success {
                StatusCode::OK
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            (
                status,
                Json(json!({
                    "success": result.success,
                    "message": result.message,
                    "data": {
                        "ipv4": ips.ipv4,
                        "ipv6": ips.ipv6,
                        "source": ips.source,
                        "sourceLabel": ips.source_label,
                    }
                })),
            )
                .into_response()
        }
        Err(error) => {
            let message = error.to_string();
            let _ = set_target_last_check(state, &target, "error", &message).await;
            let _ = append_target_log(
                state,
                "error",
                &target,
                &ddns_text(&translator, "testError", &[("message", message.clone())]),
                &translator,
            )
            .await;
            response::error(StatusCode::INTERNAL_SERVER_ERROR, message)
        }
    }
}

fn manual_test_result_message(
    translator: &Translator,
    result: &DDNSProviderUpdateResult,
) -> String {
    ddns_text(
        translator,
        if result.success {
            "updateSuccess"
        } else {
            "updateFailed"
        },
        &[("message", result.message.clone())],
    )
}
