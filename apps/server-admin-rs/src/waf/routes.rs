use std::{
    collections::{BTreeMap, HashSet},
    io::{self, Cursor, Read},
    path::{Path as FsPath, PathBuf},
    time::SystemTime,
};

use axum::{
    Router,
    body::Bytes,
    extract::{Path, Query},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use base64::{Engine as _, engine::general_purpose};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::{
    fs,
    time::{self as tokio_time, MissedTickBehavior},
};
use utoipa_axum::{router::OpenApiRouter, routes};
use zip::ZipArchive;

use crate::{
    fs_utils, http_body, i18n::Translator, response, state::AppState, store, system_events,
    time_utils,
};

mod logs;
mod rules;
mod service;

use logs::*;
use rules::*;
use service::{
    apply_waf_config, apply_waf_config_to_gateway, check_and_sync_system_waf_rules_if_needed,
    delete_custom_waf_rule, drain_waf_events_now, get_waf_details, load_waf_config,
    read_waf_rule_file, set_recommended_system_rules, set_waf_rule_enabled, sync_waf_on_boot,
    upload_custom_waf_rules, waf_drain_schedule,
};
pub(crate) use service::{
    disabled_hosts_for_config, restore_waf_runtime_after_import, sync_waf_config_to_gateway,
};

#[cfg(test)]
mod tests;

const INITIALIZATION_RULE_FILENAME: &str = "REQUEST-901-INITIALIZATION.conf";
const LFI_RULE_FILENAME: &str = "REQUEST-930-APPLICATION-ATTACK-LFI.conf";
const RECOMMENDED_LFI_RULE_PATCH_FLAG_KEY: &str = "fn_knock:patch:waf-recommended-lfi-rule:v1";
const MANIFEST_URL: &str = "https://cor.fnknock.cn/waf/manifest.json";
const MAX_WAF_MANIFEST_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_WAF_METADATA_FILE_BYTES: usize = 1024 * 1024;
const MANIFEST_REFRESH_MS: i64 = 2 * 24 * 60 * 60 * 1000;
const MAX_RULE_FILE_BYTES: usize = 1024 * 1024;
const MAX_ZIP_BYTES: usize = 20 * 1024 * 1024;
const MAX_UNPACKED_ZIP_BYTES: usize = 60 * 1024 * 1024;
const DEFAULT_DRAIN_LIMIT: i64 = 500;
const DEFAULT_WAF_DRAIN_INTERVAL_SECONDS: u64 = 2;
const WAF_SYSTEM_RULES_AUTO_UPDATE_SECONDS: u64 = 2 * 24 * 60 * 60;
const WAF_SYSTEM_RULES_AUTO_UPDATE_LOCK_TTL_SECONDS: i64 = 10 * 60;
const UNFILTERED_QUERY_SCAN_CHUNK_SIZE: isize = 500;
const FILTERED_QUERY_SCAN_CHUNK_SIZE: isize = 500;
const DEFAULT_DISABLED_SYSTEM_RULE_FILENAMES: &[&str] = &[
    "REQUEST-920-PROTOCOL-ENFORCEMENT.conf",
    "REQUEST-932-APPLICATION-ATTACK-RCE.conf",
    "REQUEST-941-APPLICATION-ATTACK-XSS.conf",
    "REQUEST-942-APPLICATION-ATTACK-SQLI.conf",
];

fn waf_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.waf.{key}"))
}

fn waf_text_params(translator: &Translator, key: &str, params: &[(&str, String)]) -> String {
    translator.t_params(&format!("server.waf.{key}"), params)
}

fn waf_error_response(
    translator: &Translator,
    status: StatusCode,
    message: impl AsRef<str>,
) -> Response {
    response::error(status, localize_waf_error(translator, message.as_ref()))
}

