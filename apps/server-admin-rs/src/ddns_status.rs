use std::{
    collections::{BTreeSet, HashMap},
    env, fs,
    future::Future,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    time::Duration,
};

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use get_if_addrs::{IfAddr, get_if_addrs};
use hmac::{Hmac, Mac};
use serde::Deserialize;
use serde_json::{Value, json};
use sha1::Sha1;
use sha2::{Digest, Sha256};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tokio::net::lookup_host;
use tokio::time as tokio_time;
use url::Url;
use uuid::Uuid;

use crate::{i18n::Translator, response, state::AppState, system_events, time_utils};

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
const DDNS_LOG_MAX_LEN: usize = 2000;
const DDNS_UPDATE_LOCK_NAME: &str = "ddns-update";
const DDNS_UPDATE_LOCK_TTL_SECONDS: usize = 600;
const DDNS_STARTUP_CHECK_DELAY_SECONDS: u64 = 30;
const DDNS_UPDATE_SCOPE_FIELD: &str = "update_scope";
const DDNS_NETWORK_INTERFACE_FIELD: &str = "network_interface";
const DDNS_IP_SOURCE_FIELD: &str = "ip_source";
const DDNS_INTERFACE_IPV4_INDEX_FIELD: &str = "interface_ipv4_index";
const DDNS_INTERFACE_IPV6_INDEX_FIELD: &str = "interface_ipv6_index";
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
const RESPONSE_PREVIEW_MAX_LENGTH: usize = 240;

type HmacSha1 = Hmac<Sha1>;
type HmacSha256 = Hmac<Sha256>;

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
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PublicCheckTestBody {
    public_check_sources: Value,
    http_transport: Option<String>,
    network_interface: Option<String>,
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
    last_check: Value,
}

#[derive(Clone, Debug)]
struct ResolvedTargetIps {
    ipv4: Option<String>,
    ipv6: Option<String>,
    source: &'static str,
    source_label: String,
    warnings: Vec<String>,
    update_scope: &'static str,
}

#[derive(Clone, Debug)]
struct DDNSProviderUpdateResult {
    success: bool,
    message: String,
    ipv4_updated: bool,
    ipv6_updated: bool,
}

pub fn ddns_status_routes() -> Router<AppState> {
    Router::new()
        .route("/api/admin/ddns/status", get(get_status))
        .route("/api/admin/ddns/toggle", post(toggle))
        .route("/api/admin/ddns/providers", get(get_providers))
        .route(
            "/api/admin/ddns/settings",
            get(get_settings).post(update_settings),
        )
        .route(
            "/api/admin/ddns/public-check/test",
            post(test_public_check_sources),
        )
        .route("/api/admin/ddns/interfaces", get(get_interfaces))
        .route("/api/admin/ddns/provider", post(set_provider))
        .route(
            "/api/admin/ddns/config/{provider}",
            get(get_config).post(save_config),
        )
        .route(
            "/api/admin/ddns/targets",
            get(get_targets).post(create_target),
        )
        .route(
            "/api/admin/ddns/targets/{id}",
            get(get_target).put(update_target).delete(delete_target),
        )
        .route(
            "/api/admin/ddns/targets/{id}/enabled",
            post(set_target_enabled),
        )
        .route("/api/admin/ddns/test", post(test_primary_target))
        .route("/api/admin/ddns/targets/{id}/test", post(test_target))
        .route("/api/admin/ddns/logs", get(get_logs).delete(clear_logs))
        .route("/api/admin/ddns/poll", get(poll))
}

pub fn start_ddns_tasks(state: AppState) {
    tokio::spawn(async move {
        tokio_time::sleep(Duration::from_secs(DDNS_STARTUP_CHECK_DELAY_SECONDS)).await;
        if let Err(error) = run_automatic_ddns_check(&state, "startup", false, false).await {
            tracing::warn!(%error, "DDNS startup check failed");
        }

        loop {
            let interval_minutes = match ddns_update_interval_minutes(&state).await {
                Ok(value) => value,
                Err(error) => {
                    tracing::warn!(%error, "failed to load DDNS scheduler interval");
                    10
                }
            };
            tokio_time::sleep(Duration::from_secs((interval_minutes.max(1) as u64) * 60)).await;
            if let Err(error) = run_automatic_ddns_check(&state, "cron", true, false).await {
                tracing::warn!(%error, "DDNS scheduled check failed");
            }
        }
    });
}

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

async fn toggle(State(state): State<AppState>, Json(body): Json<ToggleBody>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state
        .redis
        .set_string_value(DDNS_ENABLED, if body.enabled { "true" } else { "false" })
        .await
    {
        Ok(()) => response::success_empty().into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to toggle DDNS");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ddns_text(&translator, "toggleFailed", &[]),
            )
        }
    }
}

async fn get_providers(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    response::ok(provider_catalog(&translator)).into_response()
}

async fn get_settings(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.redis.get_string_value(DDNS_SETTINGS).await {
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

async fn update_settings(
    State(state): State<AppState>,
    Json(body): Json<SettingsBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let current = match state.redis.get_string_value(DDNS_SETTINGS).await {
        Ok(raw) => parse_settings(raw.as_deref()),
        Err(error) => {
            tracing::warn!(%error, "failed to load current DDNS settings");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ddns_text(&translator, "settingsLoadFailed", &[]),
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
    let http_transport = match body.http_transport.as_deref() {
        Some("node" | "fetch") => "node",
        Some("curl") | None => current
            .get("httpTransport")
            .and_then(Value::as_str)
            .unwrap_or("curl"),
        Some(_) => "curl",
    };
    let stored = json!({
        "updateIntervalMinutes": interval,
        "publicCheckSources": public_sources,
        "httpTransport": normalize_http_transport(Some(&Value::String(http_transport.to_string())))
    });
    let serialized = serde_json::to_string(&stored).unwrap_or_default();
    match state
        .redis
        .set_string_value(DDNS_SETTINGS, &serialized)
        .await
    {
        Ok(()) => response::ok(parse_settings(Some(serialized.as_str()))).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to save DDNS settings");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ddns_text(&translator, "settingsSaveFailed", &[]),
            )
        }
    }
}

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
    let transport_value = body
        .http_transport
        .as_ref()
        .map(|value| Value::String(value.clone()));
    let transport = normalize_http_transport(transport_value.as_ref());
    match test_public_check_sources_inner(&sources, transport, &translator).await {
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

async fn get_interfaces() -> Response {
    response::ok(list_ddns_network_interfaces()).into_response()
}

async fn set_provider(State(state): State<AppState>, Json(body): Json<ProviderBody>) -> Response {
    match set_primary_provider(&state, &body.provider).await {
        Ok(()) => response::success_empty().into_response(),
        Err(error) => ddns_error_response_from_state(&state, error).await,
    }
}

async fn get_config(State(state): State<AppState>, Path(provider): Path<String>) -> Response {
    match primary_target(&state).await {
        Ok(primary) if primary.meta.provider.as_deref() == Some(provider.as_str()) => {
            response::ok(primary.config).into_response()
        }
        Ok(_) => response::ok(json!({})).into_response(),
        Err(error) => ddns_error_response_from_state(&state, error).await,
    }
}

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

async fn get_targets(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match targets_overview(&state, &translator).await {
        Ok(value) => response::ok(value).into_response(),
        Err(error) => ddns_error_response_from_state(&state, error).await,
    }
}

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

async fn create_target(State(state): State<AppState>, Json(body): Json<TargetBody>) -> Response {
    let translator = Translator::from_state(&state).await;
    match create_ddns_target(&state, body, &translator).await {
        Ok(value) => response::ok(value).into_response(),
        Err(error) => ddns_error_response_from_state(&state, error).await,
    }
}

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

async fn delete_target(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    match delete_ddns_target(&state, &id).await {
        Ok(()) => response::success_empty().into_response(),
        Err(error) => ddns_error_response_from_state(&state, error).await,
    }
}

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

async fn test_primary_target(State(state): State<AppState>) -> Response {
    match primary_target(&state).await {
        Ok(target) => manual_test_target(&state, &target.meta.id).await,
        Err(error) => ddns_error_response_from_state(&state, error).await,
    }
}

async fn test_target(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    manual_test_target(&state, &id).await
}

async fn get_logs(State(state): State<AppState>, Query(query): Query<LogsQuery>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state
        .redis
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

    let settings = match state.redis.get_string_value(DDNS_SETTINGS).await {
        Ok(raw) => parse_settings(raw.as_deref()),
        Err(error) => {
            tracing::warn!(%error, "failed to load DDNS settings for manual test");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ddns_text(&translator, "settingsLoadFailed", &[]),
            );
        }
    };

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

            let result = match update_ddns_provider(
                &translator,
                provider,
                &target.config,
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
            if result.success {
                let _ = set_target_last_ip(
                    state,
                    &target,
                    scoped_ipv4.as_deref(),
                    scoped_ipv6.as_deref(),
                )
                .await;
                let _ = set_target_last_check(state, &target, "updated", &result.message).await;
                let _ = append_target_log(
                    state,
                    "info",
                    &target,
                    &ddns_text(
                        &translator,
                        "updateSuccess",
                        &[("message", result.message.clone())],
                    ),
                    &translator,
                )
                .await;
            } else {
                let _ = set_target_last_check(state, &target, "error", &result.message).await;
                let _ = append_target_log(
                    state,
                    "error",
                    &target,
                    &ddns_text(
                        &translator,
                        "updateFailed",
                        &[("message", result.message.clone())],
                    ),
                    &translator,
                )
                .await;
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
                        "ipv4Updated": result.ipv4_updated,
                        "ipv6Updated": result.ipv6_updated,
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

async fn ddns_update_interval_minutes(state: &AppState) -> anyhow::Result<i64> {
    let raw = state.redis.get_string_value(DDNS_SETTINGS).await?;
    Ok(parse_settings(raw.as_deref())
        .get("updateIntervalMinutes")
        .and_then(Value::as_i64)
        .unwrap_or(10)
        .clamp(5, 1440))
}

async fn run_automatic_ddns_check(
    state: &AppState,
    trigger: &str,
    emit_skip_log: bool,
    emit_noop_log: bool,
) -> anyhow::Result<()> {
    if state.redis.get_string_value(DDNS_ENABLED).await?.as_deref() != Some("true") {
        return Ok(());
    }

    let lock_key = format!("fn_knock:lock:{DDNS_UPDATE_LOCK_NAME}");
    let lock_id = Uuid::new_v4().to_string();
    let acquired = state
        .redis
        .set_json_value_nx_ex(
            &lock_key,
            &json!({ "lockId": lock_id, "createdAt": time_utils::now_iso() }),
            DDNS_UPDATE_LOCK_TTL_SECONDS,
        )
        .await?;
    if !acquired {
        return Ok(());
    }

    let translator = Translator::from_state(state).await;
    let result = async {
        let targets = list_targets(state).await?;
        let settings_raw = state.redis.get_string_value(DDNS_SETTINGS).await?;
        let settings = parse_settings(settings_raw.as_deref());
        for target in targets
            .into_iter()
            .filter(|target| target.meta.is_primary || target.meta.enabled)
        {
            if let Err(error) =
                run_automatic_ddns_target(
                    state,
                    &target,
                    &settings,
                    trigger,
                    emit_skip_log,
                    emit_noop_log,
                    &translator,
                )
                    .await
            {
                let task_error = ddns_text(
                    &translator,
                    "taskError",
                    &[("message", error.to_string())],
                );
                let message = trigger_message(&translator, trigger, &task_error);
                let _ = set_target_last_check(state, &target, "error", &message).await;
                let _ =
                    append_target_log(state, "error", &target, &message, &translator).await;
                tracing::warn!(target_id = %target.meta.id, %error, "automatic DDNS target check failed");
            }
        }
        Ok::<(), anyhow::Error>(())
    }
    .await;

    if let Err(error) = state.redis.delete_lock_if_owned(&lock_key, &lock_id).await {
        tracing::warn!(%error, "failed to release DDNS update lock");
    }
    result
}

async fn run_automatic_ddns_target(
    state: &AppState,
    target: &DDNSTargetRecord,
    settings: &Value,
    trigger: &str,
    emit_skip_log: bool,
    emit_noop_log: bool,
    translator: &Translator,
) -> anyhow::Result<()> {
    let Some(provider) = target.meta.provider.as_deref() else {
        record_automatic_ddns_skip(
            state,
            target,
            trigger_message(
                translator,
                trigger,
                &ddns_text(translator, "skippedNoProvider", &[]),
            ),
            emit_skip_log,
            translator,
        )
        .await?;
        return Ok(());
    };

    if let Some(reason) = target_config_incomplete_reason(target, translator) {
        let base_message = ddns_text(translator, "skippedIncompleteConfig", &[]);
        let message = if reason.is_empty() {
            base_message
        } else {
            format!("{base_message}: {reason}")
        };
        record_automatic_ddns_skip(
            state,
            target,
            trigger_message(translator, trigger, &message),
            emit_skip_log,
            translator,
        )
        .await?;
        return Ok(());
    }

    let ips = resolve_target_ips(target, settings, translator).await?;
    for warning in &ips.warnings {
        append_target_log(
            state,
            "warn",
            target,
            &trigger_message(translator, trigger, warning),
            translator,
        )
        .await?;
    }

    if ips.source == "public" && ips.ipv4.is_none() && ips.ipv6.is_none() {
        let message = trigger_message(
            translator,
            trigger,
            &ddns_text(translator, "skippedPublicIpUnavailable", &[]),
        );
        set_target_last_check(state, target, "error", &message).await?;
        append_target_log(state, "error", target, &message, translator).await?;
        return Ok(());
    }

    let (scoped_ipv4, scoped_ipv6) =
        apply_update_scope(ips.update_scope, ips.ipv4.clone(), ips.ipv6.clone());
    if scoped_ipv4.is_none() && scoped_ipv6.is_none() {
        let reason = target_ip_unavailable_message(translator, ips.source, ips.update_scope);
        let skipped = ddns_text(translator, "skippedReason", &[("reason", reason)]);
        let message = trigger_message(translator, trigger, &skipped);
        set_target_last_check(state, target, "skipped", &message).await?;
        append_target_log(state, "warn", target, &message, translator).await?;
        return Ok(());
    }

    let previous_ipv4 = target.last_ip.get("ipv4").and_then(Value::as_str);
    let previous_ipv6 = target.last_ip.get("ipv6").and_then(Value::as_str);
    let ipv4_changed = scoped_ipv4
        .as_deref()
        .is_some_and(|value| Some(value) != previous_ipv4);
    let ipv6_changed = scoped_ipv6
        .as_deref()
        .is_some_and(|value| Some(value) != previous_ipv6);

    if !ipv4_changed && !ipv6_changed {
        let message = trigger_message(
            translator,
            trigger,
            &ddns_text(translator, "targetIpNoChange", &[]),
        );
        set_target_last_check(state, target, "noop", &message).await?;
        if emit_noop_log {
            append_target_log(state, "info", target, &message, translator).await?;
        }
        return Ok(());
    }

    let mut changes = Vec::new();
    if ipv4_changed {
        changes.push(ddns_text(
            translator,
            "ipChange",
            &[
                ("family", "IPv4".to_string()),
                (
                    "before",
                    previous_ipv4
                        .map(str::to_string)
                        .unwrap_or_else(|| ddns_text(translator, "none", &[])),
                ),
                (
                    "after",
                    scoped_ipv4
                        .clone()
                        .unwrap_or_else(|| ddns_text(translator, "none", &[])),
                ),
            ],
        ));
    }
    if ipv6_changed {
        changes.push(ddns_text(
            translator,
            "ipChange",
            &[
                ("family", "IPv6".to_string()),
                (
                    "before",
                    previous_ipv6
                        .map(str::to_string)
                        .unwrap_or_else(|| ddns_text(translator, "none", &[])),
                ),
                (
                    "after",
                    scoped_ipv6
                        .clone()
                        .unwrap_or_else(|| ddns_text(translator, "none", &[])),
                ),
            ],
        ));
    }
    append_target_log(
        state,
        "info",
        target,
        &trigger_message(
            translator,
            trigger,
            &ddns_text(
                translator,
                "targetIpChanged",
                &[("changes", changes.join(", "))],
            ),
        ),
        translator,
    )
    .await?;

    let result = update_ddns_provider(
        translator,
        provider,
        &target.config,
        scoped_ipv4.as_deref(),
        scoped_ipv6.as_deref(),
    )
    .await?;

    emit_ddns_update_completed_event(
        state,
        target,
        trigger,
        provider,
        &result,
        ips.source,
        previous_ipv4,
        previous_ipv6,
        scoped_ipv4.as_deref(),
        scoped_ipv6.as_deref(),
        translator,
    )
    .await;

    if result.success {
        let message = trigger_message(
            translator,
            trigger,
            &ddns_text(
                translator,
                "dnsUpdateSuccess",
                &[
                    ("provider", provider.to_string()),
                    ("message", result.message.clone()),
                ],
            ),
        );
        set_target_last_ip(
            state,
            target,
            scoped_ipv4.as_deref(),
            scoped_ipv6.as_deref(),
        )
        .await?;
        set_target_last_check(state, target, "updated", &message).await?;
        append_target_log(state, "info", target, &message, translator).await?;
    } else {
        let message = trigger_message(
            translator,
            trigger,
            &ddns_text(
                translator,
                "dnsUpdateFailed",
                &[
                    ("provider", provider.to_string()),
                    ("message", result.message.clone()),
                ],
            ),
        );
        set_target_last_check(state, target, "error", &message).await?;
        append_target_log(state, "error", target, &message, translator).await?;
    }
    Ok(())
}

async fn record_automatic_ddns_skip(
    state: &AppState,
    target: &DDNSTargetRecord,
    message: String,
    emit_log: bool,
    translator: &Translator,
) -> anyhow::Result<()> {
    set_target_last_check(state, target, "skipped", &message).await?;
    if emit_log {
        append_target_log(state, "warn", target, &message, translator).await?;
    }
    Ok(())
}

async fn emit_ddns_update_completed_event(
    state: &AppState,
    target: &DDNSTargetRecord,
    trigger: &str,
    provider: &str,
    result: &DDNSProviderUpdateResult,
    ip_source: &str,
    previous_ipv4: Option<&str>,
    previous_ipv6: Option<&str>,
    next_ipv4: Option<&str>,
    next_ipv6: Option<&str>,
    translator: &Translator,
) {
    let summary = target_summary(target, translator);
    if let Err(error) = system_events::publish_ddns_update_completed_event(
        state,
        json!({
            "trigger": trigger,
            "target_id": target.meta.id,
            "target_name": summary.get("name").and_then(Value::as_str).unwrap_or(&target.meta.name),
            "domain_summary": summary.get("domainSummary").and_then(Value::as_str).unwrap_or(""),
            "is_primary": target.meta.is_primary,
            "provider": provider,
            "success": result.success,
            "message": result.message,
            "update_scope": normalize_update_scope(target.config.get(DDNS_UPDATE_SCOPE_FIELD).map(String::as_str)),
            "ip_source": ip_source,
            "previous_ipv4": previous_ipv4,
            "previous_ipv6": previous_ipv6,
            "next_ipv4": next_ipv4,
            "next_ipv6": next_ipv6,
        }),
    )
    .await
    {
        tracing::warn!(%error, "failed to publish DDNS update completed event");
    }
}

fn trigger_message(translator: &Translator, trigger: &str, message: &str) -> String {
    ddns_text(
        translator,
        "triggerMessage",
        &[
            ("trigger", trigger_label(translator, trigger)),
            ("message", message.to_string()),
        ],
    )
}

fn trigger_label(translator: &Translator, trigger: &str) -> String {
    match trigger {
        "startup" => ddns_text(translator, "triggerStartup", &[]),
        "enable" => ddns_text(translator, "triggerEnable", &[]),
        _ => ddns_text(translator, "triggerCron", &[]),
    }
}

async fn resolve_target_ips(
    target: &DDNSTargetRecord,
    settings: &Value,
    translator: &Translator,
) -> anyhow::Result<ResolvedTargetIps> {
    let update_scope = normalize_update_scope(
        target
            .config
            .get(DDNS_UPDATE_SCOPE_FIELD)
            .map(String::as_str),
    );
    let source = normalize_ip_source(target.config.get(DDNS_IP_SOURCE_FIELD).map(String::as_str));
    let (enable_ipv4, enable_ipv6) = update_scope_flags(update_scope);

    match source {
        "static" => Ok(ResolvedTargetIps {
            ipv4: if enable_ipv4 {
                resolve_static_address(
                    target
                        .config
                        .get(DDNS_STATIC_IPV4_FIELD)
                        .map(String::as_str),
                    4,
                    translator,
                )?
            } else {
                None
            },
            ipv6: if enable_ipv6 {
                resolve_static_address(
                    target
                        .config
                        .get(DDNS_STATIC_IPV6_FIELD)
                        .map(String::as_str),
                    6,
                    translator,
                )?
            } else {
                None
            },
            source,
            source_label: ddns_text(translator, "staticSourceLabel", &[]),
            warnings: Vec::new(),
            update_scope,
        }),
        "domain" => {
            let domain = normalize_domain(
                target
                    .config
                    .get(DDNS_SOURCE_DOMAIN_FIELD)
                    .map(String::as_str)
                    .unwrap_or(""),
            );
            let (ipv4, ipv6) = resolve_source_domain_addresses(&domain, translator).await?;
            Ok(ResolvedTargetIps {
                ipv4: enable_ipv4.then_some(ipv4).flatten(),
                ipv6: enable_ipv6.then_some(ipv6).flatten(),
                source,
                source_label: if domain.is_empty() {
                    ddns_text(translator, "domainSourceLabelEmpty", &[])
                } else {
                    ddns_text(
                        translator,
                        "domainSourceLabel",
                        &[("domain", domain.clone())],
                    )
                },
                warnings: Vec::new(),
                update_scope,
            })
        }
        "interface" => {
            let interface = normalize_network_interface(
                target
                    .config
                    .get(DDNS_NETWORK_INTERFACE_FIELD)
                    .map(String::as_str),
            );
            if interface.is_empty() {
                anyhow::bail!("{}", ddns_text(translator, "interfaceRequired", &[]));
            }
            Ok(ResolvedTargetIps {
                ipv4: if enable_ipv4 {
                    select_interface_address(
                        &interface,
                        "ipv4",
                        target
                            .config
                            .get(DDNS_INTERFACE_IPV4_INDEX_FIELD)
                            .map(String::as_str),
                        translator,
                    )?
                } else {
                    None
                },
                ipv6: if enable_ipv6 {
                    select_interface_address(
                        &interface,
                        "ipv6",
                        target
                            .config
                            .get(DDNS_INTERFACE_IPV6_INDEX_FIELD)
                            .map(String::as_str),
                        translator,
                    )?
                } else {
                    None
                },
                source,
                source_label: ddns_text(translator, "interfaceSourceLabel", &[("name", interface)]),
                warnings: Vec::new(),
                update_scope,
            })
        }
        _ => {
            let sources = settings
                .get("publicCheckSources")
                .map(normalize_public_check_sources)
                .unwrap_or_else(default_public_check_sources);
            let results = test_public_check_sources_inner(
                &sources,
                settings
                    .get("httpTransport")
                    .and_then(Value::as_str)
                    .unwrap_or("curl"),
                translator,
            )
            .await?;
            let ipv4 = if enable_ipv4 {
                first_successful_public_ip(&results, "ipv4")
            } else {
                None
            };
            let ipv6 = if enable_ipv6 {
                first_successful_public_ip(&results, "ipv6")
            } else {
                None
            };
            let mut warnings = Vec::new();
            if enable_ipv4
                && ipv4.is_none()
                && let Some(message) = first_public_check_error(&results, "ipv4")
            {
                warnings.push(ddns_text(translator, "ipv4Failed", &[("error", message)]));
            }
            if enable_ipv6
                && ipv6.is_none()
                && let Some(message) = first_public_check_error(&results, "ipv6")
            {
                warnings.push(ddns_text(translator, "ipv6Failed", &[("error", message)]));
            }
            Ok(ResolvedTargetIps {
                ipv4,
                ipv6,
                source,
                source_label: ddns_text(translator, "publicSourceLabel", &[]),
                warnings,
                update_scope,
            })
        }
    }
}

fn update_scope_flags(scope: &str) -> (bool, bool) {
    match scope {
        "ipv4_only" => (true, false),
        "ipv6_only" => (false, true),
        _ => (true, true),
    }
}

fn apply_update_scope(
    scope: &str,
    ipv4: Option<String>,
    ipv6: Option<String>,
) -> (Option<String>, Option<String>) {
    match scope {
        "ipv4_only" => (ipv4, None),
        "ipv6_only" => (None, ipv6),
        _ => (ipv4, ipv6),
    }
}

fn resolve_static_address(
    value: Option<&str>,
    family: u8,
    translator: &Translator,
) -> anyhow::Result<Option<String>> {
    let value = value.unwrap_or("").trim();
    if value.is_empty() {
        return Ok(None);
    }
    let ip = value.parse::<IpAddr>().map_err(|_| {
        anyhow::anyhow!(ddns_text(
            translator,
            if family == 4 {
                "staticIpv4Invalid"
            } else {
                "staticIpv6Invalid"
            },
            &[("value", value.to_string())],
        ))
    })?;
    match (family, ip) {
        (4, IpAddr::V4(_)) | (6, IpAddr::V6(_)) => Ok(Some(value.to_string())),
        (4, _) => anyhow::bail!(
            "{}",
            ddns_text(
                translator,
                "staticIpv4Invalid",
                &[("value", value.to_string())],
            )
        ),
        (6, _) => anyhow::bail!(
            "{}",
            ddns_text(
                translator,
                "staticIpv6Invalid",
                &[("value", value.to_string())],
            )
        ),
        _ => Ok(None),
    }
}

async fn resolve_source_domain_addresses(
    domain: &str,
    translator: &Translator,
) -> anyhow::Result<(Option<String>, Option<String>)> {
    if domain.is_empty() {
        anyhow::bail!("{}", ddns_text(translator, "sourceDomainRequired", &[]));
    }
    if !is_valid_source_domain(domain) {
        anyhow::bail!(
            "{}",
            ddns_text(
                translator,
                "sourceDomainInvalid",
                &[("domain", domain.to_string())],
            )
        );
    }
    let mut ipv4 = None;
    let mut ipv6 = None;
    for addr in lookup_host((domain, 0)).await.map_err(|error| {
        anyhow::anyhow!(ddns_text(
            translator,
            "sourceDomainResolveFailed",
            &[("domain", domain.to_string()), ("error", error.to_string()),],
        ))
    })? {
        match addr.ip() {
            IpAddr::V4(ip) if ipv4.is_none() => ipv4 = Some(ip.to_string()),
            IpAddr::V6(ip) if ipv6.is_none() => ipv6 = Some(ip.to_string()),
            _ => {}
        }
    }
    Ok((ipv4, ipv6))
}

fn is_valid_source_domain(domain: &str) -> bool {
    if domain.is_empty()
        || domain.len() > 253
        || domain.starts_with("http://")
        || domain.starts_with("https://")
        || domain.contains('/')
        || domain.contains(':')
        || domain.contains('*')
        || domain.chars().any(char::is_whitespace)
    {
        return false;
    }
    domain.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
    })
}

fn select_interface_address(
    interface: &str,
    family: &str,
    index: Option<&str>,
    translator: &Translator,
) -> anyhow::Result<Option<String>> {
    let Some(item) = list_ddns_network_interfaces()
        .into_iter()
        .find(|item| item.get("name").and_then(Value::as_str) == Some(interface))
    else {
        anyhow::bail!(
            "{}",
            ddns_text(
                translator,
                "interfaceNotFound",
                &[("name", interface.to_string())],
            )
        );
    };
    let candidates = item
        .get("selectableAddresses")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|item| item.get("family").and_then(Value::as_str) == Some(family))
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Ok(None);
    }
    let raw_index = index.unwrap_or("").trim();
    if raw_index.is_empty() {
        anyhow::bail!(
            "{}",
            ddns_text(
                translator,
                "selectInterfaceAddress",
                &[(
                    "family",
                    if family == "ipv4" { "IPv4" } else { "IPv6" }.to_string(),
                )],
            )
        );
    }
    let index = raw_index.parse::<usize>().map_err(|_| {
        anyhow::anyhow!(ddns_text(
            translator,
            "selectedInterfaceAddressUnavailable",
            &[
                ("index", raw_index.to_string()),
                (
                    "family",
                    if family == "ipv4" { "IPv4" } else { "IPv6" }.to_string()
                ),
            ]
        ))
    })?;
    candidates
        .get(index)
        .and_then(|item| item.get("address").and_then(Value::as_str))
        .map(|value| Some(value.to_string()))
        .ok_or_else(|| {
            anyhow::anyhow!(ddns_text(
                translator,
                "selectedInterfaceAddressUnavailable",
                &[
                    ("index", (index + 1).to_string()),
                    (
                        "family",
                        if family == "ipv4" { "IPv4" } else { "IPv6" }.to_string(),
                    ),
                ],
            ))
        })
}

fn first_successful_public_ip(results: &[Value], family: &str) -> Option<String> {
    results
        .iter()
        .find(|item| {
            item.get("family").and_then(Value::as_str) == Some(family)
                && item.get("success").and_then(Value::as_bool) == Some(true)
        })
        .and_then(|item| item.get("ip").and_then(Value::as_str))
        .map(str::to_string)
}

fn first_public_check_error(results: &[Value], family: &str) -> Option<String> {
    results
        .iter()
        .find(|item| item.get("family").and_then(Value::as_str) == Some(family))
        .and_then(|item| item.get("error").and_then(Value::as_str))
        .map(str::to_string)
}

fn target_ip_unavailable_message(translator: &Translator, source: &str, scope: &str) -> String {
    let key = match (source, scope) {
        ("static", "ipv6_only") => "staticIpv6Unavailable",
        ("static", "ipv4_only") => "staticIpv4Unavailable",
        ("static", _) => "staticDualStackUnavailable",
        ("domain", "ipv6_only") => "domainIpv6Unavailable",
        ("domain", "ipv4_only") => "domainIpv4Unavailable",
        ("domain", _) => "domainDualStackUnavailable",
        ("interface", "ipv6_only") => "interfaceIpv6Unavailable",
        ("interface", "ipv4_only") => "interfaceIpv4Unavailable",
        ("interface", _) => "interfaceDualStackUnavailable",
        (_, "ipv6_only") => "publicIpv6Unavailable",
        (_, "ipv4_only") => "publicIpv4Unavailable",
        _ => "publicDualStackUnavailable",
    };
    ddns_text(translator, key, &[])
}

fn target_config_incomplete_reason(
    target: &DDNSTargetRecord,
    translator: &Translator,
) -> Option<String> {
    let provider_name = target.meta.provider.as_deref()?;
    let providers = provider_catalog(translator);
    let Some(provider) = providers
        .as_array()?
        .iter()
        .find(|provider| provider.get("name").and_then(Value::as_str) == Some(provider_name))
    else {
        return Some(ddns_text(translator, "notConfigured", &[]));
    };
    let missing = provider
        .get("fields")
        .and_then(Value::as_array)?
        .iter()
        .filter(|field| field.get("required").and_then(Value::as_bool) != Some(false))
        .filter_map(|field| {
            let key = field.get("key").and_then(Value::as_str)?;
            let value = target
                .config
                .get(key)
                .map(String::as_str)
                .unwrap_or("")
                .trim();
            value.is_empty().then(|| {
                field
                    .get("label")
                    .and_then(Value::as_str)
                    .unwrap_or(key)
                    .to_string()
            })
        })
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        let provider_key = ddns_provider_i18n_key(provider_name);
        return Some(format!(
            "{}: {}",
            ddns_text(
                translator,
                &format!("providers.{provider_key}.configIncomplete"),
                &[],
            ),
            missing.join(", ")
        ));
    }

    let update_scope = normalize_update_scope(
        target
            .config
            .get(DDNS_UPDATE_SCOPE_FIELD)
            .map(String::as_str),
    );
    let address_mode = provider
        .pointer("/capabilities/addressMode")
        .and_then(Value::as_str);
    if address_mode == Some("single_address") && update_scope == "dual_stack" {
        let provider_label = provider
            .get("label")
            .and_then(Value::as_str)
            .unwrap_or(provider_name);
        return Some(ddns_text(
            translator,
            "singleAddressProviderUnsupported",
            &[("provider", provider_label.to_string())],
        ));
    }

    target_runtime_config_incomplete_reason(target, update_scope, translator)
}

