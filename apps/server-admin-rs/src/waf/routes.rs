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
    routing::{delete, get, post},
};
use base64::{Engine as _, engine::general_purpose};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::{
    fs,
    time::{self as tokio_time, MissedTickBehavior},
};
use zip::ZipArchive;

use crate::{i18n::Translator, redis_store, response, state::AppState, system_events, time_utils};

mod logs;
mod rules;
mod service;

use logs::*;
use rules::*;
pub(crate) use service::sync_waf_config_to_gateway;
use service::{
    apply_waf_config, apply_waf_config_to_gateway, check_and_sync_system_waf_rules_if_needed,
    delete_custom_waf_rule, drain_waf_events_now, get_waf_details, load_waf_config,
    read_waf_rule_file, set_waf_rule_enabled, sync_waf_on_boot, upload_custom_waf_rules,
    waf_drain_interval_seconds,
};

#[cfg(test)]
mod tests;

const INITIALIZATION_RULE_FILENAME: &str = "REQUEST-901-INITIALIZATION.conf";
const MANIFEST_URL: &str = "https://cor.fnknock.cn/waf/manifest.json";
const MANIFEST_REFRESH_MS: i64 = 2 * 24 * 60 * 60 * 1000;
const MAX_RULE_FILE_BYTES: usize = 1024 * 1024;
const MAX_ZIP_BYTES: usize = 20 * 1024 * 1024;
const MAX_UNPACKED_ZIP_BYTES: usize = 60 * 1024 * 1024;
const DEFAULT_DRAIN_LIMIT: i64 = 500;
const DEFAULT_WAF_DRAIN_INTERVAL_SECONDS: u64 = 2;
const DISABLED_WAF_DRAIN_INTERVAL_SECONDS: u64 = 30;
const WAF_SYSTEM_RULES_AUTO_UPDATE_SECONDS: u64 = 2 * 24 * 60 * 60;
const WAF_SYSTEM_RULES_AUTO_UPDATE_LOCK_TTL_SECONDS: i64 = 10 * 60;
const UNFILTERED_QUERY_SCAN_CHUNK_SIZE: isize = 500;
const FILTERED_QUERY_SCAN_CHUNK_SIZE: isize = 500;
const DEFAULT_DISABLED_SYSTEM_RULE_FILENAMES: &[&str] = &[
    "REQUEST-920-PROTOCOL-ENFORCEMENT.conf",
    "REQUEST-930-APPLICATION-ATTACK-LFI.conf",
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
    Router::new()
        .route("/api/admin/waf/details", get(details))
        .route("/api/admin/waf/status", get(status))
        .route("/api/admin/waf/config", post(config))
        .route("/api/admin/waf/manifest/refresh", post(refresh_manifest))
        .route("/api/admin/waf/system/sync", post(sync_system_rules))
        .route("/api/admin/waf/rules/enabled", post(set_rule_enabled))
        .route("/api/admin/waf/rules/{source}/{filename}", get(read_rule))
        .route("/api/admin/waf/custom/upload", post(upload_custom))
        .route("/api/admin/waf/custom/{filename}", delete(delete_custom))
        .route("/api/admin/waf/events/drain", post(drain_events))
        .route("/api/admin/waf/logs", get(logs).delete(delete_logs))
        .route("/api/admin/waf/logs/{trace_id}", get(log_detail))
}

pub fn start_waf_tasks(state: AppState) {
    let boot_state = state.clone();
    tokio::spawn(async move {
        if let Err(error) = sync_waf_on_boot(&boot_state).await {
            tracing::warn!(%error, "failed to sync WAF on boot");
        }
    });

    let drain_state = state.clone();
    tokio::spawn(async move {
        if let Err(error) = drain_waf_events_now(&drain_state).await {
            tracing::debug!(%error, "failed to drain WAF events on boot");
        }
        loop {
            let interval = waf_drain_interval_seconds(&drain_state).await;
            tokio_time::sleep(std::time::Duration::from_secs(interval)).await;
            if let Err(error) = drain_waf_events_now(&drain_state).await {
                tracing::debug!(%error, "failed to drain WAF events");
            }
        }
    });

    tokio::spawn(async move {
        let mut ticker = tokio_time::interval(std::time::Duration::from_secs(
            WAF_SYSTEM_RULES_AUTO_UPDATE_SECONDS,
        ));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        ticker.tick().await;
        loop {
            ticker.tick().await;
            if let Err(error) = check_and_sync_system_waf_rules_if_needed(&state).await {
                tracing::warn!(%error, "failed to auto update WAF system rules");
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

async fn status(axum::extract::State(state): axum::extract::State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.go_backend.get_waf_status().await {
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

async fn config(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::Json(body): axum::Json<Value>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match apply_waf_config(&state, &body).await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to save WAF config");
            waf_error_response(&translator, StatusCode::BAD_REQUEST, error.to_string())
        }
    }
}

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

    match state.redis.delete_waf_log_date(&date).await {
        Ok(deleted) => match state.redis.list_waf_log_dates(&today()).await {
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