fn localize_waf_error(translator: &Translator, message: &str) -> String {
    let message = message.trim();
    match message {
        "WAF manifest is empty" => return waf_text(translator, "manifestEmpty"),
        "Keep at least one WAF rule enabled" => return waf_text(translator, "keepOneEnabledRule"),
        "Enable WAF after at least one rule is enabled" => {
            return waf_text(translator, "enableNeedsRule");
        }
        "WAF rule file not found" => return waf_text(translator, "ruleFileNotFound"),
        "Select at least one .conf file" => return waf_text(translator, "uploadSelectConf"),
        "Invalid base64 content" => return waf_text(translator, "base64Invalid"),
        "Failed to drain WAF events" => return waf_text(translator, "eventsDrainFailed"),
        "Failed to sync WAF config" => return waf_text(translator, "configSyncFailed"),
        "Failed to load WAF rules" => return waf_text(translator, "rulesLoadFailed"),
        "Failed to sync common location exemptions" => {
            return translator.t("server.commonAuthLocations.syncFailed");
        }
        "WAF system rule bundle contains no .conf files" => {
            return waf_text(translator, "zipNoConf");
        }
        "WAF system rule bundle is too large after unpacking" => {
            return waf_text(translator, "zipUnpackedTooLarge");
        }
        "WAF .conf files must be in the bundle root" => {
            return waf_text(translator, "zipConfRootOnly");
        }
        "WAF manifest missing zip file"
        | "WAF manifest missing zip hash"
        | "WAF manifest missing zip info" => return waf_text(translator, "manifestMissingZipInfo"),
        "WAF system rule zip is too large" => return waf_text(translator, "zipTooLarge"),
        "WAF system rule zip hash mismatch" => return waf_text(translator, "zipHashMismatch"),
        "Invalid WAF manifest" => return waf_text(translator, "manifestInvalid"),
        "Only .conf WAF rule files are supported" => return waf_text(translator, "confOnly"),
        "Invalid WAF rule filename" => return waf_text(translator, "ruleFilenameInvalid"),
        "Invalid WAF rule source" => return waf_text(translator, "sourceInvalid"),
        "invalid date, expected YYYY-MM-DD" => return waf_text(translator, "dateInvalid"),
        _ => {}
    }

    if let Some(status) = message.strip_prefix("WAF manifest request failed: ") {
        return waf_text_params(
            translator,
            "manifestRequestFailed",
            &[("status", status.trim().to_string())],
        );
    }
    if let Some(status) = message.strip_prefix("WAF system rule download failed: ") {
        return waf_text_params(
            translator,
            "downloadFailed",
            &[("status", status.trim().to_string())],
        );
    }
    if let Some(path) = message.strip_prefix("Duplicate WAF bundle file: ") {
        return waf_text_params(
            translator,
            "zipDuplicateFile",
            &[("path", path.trim().to_string())],
        );
    }
    if let Some(path) = message.strip_prefix("Invalid WAF bundle path: ") {
        return waf_text_params(
            translator,
            "zipPathInvalid",
            &[("path", path.trim().to_string())],
        );
    }
    if let Some(filename) = message.strip_prefix("WAF rule file is too large: ") {
        return waf_text_params(
            translator,
            "fileTooLarge",
            &[("filename", filename.trim().to_string())],
        );
    }
    if let Some(filename) = message.strip_prefix("WAF rule file is not valid UTF-8: ") {
        return waf_text_params(
            translator,
            "fileInvalidUtf8",
            &[("filename", filename.trim().to_string())],
        );
    }
    if let Some(filename) =
        message.strip_prefix("WAF rule file contains blocked filesystem directives: ")
    {
        return waf_text_params(
            translator,
            "filesystemDirectiveBlocked",
            &[("filename", filename.trim().to_string())],
        );
    }

    message.to_string()
}

pub fn waf_routes() -> Router<AppState> {
    waf_openapi_routes().into()
}

pub(crate) fn waf_openapi_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(details))
        .routes(routes!(status))
        .routes(routes!(config))
        .routes(routes!(refresh_manifest))
        .routes(routes!(sync_system_rules))
        .routes(routes!(enable_recommended_rules))
        .routes(routes!(set_rule_enabled))
        .routes(routes!(read_rule))
        .routes(routes!(upload_custom))
        .routes(routes!(delete_custom))
        .routes(routes!(drain_events))
        .routes(routes!(logs))
        .routes(routes!(log_detail))
        .routes(routes!(delete_logs))
}