fn target_runtime_config_incomplete_reason(
    target: &DDNSTargetRecord,
    update_scope: &str,
    translator: &Translator,
) -> Option<String> {
    let ip_source =
        normalize_ip_source(target.config.get(DDNS_IP_SOURCE_FIELD).map(String::as_str));
    match ip_source {
        "static" => static_config_incomplete_reason(target, update_scope, translator),
        "domain" => {
            let domain = normalize_domain(
                target
                    .config
                    .get(DDNS_SOURCE_DOMAIN_FIELD)
                    .map(String::as_str)
                    .unwrap_or(""),
            );
            domain
                .is_empty()
                .then(|| ddns_text(translator, "sourceDomainRequired", &[]))
        }
        "interface" => interface_config_incomplete_reason(target, update_scope, translator),
        _ => None,
    }
}

fn static_config_incomplete_reason(
    target: &DDNSTargetRecord,
    update_scope: &str,
    translator: &Translator,
) -> Option<String> {
    let ipv4 = target
        .config
        .get(DDNS_STATIC_IPV4_FIELD)
        .map(String::as_str)
        .unwrap_or("")
        .trim();
    let ipv6 = target
        .config
        .get(DDNS_STATIC_IPV6_FIELD)
        .map(String::as_str)
        .unwrap_or("")
        .trim();
    let has_valid_ipv4 = ipv4.parse::<IpAddr>().is_ok_and(|ip| ip.is_ipv4());
    let has_valid_ipv6 = ipv6.parse::<IpAddr>().is_ok_and(|ip| ip.is_ipv6());

    if !ipv4.is_empty() && !has_valid_ipv4 {
        return Some(ddns_text(
            translator,
            "staticIpv4Invalid",
            &[("value", ipv4.to_string())],
        ));
    }
    if !ipv6.is_empty() && !has_valid_ipv6 {
        return Some(ddns_text(
            translator,
            "staticIpv6Invalid",
            &[("value", ipv6.to_string())],
        ));
    }

    match update_scope {
        "ipv4_only" if !has_valid_ipv4 => Some(ddns_text(translator, "staticIpv4Unavailable", &[])),
        "ipv6_only" if !has_valid_ipv6 => Some(ddns_text(translator, "staticIpv6Unavailable", &[])),
        "dual_stack" if !has_valid_ipv4 && !has_valid_ipv6 => {
            Some(ddns_text(translator, "staticDualStackUnavailable", &[]))
        }
        _ => None,
    }
}

fn interface_config_incomplete_reason(
    target: &DDNSTargetRecord,
    update_scope: &str,
    translator: &Translator,
) -> Option<String> {
    let interface = normalize_network_interface(
        target
            .config
            .get(DDNS_NETWORK_INTERFACE_FIELD)
            .map(String::as_str),
    );
    if interface.is_empty() {
        return Some(ddns_text(translator, "interfaceRequired", &[]));
    }

    let Some(network) = list_ddns_network_interfaces()
        .into_iter()
        .find(|item| item.get("name").and_then(Value::as_str) == Some(interface.as_str()))
    else {
        return Some(ddns_text(
            translator,
            "interfaceNotFound",
            &[("name", interface)],
        ));
    };

    let requires_ipv4 = update_scope != "ipv6_only";
    let requires_ipv6 = update_scope != "ipv4_only";
    if requires_ipv4
        && let Some(reason) =
            selected_interface_address_incomplete_reason(target, &network, "ipv4", translator)
    {
        return Some(reason);
    }
    if requires_ipv6 {
        selected_interface_address_incomplete_reason(target, &network, "ipv6", translator)
    } else {
        None
    }
}

fn selected_interface_address_incomplete_reason(
    target: &DDNSTargetRecord,
    network: &Value,
    family: &str,
    translator: &Translator,
) -> Option<String> {
    let candidates = network
        .get("selectableAddresses")
        .and_then(Value::as_array)
        .map(|addresses| {
            addresses
                .iter()
                .filter(|item| item.get("family").and_then(Value::as_str) == Some(family))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if candidates.is_empty() {
        return None;
    }

    let index_field = if family == "ipv4" {
        DDNS_INTERFACE_IPV4_INDEX_FIELD
    } else {
        DDNS_INTERFACE_IPV6_INDEX_FIELD
    };
    let index = normalize_interface_index(target.config.get(index_field).map(String::as_str));
    let family_label = if family == "ipv4" { "IPv4" } else { "IPv6" }.to_string();
    if index.is_empty() {
        return Some(ddns_text(
            translator,
            "selectInterfaceAddress",
            &[("family", family_label)],
        ));
    }

    let index = index.parse::<usize>().unwrap_or(usize::MAX);
    if candidates.get(index).is_some() {
        None
    } else {
        Some(ddns_text(
            translator,
            "selectedInterfaceAddressUnavailable",
            &[("index", (index + 1).to_string()), ("family", family_label)],
        ))
    }
}

fn target_config_incomplete_message(
    target: &DDNSTargetRecord,
    translator: &Translator,
) -> Option<String> {
    target_config_incomplete_reason(target, translator).map(|reason| {
        let base_key = if target.meta.is_primary {
            "primaryConfigIncomplete"
        } else {
            "targetConfigIncomplete"
        };
        let base_message = ddns_text(translator, base_key, &[]);
        if reason.is_empty() {
            base_message
        } else {
            format!("{base_message}: {reason}")
        }
    })
}

async fn append_target_log(
    state: &AppState,
    level: &str,
    target: &DDNSTargetRecord,
    message: &str,
    translator: &Translator,
) -> anyhow::Result<()> {
    let summary = target_summary(target, translator);
    let name = summary
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let entry = json!({
        "time": time_utils::now_iso(),
        "level": level,
        "message": format!("{} {message}", target_log_label(target, &summary, translator)),
        "targetId": target.meta.id,
        "targetName": name,
        "provider": target.meta.provider,
        "isPrimary": target.meta.is_primary
    });
    state
        .redis
        .append_log_buffer(
            DDNS_LOGS,
            &[serde_json::to_string(&entry)?],
            DDNS_LOG_TTL_SECONDS,
            DDNS_LOG_MAX_LEN,
        )
        .await?;
    Ok(())
}

fn target_log_label(target: &DDNSTargetRecord, summary: &Value, translator: &Translator) -> String {
    let scope = if target.meta.is_primary {
        ddns_text(translator, "primaryDomainScope", &[])
    } else {
        ddns_text(translator, "additionalDomainScope", &[])
    };
    let provider = provider_label(target.meta.provider.as_deref(), translator);
    let domain = summary
        .get("domainSummary")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .or_else(|| summary.get("name").and_then(Value::as_str))
        .unwrap_or("");
    if domain.is_empty() {
        format!("[{scope}][{provider}]")
    } else {
        format!("[{scope}][{provider}][{domain}]")
    }
}

async fn set_target_last_check(
    state: &AppState,
    target: &DDNSTargetRecord,
    outcome: &str,
    message: &str,
) -> anyhow::Result<()> {
    let payload = HashMap::from([
        ("checked_at".to_string(), time_utils::now_iso()),
        ("outcome".to_string(), outcome.to_string()),
        ("message".to_string(), message.to_string()),
    ]);
    state
        .redis
        .replace_hash_string_map(&target_last_check_key(&target.meta.id), &payload)
        .await?;
    if target.meta.is_primary {
        state
            .redis
            .replace_hash_string_map(DDNS_LEGACY_LAST_CHECK, &payload)
            .await?;
    }
    Ok(())
}

async fn set_target_last_ip(
    state: &AppState,
    target: &DDNSTargetRecord,
    ipv4: Option<&str>,
    ipv6: Option<&str>,
) -> anyhow::Result<()> {
    let mut payload = HashMap::new();
    if let Some(value) = target.last_ip.get("ipv4").and_then(Value::as_str) {
        payload.insert("ipv4".to_string(), value.to_string());
    }
    if let Some(value) = target.last_ip.get("ipv6").and_then(Value::as_str) {
        payload.insert("ipv6".to_string(), value.to_string());
    }
    if let Some(value) = ipv4 {
        payload.insert("ipv4".to_string(), value.to_string());
    }
    if let Some(value) = ipv6 {
        payload.insert("ipv6".to_string(), value.to_string());
    }
    payload.insert("updated_at".to_string(), time_utils::now_iso());
    state
        .redis
        .replace_hash_string_map(&target_last_ip_key(&target.meta.id), &payload)
        .await?;
    if target.meta.is_primary {
        state
            .redis
            .replace_hash_string_map(DDNS_LEGACY_LAST_IP, &payload)
            .await?;
    }
    Ok(())
}

async fn update_ddns_provider(
    translator: &Translator,
    provider: &str,
    config: &HashMap<String, String>,
    ipv4: Option<&str>,
    ipv6: Option<&str>,
) -> anyhow::Result<DDNSProviderUpdateResult> {
    match provider {
        "alidns" => update_alidns(translator, config, ipv4, ipv6).await,
        "baiducloud" => update_baiducloud(translator, config, ipv4, ipv6).await,
        "cloudflare" => update_cloudflare(translator, config, ipv4, ipv6).await,
        "dnspod" => update_dnspod(translator, config, ipv4, ipv6).await,
        "duckdns" => update_duckdns(translator, config, ipv4, ipv6).await,
        "dynu" => update_dynu(translator, config, ipv4, ipv6).await,
        "edgeone" => update_edgeone(translator, config, ipv4, ipv6).await,
        "edgeone_cname" => update_edgeone_cname(translator, config, ipv4, ipv6).await,
        "esa" => update_esa(translator, config, ipv4, ipv6).await,
        "godaddy" => update_godaddy(translator, config, ipv4, ipv6).await,
        "huaweicloud" => update_huaweicloud(translator, config, ipv4, ipv6).await,
        "noip" => update_noip(translator, config, ipv4, ipv6).await,
        "porkbun" => update_porkbun(translator, config, ipv4, ipv6).await,
        "tencentcloud" => update_tencentcloud(translator, config, ipv4, ipv6).await,
        "dynv6" => update_dynv6(translator, config, ipv4, ipv6).await,
        other => Ok(DDNSProviderUpdateResult {
            success: false,
            message: ddns_text(
                translator,
                "unknownProvider",
                &[("provider", other.to_string())],
            ),
            ipv4_updated: false,
            ipv6_updated: false,
        }),
    }
}

async fn update_alidns(
    translator: &Translator,
    config: &HashMap<String, String>,
    ipv4: Option<&str>,
    ipv6: Option<&str>,
) -> anyhow::Result<DDNSProviderUpdateResult> {
    let access_key_id = config_value(config, "access_key_id");
    let access_key_secret = config_value(config, "access_key_secret");
    let root_domain = config_value(config, "root_domain");
    let domain = config_value(config, "domain");
    if access_key_id.is_empty()
        || access_key_secret.is_empty()
        || root_domain.is_empty()
        || domain.is_empty()
    {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.alidns.configIncomplete",
            &[],
        )));
    }
    let ttl = positive_i64(config.get("ttl"), 600).to_string();
    let line = default_string(config_value(config, "line"), "default");
    let parsed = split_domain(translator, &domain, &root_domain)?;
    let client = ddns_http_client()?;
    let request_failed = ddns_text(translator, "providers.alidns.requestFailed", &[]);
    let update_failed = ddns_text(translator, "providers.alidns.updateFailed", &[]);
    let create_failed = ddns_text(translator, "providers.alidns.createFailed", &[]);
    let provider_label_text = provider_label(Some("alidns"), translator);

    update_dual_stack(
        translator,
        &provider_label_text,
        ipv4,
        ipv6,
        |record_type, ip| {
            let client = client.clone();
            let access_key_id = access_key_id.clone();
            let access_key_secret = access_key_secret.clone();
            let parsed = parsed.clone();
            let ttl = ttl.clone();
            let line = line.clone();
            let request_failed = request_failed.clone();
            let update_failed = update_failed.clone();
            let create_failed = create_failed.clone();
            async move {
                let records = alidns_request(
                    translator,
                    &client,
                    &access_key_id,
                    &access_key_secret,
                    vec![
                        ("Action", "DescribeSubDomainRecords".to_string()),
                        ("DomainName", parsed.root_domain.clone()),
                        ("Line", line.clone()),
                        ("PageSize", "100".to_string()),
                        ("SubDomain", parsed.fqdn.clone()),
                        ("Type", record_type.to_string()),
                    ],
                )
                .await?;
                if let Some(code) = json_text(&records, "Code") {
                    return Err(anyhow::anyhow!(
                        "{code}: {}",
                        json_text(&records, "Message").unwrap_or_else(|| request_failed.clone())
                    ));
                }
                let existing = records
                    .pointer("/DomainRecords/Record")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter(|record| {
                        record
                            .get("RR")
                            .and_then(Value::as_str)
                            .unwrap_or(&parsed.record_name)
                            == parsed.record_name
                            && record
                                .get("Type")
                                .and_then(Value::as_str)
                                .unwrap_or(record_type)
                                == record_type
                            && record
                                .get("Line")
                                .and_then(Value::as_str)
                                .unwrap_or("default")
                                == line
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                if !existing.is_empty() {
                    for record in existing {
                        if record.get("Value").and_then(Value::as_str) == Some(ip.as_str()) {
                            continue;
                        }
                        let record_id =
                            record
                                .get("RecordId")
                                .and_then(Value::as_str)
                                .ok_or_else(|| {
                                    anyhow::anyhow!(ddns_text(
                                        translator,
                                        "providers.alidns.recordIdMissing",
                                        &[],
                                    ))
                                })?;
                        let result = alidns_request(
                            translator,
                            &client,
                            &access_key_id,
                            &access_key_secret,
                            vec![
                                ("Action", "UpdateDomainRecord".to_string()),
                                ("Line", line.clone()),
                                ("RR", parsed.record_name.clone()),
                                ("RecordId", record_id.to_string()),
                                ("TTL", ttl.clone()),
                                ("Type", record_type.to_string()),
                                ("Value", ip.clone()),
                            ],
                        )
                        .await?;
                        if result.get("RecordId").and_then(Value::as_str).is_none() {
                            return Err(anyhow::anyhow!(
                                "{}: {}",
                                json_text(&result, "Code").unwrap_or_else(|| update_failed.clone()),
                                json_text(&result, "Message")
                                    .unwrap_or_else(|| update_failed.clone())
                            ));
                        }
                    }
                    return Ok(());
                }
                let result = alidns_request(
                    translator,
                    &client,
                    &access_key_id,
                    &access_key_secret,
                    vec![
                        ("Action", "AddDomainRecord".to_string()),
                        ("DomainName", parsed.root_domain.clone()),
                        ("Line", line.clone()),
                        ("RR", parsed.record_name.clone()),
                        ("TTL", ttl.clone()),
                        ("Type", record_type.to_string()),
                        ("Value", ip),
                    ],
                )
                .await?;
                if result.get("RecordId").and_then(Value::as_str).is_some() {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!(
                        "{}: {}",
                        json_text(&result, "Code").unwrap_or_else(|| create_failed.clone()),
                        json_text(&result, "Message").unwrap_or_else(|| create_failed.clone())
                    ))
                }
            }
        },
    )
    .await
}

async fn update_dnspod(
    translator: &Translator,
    config: &HashMap<String, String>,
    ipv4: Option<&str>,
    ipv6: Option<&str>,
) -> anyhow::Result<DDNSProviderUpdateResult> {
    let token_id = config_value(config, "token_id");
    let token_key = config_value(config, "token_key");
    let root_domain = config_value(config, "root_domain");
    let domain = config_value(config, "domain");
    if token_id.is_empty() || token_key.is_empty() || root_domain.is_empty() || domain.is_empty() {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.dnspod.configIncomplete",
            &[],
        )));
    }
    let ttl = positive_i64(config.get("ttl"), 600).to_string();
    let record_line = default_string(config_value(config, "record_line"), "默认");
    let parsed = split_domain(translator, &domain, &root_domain)?;
    let client = ddns_http_client()?;
    let query_failed = ddns_text(translator, "providers.dnspod.queryRecordFailed", &[]);
    let update_failed = ddns_text(translator, "providers.dnspod.updateRecordFailed", &[]);
    let create_failed = ddns_text(translator, "providers.dnspod.createRecordFailed", &[]);
    let provider_label_text = provider_label(Some("dnspod"), translator);

    update_dual_stack(
        translator,
        &provider_label_text,
        ipv4,
        ipv6,
        |record_type, ip| {
            let client = client.clone();
            let token_id = token_id.clone();
            let token_key = token_key.clone();
            let parsed = parsed.clone();
            let ttl = ttl.clone();
            let record_line = record_line.clone();
            let query_failed = query_failed.clone();
            let update_failed = update_failed.clone();
            let create_failed = create_failed.clone();
            async move {
                let list = dnspod_request(
                    translator,
                    &client,
                    "https://dnsapi.cn/Record.List",
                    &token_id,
                    &token_key,
                    vec![
                        ("domain", parsed.root_domain.clone()),
                        ("sub_domain", parsed.record_name.clone()),
                        ("record_type", record_type.to_string()),
                        ("record_line", record_line.clone()),
                    ],
                )
                .await?;
                if list.pointer("/status/code").and_then(Value::as_str) != Some("1") {
                    return Err(anyhow::anyhow!(
                        "{}",
                        list.pointer("/status/message")
                            .and_then(Value::as_str)
                            .unwrap_or(query_failed.as_str())
                    ));
                }
                let record = list
                    .get("records")
                    .and_then(Value::as_array)
                    .and_then(|records| records.first())
                    .cloned();
                if let Some(record) = record {
                    if record.get("value").and_then(Value::as_str) == Some(ip.as_str()) {
                        return Ok(());
                    }
                    let record_id = record
                        .get("id")
                        .and_then(Value::as_str)
                        .ok_or_else(|| anyhow::anyhow!(update_failed.clone()))?;
                    let result = dnspod_request(
                        translator,
                        &client,
                        "https://dnsapi.cn/Record.Modify",
                        &token_id,
                        &token_key,
                        vec![
                            ("domain", parsed.root_domain.clone()),
                            ("sub_domain", parsed.record_name.clone()),
                            ("record_type", record_type.to_string()),
                            ("record_line", record_line.clone()),
                            ("record_id", record_id.to_string()),
                            ("value", ip.clone()),
                            ("ttl", ttl.clone()),
                        ],
                    )
                    .await?;
                    if result.pointer("/status/code").and_then(Value::as_str) == Some("1") {
                        return Ok(());
                    }
                    return Err(anyhow::anyhow!(
                        "{}",
                        result
                            .pointer("/status/message")
                            .and_then(Value::as_str)
                            .unwrap_or(update_failed.as_str())
                    ));
                }
                let result = dnspod_request(
                    translator,
                    &client,
                    "https://dnsapi.cn/Record.Create",
                    &token_id,
                    &token_key,
                    vec![
                        ("domain", parsed.root_domain.clone()),
                        ("sub_domain", parsed.record_name.clone()),
                        ("record_type", record_type.to_string()),
                        ("record_line", record_line.clone()),
                        ("value", ip),
                        ("ttl", ttl),
                    ],
                )
                .await?;
                if result.pointer("/status/code").and_then(Value::as_str) == Some("1") {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!(
                        "{}",
                        result
                            .pointer("/status/message")
                            .and_then(Value::as_str)
                            .unwrap_or(create_failed.as_str())
                    ))
                }
            }
        },
    )
    .await
}

async fn update_baiducloud(
    translator: &Translator,
    config: &HashMap<String, String>,
    ipv4: Option<&str>,
    ipv6: Option<&str>,
) -> anyhow::Result<DDNSProviderUpdateResult> {
    let access_key_id = config_value(config, "access_key_id");
    let secret_access_key = config_value(config, "secret_access_key");
    let root_domain = config_value(config, "root_domain");
    let domain = config_value(config, "domain");
    if access_key_id.is_empty()
        || secret_access_key.is_empty()
        || root_domain.is_empty()
        || domain.is_empty()
    {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.baidu.configIncomplete",
            &[],
        )));
    }
    let ttl = positive_i64(config.get("ttl"), 300);
    let parsed = split_domain(translator, &domain, &root_domain)?;
    let client = ddns_http_client()?;
    let query_failed = ddns_text(translator, "providers.baidu.queryFailed", &[]);
    let update_failed = ddns_text(translator, "providers.baidu.updateFailed", &[]);
    let create_failed = ddns_text(translator, "providers.baidu.createFailed", &[]);
    let provider_label_text = provider_label(Some("baiducloud"), translator);

    update_dual_stack(translator, &provider_label_text, ipv4, ipv6, |record_type, ip| {
        let client = client.clone();
        let access_key_id = access_key_id.clone();
        let secret_access_key = secret_access_key.clone();
        let parsed = parsed.clone();
        let query_failed = query_failed.clone();
        let update_failed = update_failed.clone();
        let create_failed = create_failed.clone();
        async move {
            let list = baidu_request(
                translator,
                &client,
                &access_key_id,
                &secret_access_key,
                "/v1/domain/resolve/list",
                json!({
                    "domain": parsed.root_domain,
                    "pageNum": 1,
                    "pageSize": 1000
                }),
            )
            .await?;
            if let Some(code) = json_text(&list, "code") {
                return Err(anyhow::anyhow!(
                    "{code}: {}",
                    json_text(&list, "message").unwrap_or_else(|| query_failed.clone())
                ));
            }
            let existing = list
                .get("result")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .find(|record| {
                    record.get("domain").and_then(Value::as_str) == Some(&parsed.record_name)
                })
                .cloned();
            if let Some(record) = existing {
                if record.get("rdata").and_then(Value::as_str) == Some(ip.as_str()) {
                    return Ok(());
                }
                let record_id = record
                    .get("recordId")
                    .and_then(Value::as_i64)
                    .ok_or_else(|| anyhow::anyhow!(update_failed.clone()))?;
                let result = baidu_request(
                    translator,
                    &client,
                    &access_key_id,
                    &secret_access_key,
                    "/v1/domain/resolve/edit",
                    json!({
                        "recordId": record_id,
                        "domain": record.get("domain").and_then(Value::as_str).unwrap_or(&parsed.record_name),
                        "view": record.get("view").and_then(Value::as_str).unwrap_or("default"),
                        "rdType": record_type,
                        "ttl": record.get("ttl").and_then(Value::as_i64).unwrap_or(ttl),
                        "rdata": ip,
                        "zoneName": record.get("zoneName").and_then(Value::as_str).unwrap_or(&parsed.root_domain)
                    }),
                )
                .await?;
                if let Some(code) = json_text(&result, "code") {
                    return Err(anyhow::anyhow!(
                        "{code}: {}",
                        json_text(&result, "message").unwrap_or_else(|| update_failed.clone())
                    ));
                }
                return Ok(());
            }
            let result = baidu_request(
                translator,
                &client,
                &access_key_id,
                &secret_access_key,
                "/v1/domain/resolve/add",
                json!({
                    "domain": parsed.record_name,
                    "rdType": record_type,
                    "ttl": ttl,
                    "rdata": ip,
                    "zoneName": parsed.root_domain
                }),
            )
            .await?;
            if let Some(code) = json_text(&result, "code") {
                Err(anyhow::anyhow!(
                    "{code}: {}",
                    json_text(&result, "message").unwrap_or_else(|| create_failed.clone())
                ))
            } else {
                Ok(())
            }
        }
    })
    .await
}

async fn update_cloudflare(
    translator: &Translator,
    config: &HashMap<String, String>,
    ipv4: Option<&str>,
    ipv6: Option<&str>,
) -> anyhow::Result<DDNSProviderUpdateResult> {
    let api_token = config_value(config, "api_token");
    let zone_id = config_value(config, "zone_id");
    let domain = config_value(config, "domain");
    if api_token.is_empty() || zone_id.is_empty() || domain.is_empty() {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.cloudflare.configIncomplete",
            &[],
        )));
    }
    let proxied = config_value(config, "proxied") == "true";
    let client = ddns_http_client()?;
    let base_url = format!("https://api.cloudflare.com/client/v4/zones/{zone_id}/dns_records");
    let provider_label_text = provider_label(Some("cloudflare"), translator);

    update_dual_stack(
        translator,
        &provider_label_text,
        ipv4,
        ipv6,
        |record_type, ip| {
            let client = client.clone();
            let api_token = api_token.clone();
            let domain = domain.clone();
            let base_url = base_url.clone();
            async move {
                let search_url = build_query_url(
                    &base_url,
                    &[("type", record_type.to_string()), ("name", domain.clone())],
                );
                let (search_status, search_data, _) = response_json(
                    translator,
                    client
                        .get(search_url)
                        .bearer_auth(&api_token)
                        .header(reqwest::header::ACCEPT, "application/json")
                        .send()
                        .await?,
                )
                .await?;
                if !search_status.is_success()
                    || search_data.get("success").and_then(Value::as_bool) != Some(true)
                {
                    return Err(anyhow::anyhow!(
                        "failed to search {record_type} record: {}",
                        compact_json(search_data.get("errors").unwrap_or(&search_data))
                    ));
                }

                let existing_id = search_data
                    .get("result")
                    .and_then(Value::as_array)
                    .and_then(|items| items.first())
                    .and_then(|item| item.get("id"))
                    .and_then(Value::as_str)
                    .map(str::to_string);
                let body = json!({
                    "type": record_type,
                    "name": domain,
                    "content": ip,
                    "proxied": proxied,
                    "ttl": 1
                });
                let request = if let Some(id) = existing_id {
                    client
                        .patch(format!("{base_url}/{id}"))
                        .bearer_auth(&api_token)
                        .json(&body)
                } else {
                    client.post(&base_url).bearer_auth(&api_token).json(&body)
                };
                let (status, data, _) = response_json(
                    translator,
                    request
                        .header(reqwest::header::ACCEPT, "application/json")
                        .send()
                        .await?,
                )
                .await?;
                if status.is_success() && data.get("success").and_then(Value::as_bool) == Some(true)
                {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!(
                        "failed to upsert {record_type} record: {}",
                        compact_json(data.get("errors").unwrap_or(&data))
                    ))
                }
            }
        },
    )
    .await
}

async fn update_godaddy(
    translator: &Translator,
    config: &HashMap<String, String>,
    ipv4: Option<&str>,
    ipv6: Option<&str>,
) -> anyhow::Result<DDNSProviderUpdateResult> {
    let api_key = config_value(config, "api_key");
    let api_secret = config_value(config, "api_secret");
    let root_domain = config_value(config, "root_domain");
    let domain = config_value(config, "domain");
    if api_key.is_empty() || api_secret.is_empty() || root_domain.is_empty() || domain.is_empty() {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.godaddy.configIncomplete",
            &[],
        )));
    }
    let ttl = positive_i64(config.get("ttl"), 600);
    let parsed = split_domain(translator, &domain, &root_domain)?;
    let client = ddns_http_client()?;
    let provider_label_text = provider_label(Some("godaddy"), translator);

    update_dual_stack(
        translator,
        &provider_label_text,
        ipv4,
        ipv6,
        |record_type, ip| {
            let client = client.clone();
            let api_key = api_key.clone();
            let api_secret = api_secret.clone();
            let root_domain = parsed.root_domain.clone();
            let record_name = parsed.record_name.clone();
            async move {
                let response = client
                    .put(format!(
                        "https://api.godaddy.com/v1/domains/{}/records/{}/{}",
                        url_encode_component(&root_domain),
                        record_type,
                        url_encode_component(&record_name)
                    ))
                    .header(
                        reqwest::header::AUTHORIZATION,
                        format!("sso-key {api_key}:{api_secret}"),
                    )
                    .json(&json!([{
                        "data": ip,
                        "name": record_name,
                        "ttl": ttl,
                        "type": record_type
                    }]))
                    .send()
                    .await?;
                let status = response.status();
                let text = response_text(response).await?;
                if status.is_success() {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!(
                        "GoDaddy returned HTTP {}: {}",
                        status.as_u16(),
                        text
                    ))
                }
            }
        },
    )
    .await
}

async fn update_huaweicloud(
    translator: &Translator,
    config: &HashMap<String, String>,
    ipv4: Option<&str>,
    ipv6: Option<&str>,
) -> anyhow::Result<DDNSProviderUpdateResult> {
    let access_key_id = config_value(config, "access_key_id");
    let secret_access_key = config_value(config, "secret_access_key");
    let root_domain = config_value(config, "root_domain");
    let domain = config_value(config, "domain");
    if access_key_id.is_empty()
        || secret_access_key.is_empty()
        || root_domain.is_empty()
        || domain.is_empty()
    {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.huawei.configIncomplete",
            &[],
        )));
    }
    let ttl = positive_i64(config.get("ttl"), 300);
    let parsed = split_domain(translator, &domain, &root_domain)?;
    let normalized_root = parsed.root_domain.trim_end_matches('.').to_string();
    let fqdn_with_dot = format!("{}.", parsed.fqdn.trim_end_matches('.'));
    let expected_zone_name = format!("{normalized_root}.");
    let client = ddns_http_client()?;
    let zone_response = huawei_request(
        translator,
        &client,
        &access_key_id,
        &secret_access_key,
        &format!("/v2/zones?name={}", url_encode_component(&normalized_root)),
        "GET",
        None,
    )
    .await?;
    let zone_id = zone_response
        .get("zones")
        .and_then(Value::as_array)
        .and_then(|zones| {
            zones.iter().find_map(|zone| {
                (zone.get("name").and_then(Value::as_str) == Some(expected_zone_name.as_str()))
                    .then(|| zone.get("id").and_then(Value::as_str).map(str::to_string))
                    .flatten()
            })
        });
    let Some(zone_id) = zone_id else {
        return Ok(provider_failure(format!(
            "{}",
            ddns_text(
                translator,
                "providers.huawei.zoneNotFound",
                &[("zone", expected_zone_name.clone())],
            )
        )));
    };

    let provider_label_text = provider_label(Some("huaweicloud"), translator);
    update_dual_stack(
        translator,
        &provider_label_text,
        ipv4,
        ipv6,
        |record_type, ip| {
            let client = client.clone();
            let access_key_id = access_key_id.clone();
            let secret_access_key = secret_access_key.clone();
            let zone_id = zone_id.clone();
            let fqdn_with_dot = fqdn_with_dot.clone();
            async move {
                let recordset_path = format!(
                    "/v2/zones/{}/recordsets?search_mode=equal&type={}&name={}&limit=500",
                    url_encode_component(&zone_id),
                    record_type,
                    url_encode_component(&fqdn_with_dot)
                );
                let records = huawei_request(
                    translator,
                    &client,
                    &access_key_id,
                    &secret_access_key,
                    &recordset_path,
                    "GET",
                    None,
                )
                .await?;
                let existing = records
                    .get("recordsets")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .find(|record| {
                        record.get("zone_id").and_then(Value::as_str) == Some(zone_id.as_str())
                            && record.get("name").and_then(Value::as_str)
                                == Some(fqdn_with_dot.as_str())
                            && record.get("type").and_then(Value::as_str) == Some(record_type)
                    })
                    .cloned();
                if let Some(existing) = existing {
                    let same_records = existing
                        .get("records")
                        .and_then(Value::as_array)
                        .is_some_and(|records| {
                            records.len() == 1
                                && records.first().and_then(Value::as_str) == Some(ip.as_str())
                        });
                    let same_ttl = existing.get("ttl").and_then(Value::as_i64) == Some(ttl);
                    if same_records && same_ttl {
                        return Ok(());
                    }
                    let record_id =
                        existing.get("id").and_then(Value::as_str).ok_or_else(|| {
                            anyhow::anyhow!(ddns_text(
                                translator,
                                "providers.huawei.recordsetIdMissing",
                                &[],
                            ))
                        })?;
                    huawei_request(
                        translator,
                        &client,
                        &access_key_id,
                        &secret_access_key,
                        &format!(
                            "/v2.1/zones/{}/recordsets/{}",
                            url_encode_component(&zone_id),
                            url_encode_component(record_id)
                        ),
                        "PUT",
                        Some(json!({
                            "name": fqdn_with_dot,
                            "type": record_type,
                            "ttl": ttl,
                            "records": [ip]
                        })),
                    )
                    .await?;
                    return Ok(());
                }
                huawei_request(
                    translator,
                    &client,
                    &access_key_id,
                    &secret_access_key,
                    &format!("/v2/zones/{}/recordsets", url_encode_component(&zone_id)),
                    "POST",
                    Some(json!({
                        "name": fqdn_with_dot,
                        "type": record_type,
                        "ttl": ttl,
                        "records": [ip]
                    })),
                )
                .await?;
                Ok(())
            }
        },
    )
    .await
}

async fn update_porkbun(
    translator: &Translator,
    config: &HashMap<String, String>,
    ipv4: Option<&str>,
    ipv6: Option<&str>,
) -> anyhow::Result<DDNSProviderUpdateResult> {
    let api_key = config_value(config, "api_key");
    let secret_api_key = config_value(config, "secret_api_key");
    let root_domain = config_value(config, "root_domain");
    let domain = config_value(config, "domain");
    if api_key.is_empty()
        || secret_api_key.is_empty()
        || root_domain.is_empty()
        || domain.is_empty()
    {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.porkbun.configIncomplete",
            &[],
        )));
    }
    let ttl = positive_i64(config.get("ttl"), 600).to_string();
    let parsed = split_domain(translator, &domain, &root_domain)?;
    let client = ddns_http_client()?;
    let query_failed = ddns_text(translator, "providers.porkbun.queryRecordFailed", &[]);
    let update_failed = ddns_text(translator, "providers.porkbun.updateRecordFailed", &[]);
    let create_failed = ddns_text(translator, "providers.porkbun.createRecordFailed", &[]);
    let provider_label_text = provider_label(Some("porkbun"), translator);

    update_dual_stack(
        translator,
        &provider_label_text,
        ipv4,
        ipv6,
        |record_type, ip| {
            let client = client.clone();
            let api_key = api_key.clone();
            let secret_api_key = secret_api_key.clone();
            let root_domain = parsed.root_domain.clone();
            let record_name = parsed.record_name.clone();
            let ttl = ttl.clone();
            let query_failed = query_failed.clone();
            let update_failed = update_failed.clone();
            let create_failed = create_failed.clone();
            async move {
                let list = porkbun_request(
                    translator,
                    &client,
                    &format!(
                        "/retrieveByNameType/{}/{}/{}",
                        url_encode_component(&root_domain),
                        record_type,
                        url_encode_component(&record_name)
                    ),
                    &api_key,
                    &secret_api_key,
                    json!({}),
                )
                .await?;
                if list.get("status").and_then(Value::as_str) != Some("SUCCESS") {
                    return Err(anyhow::anyhow!(
                        "{}",
                        json_text(&list, "message").unwrap_or_else(|| query_failed.clone())
                    ));
                }
                let existing_content = list
                    .get("records")
                    .and_then(Value::as_array)
                    .and_then(|items| items.first())
                    .and_then(|record| record.get("content"))
                    .and_then(Value::as_str);
                if existing_content == Some(ip.as_str()) {
                    return Ok(());
                }
                let path = if existing_content.is_some() {
                    format!(
                        "/editByNameType/{}/{}/{}",
                        url_encode_component(&root_domain),
                        record_type,
                        url_encode_component(&record_name)
                    )
                } else {
                    format!("/create/{}", url_encode_component(&root_domain))
                };
                let mut body = json!({
                    "content": ip,
                    "ttl": ttl
                });
                if existing_content.is_none() {
                    if let Some(object) = body.as_object_mut() {
                        object.insert("name".to_string(), json!(record_name));
                        object.insert("type".to_string(), json!(record_type));
                    }
                }
                let result =
                    porkbun_request(translator, &client, &path, &api_key, &secret_api_key, body)
                        .await?;
                if result.get("status").and_then(Value::as_str) == Some("SUCCESS") {
                    Ok(())
                } else {
                    let fallback = if existing_content.is_some() {
                        update_failed.clone()
                    } else {
                        create_failed.clone()
                    };
                    Err(anyhow::anyhow!(
                        "{}",
                        json_text(&result, "message").unwrap_or(fallback)
                    ))
                }
            }
        },
    )
    .await
}

async fn update_tencentcloud(
    translator: &Translator,
    config: &HashMap<String, String>,
    ipv4: Option<&str>,
    ipv6: Option<&str>,
) -> anyhow::Result<DDNSProviderUpdateResult> {
    let secret_id = config_value(config, "secret_id");
    let secret_key = config_value(config, "secret_key");
    let root_domain = config_value(config, "root_domain");
    let domain = config_value(config, "domain");
    if secret_id.is_empty() || secret_key.is_empty() || root_domain.is_empty() || domain.is_empty()
    {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.tencentcloud.configIncomplete",
            &[],
        )));
    }
    let ttl = positive_i64(config.get("ttl"), 600);
    let record_line = default_string(config_value(config, "record_line"), "默认");
    let record_line_id = config_value(config, "record_line_id");
    let parsed = split_domain(translator, &domain, &root_domain)?;
    let client = ddns_http_client()?;
    let missing_updated_record_id = ddns_text(
        translator,
        "providers.tencentcloud.missingUpdatedRecordId",
        &[],
    );
    let missing_created_record_id = ddns_text(
        translator,
        "providers.tencentcloud.missingCreatedRecordId",
        &[],
    );

    let provider_label_text = provider_label(Some("tencentcloud"), translator);
    update_dual_stack(
        translator,
        &provider_label_text,
        ipv4,
        ipv6,
        |record_type, ip| {
            let client = client.clone();
            let secret_id = secret_id.clone();
            let secret_key = secret_key.clone();
            let parsed = parsed.clone();
            let record_line = record_line.clone();
            let record_line_id = record_line_id.clone();
            let missing_updated_record_id = missing_updated_record_id.clone();
            let missing_created_record_id = missing_created_record_id.clone();
            async move {
                let mut base_payload = serde_json::Map::new();
                base_payload.insert("Domain".to_string(), json!(parsed.root_domain));
                base_payload.insert("RecordType".to_string(), json!(record_type));
                if record_line_id.is_empty() {
                    base_payload.insert("RecordLine".to_string(), json!(record_line));
                } else {
                    base_payload.insert("RecordLineId".to_string(), json!(record_line_id));
                }

                let mut list_payload = base_payload.clone();
                list_payload.insert("Limit".to_string(), json!(100));
                list_payload.insert("Offset".to_string(), json!(0));
                list_payload.insert("Subdomain".to_string(), json!(parsed.record_name));
                let list = match tencentcloud_request(
                    translator,
                    &client,
                    &secret_id,
                    &secret_key,
                    "DescribeRecordList",
                    Value::Object(list_payload),
                )
                .await
                {
                    Ok(value) => value,
                    Err(error)
                        if error
                            .to_string()
                            .starts_with("ResourceNotFound.NoDataOfRecord:") =>
                    {
                        json!({ "RecordList": [] })
                    }
                    Err(error) => return Err(error),
                };
                let existing = list
                    .get("RecordList")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .find(|record| {
                        record
                            .get("Name")
                            .and_then(Value::as_str)
                            .unwrap_or(&parsed.record_name)
                            == parsed.record_name
                            && record
                                .get("Type")
                                .and_then(Value::as_str)
                                .unwrap_or(record_type)
                                == record_type
                            && if record_line_id.is_empty() {
                                record.get("Line").and_then(Value::as_str).unwrap_or("默认")
                                    == record_line
                            } else {
                                record
                                    .get("LineId")
                                    .and_then(Value::as_str)
                                    .unwrap_or_default()
                                    == record_line_id
                            }
                    })
                    .cloned();
                if let Some(record) = existing {
                    if record.get("Value").and_then(Value::as_str) == Some(ip.as_str()) {
                        return Ok(());
                    }
                    let record_id = record
                        .get("RecordId")
                        .and_then(Value::as_i64)
                        .ok_or_else(|| anyhow::anyhow!(missing_updated_record_id.clone()))?;
                    let mut payload = base_payload;
                    payload.insert("RecordId".to_string(), json!(record_id));
                    payload.insert("SubDomain".to_string(), json!(parsed.record_name));
                    payload.insert("TTL".to_string(), json!(ttl));
                    payload.insert("Value".to_string(), json!(ip));
                    let result = tencentcloud_request(
                        translator,
                        &client,
                        &secret_id,
                        &secret_key,
                        "ModifyRecord",
                        Value::Object(payload),
                    )
                    .await?;
                    if result.get("RecordId").and_then(Value::as_i64).is_some() {
                        return Ok(());
                    }
                    return Err(anyhow::anyhow!(missing_updated_record_id));
                }

                let mut payload = base_payload;
                payload.insert("SubDomain".to_string(), json!(parsed.record_name));
                payload.insert("TTL".to_string(), json!(ttl));
                payload.insert("Value".to_string(), json!(ip));
                let result = tencentcloud_request(
                    translator,
                    &client,
                    &secret_id,
                    &secret_key,
                    "CreateRecord",
                    Value::Object(payload),
                )
                .await?;
                if result.get("RecordId").and_then(Value::as_i64).is_some() {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!(missing_created_record_id))
                }
            }
        },
    )
    .await
}

async fn update_duckdns(
    translator: &Translator,
    config: &HashMap<String, String>,
    ipv4: Option<&str>,
    ipv6: Option<&str>,
) -> anyhow::Result<DDNSProviderUpdateResult> {
    let domains = config_value(config, "domains");
    let token = config_value(config, "token");
    if domains.is_empty() || token.is_empty() {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.duckdns.configIncomplete",
            &[],
        )));
    }
    if ipv4.is_none() && ipv6.is_none() {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.duckdns.noIpAvailable",
            &[],
        )));
    }
    let client = ddns_http_client()?;
    let response = client
        .post("https://ddns.duckdns.fnknock.cn/")
        .header(reqwest::header::ACCEPT, "text/plain")
        .json(&json!({
            "domains": domains,
            "token": token,
            "ip": ipv4,
            "ipv6": ipv6,
            "verbose": true,
        }))
        .send()
        .await?;
    let status = response.status();
    let text = response_text(response).await?;
    if !status.is_success() {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.duckdns.updateFailedWithStatus",
            &[
                ("status", status.as_u16().to_string()),
                (
                    "detail",
                    if text.is_empty() {
                        ddns_text(translator, "providers.duckdns.requestFailed", &[])
                    } else {
                        text
                    },
                ),
            ],
        )));
    }
    let lines = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    if lines.first().copied() != Some("OK") {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.duckdns.updateFailed",
            &[(
                "detail",
                if text.is_empty() {
                    ddns_text(translator, "providers.duckdns.nonOkResponse", &[])
                } else {
                    text
                },
            )],
        )));
    }
    let detail = lines
        .last()
        .copied()
        .filter(|value| *value != "OK")
        .unwrap_or("");
    Ok(DDNSProviderUpdateResult {
        success: true,
        message: if detail.is_empty() {
            ddns_text(
                translator,
                "providers.duckdns.success",
                &[("detail", String::new())],
            )
        } else {
            ddns_text(
                translator,
                "providers.duckdns.success",
                &[("detail", format!(" ({detail})"))],
            )
        },
        ipv4_updated: ipv4.is_some(),
        ipv6_updated: ipv6.is_some(),
    })
}

async fn update_noip(
    translator: &Translator,
    config: &HashMap<String, String>,
    ipv4: Option<&str>,
    ipv6: Option<&str>,
) -> anyhow::Result<DDNSProviderUpdateResult> {
    let hostname = config_value(config, "hostname");
    let username = config_value(config, "username");
    let password = config_value(config, "password");
    if hostname.is_empty() || username.is_empty() || password.is_empty() {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.noip.configIncomplete",
            &[],
        )));
    }
    if ipv4.is_none() && ipv6.is_none() {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.noip.noIpAvailable",
            &[],
        )));
    }
    let mut query = vec![("hostname", hostname)];
    let combined;
    if let (Some(ipv4), Some(ipv6)) = (ipv4, ipv6) {
        combined = format!("{ipv4},{ipv6}");
        query.push(("myip", combined.clone()));
    } else if let Some(ipv4) = ipv4 {
        query.push(("myip", ipv4.to_string()));
    } else if let Some(ipv6) = ipv6 {
        query.push(("myipv6", ipv6.to_string()));
    }
    let client = ddns_http_client()?;
    let authorization = BASE64_STANDARD.encode(format!("{username}:{password}"));
    let response = client
        .get(build_query_url(
            "https://dynupdate.no-ip.com/nic/update",
            &query,
        ))
        .header(reqwest::header::ACCEPT, "text/plain")
        .header(
            reqwest::header::USER_AGENT,
            "fn-knock-rust/1.0 https://github.com",
        )
        .header(
            reqwest::header::AUTHORIZATION,
            format!("Basic {authorization}"),
        )
        .send()
        .await?;
    let status = response.status();
    let text = response_text(response).await?;
    if !status.is_success() {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.noip.updateFailedWithStatus",
            &[
                ("status", status.as_u16().to_string()),
                (
                    "detail",
                    if text.is_empty() {
                        ddns_text(translator, "providers.noip.requestFailed", &[])
                    } else {
                        text
                    },
                ),
            ],
        )));
    }
    let statuses = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            let mut parts = line.split_whitespace();
            let code = parts.next().unwrap_or("").to_string();
            let detail = parts.collect::<Vec<_>>().join(" ");
            (code, detail)
        })
        .collect::<Vec<_>>();
    if statuses.is_empty() {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.noip.emptyResponse",
            &[],
        )));
    }
    let failures = statuses
        .iter()
        .filter(|(code, _)| code != "good" && code != "nochg")
        .collect::<Vec<_>>();
    if !failures.is_empty() {
        let detail = failures
            .into_iter()
            .map(|(code, detail)| noip_status_message(translator, code, detail))
            .collect::<Vec<_>>()
            .join("; ");
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.noip.updateFailed",
            &[("detail", detail)],
        )));
    }
    let changed = statuses.iter().any(|(code, _)| code == "good");
    Ok(DDNSProviderUpdateResult {
        success: true,
        message: if changed {
            ddns_text(
                translator,
                "providers.noip.updateSuccess",
                &[("detail", noip_detail_suffix(&statuses))],
            )
        } else {
            ddns_text(
                translator,
                "providers.noip.ipUnchanged",
                &[("detail", noip_detail_suffix(&statuses))],
            )
        },
        ipv4_updated: changed && ipv4.is_some(),
        ipv6_updated: changed && ipv6.is_some(),
    })
}

fn noip_status_message(translator: &Translator, code: &str, raw_detail: &str) -> String {
    let known = matches!(
        code,
        "nohost" | "badauth" | "badagent" | "!donator" | "abuse" | "911"
    );
    let reason = if known {
        ddns_text(
            translator,
            &format!("providers.noip.statusMessages.{code}"),
            &[],
        )
    } else if raw_detail.is_empty() {
        ddns_text(
            translator,
            "providers.noip.unknownStatus",
            &[("code", code.to_string())],
        )
    } else {
        raw_detail.to_string()
    };
    if known && !raw_detail.is_empty() {
        format!("{code} ({reason}; {raw_detail})")
    } else {
        format!("{code} ({reason})")
    }
}

fn noip_detail_suffix(statuses: &[(String, String)]) -> String {
    let details = statuses
        .iter()
        .filter_map(|(_, detail)| {
            let detail = detail.trim();
            (!detail.is_empty()).then(|| detail.to_string())
        })
        .collect::<Vec<_>>();
    if details.is_empty() {
        String::new()
    } else {
        format!(" ({})", details.join("; "))
    }
}

async fn update_dynu(
    translator: &Translator,
    config: &HashMap<String, String>,
    ipv4: Option<&str>,
    ipv6: Option<&str>,
) -> anyhow::Result<DDNSProviderUpdateResult> {
    let api_key = config_value(config, "api_key");
    let raw_domain = config_value(config, "domain");
    let wildcard = raw_domain.trim().starts_with("*.");
    let domain = if wildcard {
        normalize_domain(raw_domain.trim().trim_start_matches("*."))
    } else {
        normalize_domain(&raw_domain)
    };
    if api_key.is_empty() || domain.is_empty() {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.dynu.configIncomplete",
            &[],
        )));
    }
    if ipv4.is_none() && ipv6.is_none() {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.dynu.noIpAvailable",
            &[],
        )));
    }
    let client = ddns_http_client()?;
    if wildcard {
        return update_dynu_wildcard(translator, &client, &api_key, config, &domain, ipv4, ipv6)
            .await;
    }
    let root = resolve_dynu_root(translator, &client, &api_key, &domain).await?;
    let provider_label_text = provider_label(Some("dynu"), translator);
    update_dual_stack(translator, &provider_label_text, ipv4, ipv6, |record_type, ip| {
        let client = client.clone();
        let api_key = api_key.clone();
        let domain = domain.clone();
        let root = root.clone();
        let ttl_config = config_value(config, "ttl");
        let group_config = config_value(config, "group");
        async move {
            let list = dynu_request(
                translator,
                &client,
                &api_key,
                &format!(
                    "/dns/record/{}?recordType={}",
                    url_encode_component(&domain),
                    record_type
                ),
                None,
            )
            .await?;
            let existing = list
                .get("dnsRecords")
                .and_then(Value::as_array)
                .and_then(|records| find_dynu_record(records, record_type, &domain, &root.node_name));
            if let Some(existing) = existing.clone()
                && dynu_record_address(&existing, record_type) == ip
            {
                return Ok(());
            }
            let ttl = positive_i64(
                Some(&ttl_config),
                existing
                    .as_ref()
                    .and_then(|record| record.get("ttl"))
                    .and_then(Value::as_i64)
                    .filter(|value| *value > 0)
                    .unwrap_or(300),
            );
            let group = default_string(
                group_config.clone(),
                existing
                    .as_ref()
                    .and_then(|record| record.get("group"))
                    .and_then(Value::as_str)
                    .unwrap_or(""),
            );
            let mut body = json!({
                "nodeName": root.node_name,
                "recordType": record_type,
                "ttl": ttl,
                "state": existing.as_ref().and_then(|record| record.get("state")).and_then(Value::as_bool).unwrap_or(true),
                "group": group
            });
            if record_type == "A" {
                insert_json_field(&mut body, "ipv4Address", json!(ip));
            } else {
                insert_json_field(&mut body, "ipv6Address", json!(ip));
            }
            let path = if let Some(existing) = existing {
                let record_id = read_positive_id(existing.get("id")).ok_or_else(|| {
                    anyhow::anyhow!(ddns_text(
                        translator,
                        "providers.dynu.recordIdMissing",
                        &[],
                    ))
                })?;
                format!("/dns/{}/record/{record_id}", root.domain_id)
            } else {
                format!("/dns/{}/record", root.domain_id)
            };
            dynu_request(translator, &client, &api_key, &path, Some(body)).await?;
            Ok(())
        }
    })
    .await
}

async fn update_dynu_wildcard(
    translator: &Translator,
    client: &reqwest::Client,
    api_key: &str,
    config: &HashMap<String, String>,
    domain: &str,
    ipv4: Option<&str>,
    ipv6: Option<&str>,
) -> anyhow::Result<DDNSProviderUpdateResult> {
    let root = resolve_dynu_root(translator, client, api_key, domain).await?;
    if root.domain_name != domain || !root.node_name.is_empty() {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.dynu.wildcardUnsupported",
            &[("domain", domain.to_string())],
        )));
    }
    let details = dynu_request(
        translator,
        client,
        api_key,
        &format!("/dns/{}", root.domain_id),
        None,
    )
    .await?;
    let ipv4_unchanged = ipv4.is_none_or(|ip| {
        details.get("ipv4Address").and_then(Value::as_str) == Some(ip)
            && details
                .get("ipv4WildcardAlias")
                .and_then(Value::as_bool)
                .unwrap_or(false)
    });
    let ipv6_unchanged = ipv6.is_none_or(|ip| {
        details.get("ipv6Address").and_then(Value::as_str) == Some(ip)
            && details
                .get("ipv6WildcardAlias")
                .and_then(Value::as_bool)
                .unwrap_or(false)
    });
    if ipv4_unchanged && ipv6_unchanged {
        return Ok(DDNSProviderUpdateResult {
            success: true,
            message: ddns_text(translator, "providers.dynu.wildcardUnchanged", &[]),
            ipv4_updated: ipv4.is_some(),
            ipv6_updated: ipv6.is_some(),
        });
    }
    let ttl = positive_i64(
        config.get("ttl"),
        details
            .get("ttl")
            .and_then(Value::as_i64)
            .filter(|value| *value > 0)
            .unwrap_or(300),
    );
    let group = default_string(
        config_value(config, "group"),
        details.get("group").and_then(Value::as_str).unwrap_or(""),
    );
    let mut body = json!({
        "name": normalize_domain(details.get("name").and_then(Value::as_str).unwrap_or(domain)),
        "group": group,
        "ttl": ttl,
        "ipv4": ipv4.is_some() || details.get("ipv4").and_then(Value::as_bool).unwrap_or(false) || details.get("ipv4Address").and_then(Value::as_str).is_some(),
        "ipv6": ipv6.is_some() || details.get("ipv6").and_then(Value::as_bool).unwrap_or(false) || details.get("ipv6Address").and_then(Value::as_str).is_some(),
        "ipv4WildcardAlias": ipv4.is_some() || details.get("ipv4WildcardAlias").and_then(Value::as_bool).unwrap_or(false),
        "ipv6WildcardAlias": ipv6.is_some() || details.get("ipv6WildcardAlias").and_then(Value::as_bool).unwrap_or(false),
        "allowZoneTransfer": details.get("allowZoneTransfer").and_then(Value::as_bool).unwrap_or(false),
        "dnssec": details.get("dnssec").and_then(Value::as_bool).unwrap_or(false)
    });
    if let Some(ipv4) = ipv4.or_else(|| details.get("ipv4Address").and_then(Value::as_str)) {
        insert_json_field(&mut body, "ipv4Address", json!(ipv4));
    }
    if let Some(ipv6) = ipv6.or_else(|| details.get("ipv6Address").and_then(Value::as_str)) {
        insert_json_field(&mut body, "ipv6Address", json!(ipv6));
    }
    dynu_request(
        translator,
        client,
        api_key,
        &format!("/dns/{}", root.domain_id),
        Some(body),
    )
    .await?;
    Ok(DDNSProviderUpdateResult {
        success: true,
        message: ddns_text(translator, "providers.dynu.wildcardSuccess", &[]),
        ipv4_updated: ipv4.is_some(),
        ipv6_updated: ipv6.is_some(),
    })
}

async fn update_edgeone(
    translator: &Translator,
    config: &HashMap<String, String>,
    ipv4: Option<&str>,
    ipv6: Option<&str>,
) -> anyhow::Result<DDNSProviderUpdateResult> {
    let secret_id = config_value(config, "secret_id");
    let secret_key = config_value(config, "secret_key");
    let zone_id = config_value(config, "zone_id");
    let domain = normalize_domain(&config_value(config, "domain"));
    if secret_id.is_empty() || secret_key.is_empty() || zone_id.is_empty() || domain.is_empty() {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.edgeone.configIncomplete",
            &[],
        )));
    }
    let ttl = positive_i64(config.get("ttl"), 300);
    let desired_location = normalize_edgeone_location(config.get("location").map(String::as_str));
    let client = ddns_http_client()?;
    let missing_record_id = ddns_text(translator, "providers.edgeone.missingRecordId", &[]);
    let missing_created_record_id =
        ddns_text(translator, "providers.edgeone.missingCreatedRecordId", &[]);
    let provider_label_text = provider_label(Some("edgeone"), translator);
    update_dual_stack(
        translator,
        &provider_label_text,
        ipv4,
        ipv6,
        |record_type, ip| {
            let client = client.clone();
            let config = config.clone();
            let secret_id = secret_id.clone();
            let secret_key = secret_key.clone();
            let zone_id = zone_id.clone();
            let domain = domain.clone();
            let desired_location = desired_location.clone();
            let missing_record_id = missing_record_id.clone();
            let missing_created_record_id = missing_created_record_id.clone();
            async move {
                let list = edgeone_request(
                    translator,
                    &client,
                    &config,
                    &secret_id,
                    &secret_key,
                    "DescribeDnsRecords",
                    json!({
                        "ZoneId": zone_id,
                        "Offset": 0,
                        "Limit": 100,
                        "Match": "all",
                        "Filters": [{
                            "Name": "name",
                            "Values": [domain],
                            "Fuzzy": false
                        }]
                    }),
                )
                .await?;
                let existing = list
                    .get("DnsRecords")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .find(|record| {
                        normalize_domain(
                            record
                                .get("Name")
                                .and_then(Value::as_str)
                                .unwrap_or_default(),
                        ) == domain
                            && record
                                .get("Type")
                                .and_then(Value::as_str)
                                .is_some_and(|value| value.eq_ignore_ascii_case(record_type))
                            && normalize_edgeone_location(
                                record.get("Location").and_then(Value::as_str),
                            ) == desired_location
                    })
                    .cloned();
                if let Some(existing) = existing {
                    if existing.get("Content").and_then(Value::as_str) == Some(ip.as_str()) {
                        return Ok(());
                    }
                    let record_id = existing
                        .get("RecordId")
                        .and_then(Value::as_str)
                        .ok_or_else(|| anyhow::anyhow!(missing_record_id.clone()))?;
                    let mut record = json!({
                        "RecordId": record_id,
                        "Name": domain,
                        "Type": record_type,
                        "Content": ip,
                        "TTL": ttl
                    });
                    if desired_location != "default" {
                        insert_json_field(
                            &mut record,
                            "Location",
                            json!(config_value(&config, "location")),
                        );
                    }
                    edgeone_request(
                        translator,
                        &client,
                        &config,
                        &secret_id,
                        &secret_key,
                        "ModifyDnsRecords",
                        json!({ "ZoneId": zone_id, "DnsRecords": [record] }),
                    )
                    .await?;
                    return Ok(());
                }
                let mut payload = json!({
                    "ZoneId": zone_id,
                    "Name": domain,
                    "Type": record_type,
                    "Content": ip,
                    "TTL": ttl
                });
                if desired_location != "default" {
                    insert_json_field(
                        &mut payload,
                        "Location",
                        json!(config_value(&config, "location")),
                    );
                }
                let result = edgeone_request(
                    translator,
                    &client,
                    &config,
                    &secret_id,
                    &secret_key,
                    "CreateDnsRecord",
                    payload,
                )
                .await?;
                if result.get("RecordId").and_then(Value::as_str).is_some() {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!(missing_created_record_id))
                }
            }
        },
    )
    .await
}