pub fn start_waf_tasks(state: AppState) {
    let boot_state = state.clone();
    state.spawn_background("waf-boot-sync", async move {
        tokio::select! {
            _ = boot_state.shutdown.cancelled() => {}
            result = sync_waf_on_boot(&boot_state) => {
                if let Err(error) = result {
                    tracing::warn!(%error, "failed to sync WAF on boot");
                }
            }
        }
    });

    let drain_state = state.clone();
    state.spawn_background("waf-event-drain", async move {
        tokio::select! {
            _ = drain_state.shutdown.cancelled() => return,
            result = drain_waf_events_now(&drain_state) => {
                if let Err(error) = result {
                    tracing::debug!(%error, "failed to drain WAF events on boot");
                }
            }
        }
        loop {
            let schedule = tokio::select! {
                _ = drain_state.shutdown.cancelled() => break,
                schedule = waf_drain_schedule(&drain_state) => schedule,
            };
            if let Some(interval) = schedule {
                tokio::select! {
                    _ = drain_state.shutdown.cancelled() => break,
                    _ = drain_state.waf_event_drain_reload_notify.notified() => continue,
                    _ = tokio_time::sleep(std::time::Duration::from_secs(interval)) => {}
                }
            } else {
                tokio::select! {
                    _ = drain_state.shutdown.cancelled() => break,
                    _ = drain_state.waf_event_drain_reload_notify.notified() => continue,
                }
            }
            tokio::select! {
                _ = drain_state.shutdown.cancelled() => break,
                result = drain_waf_events_now(&drain_state) => {
                    if let Err(error) = result {
                        tracing::debug!(%error, "failed to drain WAF events");
                    }
                }
            }
        }
    });

    let update_state = state.clone();
    state.spawn_background("waf-rules-update", async move {
        let mut ticker = tokio_time::interval(std::time::Duration::from_secs(
            WAF_SYSTEM_RULES_AUTO_UPDATE_SECONDS,
        ));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        ticker.tick().await;
        loop {
            tokio::select! {
                _ = update_state.shutdown.cancelled() => break,
                _ = ticker.tick() => {}
            }
            tokio::select! {
                _ = update_state.shutdown.cancelled() => break,
                result = check_and_sync_system_waf_rules_if_needed(&update_state) => {
                    if let Err(error) = result {
                        tracing::warn!(%error, "failed to auto update WAF system rules");
                    }
                }
            }
        }
    });
}

#[derive(Deserialize)]
struct WafLogQuery {
    date: Option<String>,
    trace_id: Option<String>,
    search: Option<String>,
    host: Option<String>,
    client_ip: Option<String>,
    rule_id: Option<String>,
    route_type: Option<String>,
    mode: Option<String>,
    cursor: Option<String>,
    limit: Option<String>,
}

#[derive(Deserialize)]
struct WafRuleToggleBody {
    source: Option<String>,
    filenames: Option<Vec<String>>,
    enabled: bool,
}

#[derive(Deserialize)]
struct WafUploadBody {
    files: Vec<WafUploadFile>,
}

#[derive(Deserialize)]
struct WafUploadFile {
    filename: String,
    content_base64: String,
}

#[utoipa::path(get, path = "/api/admin/waf/details", tag = "waf", operation_id = "get_api_admin_waf_details", responses((status = 200, description = "WAF details")))]
async fn details(axum::extract::State(state): axum::extract::State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match get_waf_details(&state).await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to load WAF details");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                waf_text(&translator, "detailsLoadFailed"),
            )
        }
    }
}