async fn update_edgeone_cname(
    translator: &Translator,
    config: &HashMap<String, String>,
    ipv4: Option<&str>,
    ipv6: Option<&str>,
) -> anyhow::Result<DDNSProviderUpdateResult> {
    let secret_id = config_value(config, "secret_id");
    let secret_key = config_value(config, "secret_key");
    let zone_id = config_value(config, "zone_id");
    let domain = normalize_domain(&config_value(config, "domain"));
    if secret_id.is_empty() || secret_key.is_empty() || zone_id.is_empty() || domain.is_empty() {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.edgeone_cname.configIncomplete",
            &[],
        )));
    }
    let desired = match (ipv4, ipv6) {
        (Some(_), Some(_)) => {
            return Ok(provider_failure(ddns_text(
                translator,
                "providers.edgeone_cname.singleAddressOnly",
                &[],
            )));
        }
        (Some(value), None) => ("ipv4", value),
        (None, Some(value)) => ("ipv6", value),
        (None, None) => {
            return Ok(provider_failure(ddns_text(
                translator,
                "providers.edgeone_cname.noIpAvailable",
                &[],
            )));
        }
    };
    let client = ddns_http_client()?;
    let list = edgeone_request(
        translator,
        &client,
        config,
        &secret_id,
        &secret_key,
        "DescribeAccelerationDomains",
        json!({
            "ZoneId": zone_id,
            "Offset": 0,
            "Limit": 20,
            "Match": "all",
            "Filters": [{
                "Name": "domain-name",
                "Values": [domain],
                "Fuzzy": false
            }]
        }),
    )
    .await?;
    let existing = list
        .get("AccelerationDomains")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|item| {
            normalize_domain(
                item.get("DomainName")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            ) == domain
        })
        .cloned();
    let Some(existing) = existing else {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.edgeone_cname.domainNotFound",
            &[("domain", domain.clone())],
        )));
    };
    let origin_detail = existing.get("OriginDetail").unwrap_or(&Value::Null);
    let origin_type = origin_detail
        .get("OriginType")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_ascii_uppercase();
    if !origin_type.is_empty() && origin_type != "IP_DOMAIN" {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.edgeone_cname.unsupportedOriginType",
            &[("originType", origin_type)],
        )));
    }
    let current_origin = origin_detail
        .get("Origin")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if current_origin == desired.1 {
        return Ok(DDNSProviderUpdateResult {
            success: true,
            message: ddns_text(translator, "providers.edgeone_cname.originUnchanged", &[]),
            ipv4_updated: desired.0 == "ipv4",
            ipv6_updated: desired.0 == "ipv6",
        });
    }
    let raw_host_header = origin_detail
        .get("HostHeader")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let mut origin_info = json!({
        "OriginType": "IP_DOMAIN",
        "Origin": desired.1
    });
    if let Some(host_header) = raw_host_header
        && is_valid_edgeone_host_header(host_header)
    {
        insert_json_field(
            &mut origin_info,
            "HostHeader",
            json!(normalize_domain(host_header)),
        );
    }
    edgeone_request(
        translator,
        &client,
        config,
        &secret_id,
        &secret_key,
        "ModifyAccelerationDomain",
        json!({
            "ZoneId": zone_id,
            "DomainName": domain,
            "OriginInfo": origin_info
        }),
    )
    .await?;
    Ok(DDNSProviderUpdateResult {
        success: true,
        message: ddns_text(translator, "providers.edgeone_cname.success", &[]),
        ipv4_updated: desired.0 == "ipv4",
        ipv6_updated: desired.0 == "ipv6",
    })
}

async fn update_esa(
    translator: &Translator,
    config: &HashMap<String, String>,
    ipv4: Option<&str>,
    ipv6: Option<&str>,
) -> anyhow::Result<DDNSProviderUpdateResult> {
    let access_key_id = config_value(config, "access_key_id");
    let access_key_secret = config_value(config, "access_key_secret");
    let site_name = normalize_domain(&config_value(config, "site_name"));
    let site_id = config_value(config, "site_id");
    let domain = normalize_domain(&config_value(config, "domain"));
    if access_key_id.is_empty()
        || access_key_secret.is_empty()
        || domain.is_empty()
        || (site_name.is_empty() && site_id.is_empty())
    {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.esa.configIncomplete",
            &[],
        )));
    }
    let ttl = positive_i64(config.get("ttl"), 30);
    let proxied = config_value(config, "proxied") == "true";
    let biz_name = if proxied {
        default_string(config_value(config, "biz_name"), "web")
    } else {
        String::new()
    };
    let record_value = [ipv4, ipv6]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(",");
    if record_value.is_empty() {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.esa.noIpAvailable",
            &[],
        )));
    }
    let client = ddns_http_client()?;
    let site_id = if !site_id.is_empty() {
        site_id
    } else {
        let sites = aliyun_acs3_request(
            translator,
            &client,
            &access_key_id,
            &access_key_secret,
            "ListSites",
            "2024-09-10",
            "GET",
            vec![
                ("PageNumber".to_string(), "1".to_string()),
                ("PageSize".to_string(), "100".to_string()),
                ("SiteName".to_string(), site_name.clone()),
                ("SiteSearchType".to_string(), "exact".to_string()),
            ],
            Vec::new(),
        )
        .await?;
        sites
            .get("Sites")
            .and_then(Value::as_array)
            .and_then(|sites| {
                sites.iter().find_map(|site| {
                    (normalize_domain(
                        site.get("SiteName")
                            .and_then(Value::as_str)
                            .unwrap_or_default(),
                    ) == site_name)
                        .then(|| site.get("SiteId").map(value_to_compact_string))
                        .flatten()
                })
            })
            .ok_or_else(|| {
                anyhow::anyhow!(ddns_text(
                    translator,
                    "providers.esa.siteNotFound",
                    &[("site", site_name.clone())],
                ))
            })?
    };
    let records = aliyun_acs3_request(
        translator,
        &client,
        &access_key_id,
        &access_key_secret,
        "ListRecords",
        "2024-09-10",
        "GET",
        vec![
            ("PageNumber".to_string(), "1".to_string()),
            ("PageSize".to_string(), "100".to_string()),
            ("RecordMatchType".to_string(), "exact".to_string()),
            ("RecordName".to_string(), domain.clone()),
            ("SiteId".to_string(), site_id.clone()),
            ("Type".to_string(), "A/AAAA".to_string()),
        ],
        Vec::new(),
    )
    .await?;
    let existing = records
        .get("Records")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|record| {
            normalize_domain(
                record
                    .get("RecordName")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            ) == domain
                && record
                    .get("RecordType")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .eq_ignore_ascii_case("A/AAAA")
        })
        .cloned()
        .collect::<Vec<_>>();
    let mut payload = esa_record_payload(&record_value, ttl, proxied, &biz_name);
    if existing.is_empty() {
        payload.push(("RecordName".to_string(), domain));
        payload.push(("SiteId".to_string(), site_id));
        let result = aliyun_acs3_request(
            translator,
            &client,
            &access_key_id,
            &access_key_secret,
            "CreateRecord",
            "2024-09-10",
            "POST",
            payload,
            Vec::new(),
        )
        .await?;
        if result.get("RecordId").is_some() {
            return Ok(DDNSProviderUpdateResult {
                success: true,
                message: ddns_text(translator, "providers.esa.success", &[]),
                ipv4_updated: ipv4.is_some(),
                ipv6_updated: ipv6.is_some(),
            });
        }
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.esa.createRecordFailed",
            &[],
        )));
    }
    for record in existing {
        let current_value = record
            .pointer("/Data/Value")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let current_ttl = record.get("Ttl").and_then(Value::as_i64).unwrap_or(ttl);
        let current_proxied = record
            .get("Proxied")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let current_biz_name = record
            .get("BizName")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if same_csv_values(current_value, &record_value)
            && current_ttl == ttl
            && current_proxied == proxied
            && current_biz_name == biz_name
        {
            continue;
        }
        let record_id = record
            .get("RecordId")
            .map(value_to_compact_string)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                anyhow::anyhow!(ddns_text(translator, "providers.esa.recordIdMissing", &[],))
            })?;
        let mut update_payload = esa_record_payload(&record_value, ttl, proxied, &biz_name);
        update_payload.push(("RecordId".to_string(), record_id));
        aliyun_acs3_request(
            translator,
            &client,
            &access_key_id,
            &access_key_secret,
            "UpdateRecord",
            "2024-09-10",
            "POST",
            update_payload,
            Vec::new(),
        )
        .await?;
    }
    Ok(DDNSProviderUpdateResult {
        success: true,
        message: ddns_text(translator, "providers.esa.success", &[]),
        ipv4_updated: ipv4.is_some(),
        ipv6_updated: ipv6.is_some(),
    })
}

async fn update_dynv6(
    translator: &Translator,
    config: &HashMap<String, String>,
    ipv4: Option<&str>,
    ipv6: Option<&str>,
) -> anyhow::Result<DDNSProviderUpdateResult> {
    let token = config_value(config, "token");
    let zone = config_value(config, "zone");
    if token.is_empty() || zone.is_empty() {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.dynv6.configIncomplete",
            &[],
        )));
    }
    if ipv4.is_none() && ipv6.is_none() && config_value(config, "ipv6prefix").is_empty() {
        return Ok(provider_failure(ddns_text(
            translator,
            "dualStackUnavailable",
            &[],
        )));
    }
    let mut query = vec![("hostname", zone), ("token", token)];
    if let Some(ipv4) = ipv4 {
        query.push(("ipv4", ipv4.to_string()));
    }
    if let Some(ipv6) = ipv6 {
        query.push(("ipv6", ipv6.to_string()));
    }
    let ipv6prefix = config_value(config, "ipv6prefix");
    if !ipv6prefix.is_empty() {
        query.push(("ipv6prefix", ipv6prefix));
    }
    let client = ddns_http_client()?;
    let response = client
        .get(build_query_url("https://dynv6.com/api/update", &query))
        .send()
        .await?;
    let status = response.status();
    let text = response_text(response).await?;
    if status.is_success() && (text.contains("updated") || text.contains("unchanged")) {
        Ok(DDNSProviderUpdateResult {
            success: true,
            message: ddns_text(
                translator,
                "providers.dynv6.success",
                &[
                    ("detail", text),
                    ("params", dynv6_sent_params(translator, ipv4, ipv6, config)),
                ],
            ),
            ipv4_updated: ipv4.is_some(),
            ipv6_updated: ipv6.is_some(),
        })
    } else {
        Ok(provider_failure(ddns_text(
            translator,
            "providers.dynv6.updateFailed",
            &[("status", status.as_u16().to_string()), ("detail", text)],
        )))
    }
}

fn dynv6_sent_params(
    translator: &Translator,
    ipv4: Option<&str>,
    ipv6: Option<&str>,
    config: &HashMap<String, String>,
) -> String {
    let empty = ddns_text(translator, "providers.dynv6.empty", &[]);
    let mut parts = vec![
        format!("ipv4={}", ipv4.unwrap_or(empty.as_str())),
        format!("ipv6={}", ipv6.unwrap_or(empty.as_str())),
    ];
    let ipv6prefix = config_value(config, "ipv6prefix");
    if !ipv6prefix.is_empty() {
        parts.push(format!("ipv6prefix={ipv6prefix}"));
    }
    parts.join(", ")
}

fn config_value(config: &HashMap<String, String>, key: &str) -> String {
    config
        .get(key)
        .map(|value| value.trim().to_string())
        .unwrap_or_default()
}

fn provider_failure(message: impl Into<String>) -> DDNSProviderUpdateResult {
    DDNSProviderUpdateResult {
        success: false,
        message: message.into(),
        ipv4_updated: false,
        ipv6_updated: false,
    }
}

async fn update_dual_stack<F, Fut>(
    translator: &Translator,
    provider_label: &str,
    ipv4: Option<&str>,
    ipv6: Option<&str>,
    update_record: F,
) -> anyhow::Result<DDNSProviderUpdateResult>
where
    F: Fn(&'static str, String) -> Fut,
    Fut: Future<Output = anyhow::Result<()>>,
{
    let mut ipv4_updated = false;
    let mut ipv6_updated = false;
    let mut errors = Vec::new();

    if let Some(ip) = ipv4 {
        match update_record("A", ip.to_string()).await {
            Ok(()) => ipv4_updated = true,
            Err(error) => errors.push(format!(
                "{}: {error}",
                ddns_text(translator, "aRecordFailed", &[])
            )),
        }
    }
    if let Some(ip) = ipv6 {
        match update_record("AAAA", ip.to_string()).await {
            Ok(()) => ipv6_updated = true,
            Err(error) => errors.push(format!(
                "{}: {error}",
                ddns_text(translator, "aaaaRecordFailed", &[])
            )),
        }
    }
    if !errors.is_empty() {
        return Ok(DDNSProviderUpdateResult {
            success: false,
            message: errors.join("; "),
            ipv4_updated,
            ipv6_updated,
        });
    }
    Ok(DDNSProviderUpdateResult {
        success: true,
        message: ddns_text(
            translator,
            "providerDnsUpdateSuccess",
            &[("provider", provider_label.to_string())],
        ),
        ipv4_updated,
        ipv6_updated,
    })
}

#[derive(Clone)]
struct SplitDomain {
    fqdn: String,
    root_domain: String,
    record_name: String,
}

fn split_domain(
    translator: &Translator,
    full_domain: &str,
    root_domain: &str,
) -> anyhow::Result<SplitDomain> {
    let fqdn = normalize_domain(full_domain);
    let zone = normalize_domain(root_domain);
    if fqdn.is_empty() || zone.is_empty() {
        return Err(anyhow::anyhow!(ddns_text(
            translator,
            "domainConfigIncomplete",
            &[],
        )));
    }
    if fqdn == zone {
        return Ok(SplitDomain {
            fqdn,
            root_domain: zone,
            record_name: "@".to_string(),
        });
    }
    let suffix = format!(".{zone}");
    if !fqdn.ends_with(&suffix) {
        return Err(anyhow::anyhow!(ddns_text(
            translator,
            "domainNotInZone",
            &[("fqdn", fqdn), ("zone", zone)],
        )));
    }
    Ok(SplitDomain {
        fqdn: fqdn.clone(),
        root_domain: zone,
        record_name: fqdn[..fqdn.len() - suffix.len()].to_string(),
    })
}

fn positive_i64(value: Option<&String>, fallback: i64) -> i64 {
    value
        .and_then(|value| value.trim().parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value > 0.0)
        .map(|value| value.floor() as i64)
        .unwrap_or(fallback)
}

async fn response_json(
    translator: &Translator,
    response: reqwest::Response,
) -> anyhow::Result<(StatusCode, Value, String)> {
    let status = response.status();
    let text = response.text().await?.trim().to_string();
    let value = if text.is_empty() {
        json!({})
    } else {
        serde_json::from_str::<Value>(&text).map_err(|_| {
            anyhow::anyhow!(ddns_text(
                translator,
                "invalidJsonResponse",
                &[("text", text.clone())],
            ))
        })?
    };
    Ok((status, value, text))
}

async fn porkbun_request(
    translator: &Translator,
    client: &reqwest::Client,
    path: &str,
    api_key: &str,
    secret_api_key: &str,
    body: Value,
) -> anyhow::Result<Value> {
    let mut payload = body.as_object().cloned().unwrap_or_default();
    payload.insert("apikey".to_string(), json!(api_key));
    payload.insert("secretapikey".to_string(), json!(secret_api_key));
    let (_status, value, _text) = response_json(
        translator,
        client
            .post(format!("https://porkbun.com/api/json/v3/dns{path}"))
            .json(&Value::Object(payload))
            .send()
            .await?,
    )
    .await?;
    Ok(value)
}

async fn dnspod_request(
    translator: &Translator,
    client: &reqwest::Client,
    api: &str,
    token_id: &str,
    token_key: &str,
    params: Vec<(&str, String)>,
) -> anyhow::Result<Value> {
    let mut form = vec![
        ("login_token", format!("{token_id},{token_key}")),
        ("format", "json".to_string()),
    ];
    form.extend(params);
    let (status, value, text) = response_json(
        translator,
        client
            .post(api)
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body(form_body(&form))
            .send()
            .await?,
    )
    .await?;
    if status.is_success() {
        Ok(value)
    } else {
        Err(anyhow::anyhow!(
            "DNSPod returned HTTP {}: {}",
            status.as_u16(),
            text
        ))
    }
}

async fn alidns_request(
    translator: &Translator,
    client: &reqwest::Client,
    access_key_id: &str,
    access_key_secret: &str,
    params: Vec<(&str, String)>,
) -> anyhow::Result<Value> {
    let body = build_aliyun_signed_params(access_key_id, access_key_secret, params, "POST");
    let (status, value, text) = response_json(
        translator,
        client
            .post("https://alidns.aliyuncs.com/")
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body(body)
            .send()
            .await?,
    )
    .await?;
    if status.is_success() {
        Ok(value)
    } else {
        Err(anyhow::anyhow!(
            "AliDNS returned HTTP {}: {}",
            status.as_u16(),
            text
        ))
    }
}

async fn tencentcloud_request(
    translator: &Translator,
    client: &reqwest::Client,
    secret_id: &str,
    secret_key: &str,
    action: &str,
    payload: Value,
) -> anyhow::Result<Value> {
    const HOST: &str = "dnspod.tencentcloudapi.com";
    const SERVICE: &str = "dnspod";
    const VERSION: &str = "2021-03-23";

    let timestamp = time_utils::now_ms().div_euclid(1000);
    let date = utc_date(timestamp)?;
    let payload_string = serde_json::to_string(&payload)?;
    let hashed_payload = sha256_hex(&payload_string);
    let content_type = "application/json; charset=utf-8";
    let canonical_headers = format!(
        "content-type:{content_type}\nhost:{HOST}\nx-tc-action:{}\n",
        action.to_ascii_lowercase()
    );
    let signed_headers = "content-type;host;x-tc-action";
    let canonical_request = [
        "POST",
        "/",
        "",
        &canonical_headers,
        signed_headers,
        &hashed_payload,
    ]
    .join("\n");
    let credential_scope = format!("{date}/{SERVICE}/tc3_request");
    let string_to_sign = [
        "TC3-HMAC-SHA256",
        &timestamp.to_string(),
        &credential_scope,
        &sha256_hex(&canonical_request),
    ]
    .join("\n");
    let secret_date = hmac_sha256_bytes(format!("TC3{secret_key}").as_bytes(), date.as_bytes());
    let secret_service = hmac_sha256_bytes(&secret_date, SERVICE.as_bytes());
    let secret_signing = hmac_sha256_bytes(&secret_service, b"tc3_request");
    let signature = hmac_sha256_hex(&secret_signing, string_to_sign.as_bytes());
    let authorization = format!(
        "TC3-HMAC-SHA256 Credential={secret_id}/{credential_scope}, SignedHeaders={signed_headers}, Signature={signature}"
    );

    let (status, data, _text) = response_json(
        translator,
        client
            .post(format!("https://{HOST}/"))
            .header(reqwest::header::CONTENT_TYPE, content_type)
            .header(reqwest::header::HOST, HOST)
            .header("X-TC-Action", action)
            .header("X-TC-Timestamp", timestamp.to_string())
            .header("X-TC-Version", VERSION)
            .header(reqwest::header::AUTHORIZATION, authorization)
            .body(payload_string)
            .send()
            .await?,
    )
    .await?;
    let response = data.get("Response").cloned().ok_or_else(|| {
        anyhow::anyhow!(ddns_text(
            translator,
            "tencentMissingResponse",
            &[("status", status.as_u16().to_string())],
        ))
    })?;
    if let Some(error) = response.get("Error") {
        let code = error
            .get("Code")
            .and_then(Value::as_str)
            .unwrap_or("TencentCloudError");
        let request_failed = ddns_text(translator, "requestFailed", &[]);
        let message = error
            .get("Message")
            .and_then(Value::as_str)
            .unwrap_or(request_failed.as_str());
        let request_id = response
            .get("RequestId")
            .and_then(Value::as_str)
            .unwrap_or_default();
        return Err(anyhow::anyhow!(
            "{code}: {message}{}",
            if request_id.is_empty() {
                String::new()
            } else {
                format!(" (RequestId: {request_id})")
            }
        ));
    }
    if status.is_success() {
        Ok(response)
    } else {
        Err(anyhow::anyhow!(
            "HTTP {}: {}",
            status.as_u16(),
            ddns_text(translator, "requestFailed", &[])
        ))
    }
}

async fn edgeone_request(
    translator: &Translator,
    client: &reqwest::Client,
    config: &HashMap<String, String>,
    secret_id: &str,
    secret_key: &str,
    action: &str,
    payload: Value,
) -> anyhow::Result<Value> {
    let host = edgeone_api_host(config.get("endpoint").map(String::as_str));
    let region = config_value(config, "region");
    tencentcloud_tc3_request(
        translator,
        client,
        secret_id,
        secret_key,
        action,
        payload,
        &host,
        "teo",
        "2022-09-01",
        if region.is_empty() {
            None
        } else {
            Some(region.as_str())
        },
    )
    .await
}

async fn tencentcloud_tc3_request(
    translator: &Translator,
    client: &reqwest::Client,
    secret_id: &str,
    secret_key: &str,
    action: &str,
    payload: Value,
    host: &str,
    service: &str,
    version: &str,
    region: Option<&str>,
) -> anyhow::Result<Value> {
    let timestamp = time_utils::now_ms().div_euclid(1000);
    let date = utc_date(timestamp)?;
    let payload_string = serde_json::to_string(&payload)?;
    let hashed_payload = sha256_hex(&payload_string);
    let content_type = "application/json; charset=utf-8";
    let canonical_headers = format!(
        "content-type:{content_type}\nhost:{host}\nx-tc-action:{}\n",
        action.to_ascii_lowercase()
    );
    let signed_headers = "content-type;host;x-tc-action";
    let canonical_request = [
        "POST",
        "/",
        "",
        &canonical_headers,
        signed_headers,
        &hashed_payload,
    ]
    .join("\n");
    let credential_scope = format!("{date}/{service}/tc3_request");
    let string_to_sign = [
        "TC3-HMAC-SHA256",
        &timestamp.to_string(),
        &credential_scope,
        &sha256_hex(&canonical_request),
    ]
    .join("\n");
    let secret_date = hmac_sha256_bytes(format!("TC3{secret_key}").as_bytes(), date.as_bytes());
    let secret_service = hmac_sha256_bytes(&secret_date, service.as_bytes());
    let secret_signing = hmac_sha256_bytes(&secret_service, b"tc3_request");
    let signature = hmac_sha256_hex(&secret_signing, string_to_sign.as_bytes());
    let authorization = format!(
        "TC3-HMAC-SHA256 Credential={secret_id}/{credential_scope}, SignedHeaders={signed_headers}, Signature={signature}"
    );
    let mut request = client
        .post(format!("https://{host}/"))
        .header(reqwest::header::CONTENT_TYPE, content_type)
        .header(reqwest::header::HOST, host)
        .header("X-TC-Action", action)
        .header("X-TC-Timestamp", timestamp.to_string())
        .header("X-TC-Version", version)
        .header(reqwest::header::AUTHORIZATION, authorization)
        .body(payload_string);
    if let Some(region) = region {
        request = request.header("X-TC-Region", region);
    }
    let (status, data, _text) = response_json(translator, request.send().await?).await?;
    let response = data.get("Response").cloned().ok_or_else(|| {
        anyhow::anyhow!(ddns_text(
            translator,
            "tencentMissingResponse",
            &[("status", status.as_u16().to_string())],
        ))
    })?;
    if let Some(error) = response.get("Error") {
        let code = error
            .get("Code")
            .and_then(Value::as_str)
            .unwrap_or("TencentCloudError");
        let request_failed = ddns_text(translator, "requestFailed", &[]);
        let message = error
            .get("Message")
            .and_then(Value::as_str)
            .unwrap_or(request_failed.as_str());
        let request_id = response
            .get("RequestId")
            .and_then(Value::as_str)
            .unwrap_or_default();
        return Err(anyhow::anyhow!(
            "{code}: {message}{}",
            if request_id.is_empty() {
                String::new()
            } else {
                format!(" (RequestId: {request_id})")
            }
        ));
    }
    if status.is_success() {
        Ok(response)
    } else {
        Err(anyhow::anyhow!(
            "HTTP {}: {}",
            status.as_u16(),
            ddns_text(translator, "requestFailed", &[])
        ))
    }
}

async fn baidu_request(
    translator: &Translator,
    client: &reqwest::Client,
    access_key_id: &str,
    secret_access_key: &str,
    path: &str,
    body: Value,
) -> anyhow::Result<Value> {
    let url = format!("https://bcd.baidubce.com{path}");
    let body_string = serde_json::to_string(&body)?;
    let (timestamp, authorization) =
        baidu_bce_authorization("POST", &url, access_key_id, secret_access_key)?;
    let (status, data, text) = response_json(
        translator,
        client
            .post(&url)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header(reqwest::header::HOST, "bcd.baidubce.com")
            .header("x-bce-date", timestamp)
            .header(reqwest::header::AUTHORIZATION, authorization)
            .body(body_string)
            .send()
            .await?,
    )
    .await?;
    if status.is_success() {
        Ok(data)
    } else {
        Err(anyhow::anyhow!(
            "Baidu Cloud returned HTTP {}: {}",
            status.as_u16(),
            text
        ))
    }
}

async fn huawei_request(
    translator: &Translator,
    client: &reqwest::Client,
    access_key_id: &str,
    secret_access_key: &str,
    path: &str,
    method: &str,
    body: Option<Value>,
) -> anyhow::Result<Value> {
    let url = format!("https://dns.myhuaweicloud.com{path}");
    let body_string = body
        .as_ref()
        .map(serde_json::to_string)
        .transpose()?
        .unwrap_or_default();
    let (x_sdk_date, authorization) = huawei_sdk_authorization(
        method,
        &url,
        "application/json",
        access_key_id,
        secret_access_key,
        &body_string,
    )?;
    let method = reqwest::Method::from_bytes(method.as_bytes())?;
    let mut request = client
        .request(method, &url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header(reqwest::header::HOST, "dns.myhuaweicloud.com")
        .header("X-Sdk-Date", x_sdk_date)
        .header(reqwest::header::AUTHORIZATION, authorization);
    if !body_string.is_empty() {
        request = request.body(body_string);
    }
    let (status, data, text) = response_json(translator, request.send().await?).await?;
    if status.is_success() {
        Ok(data)
    } else {
        Err(anyhow::anyhow!(
            "Huawei Cloud DNS returned HTTP {}: {}",
            status.as_u16(),
            text
        ))
    }
}

#[derive(Clone)]
struct DynuRoot {
    domain_id: i64,
    domain_name: String,
    node_name: String,
}

async fn dynu_request(
    translator: &Translator,
    client: &reqwest::Client,
    api_key: &str,
    path: &str,
    body: Option<Value>,
) -> anyhow::Result<Value> {
    let url = format!("https://api.dynu.com/v2{path}");
    let mut request = client
        .request(
            if body.is_some() {
                reqwest::Method::POST
            } else {
                reqwest::Method::GET
            },
            &url,
        )
        .header(reqwest::header::ACCEPT, "application/json")
        .header("API-Key", api_key);
    if let Some(body) = body {
        request = request
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .json(&body);
    }
    let (status, data, text) = response_json(translator, request.send().await?).await?;
    assert_dynu_success(status, &data, &text)?;
    Ok(data)
}

fn assert_dynu_success(status: StatusCode, data: &Value, text: &str) -> anyhow::Result<()> {
    if !status.is_success() {
        return Err(anyhow::anyhow!(
            "[{}] {}",
            status.as_u16(),
            format_dynu_error(data, text)
        ));
    }
    if data.get("exception").is_some() {
        return Err(anyhow::anyhow!("{}", format_dynu_error(data, text)));
    }
    if let Some(status_code) = data.get("statusCode").and_then(Value::as_i64)
        && status_code != 200
    {
        return Err(anyhow::anyhow!(
            "[{status_code}] {}",
            format_dynu_error(data, text)
        ));
    }
    Ok(())
}

fn format_dynu_error(data: &Value, fallback: &str) -> String {
    if let Some(exception) = data.get("exception") {
        let status = exception
            .get("statusCode")
            .and_then(Value::as_i64)
            .map(|value| format!("[{value}] "))
            .unwrap_or_default();
        let error_type = exception
            .get("type")
            .and_then(Value::as_str)
            .map(|value| format!("{value}: "))
            .unwrap_or_default();
        let message = exception
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or(fallback);
        return format!("{status}{error_type}{message}");
    }
    json_text(data, "message").unwrap_or_else(|| fallback.to_string())
}

async fn resolve_dynu_root(
    translator: &Translator,
    client: &reqwest::Client,
    api_key: &str,
    domain: &str,
) -> anyhow::Result<DynuRoot> {
    let root = dynu_request(
        translator,
        client,
        api_key,
        &format!("/dns/getroot/{}", url_encode_component(domain)),
        None,
    )
    .await?;
    let domain_id = read_positive_id(root.get("id")).ok_or_else(|| {
        anyhow::anyhow!(ddns_text(translator, "providers.dynu.invalidRootInfo", &[],))
    })?;
    let domain_name = normalize_domain(
        root.get("domainName")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    );
    if domain_name.is_empty() {
        return Err(anyhow::anyhow!(ddns_text(
            translator,
            "providers.dynu.invalidRootInfo",
            &[],
        )));
    }
    let node_name = normalize_dynu_node_name(root.get("node").and_then(Value::as_str))
        .if_empty(build_dynu_fallback_node_name(domain, &domain_name));
    Ok(DynuRoot {
        domain_id,
        domain_name,
        node_name,
    })
}

fn read_positive_id(value: Option<&Value>) -> Option<i64> {
    value
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
        })
        .filter(|value| *value > 0)
}

fn normalize_dynu_node_name(value: Option<&str>) -> String {
    let trimmed = value.unwrap_or_default().trim();
    if trimmed == "@" {
        String::new()
    } else {
        trimmed.to_string()
    }
}

trait EmptyDynuString {
    fn if_empty(self, fallback: String) -> String;
}

impl EmptyDynuString for String {
    fn if_empty(self, fallback: String) -> String {
        if self.is_empty() { fallback } else { self }
    }
}

fn build_dynu_fallback_node_name(domain: &str, root_domain: &str) -> String {
    let fqdn = normalize_domain(domain);
    let root = normalize_domain(root_domain);
    if fqdn.is_empty() || root.is_empty() || fqdn == root {
        return String::new();
    }
    let suffix = format!(".{root}");
    if fqdn.ends_with(&suffix) {
        fqdn[..fqdn.len() - suffix.len()].to_string()
    } else {
        String::new()
    }
}

fn find_dynu_record(
    records: &[Value],
    record_type: &str,
    domain: &str,
    node_name: &str,
) -> Option<Value> {
    let normalized_domain = normalize_domain(domain);
    let normalized_node = normalize_dynu_node_name(Some(node_name));
    let matching = records
        .iter()
        .filter(|record| {
            record
                .get("recordType")
                .and_then(Value::as_str)
                .is_some_and(|value| value.eq_ignore_ascii_case(record_type))
        })
        .collect::<Vec<_>>();
    if let Some(record) = matching
        .iter()
        .find(|record| build_dynu_record_hostname(record) == normalized_domain)
    {
        return Some((*record).clone());
    }
    if normalized_node.is_empty() {
        return None;
    }
    matching
        .into_iter()
        .find(|record| {
            normalize_dynu_node_name(record.get("nodeName").and_then(Value::as_str))
                == normalized_node
        })
        .cloned()
}

fn build_dynu_record_hostname(record: &Value) -> String {
    if let Some(hostname) = record.get("hostname").and_then(Value::as_str) {
        return normalize_domain(hostname);
    }
    let domain_name = normalize_domain(
        record
            .get("domainName")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    );
    if domain_name.is_empty() {
        return String::new();
    }
    let node_name = normalize_dynu_node_name(record.get("nodeName").and_then(Value::as_str));
    if node_name.is_empty() {
        domain_name
    } else {
        format!("{node_name}.{domain_name}")
    }
}

fn dynu_record_address(record: &Value, record_type: &str) -> String {
    let key = if record_type == "A" {
        "ipv4Address"
    } else {
        "ipv6Address"
    };
    record
        .get(key)
        .or_else(|| record.get("content"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn normalize_edgeone_location(value: Option<&str>) -> String {
    let trimmed = value.unwrap_or_default().trim();
    if trimmed.is_empty() {
        "default".to_string()
    } else {
        trimmed.to_ascii_lowercase()
    }
}

fn edgeone_api_host(endpoint: Option<&str>) -> String {
    let value = endpoint.unwrap_or_default().trim();
    if value.is_empty() {
        return "teo.tencentcloudapi.com".to_string();
    }
    if value.starts_with("http://") || value.starts_with("https://") {
        return url::Url::parse(value)
            .ok()
            .and_then(|url| url.host_str().map(str::to_string))
            .filter(|host| !host.is_empty())
            .unwrap_or_else(|| "teo.tencentcloudapi.com".to_string());
    }
    value.trim_end_matches('/').to_string()
}

fn is_valid_edgeone_host_header(value: &str) -> bool {
    let host = normalize_domain(value);
    if host.is_empty()
        || host.contains('/')
        || host.contains(':')
        || host.contains('[')
        || host.contains(']')
        || host.contains('*')
        || host.len() > 253
        || value.split_whitespace().count() > 1
        || value.starts_with("http://")
        || value.starts_with("https://")
    {
        return false;
    }
    host.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
    })
}

fn insert_json_field(object: &mut Value, key: &str, value: Value) {
    if let Some(map) = object.as_object_mut() {
        map.insert(key.to_string(), value);
    }
}

fn baidu_bce_authorization(
    method: &str,
    url: &str,
    access_key_id: &str,
    secret_access_key: &str,
) -> anyhow::Result<(String, String)> {
    let url = url::Url::parse(url)?;
    let timestamp = iso8601_utc_without_millis();
    let signed_header_names = ["content-type", "host", "x-bce-date"];
    let header_values = [
        ("content-type", "application/json"),
        ("host", url.host_str().unwrap_or_default()),
        ("x-bce-date", timestamp.as_str()),
    ];
    let canonical_headers = signed_header_names
        .iter()
        .filter_map(|name| {
            header_values
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| format!("{name}:{}", rfc3986_encode(value.trim())))
        })
        .collect::<Vec<_>>()
        .join("\n");
    let auth_string_prefix = format!("bce-auth-v1/{access_key_id}/{timestamp}/1800");
    let signing_key = hmac_sha256_hex(secret_access_key.as_bytes(), auth_string_prefix.as_bytes());
    let canonical_request = [
        method,
        url.path(),
        &canonical_query_from_url(&url),
        &canonical_headers,
    ]
    .join("\n");
    let signature = hmac_sha256_hex(signing_key.as_bytes(), canonical_request.as_bytes());
    Ok((
        timestamp,
        format!(
            "{auth_string_prefix}/{}/{}",
            signed_header_names.join(";"),
            signature
        ),
    ))
}

fn huawei_sdk_authorization(
    method: &str,
    url: &str,
    content_type: &str,
    access_key_id: &str,
    secret_access_key: &str,
    payload: &str,
) -> anyhow::Result<(String, String)> {
    let url = url::Url::parse(url)?;
    let x_sdk_date = compact_utc_timestamp();
    let canonical_uri = canonical_huawei_uri(url.path());
    let canonical_query = canonical_query_from_url(&url);
    let payload_hash = sha256_hex(payload);
    let canonical_headers = format!(
        "content-type:{}\nhost:{}\nx-sdk-date:{}\n",
        content_type.trim(),
        url.host_str().unwrap_or_default(),
        x_sdk_date
    );
    let signed_headers = "content-type;host;x-sdk-date";
    let canonical_request = [
        method,
        &canonical_uri,
        &canonical_query,
        &canonical_headers,
        signed_headers,
        &payload_hash,
    ]
    .join("\n");
    let string_to_sign = format!(
        "SDK-HMAC-SHA256\n{}\n{}",
        x_sdk_date,
        sha256_hex(&canonical_request)
    );
    let signature = hmac_sha256_hex(secret_access_key.as_bytes(), string_to_sign.as_bytes());
    Ok((
        x_sdk_date,
        format!(
            "SDK-HMAC-SHA256 Access={access_key_id}, SignedHeaders={signed_headers}, Signature={signature}"
        ),
    ))
}

fn build_aliyun_signed_params(
    access_key_id: &str,
    access_key_secret: &str,
    extra_params: Vec<(&str, String)>,
    method: &str,
) -> String {
    let mut params = vec![
        ("AccessKeyId".to_string(), access_key_id.to_string()),
        ("Format".to_string(), "JSON".to_string()),
        ("SignatureMethod".to_string(), "HMAC-SHA1".to_string()),
        (
            "SignatureNonce".to_string(),
            uuid::Uuid::new_v4().to_string(),
        ),
        ("SignatureVersion".to_string(), "1.0".to_string()),
        ("Timestamp".to_string(), iso8601_utc_without_millis()),
        ("Version".to_string(), "2015-01-09".to_string()),
    ];
    params.extend(
        extra_params
            .into_iter()
            .map(|(key, value)| (key.to_string(), value)),
    );
    params.sort_by(|left, right| left.0.cmp(&right.0));
    let canonicalized = params
        .iter()
        .map(|(key, value)| format!("{}={}", rfc3986_encode(key), rfc3986_encode(value)))
        .collect::<Vec<_>>()
        .join("&");
    let string_to_sign = format!(
        "{}&{}&{}",
        method,
        rfc3986_encode("/"),
        rfc3986_encode(&canonicalized)
    );
    let signature = hmac_sha1_base64(
        format!("{access_key_secret}&").as_bytes(),
        string_to_sign.as_bytes(),
    );
    params.push(("Signature".to_string(), signature));
    params
        .iter()
        .map(|(key, value)| format!("{}={}", rfc3986_encode(key), rfc3986_encode(value)))
        .collect::<Vec<_>>()
        .join("&")
}

async fn aliyun_acs3_request(
    translator: &Translator,
    client: &reqwest::Client,
    access_key_id: &str,
    access_key_secret: &str,
    action: &str,
    version: &str,
    method: &str,
    query: Vec<(String, String)>,
    form_data: Vec<(String, String)>,
) -> anyhow::Result<Value> {
    let endpoint = "https://esa.cn-hangzhou.aliyuncs.com/";
    let url = url::Url::parse(endpoint)?;
    let query_string = aliyun_canonical_param_string(&query);
    let body_string = aliyun_canonical_param_string(&form_data);
    let payload_hash = sha256_hex(&body_string);
    let acs_date = iso8601_utc_without_millis();
    let nonce = uuid::Uuid::new_v4().to_string();
    let mut headers = vec![
        (
            "host".to_string(),
            url.host_str().unwrap_or_default().to_string(),
        ),
        ("x-acs-action".to_string(), action.to_string()),
        ("x-acs-content-sha256".to_string(), payload_hash.clone()),
        ("x-acs-date".to_string(), acs_date.clone()),
        ("x-acs-signature-nonce".to_string(), nonce),
        ("x-acs-version".to_string(), version.to_string()),
    ];
    if !body_string.is_empty() {
        headers.push((
            "content-type".to_string(),
            "application/x-www-form-urlencoded".to_string(),
        ));
    }
    headers.sort_by(|left, right| left.0.cmp(&right.0));
    let canonical_headers = format!(
        "{}\n",
        headers
            .iter()
            .map(|(key, value)| format!("{key}:{}", value.trim()))
            .collect::<Vec<_>>()
            .join("\n")
    );
    let signed_headers = headers
        .iter()
        .map(|(key, _)| key.as_str())
        .collect::<Vec<_>>()
        .join(";");
    let canonical_request = [
        method,
        url.path(),
        &query_string,
        &canonical_headers,
        &signed_headers,
        &payload_hash,
    ]
    .join("\n");
    let string_to_sign = format!("ACS3-HMAC-SHA256\n{}", sha256_hex(&canonical_request));
    let signature = hmac_sha256_hex(access_key_secret.as_bytes(), string_to_sign.as_bytes());
    let authorization = format!(
        "ACS3-HMAC-SHA256 Credential={access_key_id},SignedHeaders={signed_headers},Signature={signature}"
    );
    let request_url = if query_string.is_empty() {
        endpoint.to_string()
    } else {
        format!("{endpoint}?{query_string}")
    };
    let method = reqwest::Method::from_bytes(method.as_bytes())?;
    let mut request = client
        .request(method, request_url)
        .header(reqwest::header::HOST, url.host_str().unwrap_or_default())
        .header("x-acs-action", action)
        .header("x-acs-content-sha256", payload_hash)
        .header("x-acs-date", acs_date)
        .header(
            "x-acs-signature-nonce",
            headers
                .iter()
                .find(|(key, _)| key == "x-acs-signature-nonce")
                .map(|(_, value)| value.as_str())
                .unwrap_or_default(),
        )
        .header("x-acs-version", version)
        .header(reqwest::header::AUTHORIZATION, authorization);
    if !body_string.is_empty() {
        request = request
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body(body_string);
    }
    let (status, data, text) = response_json(translator, request.send().await?).await?;
    if !status.is_success() || data.get("Code").is_some() {
        return Err(anyhow::anyhow!(
            "{}: {}",
            data.get("Code")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| format!("HTTP {}", status.as_u16())),
            data.get("Message")
                .and_then(Value::as_str)
                .unwrap_or(if text.is_empty() {
                    "Aliyun ACS3 request failed"
                } else {
                    &text
                })
        ));
    }
    Ok(data)
}

fn aliyun_canonical_param_string(params: &[(String, String)]) -> String {
    let mut values = params.to_vec();
    values.sort_by(|left, right| {
        let key_order = left.0.cmp(&right.0);
        if key_order == std::cmp::Ordering::Equal {
            left.1.cmp(&right.1)
        } else {
            key_order
        }
    });
    values
        .into_iter()
        .map(|(key, value)| format!("{}={}", rfc3986_encode(&key), rfc3986_encode(&value)))
        .collect::<Vec<_>>()
        .join("&")
}

fn esa_record_payload(
    value: &str,
    ttl: i64,
    proxied: bool,
    biz_name: &str,
) -> Vec<(String, String)> {
    let mut payload = vec![
        ("Data".to_string(), json!({ "Value": value }).to_string()),
        ("Proxied".to_string(), proxied.to_string()),
        ("Ttl".to_string(), ttl.to_string()),
        ("Type".to_string(), "A/AAAA".to_string()),
    ];
    if proxied {
        payload.push((
            "BizName".to_string(),
            default_string(biz_name.to_string(), "web"),
        ));
    }
    payload
}

fn value_to_compact_string(value: &Value) -> String {
    match value {
        Value::String(value) => value.trim().to_string(),
        Value::Number(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        _ => String::new(),
    }
}

fn same_csv_values(left: &str, right: &str) -> bool {
    let mut left_values = left
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    let mut right_values = right
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    left_values.sort_unstable();
    right_values.sort_unstable();
    left_values == right_values
}

fn form_body(params: &[(&str, String)]) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in params {
        serializer.append_pair(key, value);
    }
    serializer.finish()
}

fn default_string(value: String, fallback: &str) -> String {
    if value.trim().is_empty() {
        fallback.to_string()
    } else {
        value
    }
}

fn iso8601_utc_without_millis() -> String {
    let value = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| time_utils::now_iso());
    strip_fractional_seconds(&value)
}

fn utc_date(timestamp: i64) -> anyhow::Result<String> {
    let value = OffsetDateTime::from_unix_timestamp(timestamp)?
        .format(&Rfc3339)
        .unwrap_or_else(|_| time_utils::now_iso());
    Ok(strip_fractional_seconds(&value).chars().take(10).collect())
}

fn strip_fractional_seconds(value: &str) -> String {
    if let Some(dot) = value.find('.')
        && let Some(z_index) = value[dot..].find('Z')
    {
        return format!("{}Z", &value[..dot + z_index]);
    }
    value.to_string()
}

fn compact_utc_timestamp() -> String {
    iso8601_utc_without_millis()
        .replace(['-', ':'], "")
        .replace('Z', "Z")
}

fn canonical_query_from_url(url: &url::Url) -> String {
    let mut pairs = url
        .query_pairs()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect::<Vec<_>>();
    pairs.sort_by(|left, right| {
        let key_order = rfc3986_encode(&left.0).cmp(&rfc3986_encode(&right.0));
        if key_order == std::cmp::Ordering::Equal {
            rfc3986_encode(&left.1).cmp(&rfc3986_encode(&right.1))
        } else {
            key_order
        }
    });
    pairs
        .into_iter()
        .map(|(key, value)| format!("{}={}", rfc3986_encode(&key), rfc3986_encode(&value)))
        .collect::<Vec<_>>()
        .join("&")
}

fn canonical_huawei_uri(path: &str) -> String {
    let mut uri = path
        .split('/')
        .map(rfc3986_encode)
        .collect::<Vec<_>>()
        .join("/");
    if !uri.starts_with('/') {
        uri.insert(0, '/');
    }
    if !uri.ends_with('/') {
        uri.push('/');
    }
    uri
}

fn rfc3986_encode(value: &str) -> String {
    let mut output = String::new();
    for byte in value.bytes() {
        let ch = byte as char;
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '~') {
            output.push(ch);
        } else {
            output.push_str(&format!("%{byte:02X}"));
        }
    }
    output
}

fn sha256_hex(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    hex::encode(digest)
}

fn hmac_sha1_base64(key: &[u8], payload: &[u8]) -> String {
    let mut mac = HmacSha1::new_from_slice(key).expect("HMAC accepts keys of any size");
    mac.update(payload);
    BASE64_STANDARD.encode(mac.finalize().into_bytes())
}

fn hmac_sha256_bytes(key: &[u8], payload: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts keys of any size");
    mac.update(payload);
    mac.finalize().into_bytes().to_vec()
}

fn hmac_sha256_hex(key: &[u8], payload: &[u8]) -> String {
    hex::encode(hmac_sha256_bytes(key, payload))
}

fn compact_json(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| value.to_string())
}

fn json_text(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn url_encode_component(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

fn build_query_url(base: &str, pairs: &[(&str, String)]) -> String {
    let query = pairs
        .iter()
        .fold(
            url::form_urlencoded::Serializer::new(String::new()),
            |mut serializer, (key, value)| {
                serializer.append_pair(key, value);
                serializer
            },
        )
        .finish();
    if query.is_empty() {
        base.to_string()
    } else {
        format!("{base}?{query}")
    }
}

fn ddns_http_client() -> anyhow::Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .timeout(Duration::from_millis(env_u64("DDNS_TIMEOUT_MS", 15_000)))
        .build()?)
}

fn env_u64(name: &str, fallback: u64) -> u64 {
    env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(fallback)
}

async fn response_text(response: reqwest::Response) -> anyhow::Result<String> {
    Ok(response.text().await?.trim().to_string())
}

async fn clear_logs(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.redis.clear_log_buffer(DDNS_LOGS).await {
        Ok(()) => response::success_empty().into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to clear DDNS logs");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ddns_text(&translator, "logsClearFailed", &[]),
            )
        }
    }
}

async fn poll(State(state): State<AppState>, Query(query): Query<PollQuery>) -> Response {
    let translator = Translator::from_state(&state).await;
    let logs = match state
        .redis
        .poll_log_buffer(DDNS_LOGS, query.cursor.as_deref())
        .await
    {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, "failed to poll DDNS logs");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ddns_text(&translator, "pollFailed", &[]),
            );
        }
    };
    let status = match build_ddns_status(&state, &translator).await {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, "failed to build DDNS poll status");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ddns_text(&translator, "pollFailed", &[]),
            );
        }
    };
    response::ok(json!({
        "cursor": logs.get("cursor").cloned().unwrap_or(json!(0)),
        "reset": logs.get("reset").cloned().unwrap_or(json!(false)),
        "logs": parse_log_entries(logs.get("items").and_then(Value::as_array).cloned().unwrap_or_default().into_iter().filter_map(|item| item.as_str().map(str::to_string)).collect()),
        "status": status
    }))
    .into_response()
}

async fn build_ddns_status(state: &AppState, translator: &Translator) -> anyhow::Result<Value> {
    let enabled = state.redis.get_string_value(DDNS_ENABLED).await?.as_deref() == Some("true");
    let settings = parse_settings(
        state
            .redis
            .get_string_value(DDNS_SETTINGS)
            .await?
            .as_deref(),
    );
    let targets = list_targets(state).await?;
    let primary = targets
        .iter()
        .find(|target| target.meta.is_primary)
        .or_else(|| targets.first())
        .cloned()
        .unwrap_or_else(default_primary_target);
    let summaries = targets
        .iter()
        .map(|target| target_summary(target, translator))
        .collect::<Vec<_>>();
    let primary_target_id = summaries
        .iter()
        .find(|item| item.get("isPrimary").and_then(Value::as_bool) == Some(true))
        .and_then(|item| item.get("id").and_then(Value::as_str))
        .map(str::to_string);
    let extra_count = summaries
        .iter()
        .filter(|item| item.get("isPrimary").and_then(Value::as_bool) != Some(true))
        .count();
    let enabled_extra_count = summaries
        .iter()
        .filter(|item| {
            item.get("isPrimary").and_then(Value::as_bool) != Some(true)
                && item.get("enabled").and_then(Value::as_bool) == Some(true)
        })
        .count();

    Ok(json!({
        "enabled": enabled,
        "provider": primary.meta.provider,
        "updateIntervalMinutes": settings.get("updateIntervalMinutes").cloned().unwrap_or(json!(10)),
        "publicCheckSources": settings.get("publicCheckSources").cloned().unwrap_or_else(default_public_check_sources),
        "defaultPublicCheckSources": settings.get("defaultPublicCheckSources").cloned().unwrap_or_else(default_public_check_sources),
        "httpTransport": settings.get("httpTransport").cloned().unwrap_or(json!("curl")),
        "updateScope": normalize_update_scope(primary.config.get("update_scope").map(String::as_str)),
        "ipSource": normalize_ip_source(primary.config.get("ip_source").map(String::as_str)),
        "networkInterface": normalize_network_interface(primary.config.get("network_interface").map(String::as_str)),
        "lastIP": primary.last_ip,
        "lastCheck": primary.last_check,
        "primaryTargetId": primary_target_id,
        "extraTargetCount": extra_count,
        "enabledExtraTargetCount": enabled_extra_count,
        "targets": summaries
    }))
}

async fn list_targets(state: &AppState) -> anyhow::Result<Vec<DDNSTargetRecord>> {
    let primary_id = state
        .redis
        .get_string_value(DDNS_PRIMARY_TARGET_ID)
        .await?
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| PRIMARY_TARGET_ID.to_string());
    let mut ids = BTreeSet::new();
    ids.insert(primary_id.clone());
    for id in state.redis.smembers_strings(DDNS_TARGET_IDS).await? {
        let id = id.trim();
        if !id.is_empty() {
            ids.insert(id.to_string());
        }
    }

    let mut targets = Vec::new();
    for id in ids {
        if let Some(target) = read_target(state, &id, id == primary_id).await? {
            targets.push(target);
        }
    }
    if targets.iter().all(|target| !target.meta.is_primary) {
        targets.push(read_legacy_primary_target(state).await?);
    }
    targets.sort_by(compare_targets);
    Ok(targets)
}

async fn read_target(
    state: &AppState,
    id: &str,
    primary_hint: bool,
) -> anyhow::Result<Option<DDNSTargetRecord>> {
    let meta_key = target_meta_key(id);
    let meta_hash = state.redis.hgetall_string_map(&meta_key).await?;
    if meta_hash.is_empty() {
        if id == PRIMARY_TARGET_ID || primary_hint {
            return Ok(Some(read_legacy_primary_target(state).await?));
        }
        return Ok(None);
    }
    let meta = parse_target_meta(id, &meta_hash, primary_hint);
    let config = state
        .redis
        .hgetall_string_map(&target_config_key(id))
        .await?;
    let last_ip = parse_last_ip(
        &state
            .redis
            .hgetall_string_map(&target_last_ip_key(id))
            .await?,
    );
    let last_check = parse_last_check(
        &state
            .redis
            .hgetall_string_map(&target_last_check_key(id))
            .await?,
    );
    Ok(Some(DDNSTargetRecord {
        meta,
        config,
        last_ip,
        last_check,
    }))
}

async fn read_legacy_primary_target(state: &AppState) -> anyhow::Result<DDNSTargetRecord> {
    let provider = state
        .redis
        .get_string_value(DDNS_LEGACY_PROVIDER)
        .await?
        .and_then(|value| normalize_provider_name(&value));
    let config = if let Some(provider) = provider.as_deref() {
        state
            .redis
            .hgetall_string_map(&(DDNS_LEGACY_CONFIG_PREFIX.to_string() + provider))
            .await?
    } else {
        HashMap::new()
    };
    let last_ip = parse_last_ip(&state.redis.hgetall_string_map(DDNS_LEGACY_LAST_IP).await?);
    let last_check = parse_last_check(
        &state
            .redis
            .hgetall_string_map(DDNS_LEGACY_LAST_CHECK)
            .await?,
    );
    let now = time_utils::now_iso();
    Ok(DDNSTargetRecord {
        meta: DDNSTargetMeta {
            id: PRIMARY_TARGET_ID.to_string(),
            name: PRIMARY_TARGET_NAME.to_string(),
            is_primary: true,
            enabled: true,
            provider,
            created_at: now.clone(),
            updated_at: now,
            sort_order: 0,
        },
        config,
        last_ip,
        last_check,
    })
}

fn default_primary_target() -> DDNSTargetRecord {
    let now = time_utils::now_iso();
    DDNSTargetRecord {
        meta: DDNSTargetMeta {
            id: PRIMARY_TARGET_ID.to_string(),
            name: PRIMARY_TARGET_NAME.to_string(),
            is_primary: true,
            enabled: true,
            provider: None,
            created_at: now.clone(),
            updated_at: now,
            sort_order: 0,
        },
        config: HashMap::new(),
        last_ip: empty_last_ip(),
        last_check: empty_last_check(),
    }
}

fn parse_target_meta(
    id: &str,
    data: &HashMap<String, String>,
    primary_hint: bool,
) -> DDNSTargetMeta {
    let now = time_utils::now_iso();
    let is_primary = data.get("is_primary").map(String::as_str) == Some("true")
        || id == PRIMARY_TARGET_ID
        || primary_hint;
    DDNSTargetMeta {
        id: id.to_string(),
        name: data
            .get("name")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| {
                if is_primary {
                    PRIMARY_TARGET_NAME.to_string()
                } else {
                    String::new()
                }
            }),
        is_primary,
        enabled: if is_primary {
            true
        } else {
            data.get("enabled").map(String::as_str) != Some("false")
        },
        provider: data
            .get("provider")
            .and_then(|value| normalize_provider_name(value)),
        created_at: data
            .get("created_at")
            .cloned()
            .unwrap_or_else(|| now.clone()),
        updated_at: data
            .get("updated_at")
            .or_else(|| data.get("created_at"))
            .cloned()
            .unwrap_or(now),
        sort_order: data
            .get("sort_order")
            .and_then(|value| value.parse::<i64>().ok())
            .unwrap_or(if is_primary { 0 } else { 1 }),
    }
}

fn target_summary(target: &DDNSTargetRecord, translator: &Translator) -> Value {
    let provider_label = provider_label(target.meta.provider.as_deref(), translator);
    let domain_summary =
        domain_summary(target.meta.provider.as_deref(), &target.config, translator);
    let name = if !target.meta.name.trim().is_empty() {
        target.meta.name.trim().to_string()
    } else if target.meta.is_primary {
        ddns_text(translator, "primaryDomainName", &[])
    } else if !domain_summary.is_empty() {
        domain_summary.clone()
    } else {
        provider_label.clone()
    };

    json!({
        "id": target.meta.id,
        "name": name,
        "isPrimary": target.meta.is_primary,
        "enabled": if target.meta.is_primary { true } else { target.meta.enabled },
        "provider": target.meta.provider,
        "updateScope": normalize_update_scope(target.config.get("update_scope").map(String::as_str)),
        "providerLabel": provider_label,
        "domainSummary": domain_summary,
        "createdAt": target.meta.created_at,
        "updatedAt": target.meta.updated_at,
        "sortOrder": target.meta.sort_order,
        "lastIP": target.last_ip,
        "lastCheck": target.last_check
    })
}

fn compare_targets(left: &DDNSTargetRecord, right: &DDNSTargetRecord) -> std::cmp::Ordering {
    match (left.meta.is_primary, right.meta.is_primary) {
        (true, false) => return std::cmp::Ordering::Less,
        (false, true) => return std::cmp::Ordering::Greater,
        _ => {}
    }
    left.meta
        .sort_order
        .cmp(&right.meta.sort_order)
        .then_with(|| left.meta.created_at.cmp(&right.meta.created_at))
        .then_with(|| left.meta.id.cmp(&right.meta.id))
}

async fn test_public_check_sources_inner(
    sources: &Value,
    _transport: &str,
    translator: &Translator,
) -> anyhow::Result<Vec<Value>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(IP_DETECTION_TIMEOUT_MS))
        .redirect(reqwest::redirect::Policy::limited(20))
        .build()?;
    let mut results = Vec::new();
    for (family, version) in [("ipv4", 4_u8), ("ipv6", 6_u8)] {
        let urls = sources
            .get(family)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for url in urls
            .into_iter()
            .filter_map(|value| value.as_str().map(str::to_string))
        {
            results.push(
                test_single_public_check_source(&client, &url, family, version, translator).await,
            );
        }
    }
    Ok(results)
}

async fn test_single_public_check_source(
    client: &reqwest::Client,
    url: &str,
    family: &str,
    version: u8,
    translator: &Translator,
) -> Value {
    match client
        .get(url)
        .header("Accept", "application/json, text/plain")
        .send()
        .await
    {
        Ok(response) => {
            let status = response.status().as_u16();
            let ok = response.status().is_success();
            let text = response.text().await.unwrap_or_default();
            let preview = response_preview(&text);
            if !ok {
                return json!({
                    "family": family,
                    "url": url,
                    "success": false,
                    "status": status,
                    "ip": null,
                    "responsePreview": preview,
                    "error": public_check_request_failed_message(translator, url, status)
                });
            }
            let ip = parse_detected_ip_text(&text, version);
            if let Some(ip) = ip {
                json!({
                    "family": family,
                    "url": url,
                    "success": true,
                    "status": status,
                    "ip": ip,
                    "responsePreview": preview
                })
            } else {
                json!({
                    "family": family,
                    "url": url,
                    "success": false,
                    "status": status,
                    "ip": null,
                    "responsePreview": preview,
                    "error": public_check_invalid_payload_message(translator, url, version)
                })
            }
        }
        Err(error) => json!({
            "family": family,
            "url": url,
            "success": false,
            "status": null,
            "ip": null,
            "error": error.to_string()
        }),
    }
}

fn public_check_request_failed_message(translator: &Translator, url: &str, status: u16) -> String {
    ddns_text(
        translator,
        "publicCheckSourceRequestFailed",
        &[("url", url.to_string()), ("status", status.to_string())],
    )
}

fn public_check_invalid_payload_message(translator: &Translator, url: &str, version: u8) -> String {
    ddns_text(
        translator,
        "publicCheckSourceInvalidPayload",
        &[
            ("url", url.to_string()),
            (
                "family",
                if version == 4 { "IPv4" } else { "IPv6" }.to_string(),
            ),
        ],
    )
}

fn parse_detected_ip_text(text: &str, version: u8) -> Option<String> {
    parse_detected_ip(text.trim(), version).or_else(|| {
        let value = serde_json::from_str::<Value>(text).ok()?;
        if let Some(ip) = value.get("ip").and_then(Value::as_str) {
            return parse_detected_ip(ip, version);
        }
        if let Some(ip) = value.get("address").and_then(Value::as_str) {
            return parse_detected_ip(ip, version);
        }
        value
            .as_str()
            .and_then(|value| parse_detected_ip(value, version))
    })
}

fn parse_detected_ip(value: &str, version: u8) -> Option<String> {
    let ip = value.trim().parse::<IpAddr>().ok()?;
    match (version, ip) {
        (4, IpAddr::V4(_)) | (6, IpAddr::V6(_)) => Some(value.trim().to_string()),
        _ => None,
    }
}

fn response_preview(text: &str) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.len() > RESPONSE_PREVIEW_MAX_LENGTH {
        format!("{}...", &normalized[..RESPONSE_PREVIEW_MAX_LENGTH])
    } else {
        normalized
    }
}

fn list_ddns_network_interfaces() -> Vec<Value> {
    let mut interfaces = list_docker_host_ipv6_interfaces();
    let mut runtime = HashMap::<String, Vec<Value>>::new();
    if let Ok(addrs) = get_if_addrs() {
        for iface in addrs {
            if iface.is_loopback() {
                continue;
            }
            let address = match iface.addr {
                IfAddr::V4(addr) if is_usable_ipv4(addr.ip) => json!({
                    "family": "ipv4",
                    "address": addr.ip.to_string(),
                    "cidr": format!("{}/{}", addr.ip, ipv4_prefix_len(addr.netmask)),
                    "internal": false,
                    "source": "runtime"
                }),
                IfAddr::V6(addr) if is_usable_ipv6(addr.ip) => json!({
                    "family": "ipv6",
                    "address": addr.ip.to_string(),
                    "cidr": format!("{}/{}", addr.ip, ipv6_prefix_len(addr.netmask)),
                    "internal": false,
                    "source": "runtime"
                }),
                _ => continue,
            };
            runtime.entry(iface.name).or_default().push(address);
        }
    }

    let mut runtime_items = runtime
        .into_iter()
        .filter_map(|(name, addresses)| interface_option(&name, "runtime", addresses))
        .collect::<Vec<_>>();
    runtime_items.sort_by(|left, right| {
        left.get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .cmp(right.get("name").and_then(Value::as_str).unwrap_or(""))
    });
    interfaces.extend(runtime_items);
    interfaces.sort_by(|left, right| {
        let left_source = left
            .get("source")
            .and_then(Value::as_str)
            .unwrap_or("runtime");
        let right_source = right
            .get("source")
            .and_then(Value::as_str)
            .unwrap_or("runtime");
        if left_source != right_source {
            return if left_source == "docker_host" {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            };
        }
        left.get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .cmp(right.get("name").and_then(Value::as_str).unwrap_or(""))
    });
    interfaces
}