#[utoipa::path(get, path = "/api/admin/waf/status", tag = "waf", operation_id = "get_api_admin_waf_status", responses((status = 200, description = "WAF status")))]
async fn status(axum::extract::State(state): axum::extract::State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.gateway.client.get_waf_status().await {
        Ok(value) => {
            if !value
                .get("success")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                let message = value
                    .get("message")
                    .and_then(Value::as_str)
                    .map(|message| localize_waf_error(&translator, message))
                    .unwrap_or_else(|| waf_text(&translator, "statusReadFailed"));
                return response::error(StatusCode::BAD_GATEWAY, message);
            }
            match value.get("data") {
                Some(data) => response::ok(data.clone()).into_response(),
                None => response::error(
                    StatusCode::BAD_GATEWAY,
                    waf_text(&translator, "statusReadFailed"),
                ),
            }
        }
        Err(error) => {
            tracing::warn!(%error, "failed to load WAF status from Go backend");
            response::error(
                StatusCode::BAD_GATEWAY,
                waf_text(&translator, "statusReadFailed"),
            )
        }
    }
}

#[utoipa::path(post, path = "/api/admin/waf/config", tag = "waf", operation_id = "post_api_admin_waf_config", responses((status = 200, description = "Updated WAF configuration")))]
async fn config(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::Json(body): axum::Json<Value>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match apply_waf_config(&state, &body).await {
        Ok(data) => {
            state.request_waf_event_drain_reload();
            response::ok(data).into_response()
        }
        Err(error) => {
            tracing::warn!(%error, "failed to save WAF config");
            waf_error_response(&translator, StatusCode::BAD_REQUEST, error.to_string())
        }
    }
}

#[utoipa::path(post, path = "/api/admin/waf/manifest/refresh", tag = "waf", operation_id = "post_api_admin_waf_manifest_refresh", responses((status = 200, description = "Refreshed WAF manifest")))]
async fn refresh_manifest(axum::extract::State(state): axum::extract::State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match refresh_system_manifest_cache(&state).await {
        Ok(_) => match get_waf_details(&state).await {
            Ok(data) => response::ok(data).into_response(),
            Err(error) => {
                tracing::warn!(%error, "failed to load WAF details after manifest refresh");
                response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    waf_text(&translator, "detailsLoadFailed"),
                )
            }
        },
        Err(error) => {
            tracing::warn!(%error, "failed to refresh WAF manifest");
            waf_error_response(&translator, StatusCode::BAD_GATEWAY, error.to_string())
        }
    }
}

#[utoipa::path(post, path = "/api/admin/waf/system/sync", tag = "waf", operation_id = "post_api_admin_waf_system_sync", responses((status = 200, description = "Synchronized system WAF rules")))]
async fn sync_system_rules(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match sync_system_waf_rules(&state).await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to sync system WAF rules");
            waf_error_response(&translator, StatusCode::BAD_GATEWAY, error.to_string())
        }
    }
}

#[utoipa::path(post, path = "/api/admin/waf/rules/enabled", tag = "waf", operation_id = "post_api_admin_waf_rules_enabled", responses((status = 200, description = "Updated WAF rule state")))]
async fn set_rule_enabled(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::Json(body): axum::Json<WafRuleToggleBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match set_waf_rule_enabled(&state, body).await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to update WAF rule state");
            waf_error_response(&translator, StatusCode::BAD_REQUEST, error.to_string())
        }
    }
}

#[utoipa::path(post, path = "/api/admin/waf/rules/recommended", tag = "waf", operation_id = "post_api_admin_waf_rules_recommended", responses((status = 200, description = "Enabled recommended WAF rules")))]
async fn enable_recommended_rules(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match set_recommended_system_rules(&state).await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to restore recommended WAF rules");
            waf_error_response(&translator, StatusCode::BAD_REQUEST, error.to_string())
        }
    }
}

#[utoipa::path(get, path = "/api/admin/waf/rules/{source}/{filename}", tag = "waf", operation_id = "get_api_admin_waf_rules_source_filename", responses((status = 200, description = "WAF rule content")))]
async fn read_rule(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path((source, filename)): Path<(String, String)>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match read_waf_rule_file(&state, &source, &filename).await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to read WAF rule file");
            waf_error_response(&translator, StatusCode::NOT_FOUND, error.to_string())
        }
    }
}