fn interface_option(name: &str, source: &str, addresses: Vec<Value>) -> Option<Value> {
    if addresses.is_empty() {
        return None;
    }
    let selectable = addresses
        .iter()
        .filter(|item| is_selectable_interface_address(item))
        .cloned()
        .collect::<Vec<_>>();
    let summary = addresses
        .iter()
        .filter_map(|item| {
            let family = item.get("family").and_then(Value::as_str)?;
            let address = item.get("address").and_then(Value::as_str)?;
            Some(format!(
                "{}: {}",
                if family == "ipv4" { "IPv4" } else { "IPv6" },
                address
            ))
        })
        .collect::<Vec<_>>()
        .join(" / ");
    if selectable.is_empty() {
        return None;
    }
    Some(json!({
        "name": name,
        "label": format!("{name} ({summary})"),
        "summary": summary,
        "source": source,
        "hasIpv4": addresses.iter().any(|item| item.get("family").and_then(Value::as_str) == Some("ipv4")),
        "hasIpv6": addresses.iter().any(|item| item.get("family").and_then(Value::as_str) == Some("ipv6")),
        "addresses": addresses,
        "selectableAddresses": selectable
    }))
}

fn list_docker_host_ipv6_interfaces() -> Vec<Value> {
    let path = env::var("DDNS_HOST_IF_INET6_PATH")
        .unwrap_or_else(|_| DEFAULT_DOCKER_HOST_IF_INET6_PATH.to_string());
    fs::read_to_string(path)
        .ok()
        .map(|content| parse_host_if_inet6(&content))
        .unwrap_or_default()
}

fn parse_host_if_inet6(content: &str) -> Vec<Value> {
    let mut by_interface = HashMap::<String, Vec<Value>>::new();
    for line in content.lines() {
        let parts = line.split_whitespace().collect::<Vec<_>>();
        if parts.len() < 6 {
            continue;
        }
        let Some(address) = format_ipv6_from_proc_hex(parts[0]) else {
            continue;
        };
        let prefix_len = u8::from_str_radix(parts[2], 16).unwrap_or(0);
        let scope = u8::from_str_radix(parts[3], 16).unwrap_or(255);
        if scope != 0 {
            continue;
        }
        let Ok(ip) = address.parse::<Ipv6Addr>() else {
            continue;
        };
        if !is_usable_ipv6(ip) {
            continue;
        }
        let name = parts[5].to_string();
        by_interface.entry(name).or_default().push(json!({
            "family": "ipv6",
            "address": address,
            "cidr": format!("{address}/{prefix_len}"),
            "internal": false,
            "source": "docker_host"
        }));
    }
    let mut items = by_interface
        .into_iter()
        .filter_map(|(name, mut addresses)| {
            addresses.sort_by(|left, right| {
                left.get("address")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .cmp(right.get("address").and_then(Value::as_str).unwrap_or(""))
            });
            interface_option(
                &format!("{DOCKER_HOST_INTERFACE_PREFIX}{name}"),
                "docker_host",
                addresses,
            )
        })
        .collect::<Vec<_>>();
    items.sort_by(|left, right| {
        left.get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .cmp(right.get("name").and_then(Value::as_str).unwrap_or(""))
    });
    items
}

fn format_ipv6_from_proc_hex(value: &str) -> Option<String> {
    if value.len() != 32 || !value.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return None;
    }
    let mut segments = Vec::new();
    for chunk in value.as_bytes().chunks(4) {
        let raw = std::str::from_utf8(chunk).ok()?;
        segments.push(u16::from_str_radix(raw, 16).ok()?);
    }
    Some(
        Ipv6Addr::new(
            segments[0],
            segments[1],
            segments[2],
            segments[3],
            segments[4],
            segments[5],
            segments[6],
            segments[7],
        )
        .to_string(),
    )
}

fn is_selectable_interface_address(value: &Value) -> bool {
    let Some(address) = value.get("address").and_then(Value::as_str) else {
        return false;
    };
    match value.get("family").and_then(Value::as_str) {
        Some("ipv4") => address
            .parse::<Ipv4Addr>()
            .is_ok_and(|ip| !is_private_ipv4(ip)),
        Some("ipv6") => address
            .parse::<Ipv6Addr>()
            .is_ok_and(|ip| !is_unique_local_ipv6(ip)),
        _ => false,
    }
}

fn is_usable_ipv4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    octets[0] != 127 && !(octets[0] == 169 && octets[1] == 254)
}

fn is_usable_ipv6(ip: Ipv6Addr) -> bool {
    !(ip.is_loopback() || ip.is_unicast_link_local() || ip.is_unspecified())
}

fn is_private_ipv4(ip: Ipv4Addr) -> bool {
    ip.is_private()
}

fn is_unique_local_ipv6(ip: Ipv6Addr) -> bool {
    let first = ip.octets()[0];
    first == 0xfc || first == 0xfd
}

fn ipv4_prefix_len(mask: Ipv4Addr) -> u32 {
    mask.octets().iter().map(|byte| byte.count_ones()).sum()
}

fn ipv6_prefix_len(mask: Ipv6Addr) -> u32 {
    mask.octets().iter().map(|byte| byte.count_ones()).sum()
}

fn parse_settings(raw: Option<&str>) -> Value {
    let parsed = raw.and_then(|value| serde_json::from_str::<Value>(value).ok());
    let default_sources = default_public_check_sources();
    let public_sources = parsed
        .as_ref()
        .and_then(|value| value.get("publicCheckSources"))
        .map(normalize_public_check_sources)
        .unwrap_or_else(default_public_check_sources);
    json!({
        "updateIntervalMinutes": parsed
            .as_ref()
            .and_then(|value| value.get("updateIntervalMinutes"))
            .and_then(normalize_update_interval_minutes)
            .unwrap_or(10),
        "publicCheckSources": public_sources,
        "defaultPublicCheckSources": default_sources,
        "httpTransport": normalize_http_transport(parsed.as_ref().and_then(|value| value.get("httpTransport")))
    })
}

fn normalize_public_check_sources(value: &Value) -> Value {
    let fallback = default_public_check_sources();
    json!({
        "ipv4": normalize_public_check_source_list(
            value.get("ipv4"),
            fallback.get("ipv4").and_then(Value::as_array).cloned().unwrap_or_default(),
        ),
        "ipv6": normalize_public_check_source_list(
            value.get("ipv6"),
            fallback.get("ipv6").and_then(Value::as_array).cloned().unwrap_or_default(),
        )
    })
}

fn normalize_public_check_sources_strict(
    value: &Value,
    fallback: &Value,
    translator: &Translator,
) -> Result<Value, String> {
    if !value.is_object() {
        return Err(ddns_text(
            translator,
            "publicCheckSourceInvalidUrl",
            &[
                ("family", "IPv4/IPv6".to_string()),
                ("source", public_check_source_value_string(value)),
            ],
        ));
    }
    Ok(json!({
        "ipv4": normalize_public_check_source_list_strict(
            value.get("ipv4"),
            "ipv4",
            fallback
                .get("ipv4")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
            translator,
        )?,
        "ipv6": normalize_public_check_source_list_strict(
            value.get("ipv6"),
            "ipv6",
            fallback
                .get("ipv6")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
            translator,
        )?
    }))
}

fn normalize_public_check_source_list(value: Option<&Value>, fallback: Vec<Value>) -> Vec<Value> {
    let Some(items) = value.and_then(Value::as_array) else {
        return fallback;
    };
    let mut seen = BTreeSet::new();
    let mut normalized = Vec::new();
    for item in items.iter().filter_map(Value::as_str) {
        let source = item.trim();
        if source.is_empty() {
            continue;
        }
        let candidate = if has_explicit_scheme(source) {
            source.to_string()
        } else {
            format!("https://{source}")
        };
        if !candidate.starts_with("http://") && !candidate.starts_with("https://") {
            continue;
        }
        if seen.insert(candidate.clone()) {
            normalized.push(Value::String(candidate));
        }
    }
    if normalized.is_empty() {
        fallback
    } else {
        normalized
    }
}

fn normalize_public_check_source_list_strict(
    value: Option<&Value>,
    family: &str,
    fallback: Vec<Value>,
    translator: &Translator,
) -> Result<Vec<Value>, String> {
    let Some(value) = value else {
        return Ok(fallback);
    };
    let Some(items) = value.as_array() else {
        return Err(ddns_text(
            translator,
            "publicCheckSourceInvalidUrl",
            &[
                ("family", public_check_family_label(family).to_string()),
                ("source", public_check_source_value_string(value)),
            ],
        ));
    };
    let mut seen = BTreeSet::new();
    let mut normalized = Vec::new();
    for item in items {
        let source = normalize_public_check_source_strict(item, family, translator)?;
        if seen.insert(source.clone()) {
            normalized.push(Value::String(source));
        }
    }
    Ok(normalized)
}

fn normalize_public_check_source_strict(
    value: &Value,
    family: &str,
    translator: &Translator,
) -> Result<String, String> {
    let source = public_check_source_value_string(value);
    let family_label = public_check_family_label(family);
    if source.is_empty() {
        return Err(ddns_text(
            translator,
            "publicCheckSourceEmpty",
            &[("family", family_label.to_string())],
        ));
    }

    let candidate = build_public_check_candidate_url(&source, family_label, translator)?;
    let parsed = Url::parse(&candidate).map_err(|_| {
        ddns_text(
            translator,
            "publicCheckSourceInvalidUrl",
            &[
                ("family", family_label.to_string()),
                ("source", source.clone()),
            ],
        )
    })?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err(ddns_text(
            translator,
            "publicCheckSourceUnsupportedProtocol",
            &[("family", family_label.to_string()), ("source", source)],
        ));
    }
    if parsed.host_str().unwrap_or("").is_empty() {
        return Err(ddns_text(
            translator,
            "publicCheckSourceInvalidUrl",
            &[("family", family_label.to_string()), ("source", source)],
        ));
    }
    Ok(candidate)
}

fn build_public_check_candidate_url(
    source: &str,
    family_label: &str,
    translator: &Translator,
) -> Result<String, String> {
    let Some(scheme) = explicit_url_scheme(source) else {
        return Ok(format!("https://{source}"));
    };
    if scheme != "http" && scheme != "https" {
        return Err(ddns_text(
            translator,
            "publicCheckSourceUnsupportedProtocol",
            &[
                ("family", family_label.to_string()),
                ("source", source.to_string()),
            ],
        ));
    }
    let lower = source.to_ascii_lowercase();
    if !lower.starts_with("http://") && !lower.starts_with("https://") {
        return Err(ddns_text(
            translator,
            "publicCheckSourceInvalidUrl",
            &[
                ("family", family_label.to_string()),
                ("source", source.to_string()),
            ],
        ));
    }
    Ok(source.to_string())
}

fn explicit_url_scheme(source: &str) -> Option<String> {
    let (scheme, _) = source.split_once(':')?;
    let mut chars = scheme.chars();
    let first = chars.next()?;
    if !first.is_ascii_alphabetic() {
        return None;
    }
    if chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '.' | '-')) {
        Some(scheme.to_ascii_lowercase())
    } else {
        None
    }
}

fn public_check_family_label(family: &str) -> &'static str {
    if family == "ipv4" { "IPv4" } else { "IPv6" }
}

fn public_check_source_value_string(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value.trim().to_string(),
        _ => value.to_string(),
    }
}

fn default_public_check_sources() -> Value {
    json!({
        "ipv4": DEFAULT_PUBLIC_CHECK_IPV4,
        "ipv6": DEFAULT_PUBLIC_CHECK_IPV6
    })
}

fn normalize_update_interval_minutes(value: &Value) -> Option<i64> {
    let parsed = value.as_i64().or_else(|| {
        value
            .as_str()
            .and_then(|value| value.trim().parse::<i64>().ok())
    })?;
    (5..=1440).contains(&parsed).then_some(parsed)
}

fn normalize_http_transport(value: Option<&Value>) -> &'static str {
    match value.and_then(Value::as_str) {
        Some("node" | "fetch") => "node",
        _ => "curl",
    }
}

fn normalize_update_scope(value: Option<&str>) -> &'static str {
    match value {
        Some("ipv6_only") => "ipv6_only",
        Some("ipv4_only") => "ipv4_only",
        _ => "dual_stack",
    }
}

fn normalize_ip_source(value: Option<&str>) -> &'static str {
    match value {
        Some("interface") => "interface",
        Some("static") => "static",
        Some("domain") => "domain",
        _ => "public",
    }
}

fn normalize_network_interface(value: Option<&str>) -> String {
    value.unwrap_or("").trim().to_string()
}

fn parse_last_ip(data: &HashMap<String, String>) -> Value {
    json!({
        "ipv4": non_empty_string(data.get("ipv4")),
        "ipv6": non_empty_string(data.get("ipv6")),
        "updated_at": non_empty_string(data.get("updated_at"))
    })
}

fn parse_last_check(data: &HashMap<String, String>) -> Value {
    json!({
        "checked_at": non_empty_string(data.get("checked_at")),
        "outcome": normalize_last_check_outcome(data.get("outcome").map(String::as_str)),
        "message": non_empty_string(data.get("message"))
    })
}

fn empty_last_ip() -> Value {
    json!({ "ipv4": null, "ipv6": null, "updated_at": null })
}

fn empty_last_check() -> Value {
    json!({ "checked_at": null, "outcome": null, "message": null })
}

fn normalize_last_check_outcome(value: Option<&str>) -> Option<&'static str> {
    match value {
        Some("updated") => Some("updated"),
        Some("noop") => Some("noop"),
        Some("skipped") => Some("skipped"),
        Some("error") => Some("error"),
        _ => None,
    }
}

fn non_empty_string(value: Option<&String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn domain_summary(
    provider: Option<&str>,
    config: &HashMap<String, String>,
    translator: &Translator,
) -> String {
    if let Some(value) = domain_summary_candidate(config) {
        return value;
    }
    if provider.and_then(normalize_provider_name).is_some() {
        String::new()
    } else {
        ddns_text(translator, "noProviderSelected", &[])
    }
}

fn domain_summary_candidate(config: &HashMap<String, String>) -> Option<String> {
    for key in [
        "domain",
        "hostname",
        "domains",
        "zone",
        "root_domain",
        "site_name",
        "site_id",
    ] {
        if let Some(value) = config
            .get(key)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        {
            return Some(value);
        }
    }
    None
}

fn provider_label(provider: Option<&str>, translator: &Translator) -> String {
    let Some(provider) = provider.and_then(normalize_provider_name) else {
        return ddns_text(translator, "notConfigured", &[]);
    };
    let fallback = provider_label_fallback(&provider);
    ddns_catalog_text(
        translator,
        &format!("providers.{}.label", ddns_provider_i18n_key(&provider)),
        &fallback,
        &[],
    )
}

fn provider_label_fallback(provider: &str) -> String {
    match provider {
        "alidns" => "阿里云 DNS".to_string(),
        "baiducloud" => "百度智能云".to_string(),
        "cloudflare" => "Cloudflare".to_string(),
        "dnspod" => "DNSPod".to_string(),
        "duckdns" => "DuckDNS".to_string(),
        "dynu" => "Dynu".to_string(),
        "dynv6" => "dynv6".to_string(),
        "edgeone_cname" => "EdgeOne CNAME".to_string(),
        "edgeone" => "Tencent EdgeOne".to_string(),
        "esa" => "阿里云 ESA".to_string(),
        "godaddy" => "GoDaddy".to_string(),
        "huaweicloud" => "华为云 DNS".to_string(),
        "noip" => "NO-IP".to_string(),
        "porkbun" => "Porkbun".to_string(),
        "tencentcloud" => "腾讯云 DNSPod".to_string(),
        _ => provider.to_string(),
    }
}

fn normalize_provider_name(value: &str) -> Option<String> {
    let normalized = value.trim();
    if provider_names().contains(normalized) {
        Some(normalized.to_string())
    } else {
        None
    }
}

fn provider_names() -> BTreeSet<&'static str> {
    [
        "alidns",
        "baiducloud",
        "cloudflare",
        "dnspod",
        "duckdns",
        "dynu",
        "dynv6",
        "edgeone_cname",
        "edgeone",
        "esa",
        "godaddy",
        "huaweicloud",
        "noip",
        "porkbun",
        "tencentcloud",
    ]
    .into_iter()
    .collect()
}

fn has_explicit_scheme(value: &str) -> bool {
    value.find(':').is_some_and(|index| {
        value[..index]
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_alphabetic())
    })
}

fn target_meta_key(id: &str) -> String {
    format!("{DDNS_TARGET_PREFIX}{id}:meta")
}

fn target_config_key(id: &str) -> String {
    format!("{DDNS_TARGET_PREFIX}{id}:config")
}

fn target_last_ip_key(id: &str) -> String {
    format!("{DDNS_TARGET_PREFIX}{id}:last_ip")
}

fn target_last_check_key(id: &str) -> String {
    format!("{DDNS_TARGET_PREFIX}{id}:last_check")
}

async fn targets_overview(state: &AppState, translator: &Translator) -> anyhow::Result<Value> {
    let targets = list_targets(state).await?;
    let items = targets
        .iter()
        .map(|target| target_summary(target, translator))
        .collect::<Vec<_>>();
    let primary_target_id = items
        .iter()
        .find(|item| item.get("isPrimary").and_then(Value::as_bool) == Some(true))
        .and_then(|item| item.get("id").and_then(Value::as_str))
        .map(str::to_string);
    let extra_count = items
        .iter()
        .filter(|item| item.get("isPrimary").and_then(Value::as_bool) != Some(true))
        .count();
    let enabled_extra_count = items
        .iter()
        .filter(|item| {
            item.get("isPrimary").and_then(Value::as_bool) != Some(true)
                && item.get("enabled").and_then(Value::as_bool) == Some(true)
        })
        .count();
    Ok(json!({
        "primaryTargetId": primary_target_id,
        "total": items.len(),
        "extraCount": extra_count,
        "enabledExtraCount": enabled_extra_count,
        "items": items
    }))
}

async fn target_detail(
    state: &AppState,
    id: &str,
    translator: &Translator,
) -> anyhow::Result<Option<Value>> {
    let target = list_targets(state)
        .await?
        .into_iter()
        .find(|target| target.meta.id == id);
    Ok(target.map(|target| {
        let mut summary = target_summary(&target, translator);
        if let Some(object) = summary.as_object_mut() {
            object.insert("rawName".to_string(), json!(target.meta.name));
            object.insert("config".to_string(), json!(target.config));
        }
        summary
    }))
}

async fn create_ddns_target(
    state: &AppState,
    body: TargetBody,
    translator: &Translator,
) -> anyhow::Result<Value> {
    ensure_primary_initialized(state).await?;
    let provider = normalize_provider_name(&body.provider)
        .ok_or_else(|| anyhow::anyhow!("Unknown DDNS provider: {}", body.provider))?;
    let config = normalize_config(&provider, body.config.unwrap_or_default());
    assert_no_duplicate_target(state, &provider, &config, None).await?;
    let targets = list_targets(state).await?;
    let sort_order = targets
        .iter()
        .map(|target| target.meta.sort_order)
        .max()
        .unwrap_or(0)
        + 1;
    let now = time_utils::now_iso();
    let record = DDNSTargetRecord {
        meta: DDNSTargetMeta {
            id: uuid::Uuid::new_v4().to_string(),
            name: body.name.unwrap_or_default().trim().to_string(),
            is_primary: false,
            enabled: body.enabled.unwrap_or(true),
            provider: Some(provider.clone()),
            created_at: now.clone(),
            updated_at: now,
            sort_order,
        },
        config,
        last_ip: empty_last_ip(),
        last_check: empty_last_check(),
    };
    save_target_record(state, &record).await?;
    Ok(detail_from_record(record, translator))
}

async fn update_ddns_target(
    state: &AppState,
    id: &str,
    body: TargetBody,
    translator: &Translator,
) -> anyhow::Result<Value> {
    let mut target = find_target_or_err(state, id).await?;
    let provider = normalize_provider_name(&body.provider)
        .ok_or_else(|| anyhow::anyhow!("Unknown DDNS provider: {}", body.provider))?;
    let config = normalize_config(&provider, body.config.unwrap_or_default());
    assert_no_duplicate_target(state, &provider, &config, Some(id)).await?;
    let should_reset = comparable_config_key(target.meta.provider.as_deref(), &target.config)
        != comparable_config_key(Some(&provider), &config);
    target.meta.name = body
        .name
        .map(|value| value.trim().to_string())
        .unwrap_or(target.meta.name);
    target.meta.provider = Some(provider.clone());
    target.meta.enabled = if target.meta.is_primary {
        true
    } else {
        body.enabled.unwrap_or(target.meta.enabled)
    };
    target.meta.updated_at = time_utils::now_iso();
    target.config = config;
    save_target_record(state, &target).await?;
    if should_reset {
        reset_target_runtime_state(state, &target.meta).await?;
    }
    if target.meta.is_primary {
        save_legacy_config_draft(state, &provider, &target.config).await?;
        mirror_primary_provider(state, Some(&provider)).await?;
    }
    Ok(detail_from_record(
        find_target_or_err(state, id).await?,
        translator,
    ))
}

async fn delete_ddns_target(state: &AppState, id: &str) -> anyhow::Result<()> {
    let target = find_target_or_err(state, id).await?;
    if target.meta.is_primary {
        return Err(anyhow::anyhow!("Primary DDNS target cannot be deleted"));
    }
    state.redis.srem_string_member(DDNS_TARGET_IDS, id).await?;
    state
        .redis
        .delete_keys(&[
            target_meta_key(id),
            target_config_key(id),
            target_last_ip_key(id),
            target_last_check_key(id),
        ])
        .await?;
    Ok(())
}

async fn set_ddns_target_enabled(state: &AppState, id: &str, enabled: bool) -> anyhow::Result<()> {
    let mut target = find_target_or_err(state, id).await?;
    if target.meta.is_primary && !enabled {
        return Err(anyhow::anyhow!("Primary DDNS target cannot be disabled"));
    }
    target.meta.enabled = if target.meta.is_primary {
        true
    } else {
        enabled
    };
    target.meta.updated_at = time_utils::now_iso();
    save_target_meta(state, &target.meta).await
}

async fn set_primary_provider(state: &AppState, provider: &str) -> anyhow::Result<()> {
    let provider = normalize_provider_name(provider)
        .ok_or_else(|| anyhow::anyhow!("Unknown DDNS provider: {provider}"))?;
    let mut primary = primary_target(state).await?;
    if let Some(previous) = primary.meta.provider.as_deref()
        && previous != provider
    {
        save_legacy_config_draft(state, previous, &primary.config).await?;
    }
    if primary.meta.provider.as_deref() == Some(provider.as_str()) {
        mirror_primary_provider(state, Some(&provider)).await?;
        return Ok(());
    }
    let next_config = read_legacy_config_draft(state, &provider).await?;
    assert_no_duplicate_target(state, &provider, &next_config, Some(&primary.meta.id)).await?;
    let should_reset = comparable_config_key(primary.meta.provider.as_deref(), &primary.config)
        != comparable_config_key(Some(&provider), &next_config);
    primary.meta.provider = Some(provider.clone());
    primary.meta.enabled = true;
    primary.meta.updated_at = time_utils::now_iso();
    primary.config = next_config;
    save_target_record(state, &primary).await?;
    if should_reset {
        reset_target_runtime_state(state, &primary.meta).await?;
    }
    mirror_primary_provider(state, Some(&provider)).await
}

async fn save_primary_config(
    state: &AppState,
    provider: &str,
    config: HashMap<String, String>,
) -> anyhow::Result<()> {
    let provider = normalize_provider_name(provider)
        .ok_or_else(|| anyhow::anyhow!("Unknown DDNS provider: {provider}"))?;
    let mut primary = primary_target(state).await?;
    let normalized = normalize_config(&provider, config);
    if primary.meta.provider.as_deref() == Some(provider.as_str()) {
        assert_no_duplicate_target(state, &provider, &normalized, Some(&primary.meta.id)).await?;
        let should_reset = comparable_config_key(primary.meta.provider.as_deref(), &primary.config)
            != comparable_config_key(Some(&provider), &normalized);
        primary.config = normalized.clone();
        save_target_config(state, &primary.meta, &normalized).await?;
        if should_reset {
            reset_target_runtime_state(state, &primary.meta).await?;
        }
        save_legacy_config_draft(state, &provider, &normalized).await?;
    } else {
        save_legacy_config_draft(state, &provider, &normalized).await?;
    }
    Ok(())
}

async fn primary_target(state: &AppState) -> anyhow::Result<DDNSTargetRecord> {
    ensure_primary_initialized(state).await?;
    list_targets(state)
        .await?
        .into_iter()
        .find(|target| target.meta.is_primary)
        .ok_or_else(|| anyhow::anyhow!("Failed to initialize primary DDNS target"))
}

async fn ensure_primary_initialized(state: &AppState) -> anyhow::Result<()> {
    let primary_id = state
        .redis
        .get_string_value(DDNS_PRIMARY_TARGET_ID)
        .await?
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| PRIMARY_TARGET_ID.to_string());
    if !state
        .redis
        .hgetall_string_map(&target_meta_key(&primary_id))
        .await?
        .is_empty()
    {
        state
            .redis
            .sadd_string_member(DDNS_TARGET_IDS, &primary_id)
            .await?;
        return Ok(());
    }
    let legacy = read_legacy_primary_target(state).await?;
    save_target_record(state, &legacy).await
}

async fn find_target_or_err(state: &AppState, id: &str) -> anyhow::Result<DDNSTargetRecord> {
    ensure_primary_initialized(state).await?;
    list_targets(state)
        .await?
        .into_iter()
        .find(|target| target.meta.id == id)
        .ok_or_else(|| anyhow::anyhow!("DDNS target not found"))
}

async fn save_target_record(state: &AppState, record: &DDNSTargetRecord) -> anyhow::Result<()> {
    save_target_meta(state, &record.meta).await?;
    save_target_config(state, &record.meta, &record.config).await
}

async fn save_target_meta(state: &AppState, meta: &DDNSTargetMeta) -> anyhow::Result<()> {
    let mut payload = HashMap::new();
    payload.insert("name".to_string(), meta.name.trim().to_string());
    payload.insert(
        "is_primary".to_string(),
        if meta.is_primary { "true" } else { "false" }.to_string(),
    );
    payload.insert(
        "enabled".to_string(),
        if meta.enabled { "true" } else { "false" }.to_string(),
    );
    payload.insert(
        "provider".to_string(),
        meta.provider.clone().unwrap_or_default(),
    );
    payload.insert("created_at".to_string(), meta.created_at.clone());
    payload.insert("updated_at".to_string(), meta.updated_at.clone());
    payload.insert("sort_order".to_string(), meta.sort_order.to_string());
    state
        .redis
        .replace_hash_string_map(&target_meta_key(&meta.id), &payload)
        .await?;
    state
        .redis
        .sadd_string_member(DDNS_TARGET_IDS, &meta.id)
        .await?;
    if meta.is_primary {
        state
            .redis
            .set_string_value(DDNS_PRIMARY_TARGET_ID, &meta.id)
            .await?;
    }
    Ok(())
}

async fn save_target_config(
    state: &AppState,
    meta: &DDNSTargetMeta,
    config: &HashMap<String, String>,
) -> anyhow::Result<()> {
    let provider = meta.provider.as_deref();
    let prepared = prepare_config_for_storage(provider, normalize_config_map(provider, config));
    state
        .redis
        .replace_hash_string_map(&target_config_key(&meta.id), &prepared)
        .await?;
    if meta.is_primary
        && let Some(provider) = provider
    {
        save_legacy_config_draft(state, provider, &prepared).await?;
    }
    Ok(())
}

async fn save_legacy_config_draft(
    state: &AppState,
    provider: &str,
    config: &HashMap<String, String>,
) -> anyhow::Result<()> {
    let prepared =
        prepare_config_for_storage(Some(provider), normalize_config_map(Some(provider), config));
    state
        .redis
        .replace_hash_string_map(
            &(DDNS_LEGACY_CONFIG_PREFIX.to_string() + provider),
            &prepared,
        )
        .await?;
    Ok(())
}

async fn read_legacy_config_draft(
    state: &AppState,
    provider: &str,
) -> anyhow::Result<HashMap<String, String>> {
    let raw = state
        .redis
        .hgetall_string_map(&(DDNS_LEGACY_CONFIG_PREFIX.to_string() + provider))
        .await?;
    Ok(normalize_config(provider, raw))
}

async fn mirror_primary_provider(state: &AppState, provider: Option<&str>) -> anyhow::Result<()> {
    if let Some(provider) = provider.filter(|value| !value.trim().is_empty()) {
        state
            .redis
            .set_string_value(DDNS_LEGACY_PROVIDER, provider)
            .await?;
    } else {
        state.redis.delete_key(DDNS_LEGACY_PROVIDER).await?;
    }
    Ok(())
}

async fn reset_target_runtime_state(state: &AppState, meta: &DDNSTargetMeta) -> anyhow::Result<()> {
    state
        .redis
        .replace_hash_string_map(&target_last_ip_key(&meta.id), &HashMap::new())
        .await?;
    state
        .redis
        .replace_hash_string_map(&target_last_check_key(&meta.id), &HashMap::new())
        .await?;
    if meta.is_primary {
        state
            .redis
            .replace_hash_string_map(DDNS_LEGACY_LAST_IP, &HashMap::new())
            .await?;
        state
            .redis
            .replace_hash_string_map(DDNS_LEGACY_LAST_CHECK, &HashMap::new())
            .await?;
    }
    Ok(())
}

async fn assert_no_duplicate_target(
    state: &AppState,
    provider: &str,
    config: &HashMap<String, String>,
    except_id: Option<&str>,
) -> anyhow::Result<()> {
    let next = duplicate_key(provider, config);
    if next.is_empty() {
        return Ok(());
    }
    for target in list_targets(state).await? {
        if except_id == Some(target.meta.id.as_str()) {
            continue;
        }
        if duplicate_key(
            target.meta.provider.as_deref().unwrap_or(""),
            &target.config,
        ) == next
        {
            return Err(anyhow::anyhow!("Duplicate DDNS target"));
        }
    }
    Ok(())
}

fn detail_from_record(record: DDNSTargetRecord, translator: &Translator) -> Value {
    let mut summary = target_summary(&record, translator);
    if let Some(object) = summary.as_object_mut() {
        object.insert("rawName".to_string(), json!(record.meta.name));
        object.insert("config".to_string(), json!(record.config));
    }
    summary
}