#[utoipa::path(post, path = "/api/admin/waf/custom/upload", tag = "waf", operation_id = "post_api_admin_waf_custom_upload", responses((status = 200, description = "Uploaded custom WAF rules")))]
async fn upload_custom(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::Json(body): axum::Json<WafUploadBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match upload_custom_waf_rules(&state, body).await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to upload custom WAF rules");
            waf_error_response(&translator, StatusCode::BAD_REQUEST, error.to_string())
        }
    }
}

#[utoipa::path(delete, path = "/api/admin/waf/custom/{filename}", tag = "waf", operation_id = "delete_api_admin_waf_custom_filename", responses((status = 200, description = "Deleted custom WAF rule")))]
async fn delete_custom(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(filename): Path<String>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match delete_custom_waf_rule(&state, &filename).await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to delete custom WAF rule");
            waf_error_response(&translator, StatusCode::BAD_REQUEST, error.to_string())
        }
    }
}

#[utoipa::path(post, path = "/api/admin/waf/events/drain", tag = "waf", operation_id = "post_api_admin_waf_events_drain", responses((status = 200, description = "Drained WAF events")))]
async fn drain_events(axum::extract::State(state): axum::extract::State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match drain_waf_events_now(&state).await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to drain WAF events");
            waf_error_response(&translator, StatusCode::BAD_GATEWAY, error.to_string())
        }
    }
}

#[utoipa::path(get, path = "/api/admin/waf/logs", tag = "waf", operation_id = "get_api_admin_waf_logs", responses((status = 200, description = "WAF logs")))]
async fn logs(
    axum::extract::State(state): axum::extract::State<AppState>,
    Query(query): Query<WafLogQuery>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match query_waf_logs(&state, &query).await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to query WAF logs");
            waf_error_response(&translator, StatusCode::BAD_REQUEST, error.to_string())
        }
    }
}

#[utoipa::path(get, path = "/api/admin/waf/logs/{trace_id}", tag = "waf", operation_id = "get_api_admin_waf_logs_trace_id", responses((status = 200, description = "WAF log event")))]
async fn log_detail(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(trace_id): Path<String>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match get_waf_log_event(&state, &trace_id).await {
        Ok(Some(event)) => response::ok(event).into_response(),
        Ok(None) => response::error(StatusCode::NOT_FOUND, waf_text(&translator, "logNotFound")),
        Err(error) => {
            tracing::warn!(%error, %trace_id, "failed to get WAF log detail");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                waf_text(&translator, "logLoadFailed"),
            )
        }
    }
}

#[utoipa::path(delete, path = "/api/admin/waf/logs", tag = "waf", operation_id = "delete_api_admin_waf_logs", responses((status = 200, description = "Deleted WAF logs")))]
async fn delete_logs(
    axum::extract::State(state): axum::extract::State<AppState>,
    body: Bytes,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let parsed: Value = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(_) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                waf_text(&translator, "invalidRequestBody"),
            );
        }
    };
    let date = match normalize_date(parsed.get("date").and_then(Value::as_str)) {
        Ok(date) => date,
        Err(message) => {
            return waf_error_response(&translator, StatusCode::BAD_REQUEST, message);
        }
    };

    match state.storage.store.delete_waf_log_date(&date).await {
        Ok(deleted) => match state.storage.store.list_waf_log_dates(&today()).await {
            Ok(available_dates) => response::ok(json!({
                "date": date,
                "deleted": deleted,
                "available_dates": available_dates,
            }))
            .into_response(),
            Err(error) => {
                tracing::warn!(%error, "failed to list WAF log dates after delete");
                response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    waf_text(&translator, "logsDeleteFailed"),
                )
            }
        },
        Err(error) => {
            tracing::warn!(%error, %date, "failed to delete WAF logs");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                waf_text(&translator, "logsDeleteFailed"),
            )
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct WafRulesState {
    #[serde(default)]
    system_enabled: BTreeMap<String, bool>,
    #[serde(default)]
    custom_enabled: BTreeMap<String, bool>,
}