async fn ddns_error_response_from_state(state: &AppState, error: anyhow::Error) -> Response {
    let translator = Translator::from_state(state).await;
    ddns_error_response(&translator, error)
}

fn ddns_error_response(translator: &Translator, error: anyhow::Error) -> Response {
    let message = error.to_string();
    let status = if message.contains("not found") {
        StatusCode::NOT_FOUND
    } else if message.contains("Unknown")
        || message.contains("Duplicate")
        || message.contains("Primary")
        || message.contains("interval")
    {
        StatusCode::BAD_REQUEST
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    };
    if status == StatusCode::INTERNAL_SERVER_ERROR {
        tracing::warn!(%message, "DDNS route failed");
    }
    response::error(status, localize_ddns_error(translator, &message))
}

fn localize_ddns_error(translator: &Translator, message: &str) -> String {
    if message == "DDNS target not found" {
        return ddns_text(translator, "targetNotFound", &[]);
    }
    if message == "Failed to initialize primary DDNS target" {
        return ddns_text(translator, "primaryInitFailed", &[]);
    }
    if message == "Primary DDNS target cannot be deleted" {
        return ddns_text(translator, "primaryDeleteForbidden", &[]);
    }
    if message == "Primary DDNS target cannot be disabled" {
        return ddns_text(translator, "primaryDisableForbidden", &[]);
    }
    if message == "Duplicate DDNS target" {
        return ddns_text(translator, "duplicateTarget", &[]);
    }
    if let Some(provider) = message.strip_prefix("Unknown DDNS provider: ") {
        return ddns_text(
            translator,
            "unknownProvider",
            &[("provider", provider.to_string())],
        );
    }
    message.to_string()
}

fn parse_ddns_log_limit(value: Option<&str>) -> usize {
    let raw = value.filter(|value| !value.is_empty()).unwrap_or("200");
    let parsed = parse_node_parse_int(raw).unwrap_or(200);
    parsed.clamp(1, 1000) as usize
}

fn parse_node_parse_int(value: &str) -> Option<i64> {
    let trimmed = value.trim_start();
    let (negative, rest) = if let Some(rest) = trimmed.strip_prefix('-') {
        (true, rest)
    } else if let Some(rest) = trimmed.strip_prefix('+') {
        (false, rest)
    } else {
        (false, trimmed)
    };
    let digits = rest
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return None;
    }
    let parsed = digits.parse::<i64>().ok()?;
    Some(if negative { -parsed } else { parsed })
}

fn parse_log_entries(lines: Vec<String>) -> Vec<Value> {
    lines
        .into_iter()
        .map(|line| {
            serde_json::from_str::<Value>(&line)
                .unwrap_or_else(|_| json!({ "time": "", "level": "info", "message": line }))
        })
        .collect()
}

fn provider_catalog(translator: &Translator) -> Value {
    localize_ddns_provider_catalog(
        json!([
            provider(
                "alidns",
                "阿里云 DNS",
                vec![
                    field("access_key_id", "AccessKey ID", "text", "LTAI...", true),
                    field(
                        "access_key_secret",
                        "AccessKey Secret",
                        "password",
                        "AccessKey Secret",
                        true
                    ),
                    field("root_domain", "Root Domain", "text", "example.com", true),
                    field("domain", "Domain", "text", "home.example.com", true),
                    field("line", "Line", "text", "default", false),
                    field("ttl", "TTL", "text", "600", false),
                ]
            ),
            provider(
                "baiducloud",
                "百度智能云",
                vec![
                    field("access_key_id", "Access Key", "text", "Access Key", true),
                    field(
                        "secret_access_key",
                        "Secret Key",
                        "password",
                        "Secret Key",
                        true
                    ),
                    field("root_domain", "Root Domain", "text", "example.com", true),
                    field("domain", "Domain", "text", "home.example.com", true),
                    field("ttl", "TTL", "text", "300", false),
                ]
            ),
            provider(
                "cloudflare",
                "Cloudflare",
                vec![
                    field(
                        "api_token",
                        "API Token",
                        "password",
                        "Cloudflare API Token",
                        true
                    ),
                    field("zone_id", "Zone ID", "text", "Zone ID", true),
                    field("domain", "Domain", "text", "home.example.com", true),
                    select_field(
                        "proxied",
                        "Proxied",
                        false,
                        vec![("DNS only", "false"), ("Orange cloud", "true")]
                    ),
                ]
            ),
            provider(
                "dnspod",
                "DNSPod",
                vec![
                    field("token_id", "Token ID", "text", "DNSPod Token ID", true),
                    field(
                        "token_key",
                        "Token Key",
                        "password",
                        "DNSPod Token Key",
                        true
                    ),
                    field("root_domain", "Root Domain", "text", "example.com", true),
                    field("domain", "Domain", "text", "home.example.com", true),
                    field("record_line", "Record Line", "text", "默认", false),
                    field("ttl", "TTL", "text", "600", false),
                ]
            ),
            provider(
                "duckdns",
                "DuckDNS",
                vec![
                    field("domains", "Domains", "text", "home,lab", true),
                    field("token", "Token", "password", "DuckDNS Token", true),
                ]
            ),
            provider(
                "dynu",
                "Dynu",
                vec![
                    field("api_key", "API Key", "password", "Dynu API Key", true),
                    field("domain", "Domain", "text", "home.example.com", true),
                    field("ttl", "TTL", "text", "120", false),
                    field("group", "Group", "text", "default", false),
                ]
            ),
            provider(
                "dynv6",
                "dynv6",
                vec![
                    field("token", "HTTP Token", "password", "dynv6 HTTP Token", true),
                    field("zone", "Zone", "text", "myhost.dynv6.net", true),
                    field(
                        "ipv6prefix",
                        "IPv6 Prefix",
                        "text",
                        "2001:db8:1234::/64",
                        false
                    ),
                ]
            ),
            edgeone_cname_provider(),
            edgeone_provider(),
            provider(
                "esa",
                "阿里云 ESA",
                vec![
                    field("access_key_id", "AccessKey ID", "text", "LTAI...", true),
                    field(
                        "access_key_secret",
                        "AccessKey Secret",
                        "password",
                        "AccessKey Secret",
                        true
                    ),
                    field("site_name", "Site Name", "text", "example.com", true),
                    field("site_id", "Site ID", "text", "123456", false),
                    field("domain", "Domain", "text", "home.example.com", true),
                    select_field(
                        "proxied",
                        "Proxied",
                        false,
                        vec![("DNS only", "false"), ("Enabled", "true")]
                    ),
                    select_field(
                        "biz_name",
                        "Business",
                        false,
                        vec![
                            ("Web", "web"),
                            ("API", "api"),
                            ("Image/Video", "image_video")
                        ]
                    ),
                    field("ttl", "TTL", "text", "30", false),
                ]
            ),
            provider(
                "godaddy",
                "GoDaddy",
                vec![
                    field("api_key", "API Key", "text", "GoDaddy API Key", true),
                    field(
                        "api_secret",
                        "API Secret",
                        "password",
                        "GoDaddy API Secret",
                        true
                    ),
                    field("root_domain", "Root Domain", "text", "example.com", true),
                    field("domain", "Domain", "text", "home.example.com", true),
                    field("ttl", "TTL", "text", "600", false),
                ]
            ),
            provider(
                "huaweicloud",
                "华为云 DNS",
                vec![
                    field("access_key_id", "Access Key", "text", "Access Key", true),
                    field(
                        "secret_access_key",
                        "Secret Key",
                        "password",
                        "Secret Key",
                        true
                    ),
                    field("root_domain", "Root Domain", "text", "example.com", true),
                    field("domain", "Domain", "text", "home.example.com", true),
                    field("ttl", "TTL", "text", "300", false),
                ]
            ),
            provider(
                "noip",
                "NO-IP",
                vec![
                    field("hostname", "Hostname", "text", "home.ddns.net", true),
                    field("username", "Username", "text", "DDNS Key Username", true),
                    field(
                        "password",
                        "Password",
                        "password",
                        "DDNS Key Password",
                        true
                    ),
                ]
            ),
            provider(
                "porkbun",
                "Porkbun",
                vec![
                    field("api_key", "API Key", "text", "Porkbun API Key", true),
                    field(
                        "secret_api_key",
                        "Secret API Key",
                        "password",
                        "Porkbun Secret API Key",
                        true
                    ),
                    field("root_domain", "Root Domain", "text", "example.com", true),
                    field("domain", "Domain", "text", "home.example.com", true),
                    field("ttl", "TTL", "text", "600", false),
                ]
            ),
            provider(
                "tencentcloud",
                "腾讯云 DNSPod",
                vec![
                    field("secret_id", "SecretId", "text", "AKID...", true),
                    field("secret_key", "SecretKey", "password", "SecretKey", true),
                    field("root_domain", "Root Domain", "text", "example.com", true),
                    field("domain", "Domain", "text", "home.example.com", true),
                    field("record_line", "Record Line", "text", "默认", false),
                    field("record_line_id", "Record Line ID", "text", "0", false),
                    field("ttl", "TTL", "text", "600", false),
                ]
            ),
        ]),
        translator,
    )
}

fn localize_ddns_provider_catalog(mut catalog: Value, translator: &Translator) -> Value {
    if let Some(providers) = catalog.as_array_mut() {
        for provider in providers {
            localize_ddns_provider(provider, translator);
        }
    }
    catalog
}

fn localize_ddns_provider(provider: &mut Value, translator: &Translator) {
    let provider_name = provider
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let provider_key = ddns_provider_i18n_key(&provider_name);
    if let Some(object) = provider.as_object_mut() {
        if let Some(label) = object.get("label").and_then(Value::as_str) {
            object.insert(
                "label".to_string(),
                Value::String(ddns_catalog_text(
                    translator,
                    &format!("providers.{provider_key}.label"),
                    label,
                    &[],
                )),
            );
        }
        if let Some(fields) = object.get_mut("fields").and_then(Value::as_array_mut) {
            for field in fields {
                localize_ddns_field(field, &provider_key, translator);
            }
        }
    }
}

fn localize_ddns_field(field: &mut Value, provider_key: &str, translator: &Translator) {
    let field_key = field
        .get("key")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let ttl_seconds = field
        .get("placeholder")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(600)
        .to_string();
    let params = [("seconds", ttl_seconds)];

    let label = field
        .get("label")
        .and_then(Value::as_str)
        .map(str::to_string);
    let placeholder = field
        .get("placeholder")
        .and_then(Value::as_str)
        .map(str::to_string);
    let description = field
        .get("description")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            ddns_optional_catalog_text(
                translator,
                provider_key,
                &field_key,
                "description",
                "",
                &params,
            )
        });

    if let Some(object) = field.as_object_mut() {
        if let Some(label) = label {
            object.insert(
                "label".to_string(),
                Value::String(
                    ddns_optional_catalog_text(
                        translator,
                        provider_key,
                        &field_key,
                        "label",
                        &label,
                        &params,
                    )
                    .unwrap_or(label),
                ),
            );
        }
        if let Some(placeholder) = placeholder {
            object.insert(
                "placeholder".to_string(),
                Value::String(
                    ddns_optional_catalog_text(
                        translator,
                        provider_key,
                        &field_key,
                        "placeholder",
                        &placeholder,
                        &params,
                    )
                    .unwrap_or(placeholder),
                ),
            );
        }
        if let Some(description) = description.filter(|value| !value.is_empty()) {
            object.insert("description".to_string(), Value::String(description));
        }
        if let Some(options) = object.get_mut("options").and_then(Value::as_array_mut) {
            for option in options {
                localize_ddns_option(option, provider_key, &field_key, translator);
            }
        }
    }
}

fn localize_ddns_option(
    option: &mut Value,
    provider_key: &str,
    field_key: &str,
    translator: &Translator,
) {
    let Some(value) = option.get("value").and_then(Value::as_str) else {
        return;
    };
    let option_key = ddns_option_i18n_key(provider_key, field_key, value);
    let Some(label) = option
        .get("label")
        .and_then(Value::as_str)
        .map(str::to_string)
    else {
        return;
    };
    let field_i18n_key = ddns_field_i18n_key(provider_key, field_key);
    let translated = ddns_catalog_text(
        translator,
        &format!("providers.{provider_key}.fields.{field_i18n_key}.options.{option_key}"),
        &label,
        &[],
    );
    if let Some(object) = option.as_object_mut() {
        object.insert("label".to_string(), Value::String(translated));
    }
}

fn ddns_optional_catalog_text(
    translator: &Translator,
    provider_key: &str,
    field_key: &str,
    part: &str,
    fallback: &str,
    params: &[(&str, String)],
) -> Option<String> {
    let field_i18n_key = ddns_field_i18n_key(provider_key, field_key);
    let provider_value = ddns_catalog_text(
        translator,
        &format!("providers.{provider_key}.fields.{field_i18n_key}.{part}"),
        fallback,
        params,
    );
    if provider_value != fallback {
        return Some(provider_value);
    }
    if part == "placeholder"
        && field_key == "record_line"
        && matches!(provider_key, "dnspod" | "tencentcloud")
    {
        let default_line = ddns_catalog_text(
            translator,
            &format!("providers.{provider_key}.defaultLine"),
            fallback,
            params,
        );
        if default_line != fallback {
            return Some(default_line);
        }
    }
    if part == "label" && provider_key == "cloudflare" && field_key == "domain" {
        let short_label = ddns_catalog_text(
            translator,
            "providers.common.fields.domain.shortLabel",
            fallback,
            params,
        );
        if short_label != fallback {
            return Some(short_label);
        }
    }
    if part == "description"
        && field_key == "domain"
        && matches!(provider_key, "alidns" | "tencentcloud" | "esa")
    {
        let host_description = ddns_catalog_text(
            translator,
            "providers.common.fields.domain.hostDescription",
            fallback,
            params,
        );
        if host_description != fallback {
            return Some(host_description);
        }
    }
    let common_value = ddns_catalog_text(
        translator,
        &format!("providers.common.fields.{field_key}.{part}"),
        fallback,
        params,
    );
    if common_value != fallback {
        Some(common_value)
    } else if fallback.is_empty() {
        None
    } else {
        Some(fallback.to_string())
    }
}

fn ddns_catalog_text(
    translator: &Translator,
    key: &str,
    fallback: &str,
    params: &[(&str, String)],
) -> String {
    let translated = ddns_text(translator, key, params);
    if translated == format!("server.ddns.{key}") {
        fallback.to_string()
    } else {
        translated
    }
}

fn ddns_text(translator: &Translator, key: &str, params: &[(&str, String)]) -> String {
    let full_key = format!("server.ddns.{key}");
    let translated = if params.is_empty() {
        translator.t(&full_key)
    } else {
        translator.t_params(&full_key, params)
    };
    translated
}

fn ddns_provider_i18n_key(provider_name: &str) -> &str {
    match provider_name {
        "baiducloud" => "baidu",
        "huaweicloud" => "huawei",
        value => value,
    }
}

fn ddns_field_i18n_key<'a>(provider_key: &str, field_key: &'a str) -> &'a str {
    match (provider_key, field_key) {
        ("edgeone" | "edgeone_cname", DDNS_EDGEONE_OVERSEAS_ACCESS_FIELD) => "overseas_access",
        _ => field_key,
    }
}

fn ddns_option_i18n_key(provider_key: &str, field_key: &str, value: &str) -> String {
    match (field_key, value) {
        ("proxied", "false") => "dnsOnly".to_string(),
        ("proxied", "true") if provider_key == "esa" => "enabled".to_string(),
        ("proxied", "true") => "orangeCloud".to_string(),
        ("biz_name", "image_video") => "imageVideo".to_string(),
        ("edgeone_overseas_access", "block_overseas") => "blockOverseas".to_string(),
        _ => value.to_string(),
    }
}

fn provider(name: &str, label: &str, fields: Vec<Value>) -> Value {
    json!({ "name": name, "label": label, "fields": fields })
}

fn edgeone_cname_provider() -> Value {
    let mut value = provider(
        "edgeone_cname",
        "EdgeOne CNAME",
        vec![
            field("secret_id", "SecretId", "text", "AKID...", true),
            field("secret_key", "SecretKey", "password", "SecretKey", true),
            field("zone_id", "Zone ID", "text", "zone-xxxxxxxx", true),
            field("domain", "Domain", "text", "home.example.com", true),
            select_field(
                DDNS_EDGEONE_OVERSEAS_ACCESS_FIELD,
                "Overseas access control",
                false,
                vec![("Off", "off"), ("Block overseas IPs", "block_overseas")],
            ),
            field(
                "endpoint",
                "API Endpoint",
                "text",
                "https://teo.tencentcloudapi.com",
                false,
            ),
            field("region", "Region", "text", "", false),
        ],
    );
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "capabilities".to_string(),
            json!({ "addressMode": "single_address" }),
        );
    }
    value
}

fn edgeone_provider() -> Value {
    provider(
        "edgeone",
        "Tencent EdgeOne",
        vec![
            field("secret_id", "SecretId", "text", "AKID...", true),
            field("secret_key", "SecretKey", "password", "SecretKey", true),
            field("zone_id", "Zone ID", "text", "zone-xxxxxxxx", true),
            field("domain", "Domain", "text", "home.example.com", true),
            field("location", "Location", "text", "", false),
            field("ttl", "TTL", "text", "300", false),
            select_field(
                DDNS_EDGEONE_OVERSEAS_ACCESS_FIELD,
                "Overseas access control",
                false,
                vec![("Off", "off"), ("Block overseas IPs", "block_overseas")],
            ),
            field(
                "endpoint",
                "API Endpoint",
                "text",
                "https://teo.tencentcloudapi.com",
                false,
            ),
            field("region", "Region", "text", "", false),
        ],
    )
}

fn field(key: &str, label: &str, field_type: &str, placeholder: &str, required: bool) -> Value {
    json!({
        "key": key,
        "label": label,
        "type": field_type,
        "placeholder": placeholder,
        "required": required
    })
}

fn select_field(key: &str, label: &str, required: bool, options: Vec<(&str, &str)>) -> Value {
    json!({
        "key": key,
        "label": label,
        "type": "select",
        "required": required,
        "options": options
            .into_iter()
            .map(|(label, value)| json!({ "label": label, "value": value }))
            .collect::<Vec<_>>()
    })
}

fn normalize_config(provider: &str, config: HashMap<String, String>) -> HashMap<String, String> {
    normalize_config_map(Some(provider), &config)
}

fn normalize_config_map(
    provider: Option<&str>,
    config: &HashMap<String, String>,
) -> HashMap<String, String> {
    let mut data = config.clone();
    data.insert(
        "update_scope".to_string(),
        normalize_update_scope(data.get(DDNS_UPDATE_SCOPE_FIELD).map(String::as_str)).to_string(),
    );
    data.insert(
        DDNS_IP_SOURCE_FIELD.to_string(),
        normalize_ip_source(data.get(DDNS_IP_SOURCE_FIELD).map(String::as_str)).to_string(),
    );
    data.insert(
        DDNS_NETWORK_INTERFACE_FIELD.to_string(),
        normalize_network_interface(data.get(DDNS_NETWORK_INTERFACE_FIELD).map(String::as_str)),
    );
    data.insert(
        DDNS_INTERFACE_IPV4_INDEX_FIELD.to_string(),
        normalize_interface_index(
            data.get(DDNS_INTERFACE_IPV4_INDEX_FIELD)
                .map(String::as_str),
        ),
    );
    data.insert(
        DDNS_INTERFACE_IPV6_INDEX_FIELD.to_string(),
        normalize_interface_index(
            data.get(DDNS_INTERFACE_IPV6_INDEX_FIELD)
                .map(String::as_str),
        ),
    );
    data.insert(
        DDNS_STATIC_IPV4_FIELD.to_string(),
        normalize_static_ip(data.get(DDNS_STATIC_IPV4_FIELD).map(String::as_str), 4),
    );
    data.insert(
        DDNS_STATIC_IPV6_FIELD.to_string(),
        normalize_static_ip(data.get(DDNS_STATIC_IPV6_FIELD).map(String::as_str), 6),
    );
    data.insert(
        DDNS_SOURCE_DOMAIN_FIELD.to_string(),
        normalize_domain(
            data.get(DDNS_SOURCE_DOMAIN_FIELD)
                .map(String::as_str)
                .unwrap_or(""),
        ),
    );
    if is_edgeone_provider(provider.unwrap_or("")) {
        let mode = if data
            .get(DDNS_EDGEONE_OVERSEAS_ACCESS_FIELD)
            .map(String::as_str)
            == Some("block_overseas")
        {
            "block_overseas"
        } else {
            "off"
        };
        data.insert(
            DDNS_EDGEONE_OVERSEAS_ACCESS_FIELD.to_string(),
            mode.to_string(),
        );
    }
    data
}

fn prepare_config_for_storage(
    provider: Option<&str>,
    mut config: HashMap<String, String>,
) -> HashMap<String, String> {
    let ip_source = normalize_ip_source(config.get(DDNS_IP_SOURCE_FIELD).map(String::as_str));
    if ip_source == "public" {
        config.remove(DDNS_IP_SOURCE_FIELD);
    }
    if ip_source != "interface" {
        config.remove(DDNS_INTERFACE_IPV4_INDEX_FIELD);
        config.remove(DDNS_INTERFACE_IPV6_INDEX_FIELD);
    } else {
        remove_empty(&mut config, DDNS_INTERFACE_IPV4_INDEX_FIELD);
        remove_empty(&mut config, DDNS_INTERFACE_IPV6_INDEX_FIELD);
    }
    if ip_source != "static" {
        config.remove(DDNS_STATIC_IPV4_FIELD);
        config.remove(DDNS_STATIC_IPV6_FIELD);
    } else {
        remove_empty(&mut config, DDNS_STATIC_IPV4_FIELD);
        remove_empty(&mut config, DDNS_STATIC_IPV6_FIELD);
    }
    if ip_source != "domain" {
        config.remove(DDNS_SOURCE_DOMAIN_FIELD);
    } else {
        remove_empty(&mut config, DDNS_SOURCE_DOMAIN_FIELD);
    }
    if !is_edgeone_provider(provider.unwrap_or(""))
        || config
            .get(DDNS_EDGEONE_OVERSEAS_ACCESS_FIELD)
            .map(String::as_str)
            == Some("off")
    {
        config.remove(DDNS_EDGEONE_OVERSEAS_ACCESS_FIELD);
    }
    config
}

fn remove_empty(config: &mut HashMap<String, String>, key: &str) {
    if config.get(key).is_none_or(|value| value.trim().is_empty()) {
        config.remove(key);
    }
}

fn duplicate_key(provider: &str, config: &HashMap<String, String>) -> String {
    let provider = provider.trim();
    let domain = domain_summary_candidate(config)
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    if provider.is_empty() || domain.is_empty() {
        String::new()
    } else {
        format!("{provider}::{domain}")
    }
}

fn comparable_config_key(provider: Option<&str>, config: &HashMap<String, String>) -> String {
    let prepared = prepare_config_for_storage(provider, normalize_config_map(provider, config));
    let mut entries = prepared.into_iter().collect::<Vec<_>>();
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    serde_json::to_string(&entries).unwrap_or_default()
}

fn normalize_interface_index(value: Option<&str>) -> String {
    let value = value.unwrap_or("").trim();
    if value.is_empty() {
        return String::new();
    }
    value
        .parse::<u32>()
        .ok()
        .map(|value| value.to_string())
        .unwrap_or_default()
}

fn normalize_static_ip(value: Option<&str>, _family: u8) -> String {
    value.unwrap_or("").trim().to_string()
}

fn normalize_domain(value: &str) -> String {
    value.trim().trim_end_matches('.').to_string()
}

fn is_edgeone_provider(provider: &str) -> bool {
    matches!(provider, "edgeone" | "edgeone_cname")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider_by_name<'a>(providers: &'a Value, name: &str) -> &'a Value {
        providers
            .as_array()
            .unwrap()
            .iter()
            .find(|provider| provider.get("name").and_then(Value::as_str) == Some(name))
            .unwrap()
    }

    fn provider_field<'a>(provider: &'a Value, key: &str) -> &'a Value {
        provider
            .get("fields")
            .and_then(Value::as_array)
            .unwrap()
            .iter()
            .find(|field| field.get("key").and_then(Value::as_str) == Some(key))
            .unwrap()
    }

    fn catalog_signature(providers: &Value) -> Value {
        let items = providers
            .as_array()
            .unwrap()
            .iter()
            .map(|provider| {
                let fields = provider
                    .get("fields")
                    .and_then(Value::as_array)
                    .unwrap()
                    .iter()
                    .map(|field| {
                        let options = field.get("options").and_then(Value::as_array).map(|items| {
                            items
                                .iter()
                                .filter_map(|item| item.get("value").and_then(Value::as_str))
                                .collect::<Vec<_>>()
                        });
                        json!({
                            "key": field.get("key").and_then(Value::as_str).unwrap(),
                            "type": field.get("type").and_then(Value::as_str).unwrap(),
                            "required": field.get("required").and_then(Value::as_bool) != Some(false),
                            "options": options,
                        })
                    })
                    .collect::<Vec<_>>();
                json!({
                    "name": provider.get("name").and_then(Value::as_str).unwrap(),
                    "capabilities": provider.get("capabilities").cloned().unwrap_or(Value::Null),
                    "fields": fields,
                })
            })
            .collect::<Vec<_>>();
        json!(items)
    }

    #[test]
    fn parses_ddns_settings_with_defaults() {
        let value = parse_settings(Some(
            r#"{"updateIntervalMinutes":5,"httpTransport":"fetch","publicCheckSources":{"ipv4":["4.example.com","https://4.example.com"],"ipv6":["https://6.example.com"]}}"#,
        ));
        assert_eq!(value["updateIntervalMinutes"], json!(5));
        assert_eq!(value["httpTransport"], json!("node"));
        assert_eq!(
            value["publicCheckSources"]["ipv4"],
            json!(["https://4.example.com"])
        );
    }

    #[test]
    fn strict_public_check_sources_match_node_validation() {
        let zh = Translator::new("zh-CN");
        let fallback = json!({ "ipv4": ["https://fallback4.example.com"], "ipv6": ["https://fallback6.example.com"] });
        let normalized = normalize_public_check_sources_strict(
            &json!({
                "ipv4": ["4.example.com", "https://4.example.com"],
                "ipv6": []
            }),
            &fallback,
            &zh,
        )
        .unwrap();
        assert_eq!(normalized["ipv4"], json!(["https://4.example.com"]));
        assert_eq!(normalized["ipv6"], json!([]));

        assert_eq!(
            normalize_public_check_sources_strict(
                &json!({ "ipv4": [""], "ipv6": [] }),
                &fallback,
                &zh,
            )
            .expect_err("empty source should fail"),
            "IPv4 公网探测地址不能为空"
        );
        assert_eq!(
            normalize_public_check_sources_strict(
                &json!({ "ipv4": ["ftp://example.com"], "ipv6": [] }),
                &fallback,
                &zh,
            )
            .expect_err("unsupported protocol should fail"),
            "IPv4 公网探测地址仅支持 HTTP/HTTPS: ftp://example.com"
        );
    }

    #[test]
    fn builds_target_summary_fallbacks() {
        let zh = Translator::new("zh-CN");
        let target = default_primary_target();
        let summary = target_summary(&target, &zh);
        assert_eq!(summary["id"], json!("primary"));
        assert_eq!(summary["enabled"], json!(true));
        assert_eq!(summary["updateScope"], json!("dual_stack"));
        assert_eq!(summary["name"], json!("主域名"));
        assert_eq!(summary["providerLabel"], json!("未配置"));
        assert_eq!(summary["domainSummary"], json!("未选择提供商"));
        assert_eq!(
            target_log_label(&target, &summary, &zh),
            "[主域][未配置][未选择提供商]"
        );

        let mut extra = target;
        extra.meta.is_primary = false;
        extra.meta.name = "备用域名".to_string();
        let extra_summary = target_summary(&extra, &zh);
        assert_eq!(
            target_log_label(&extra, &extra_summary, &zh),
            "[附加域][未配置][未选择提供商]"
        );
    }

    #[test]
    fn normalizes_last_check_outcomes() {
        assert_eq!(
            normalize_last_check_outcome(Some("updated")),
            Some("updated")
        );
        assert_eq!(normalize_last_check_outcome(Some("bad")), None);
    }

    #[test]
    fn ddns_log_limit_parser_matches_node_parse_int_prefixes() {
        assert_eq!(parse_ddns_log_limit(None), 200);
        assert_eq!(parse_ddns_log_limit(Some("")), 200);
        assert_eq!(parse_ddns_log_limit(Some("10x")), 10);
        assert_eq!(parse_ddns_log_limit(Some("0x10")), 1);
        assert_eq!(parse_ddns_log_limit(Some("-5")), 1);
        assert_eq!(parse_ddns_log_limit(Some("5000")), 1000);
        assert_eq!(parse_ddns_log_limit(Some("abc")), 200);
    }

    #[test]
    fn prepares_config_for_storage_like_node() {
        let config = HashMap::from([
            ("domain".to_string(), " home.example.com ".to_string()),
            (DDNS_IP_SOURCE_FIELD.to_string(), "public".to_string()),
            (DDNS_STATIC_IPV4_FIELD.to_string(), "1.2.3.4".to_string()),
            (DDNS_INTERFACE_IPV4_INDEX_FIELD.to_string(), "2".to_string()),
            (" custom ".to_string(), " keep spaces ".to_string()),
            ("".to_string(), "blank-key".to_string()),
        ]);
        let prepared = prepare_config_for_storage(
            Some("cloudflare"),
            normalize_config_map(Some("cloudflare"), &config),
        );
        assert_eq!(
            prepared.get("domain").map(String::as_str),
            Some(" home.example.com ")
        );
        assert_eq!(
            prepared.get(" custom ").map(String::as_str),
            Some(" keep spaces ")
        );
        assert_eq!(prepared.get("").map(String::as_str), Some("blank-key"));
        assert_eq!(
            prepared.get(DDNS_UPDATE_SCOPE_FIELD).map(String::as_str),
            Some("dual_stack")
        );
        assert!(!prepared.contains_key(DDNS_IP_SOURCE_FIELD));
        assert!(!prepared.contains_key(DDNS_STATIC_IPV4_FIELD));
        assert!(!prepared.contains_key(DDNS_INTERFACE_IPV4_INDEX_FIELD));

        let static_config = HashMap::from([
            (DDNS_IP_SOURCE_FIELD.to_string(), "static".to_string()),
            (
                DDNS_STATIC_IPV4_FIELD.to_string(),
                " not-an-ip ".to_string(),
            ),
        ]);
        let prepared = prepare_config_for_storage(
            Some("cloudflare"),
            normalize_config_map(Some("cloudflare"), &static_config),
        );
        assert_eq!(
            prepared.get(DDNS_STATIC_IPV4_FIELD).map(String::as_str),
            Some("not-an-ip")
        );
    }

    #[test]
    fn normalizes_only_ddns_common_config_fields_like_node() {
        let config = HashMap::from([
            ("domain".to_string(), " home.example.com ".to_string()),
            ("api_token".to_string(), " token-with-spaces ".to_string()),
            (DDNS_UPDATE_SCOPE_FIELD.to_string(), "ipv4_only".to_string()),
            (DDNS_IP_SOURCE_FIELD.to_string(), "static".to_string()),
            (
                DDNS_STATIC_IPV4_FIELD.to_string(),
                " 203.0.113.10 ".to_string(),
            ),
        ]);
        let normalized = normalize_config_map(Some("cloudflare"), &config);
        assert_eq!(
            normalized.get("domain").map(String::as_str),
            Some(" home.example.com ")
        );
        assert_eq!(
            normalized.get("api_token").map(String::as_str),
            Some(" token-with-spaces ")
        );
        assert_eq!(
            normalized.get(DDNS_UPDATE_SCOPE_FIELD).map(String::as_str),
            Some("ipv4_only")
        );
        assert_eq!(
            normalized.get(DDNS_IP_SOURCE_FIELD).map(String::as_str),
            Some("static")
        );
        assert_eq!(
            normalized.get(DDNS_STATIC_IPV4_FIELD).map(String::as_str),
            Some("203.0.113.10")
        );
    }

    #[test]
    fn duplicate_key_uses_provider_and_domain_summary() {
        let config = HashMap::from([("domain".to_string(), "Home.Example.com".to_string())]);
        assert_eq!(
            duplicate_key("cloudflare", &config),
            "cloudflare::home.example.com"
        );
        assert_eq!(duplicate_key("", &config), "");
    }

    #[test]
    fn parses_public_check_ip_payloads_like_node_detector() {
        assert_eq!(
            parse_detected_ip_text(r#"{"ip":"203.0.113.8"}"#, 4),
            Some("203.0.113.8".to_string())
        );
        assert_eq!(
            parse_detected_ip_text("2001:db8::8\n", 6),
            Some("2001:db8::8".to_string())
        );
        assert_eq!(parse_detected_ip_text(r#"{"ip":"2001:db8::8"}"#, 4), None);
    }

    #[test]
    fn applies_update_scope_to_resolved_ddns_ips() {
        assert_eq!(
            apply_update_scope(
                "ipv4_only",
                Some("203.0.113.8".to_string()),
                Some("2001:db8::8".to_string())
            ),
            (Some("203.0.113.8".to_string()), None)
        );
        assert_eq!(
            apply_update_scope(
                "ipv6_only",
                Some("203.0.113.8".to_string()),
                Some("2001:db8::8".to_string())
            ),
            (None, Some("2001:db8::8".to_string()))
        );
    }

    #[test]
    fn validates_ddns_source_domain_like_node() {
        assert!(is_valid_source_domain("home.example.com"));
        assert!(!is_valid_source_domain("https://home.example.com"));
        assert!(!is_valid_source_domain("*.example.com"));
        assert!(!is_valid_source_domain("-bad.example.com"));
    }

    #[test]
    fn detects_incomplete_ddns_target_config() {
        let now = time_utils::now_iso();
        let target = DDNSTargetRecord {
            meta: DDNSTargetMeta {
                id: "primary".to_string(),
                name: "Primary".to_string(),
                is_primary: true,
                enabled: true,
                provider: Some("cloudflare".to_string()),
                created_at: now.clone(),
                updated_at: now,
                sort_order: 0,
            },
            config: HashMap::from([("domain".to_string(), "home.example.com".to_string())]),
            last_ip: empty_last_ip(),
            last_check: empty_last_check(),
        };
        let translator = Translator::new(crate::i18n::DEFAULT_LOCALE);
        let message = target_config_incomplete_message(&target, &translator).unwrap();
        assert!(message.contains("API 令牌"));
        assert!(message.contains("Zone ID"));
        assert!(message.contains("当前主域配置不完整"));
    }

    #[test]
    fn detects_single_address_provider_dual_stack_like_node() {
        let now = time_utils::now_iso();
        let target = DDNSTargetRecord {
            meta: DDNSTargetMeta {
                id: "target-1".to_string(),
                name: "EdgeOne CNAME".to_string(),
                is_primary: false,
                enabled: true,
                provider: Some("edgeone_cname".to_string()),
                created_at: now.clone(),
                updated_at: now,
                sort_order: 1,
            },
            config: HashMap::from([
                ("secret_id".to_string(), "sid".to_string()),
                ("secret_key".to_string(), "skey".to_string()),
                ("zone_id".to_string(), "zone-1".to_string()),
                ("domain".to_string(), "home.example.com".to_string()),
                (
                    DDNS_UPDATE_SCOPE_FIELD.to_string(),
                    "dual_stack".to_string(),
                ),
            ]),
            last_ip: empty_last_ip(),
            last_check: empty_last_check(),
        };
        let translator = Translator::new("zh-CN");
        let message = target_config_incomplete_message(&target, &translator).unwrap();

        assert_eq!(
            message,
            "当前条目配置不完整，请填写所有必填字段: 腾讯云 EdgeOne（CNAME 接入） 一次只能更新一个地址，请将更新范围设置为仅 IPv4 或仅 IPv6"
        );
    }

    #[test]
    fn target_config_completeness_matches_node_runtime_inputs() {
        let now = time_utils::now_iso();
        let mut target = DDNSTargetRecord {
            meta: DDNSTargetMeta {
                id: "target-1".to_string(),
                name: "Static".to_string(),
                is_primary: false,
                enabled: true,
                provider: Some("duckdns".to_string()),
                created_at: now.clone(),
                updated_at: now.clone(),
                sort_order: 1,
            },
            config: HashMap::from([
                ("domains".to_string(), "home".to_string()),
                ("token".to_string(), "token".to_string()),
                (DDNS_IP_SOURCE_FIELD.to_string(), "static".to_string()),
                (DDNS_STATIC_IPV4_FIELD.to_string(), "not-an-ip".to_string()),
            ]),
            last_ip: empty_last_ip(),
            last_check: empty_last_check(),
        };
        let translator = Translator::new("zh-CN");
        let message = target_config_incomplete_message(&target, &translator).unwrap();
        assert_eq!(
            message,
            "当前条目配置不完整，请填写所有必填字段: 静态 IPv4 地址无效: not-an-ip"
        );

        target.config.insert(
            DDNS_STATIC_IPV4_FIELD.to_string(),
            "203.0.113.10".to_string(),
        );
        assert!(target_config_incomplete_message(&target, &translator).is_none());

        target.meta.provider = Some("missing-provider".to_string());
        let message = target_config_incomplete_message(&target, &translator).unwrap();
        assert_eq!(message, "当前条目配置不完整，请填写所有必填字段: 未配置");
    }

    #[test]
    fn localizes_ddns_route_and_provider_messages() {
        let zh = Translator::new("zh-CN");
        assert_eq!(
            ddns_text(&zh, "statusLoadFailed", &[]),
            "读取 DDNS 状态失败"
        );
        assert_eq!(
            localize_ddns_error(&zh, "Primary DDNS target cannot be disabled"),
            "主域条目不可单独停用"
        );
        assert_eq!(
            localize_ddns_error(&zh, "Unknown DDNS provider: unknown"),
            "未知的 DDNS 提供商: unknown"
        );
        assert_eq!(
            public_check_request_failed_message(&zh, "https://ip.example.com", 503),
            "探测源 https://ip.example.com 请求失败: HTTP 503"
        );
        assert_eq!(
            public_check_invalid_payload_message(&zh, "https://ip.example.com", 6),
            "探测源 https://ip.example.com 未返回有效的 IPv6 地址"
        );
        assert_eq!(provider_label(Some("tencentcloud"), &zh), "腾讯云 DNS");
        assert_eq!(provider_label(Some("edgeone"), &zh), "腾讯云 EdgeOne");
        assert_eq!(
            noip_status_message(&zh, "badauth", ""),
            "badauth (用户名或密码错误)"
        );
        assert_eq!(
            noip_status_message(&zh, "custom", "raw detail"),
            "custom (raw detail)"
        );
        assert_eq!(
            ddns_text(&zh, "providers.dynu.wildcardUnchanged", &[]),
            "Dynu Wildcard Alias IP 未变化"
        );
        assert_eq!(
            ddns_text(&zh, "providers.alidns.requestFailed", &[]),
            "请求失败"
        );
        assert_eq!(
            ddns_text(&zh, "providers.alidns.recordIdMissing", &[]),
            "阿里云 DNS 返回的记录缺少 RecordId"
        );
        assert_eq!(
            ddns_text(&zh, "providers.huawei.recordsetIdMissing", &[]),
            "华为云 DNS 返回的记录集缺少 ID"
        );
        assert_eq!(
            ddns_text(&zh, "providers.esa.recordIdMissing", &[]),
            "UpdateFailed: 记录缺少 RecordId"
        );
        assert_eq!(
            ddns_text(&zh, "providers.dynu.invalidRootInfo", &[]),
            "Dynu 未返回有效的根域信息"
        );
        assert_eq!(
            ddns_text(&zh, "providers.dnspod.queryRecordFailed", &[]),
            "查询记录失败"
        );
        assert_eq!(
            ddns_text(&zh, "providers.baidu.updateFailed", &[]),
            "更新失败"
        );
        assert_eq!(
            ddns_text(&zh, "providers.porkbun.createRecordFailed", &[]),
            "创建记录失败"
        );
        assert_eq!(
            ddns_text(&zh, "providers.tencentcloud.missingCreatedRecordId", &[]),
            "腾讯云未返回创建后的 RecordId"
        );
        assert_eq!(
            ddns_text(&zh, "providers.edgeone.missingRecordId", &[]),
            "EdgeOne 返回的记录缺少 RecordId"
        );
        assert!(
            ddns_text(
                &zh,
                "providers.dynu.wildcardUnsupported",
                &[("domain", "example.com".to_string())],
            )
            .contains("example.com")
        );
        assert_eq!(
            ddns_text(
                &zh,
                "domainNotInZone",
                &[
                    ("fqdn", "app.other.com".to_string()),
                    ("zone", "example.com".to_string())
                ],
            ),
            "域名 app.other.com 不属于根域 example.com"
        );
        assert_eq!(
            ddns_text(
                &zh,
                "invalidJsonResponse",
                &[("text", "<html>bad</html>".to_string())],
            ),
            "响应不是合法 JSON: <html>bad</html>"
        );
        let config = HashMap::from([("ipv6prefix".to_string(), "2001:db8:1234::/64".to_string())]);
        assert_eq!(
            dynv6_sent_params(&zh, None, Some("2001:db8::8"), &config),
            "ipv4=(空), ipv6=2001:db8::8, ipv6prefix=2001:db8:1234::/64"
        );
    }

    #[test]
    fn parses_docker_host_ipv6_interfaces() {
        let items = parse_host_if_inet6(
            "20010db8000000000000000000000001 02 40 00 00 eth0\nfe800000000000000000000000000001 02 40 20 00 eth1",
        );
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["name"], json!("docker-host:eth0"));
        assert_eq!(items[0]["hasIpv6"], json!(true));
        assert_eq!(items[0]["addresses"][0]["address"], json!("2001:db8::1"));
    }

    #[test]
    fn interface_selectability_filters_private_ranges() {
        assert!(!is_selectable_interface_address(&json!({
            "family": "ipv4",
            "address": "192.168.1.10"
        })));
        assert!(is_selectable_interface_address(&json!({
            "family": "ipv4",
            "address": "8.8.8.8"
        })));
        assert!(!is_selectable_interface_address(&json!({
            "family": "ipv6",
            "address": "fd00::1"
        })));
    }

    #[test]
    fn builds_ddns_provider_query_urls() {
        let url = build_query_url(
            "https://example.com/update",
            &[
                ("hostname", "home.example.com".to_string()),
                ("myip", "203.0.113.8,2001:db8::8".to_string()),
            ],
        );
        assert_eq!(
            url,
            "https://example.com/update?hostname=home.example.com&myip=203.0.113.8%2C2001%3Adb8%3A%3A8"
        );
        let config = HashMap::from([("token".to_string(), " secret ".to_string())]);
        assert_eq!(config_value(&config, "token"), "secret");
    }

    #[test]
    fn provider_catalog_signature_matches_node_definitions() {
        let providers = provider_catalog(&Translator::new("en"));
        assert_eq!(
            catalog_signature(&providers),
            json!([
                {
                    "name": "alidns",
                    "capabilities": null,
                    "fields": [
                        { "key": "access_key_id", "type": "text", "required": true, "options": null },
                        { "key": "access_key_secret", "type": "password", "required": true, "options": null },
                        { "key": "root_domain", "type": "text", "required": true, "options": null },
                        { "key": "domain", "type": "text", "required": true, "options": null },
                        { "key": "line", "type": "text", "required": false, "options": null },
                        { "key": "ttl", "type": "text", "required": false, "options": null }
                    ]
                },
                {
                    "name": "baiducloud",
                    "capabilities": null,
                    "fields": [
                        { "key": "access_key_id", "type": "text", "required": true, "options": null },
                        { "key": "secret_access_key", "type": "password", "required": true, "options": null },
                        { "key": "root_domain", "type": "text", "required": true, "options": null },
                        { "key": "domain", "type": "text", "required": true, "options": null },
                        { "key": "ttl", "type": "text", "required": false, "options": null }
                    ]
                },
                {
                    "name": "cloudflare",
                    "capabilities": null,
                    "fields": [
                        { "key": "api_token", "type": "password", "required": true, "options": null },
                        { "key": "zone_id", "type": "text", "required": true, "options": null },
                        { "key": "domain", "type": "text", "required": true, "options": null },
                        { "key": "proxied", "type": "select", "required": false, "options": ["false", "true"] }
                    ]
                },
                {
                    "name": "dnspod",
                    "capabilities": null,
                    "fields": [
                        { "key": "token_id", "type": "text", "required": true, "options": null },
                        { "key": "token_key", "type": "password", "required": true, "options": null },
                        { "key": "root_domain", "type": "text", "required": true, "options": null },
                        { "key": "domain", "type": "text", "required": true, "options": null },
                        { "key": "record_line", "type": "text", "required": false, "options": null },
                        { "key": "ttl", "type": "text", "required": false, "options": null }
                    ]
                },
                {
                    "name": "duckdns",
                    "capabilities": null,
                    "fields": [
                        { "key": "domains", "type": "text", "required": true, "options": null },
                        { "key": "token", "type": "password", "required": true, "options": null }
                    ]
                },
                {
                    "name": "dynu",
                    "capabilities": null,
                    "fields": [
                        { "key": "api_key", "type": "password", "required": true, "options": null },
                        { "key": "domain", "type": "text", "required": true, "options": null },
                        { "key": "ttl", "type": "text", "required": false, "options": null },
                        { "key": "group", "type": "text", "required": false, "options": null }
                    ]
                },
                {
                    "name": "dynv6",
                    "capabilities": null,
                    "fields": [
                        { "key": "token", "type": "password", "required": true, "options": null },
                        { "key": "zone", "type": "text", "required": true, "options": null },
                        { "key": "ipv6prefix", "type": "text", "required": false, "options": null }
                    ]
                },
                {
                    "name": "edgeone_cname",
                    "capabilities": { "addressMode": "single_address" },
                    "fields": [
                        { "key": "secret_id", "type": "text", "required": true, "options": null },
                        { "key": "secret_key", "type": "password", "required": true, "options": null },
                        { "key": "zone_id", "type": "text", "required": true, "options": null },
                        { "key": "domain", "type": "text", "required": true, "options": null },
                        { "key": "edgeone_overseas_access", "type": "select", "required": false, "options": ["off", "block_overseas"] },
                        { "key": "endpoint", "type": "text", "required": false, "options": null },
                        { "key": "region", "type": "text", "required": false, "options": null }
                    ]
                },
                {
                    "name": "edgeone",
                    "capabilities": null,
                    "fields": [
                        { "key": "secret_id", "type": "text", "required": true, "options": null },
                        { "key": "secret_key", "type": "password", "required": true, "options": null },
                        { "key": "zone_id", "type": "text", "required": true, "options": null },
                        { "key": "domain", "type": "text", "required": true, "options": null },
                        { "key": "location", "type": "text", "required": false, "options": null },
                        { "key": "ttl", "type": "text", "required": false, "options": null },
                        { "key": "edgeone_overseas_access", "type": "select", "required": false, "options": ["off", "block_overseas"] },
                        { "key": "endpoint", "type": "text", "required": false, "options": null },
                        { "key": "region", "type": "text", "required": false, "options": null }
                    ]
                },
                {
                    "name": "esa",
                    "capabilities": null,
                    "fields": [
                        { "key": "access_key_id", "type": "text", "required": true, "options": null },
                        { "key": "access_key_secret", "type": "password", "required": true, "options": null },
                        { "key": "site_name", "type": "text", "required": true, "options": null },
                        { "key": "site_id", "type": "text", "required": false, "options": null },
                        { "key": "domain", "type": "text", "required": true, "options": null },
                        { "key": "proxied", "type": "select", "required": false, "options": ["false", "true"] },
                        { "key": "biz_name", "type": "select", "required": false, "options": ["web", "api", "image_video"] },
                        { "key": "ttl", "type": "text", "required": false, "options": null }
                    ]
                },
                {
                    "name": "godaddy",
                    "capabilities": null,
                    "fields": [
                        { "key": "api_key", "type": "text", "required": true, "options": null },
                        { "key": "api_secret", "type": "password", "required": true, "options": null },
                        { "key": "root_domain", "type": "text", "required": true, "options": null },
                        { "key": "domain", "type": "text", "required": true, "options": null },
                        { "key": "ttl", "type": "text", "required": false, "options": null }
                    ]
                },
                {
                    "name": "huaweicloud",
                    "capabilities": null,
                    "fields": [
                        { "key": "access_key_id", "type": "text", "required": true, "options": null },
                        { "key": "secret_access_key", "type": "password", "required": true, "options": null },
                        { "key": "root_domain", "type": "text", "required": true, "options": null },
                        { "key": "domain", "type": "text", "required": true, "options": null },
                        { "key": "ttl", "type": "text", "required": false, "options": null }
                    ]
                },
                {
                    "name": "noip",
                    "capabilities": null,
                    "fields": [
                        { "key": "hostname", "type": "text", "required": true, "options": null },
                        { "key": "username", "type": "text", "required": true, "options": null },
                        { "key": "password", "type": "password", "required": true, "options": null }
                    ]
                },
                {
                    "name": "porkbun",
                    "capabilities": null,
                    "fields": [
                        { "key": "api_key", "type": "text", "required": true, "options": null },
                        { "key": "secret_api_key", "type": "password", "required": true, "options": null },
                        { "key": "root_domain", "type": "text", "required": true, "options": null },
                        { "key": "domain", "type": "text", "required": true, "options": null },
                        { "key": "ttl", "type": "text", "required": false, "options": null }
                    ]
                },
                {
                    "name": "tencentcloud",
                    "capabilities": null,
                    "fields": [
                        { "key": "secret_id", "type": "text", "required": true, "options": null },
                        { "key": "secret_key", "type": "password", "required": true, "options": null },
                        { "key": "root_domain", "type": "text", "required": true, "options": null },
                        { "key": "domain", "type": "text", "required": true, "options": null },
                        { "key": "record_line", "type": "text", "required": false, "options": null },
                        { "key": "record_line_id", "type": "text", "required": false, "options": null },
                        { "key": "ttl", "type": "text", "required": false, "options": null }
                    ]
                }
            ])
        );
    }

    #[test]
    fn provider_catalog_localizes_edgeone_overseas_access_alias() {
        let zh_providers = provider_catalog(&Translator::new("zh-CN"));
        for provider_name in ["edgeone", "edgeone_cname"] {
            let provider = provider_by_name(&zh_providers, provider_name);
            let field = provider_field(provider, DDNS_EDGEONE_OVERSEAS_ACCESS_FIELD);
            assert_eq!(field.get("label"), Some(&json!("海外访问控制")));
            assert_eq!(
                field.get("description"),
                Some(&json!(
                    "当开启时，将调用 EdgeOne 安全策略 API 屏蔽海外 IP 访问；港澳台不属于海外。该设置只会在配置变更时同步一次，不会随每次 DDNS 更新重复执行。"
                ))
            );
            assert_eq!(
                field.get("options"),
                Some(&json!([
                    { "label": "不使用", "value": "off" },
                    { "label": "屏蔽海外 IP", "value": "block_overseas" }
                ]))
            );
        }

        let en_providers = provider_catalog(&Translator::new("en"));
        for provider_name in ["edgeone", "edgeone_cname"] {
            let provider = provider_by_name(&en_providers, provider_name);
            let field = provider_field(provider, DDNS_EDGEONE_OVERSEAS_ACCESS_FIELD);
            assert_eq!(field.get("label"), Some(&json!("Overseas access control")));
            assert_eq!(
                field.get("options"),
                Some(&json!([
                    { "label": "Off", "value": "off" },
                    { "label": "Block overseas IPs", "value": "block_overseas" }
                ]))
            );
        }
    }

    #[test]
    fn provider_catalog_localizes_select_option_aliases() {
        let zh_providers = provider_catalog(&Translator::new("zh-CN"));
        let zh_cloudflare = provider_by_name(&zh_providers, "cloudflare");
        assert_eq!(
            provider_field(zh_cloudflare, "proxied").get("options"),
            Some(&json!([
                { "label": "仅解析", "value": "false" },
                { "label": "橙色云朵", "value": "true" }
            ]))
        );
        for provider_name in ["edgeone", "edgeone_cname"] {
            let provider = provider_by_name(&zh_providers, provider_name);
            assert_eq!(
                provider_field(provider, DDNS_EDGEONE_OVERSEAS_ACCESS_FIELD).get("options"),
                Some(&json!([
                    { "label": "不使用", "value": "off" },
                    { "label": "屏蔽海外 IP", "value": "block_overseas" }
                ]))
            );
        }
        let zh_esa = provider_by_name(&zh_providers, "esa");
        assert_eq!(
            provider_field(zh_esa, "proxied").get("options"),
            Some(&json!([
                { "label": "仅解析", "value": "false" },
                { "label": "开启代理", "value": "true" }
            ]))
        );
        assert_eq!(
            provider_field(zh_esa, "biz_name").get("options"),
            Some(&json!([
                { "label": "网页", "value": "web" },
                { "label": "接口", "value": "api" },
                { "label": "音视频", "value": "image_video" }
            ]))
        );

        let en_providers = provider_catalog(&Translator::new("en"));
        let en_cloudflare = provider_by_name(&en_providers, "cloudflare");
        assert_eq!(
            provider_field(en_cloudflare, "proxied").get("options"),
            Some(&json!([
                { "label": "DNS only", "value": "false" },
                { "label": "Orange cloud", "value": "true" }
            ]))
        );
        for provider_name in ["edgeone", "edgeone_cname"] {
            let provider = provider_by_name(&en_providers, provider_name);
            assert_eq!(
                provider_field(provider, DDNS_EDGEONE_OVERSEAS_ACCESS_FIELD).get("options"),
                Some(&json!([
                    { "label": "Off", "value": "off" },
                    { "label": "Block overseas IPs", "value": "block_overseas" }
                ]))
            );
        }
        let en_esa = provider_by_name(&en_providers, "esa");
        assert_eq!(
            provider_field(en_esa, "proxied").get("options"),
            Some(&json!([
                { "label": "DNS only", "value": "false" },
                { "label": "Enable proxy", "value": "true" }
            ]))
        );
        assert_eq!(
            provider_field(en_esa, "biz_name").get("options"),
            Some(&json!([
                { "label": "Web", "value": "web" },
                { "label": "API", "value": "api" },
                { "label": "Audio/video", "value": "image_video" }
            ]))
        );
    }

    #[test]
    fn provider_catalog_preserves_node_field_descriptions() {
        let described_fields: &[(&str, &[&str])] = &[
            (
                "alidns",
                &[
                    "access_key_id",
                    "access_key_secret",
                    "root_domain",
                    "domain",
                    "line",
                    "ttl",
                ],
            ),
            (
                "baiducloud",
                &[
                    "access_key_id",
                    "secret_access_key",
                    "root_domain",
                    "domain",
                    "ttl",
                ],
            ),
            ("cloudflare", &["api_token", "zone_id", "domain", "proxied"]),
            (
                "dnspod",
                &[
                    "token_id",
                    "token_key",
                    "root_domain",
                    "domain",
                    "record_line",
                    "ttl",
                ],
            ),
            ("duckdns", &["domains", "token"]),
            ("dynu", &["api_key", "domain", "ttl", "group"]),
            ("dynv6", &["token", "zone", "ipv6prefix"]),
            (
                "edgeone_cname",
                &[
                    "secret_id",
                    "secret_key",
                    "zone_id",
                    "domain",
                    DDNS_EDGEONE_OVERSEAS_ACCESS_FIELD,
                    "endpoint",
                    "region",
                ],
            ),
            (
                "edgeone",
                &[
                    "secret_id",
                    "secret_key",
                    "zone_id",
                    "domain",
                    "location",
                    "ttl",
                    DDNS_EDGEONE_OVERSEAS_ACCESS_FIELD,
                    "endpoint",
                    "region",
                ],
            ),
            (
                "esa",
                &[
                    "access_key_id",
                    "access_key_secret",
                    "site_name",
                    "site_id",
                    "domain",
                    "proxied",
                    "biz_name",
                    "ttl",
                ],
            ),
            (
                "godaddy",
                &["api_key", "api_secret", "root_domain", "domain", "ttl"],
            ),
            (
                "huaweicloud",
                &[
                    "access_key_id",
                    "secret_access_key",
                    "root_domain",
                    "domain",
                    "ttl",
                ],
            ),
            ("noip", &["hostname", "username", "password"]),
            (
                "porkbun",
                &["api_key", "secret_api_key", "root_domain", "domain", "ttl"],
            ),
            (
                "tencentcloud",
                &[
                    "secret_id",
                    "secret_key",
                    "root_domain",
                    "domain",
                    "record_line",
                    "record_line_id",
                    "ttl",
                ],
            ),
        ];

        for locale in ["zh-CN", "en"] {
            let providers = provider_catalog(&Translator::new(locale));
            for &(provider_name, field_keys) in described_fields {
                let provider = provider_by_name(&providers, provider_name);
                for &key in field_keys {
                    let field = provider_field(provider, key);
                    assert!(
                        field
                            .get("description")
                            .and_then(Value::as_str)
                            .is_some_and(|value| !value.trim().is_empty()),
                        "{locale} {provider_name}.{key} missing description"
                    );
                }
            }
        }
    }

    #[test]
    fn provider_catalog_localizes_required_field_help() {
        for locale in ["zh-CN", "zh-Hant", "en", "ko-KR", "ja-JP"] {
            let providers = provider_catalog(&Translator::new(locale));
            for provider in providers.as_array().unwrap() {
                let provider_name = provider.get("name").and_then(Value::as_str).unwrap();
                for field in provider.get("fields").and_then(Value::as_array).unwrap() {
                    if field.get("required").and_then(Value::as_bool) == Some(false) {
                        continue;
                    }
                    let key = field.get("key").and_then(Value::as_str).unwrap();
                    assert!(
                        field
                            .get("label")
                            .and_then(Value::as_str)
                            .is_some_and(|value| !value.trim().is_empty()),
                        "{locale} {provider_name}.{key} missing label"
                    );
                    assert!(
                        field
                            .get("description")
                            .and_then(Value::as_str)
                            .is_some_and(|value| !value.trim().is_empty()),
                        "{locale} {provider_name}.{key} missing description"
                    );
                }
            }
        }
    }

    #[test]
    fn provider_catalog_contains_all_node_providers() {
        let providers = provider_catalog(&Translator::new("zh-CN"));
        let names = providers
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|provider| provider.get("name").and_then(Value::as_str))
            .collect::<BTreeSet<_>>();
        assert_eq!(names, provider_names());
        assert!(providers.as_array().unwrap().iter().all(|provider| {
            provider
                .get("fields")
                .and_then(Value::as_array)
                .is_some_and(|fields| !fields.is_empty())
        }));
        let cloudflare = provider_by_name(&providers, "cloudflare");
        let proxied = provider_field(cloudflare, "proxied");
        assert_eq!(proxied.get("label"), Some(&json!("Cloudflare 代理")));
        assert!(proxied.get("description").and_then(Value::as_str).is_some());
        assert_eq!(
            provider_field(cloudflare, "domain").get("label"),
            Some(&json!("域名"))
        );
        let alidns = provider_by_name(&providers, "alidns");
        assert_eq!(
            provider_field(alidns, "domain").get("description"),
            Some(&json!("要更新的完整主机名"))
        );
        assert_eq!(
            provider_field(alidns, "access_key_id").get("label"),
            Some(&json!("访问密钥 ID"))
        );

        let en_providers = provider_catalog(&Translator::new("en"));
        let dnspod = provider_by_name(&en_providers, "dnspod");
        assert_eq!(
            provider_field(dnspod, "record_line").get("placeholder"),
            Some(&json!("Default"))
        );
        assert_eq!(
            provider_field(dnspod, "token_id").get("description"),
            Some(&json!("API Token ID generated in the DNSPod console"))
        );

        let zh_tencentcloud = provider_by_name(&providers, "tencentcloud");
        assert_eq!(
            provider_field(zh_tencentcloud, "secret_id").get("label"),
            Some(&json!("SecretId（密钥 ID）"))
        );
        assert_eq!(
            provider_field(zh_tencentcloud, "secret_id").get("description"),
            Some(&json!(
                "腾讯云 API 访问密钥 SecretId，需具备对应 DNS 服务权限"
            ))
        );
        let tencentcloud = provider_by_name(&en_providers, "tencentcloud");
        assert_eq!(
            provider_field(tencentcloud, "record_line").get("placeholder"),
            Some(&json!("Default"))
        );
        let esa = provider_by_name(&en_providers, "esa");
        assert_eq!(
            provider_field(esa, "proxied").get("options"),
            Some(&json!([
                { "label": "DNS only", "value": "false" },
                { "label": "Enable proxy", "value": "true" }
            ]))
        );
        assert_eq!(
            provider_field(esa, "biz_name").get("options"),
            Some(&json!([
                { "label": "Web", "value": "web" },
                { "label": "API", "value": "api" },
                { "label": "Audio/video", "value": "image_video" }
            ]))
        );
    }
}
