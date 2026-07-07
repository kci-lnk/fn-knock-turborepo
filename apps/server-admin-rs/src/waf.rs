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
    http::{Method, StatusCode},
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

const INITIALIZATION_RULE_FILENAME: &str = "REQUEST-901-INITIALIZATION.conf";
const MANIFEST_URL: &str = "https://cor.fnknock.cn/waf/manifest.json";
const MANIFEST_REFRESH_MS: i64 = 2 * 24 * 60 * 60 * 1000;
const MAX_RULE_FILE_BYTES: usize = 1024 * 1024;
const MAX_ZIP_BYTES: usize = 20 * 1024 * 1024;
const MAX_UNPACKED_ZIP_BYTES: usize = 60 * 1024 * 1024;
const DEFAULT_DRAIN_LIMIT: i64 = 500;
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
    match state
        .go_backend
        .request_json(Method::GET, "/api/waf/status", Option::<&Value>::None)
        .await
    {
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

async fn get_waf_details(state: &AppState) -> anyhow::Result<Value> {
    ensure_waf_directories(state).await?;
    let config = load_waf_config(state).await?;
    let manifest_cache = get_manifest_cache_for_details(state).await?;
    let synced = read_system_sync_state(state).await?;
    let rules_state = read_rules_state(state).await?;
    let system_rules = list_rule_files(state, "system", &manifest_cache, &rules_state).await?;
    let custom_rules = list_rule_files(state, "custom", &manifest_cache, &rules_state).await?;
    let status = match state
        .go_backend
        .request_json(Method::GET, "/api/waf/status", Option::<&Value>::None)
        .await
    {
        Ok(value)
            if value
                .get("success")
                .and_then(Value::as_bool)
                .unwrap_or(false) =>
        {
            value.get("data").cloned().unwrap_or(Value::Null)
        }
        Ok(_) | Err(_) => Value::Null,
    };
    let manifest = manifest_cache
        .get("manifest")
        .cloned()
        .unwrap_or(Value::Null);
    let update_available = manifest
        .get("zipHash")
        .and_then(Value::as_str)
        .filter(|hash| !hash.is_empty())
        .is_some_and(|hash| {
            synced
                .as_ref()
                .and_then(|value| value.get("zip_hash"))
                .and_then(Value::as_str)
                != Some(hash)
        });

    Ok(json!({
        "config": config,
        "status": status,
        "rules_dir": waf_root_dir(state).to_string_lossy(),
        "system": {
            "manifest": manifest,
            "manifest_cached_at": manifest_cache.get("cached_at").cloned().unwrap_or(Value::Null),
            "manifest_last_checked_at": manifest_cache.get("last_checked_at").cloned().unwrap_or(Value::Null),
            "manifest_last_error": manifest_cache.get("last_error").cloned().unwrap_or(Value::Null),
            "synced": synced.unwrap_or(Value::Null),
            "update_available": update_available,
            "rules": system_rules,
        },
        "custom": {
            "rules": custom_rules,
        },
    }))
}

async fn apply_waf_config(state: &AppState, patch: &Value) -> anyhow::Result<Value> {
    let mut full_config = state.redis.get_config().await?;
    if !full_config.is_object() {
        full_config = redis_store::default_config();
    }
    let current = normalize_fixed_waf_config(full_config.get("waf"), state);
    let mut next_raw = current.as_object().cloned().unwrap_or_default();
    if let Some(patch) = patch.as_object() {
        for key in [
            "enabled",
            "system_rules_auto_update_enabled",
            "common_location_exempt_enabled",
            "paranoia_level",
            "executing_paranoia_level",
        ] {
            if let Some(value) = patch.get(key) {
                next_raw.insert(key.to_string(), value.clone());
            }
        }
    }
    next_raw.insert(
        "updated_at".to_string(),
        Value::String(time_utils::now_iso()),
    );
    let next = normalize_fixed_waf_config(Some(&Value::Object(next_raw)), state);
    if let Some(object) = full_config.as_object_mut() {
        object.insert("waf".to_string(), next.clone());
    }
    state.redis.save_config(&full_config).await?;

    let should_apply_to_gateway = has_any_key(
        patch,
        &["enabled", "paranoia_level", "executing_paranoia_level"],
    );
    if should_apply_to_gateway {
        apply_waf_config_to_gateway(
            state,
            &next,
            "Enable WAF after at least one rule is enabled",
        )
        .await?;
    }
    if should_apply_to_gateway || has_any_key(patch, &["common_location_exempt_enabled"]) {
        sync_common_auth_location_exemptions_to_gateway(state, &next).await?;
    }

    get_waf_details(state).await
}

pub(crate) async fn sync_waf_config_to_gateway(
    state: &AppState,
    config: Option<&Value>,
) -> anyhow::Result<Value> {
    let normalized = normalize_fixed_waf_config(config, state);
    apply_waf_config_to_gateway(
        state,
        &normalized,
        "Enable WAF after at least one rule is enabled",
    )
    .await?;
    sync_common_auth_location_exemptions_to_gateway(state, &normalized).await?;
    Ok(normalized)
}

async fn sync_waf_on_boot(state: &AppState) -> anyhow::Result<()> {
    ensure_waf_directories(state).await?;
    let config = load_waf_config(state).await?;
    if config
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && config
            .get("system_rules_auto_update_enabled")
            .and_then(Value::as_bool)
            .unwrap_or(true)
    {
        match read_manifest_cache(state).await {
            Ok(cache)
                if cache.get("manifest").is_some_and(|value| !value.is_null())
                    && !is_manifest_stale(&cache) => {}
            _ => {
                if let Err(error) = refresh_system_manifest_cache(state).await {
                    tracing::warn!(%error, "failed to refresh WAF manifest on boot");
                }
            }
        }
    }
    sync_waf_config_to_gateway(state, Some(&config)).await?;
    Ok(())
}

async fn check_and_sync_system_waf_rules_if_needed(state: &AppState) -> anyhow::Result<Value> {
    ensure_waf_directories(state).await?;
    let checked_at = time_utils::now_iso();
    let config = load_waf_config(state).await?;
    if !config
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Ok(json!({
            "checked_at": checked_at,
            "updated": false,
            "skipped_reason": "waf_disabled",
        }));
    }
    if !config
        .get("system_rules_auto_update_enabled")
        .and_then(Value::as_bool)
        .unwrap_or(true)
    {
        return Ok(json!({
            "checked_at": checked_at,
            "updated": false,
            "skipped_reason": "disabled",
        }));
    }
    if !state
        .redis
        .set_lock_if_not_exists(
            "waf-system-rules-auto-update",
            WAF_SYSTEM_RULES_AUTO_UPDATE_LOCK_TTL_SECONDS as usize,
        )
        .await?
    {
        return Ok(json!({
            "checked_at": checked_at,
            "updated": false,
            "skipped_reason": "locked",
        }));
    }

    let cache = refresh_system_manifest_cache(state).await?;
    let manifest = cache
        .get("manifest")
        .filter(|value| !value.is_null())
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("WAF manifest is empty"))?;
    let synced = read_system_sync_state(state).await?;
    let has_local_rules = has_system_rule_files(state).await?;
    let manifest_zip_hash = manifest
        .get("zipHash")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let synced_zip_hash = synced
        .as_ref()
        .and_then(|value| value.get("zip_hash"))
        .and_then(Value::as_str)
        .map(str::to_string);
    if synced_zip_hash.as_deref() == Some(manifest_zip_hash.as_str()) && has_local_rules {
        return Ok(json!({
            "checked_at": checked_at,
            "updated": false,
            "manifest_zip_hash": manifest_zip_hash,
            "synced_zip_hash": synced_zip_hash,
            "skipped_reason": "up_to_date",
        }));
    }

    sync_system_waf_rules_from_manifest(state, &manifest).await?;
    Ok(json!({
        "checked_at": checked_at,
        "updated": true,
        "manifest_zip_hash": manifest_zip_hash,
        "synced_zip_hash": synced_zip_hash,
    }))
}

async fn has_system_rule_files(state: &AppState) -> anyhow::Result<bool> {
    let mut entries = match fs::read_dir(system_dir(state)).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    while let Some(entry) = entries.next_entry().await? {
        if entry
            .file_name()
            .to_string_lossy()
            .to_ascii_lowercase()
            .ends_with(".conf")
        {
            return Ok(true);
        }
    }
    Ok(false)
}

async fn waf_drain_interval_seconds(state: &AppState) -> u64 {
    state
        .redis
        .get_config()
        .await
        .ok()
        .and_then(|config| {
            config
                .pointer("/waf/drain_interval_seconds")
                .and_then(Value::as_i64)
        })
        .unwrap_or(2)
        .clamp(1, 3600) as u64
}

async fn set_waf_rule_enabled(state: &AppState, input: WafRuleToggleBody) -> anyhow::Result<Value> {
    ensure_waf_directories(state).await?;
    let source = if input.source.as_deref() == Some("custom") {
        "custom"
    } else {
        "system"
    };
    let details = get_waf_details(state).await?;
    let existing = details
        .pointer(if source == "system" {
            "/system/rules"
        } else {
            "/custom/rules"
        })
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let existing_names = existing
        .iter()
        .filter_map(|rule| rule.get("filename").and_then(Value::as_str))
        .map(str::to_string)
        .collect::<HashSet<_>>();
    let filenames = match input.filenames {
        Some(values) if !values.is_empty() => values
            .into_iter()
            .map(|value| safe_rule_filename(&value))
            .collect::<anyhow::Result<Vec<_>>>()?,
        _ => existing_names.iter().cloned().collect::<Vec<_>>(),
    };

    let mut state_file = read_rules_state(state).await?;
    let enabled_map = if source == "system" {
        &mut state_file.system_enabled
    } else {
        &mut state_file.custom_enabled
    };
    for filename in filenames {
        if existing_names.contains(&filename) {
            enabled_map.insert(filename, input.enabled);
        }
    }
    let config = load_waf_config(state).await?;
    if config
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && !has_any_enabled_rule_files(state, &state_file, None).await?
    {
        anyhow::bail!("Keep at least one WAF rule enabled");
    }

    write_rules_state(state, &state_file).await?;
    apply_waf_config_to_gateway(state, &config, "Keep at least one WAF rule enabled").await?;
    get_waf_details(state).await
}

async fn read_waf_rule_file(
    state: &AppState,
    source: &str,
    filename: &str,
) -> anyhow::Result<Value> {
    ensure_waf_directories(state).await?;
    let source = normalize_rule_source(source)?;
    let safe = safe_rule_filename(filename)?;
    let manifest_cache = get_manifest_cache_for_details(state).await?;
    let rules_state = read_rules_state(state).await?;
    let rules = list_rule_files(state, source, &manifest_cache, &rules_state).await?;
    let rule = rules
        .into_iter()
        .find(|rule| rule.get("filename").and_then(Value::as_str) == Some(safe.as_str()))
        .ok_or_else(|| anyhow::anyhow!("WAF rule file not found"))?;
    let content = read_utf8_rule_text(
        &fs::read(rule_file_path(state, source, &safe)).await?,
        &safe,
    )?;
    let mut object = rule.as_object().cloned().unwrap_or_default();
    object.insert("content".to_string(), Value::String(content));
    Ok(Value::Object(object))
}

async fn upload_custom_waf_rules(state: &AppState, input: WafUploadBody) -> anyhow::Result<Value> {
    ensure_waf_directories(state).await?;
    if input.files.is_empty() {
        anyhow::bail!("Select at least one .conf file");
    }
    let mut rules_state = read_rules_state(state).await?;
    for file in input.files {
        let filename =
            make_unique_custom_filename(state, &safe_rule_filename(&file.filename)?).await?;
        let raw = general_purpose::STANDARD
            .decode(file.content_base64.as_bytes())
            .map_err(|_| anyhow::anyhow!("Invalid base64 content"))?;
        let content = decode_utf8_rule(&raw, &filename)?;
        fs::write(custom_dir(state).join(&filename), content).await?;
        rules_state.custom_enabled.insert(filename, true);
    }
    write_rules_state(state, &rules_state).await?;
    let config = load_waf_config(state).await?;
    apply_waf_config_to_gateway(
        state,
        &config,
        "Enable WAF after at least one rule is enabled",
    )
    .await?;
    get_waf_details(state).await
}

async fn delete_custom_waf_rule(state: &AppState, filename: &str) -> anyhow::Result<Value> {
    ensure_waf_directories(state).await?;
    let safe = safe_rule_filename(filename)?;
    let config = load_waf_config(state).await?;
    if config
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        let rules_state = read_rules_state(state).await?;
        if !has_any_enabled_rule_files(state, &rules_state, Some(("custom", safe.as_str()))).await?
        {
            anyhow::bail!("Keep at least one WAF rule enabled");
        }
    }
    match fs::remove_file(custom_dir(state).join(&safe)).await {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let mut rules_state = read_rules_state(state).await?;
    rules_state.custom_enabled.remove(&safe);
    write_rules_state(state, &rules_state).await?;
    apply_waf_config_to_gateway(
        state,
        &config,
        "Enable WAF after at least one rule is enabled",
    )
    .await?;
    get_waf_details(state).await
}

async fn drain_waf_events_now(state: &AppState) -> anyhow::Result<Value> {
    let config = load_waf_config(state).await?;
    let response = state
        .go_backend
        .drain_waf_events(DEFAULT_DRAIN_LIMIT)
        .await?;
    let data = go_response_data(response, "Failed to drain WAF events")?;
    let raw_events = data
        .get("events")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let events = raw_events
        .into_iter()
        .filter_map(sanitize_event)
        .collect::<Vec<_>>();
    if !events.is_empty() {
        state
            .redis
            .persist_waf_events(
                &events,
                config
                    .get("log_retention_days")
                    .and_then(Value::as_i64)
                    .unwrap_or(7),
            )
            .await?;
        for event in events.iter().filter(|event| is_waf_blocking_event(event)) {
            if let Err(error) = system_events::publish_waf_blocked_event(state, event).await {
                tracing::warn!(%error, "failed to publish WAF blocked event");
            }
        }
    }
    Ok(json!({
        "drained": data.get("drained").and_then(Value::as_i64).unwrap_or(0),
        "remaining": data.get("remaining").and_then(Value::as_i64).unwrap_or(0),
    }))
}

async fn load_waf_config(state: &AppState) -> redis::RedisResult<Value> {
    let config = state.redis.get_config().await?;
    Ok(normalize_fixed_waf_config(config.get("waf"), state))
}

fn normalize_fixed_waf_config(value: Option<&Value>, state: &AppState) -> Value {
    let raw = value.and_then(Value::as_object);
    let paranoia_level =
        normalize_i64(raw.and_then(|object| object.get("paranoia_level")), 1, 1, 4);
    let executing_fallback = if raw
        .and_then(|object| object.get("paranoia_level"))
        .is_some()
    {
        paranoia_level
    } else {
        1
    };
    let executing_paranoia_level = normalize_i64(
        raw.and_then(|object| object.get("executing_paranoia_level")),
        executing_fallback,
        1,
        4,
    )
    .max(paranoia_level);
    let request_body_limit = normalize_i64(
        raw.and_then(|object| object.get("request_body_limit_bytes")),
        131_072,
        1024,
        128 * 1024 * 1024,
    );
    let request_body_memory_limit = normalize_i64(
        raw.and_then(|object| object.get("request_body_in_memory_limit_bytes")),
        65_536.min(request_body_limit),
        1024,
        request_body_limit,
    );

    json!({
        "enabled": raw
            .and_then(|object| object.get("enabled"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        "system_rules_auto_update_enabled": raw
            .and_then(|object| object.get("system_rules_auto_update_enabled"))
            .and_then(Value::as_bool)
            .unwrap_or(true),
        "common_location_exempt_enabled": raw
            .and_then(|object| object.get("common_location_exempt_enabled"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        "mode": "blocking",
        "active_bundle_id": "local",
        "rules_dir": waf_root_dir(state).to_string_lossy(),
        "paranoia_level": paranoia_level,
        "executing_paranoia_level": executing_paranoia_level,
        "inbound_anomaly_threshold": 5,
        "outbound_anomaly_threshold": 4,
        "request_body_access": true,
        "request_body_limit_bytes": request_body_limit,
        "request_body_in_memory_limit_bytes": request_body_memory_limit,
        "response_body_access": false,
        "disabled_hosts": [],
        "disabled_path_prefixes": [],
        "log_retention_days": normalize_i64(
            raw.and_then(|object| object.get("log_retention_days")),
            7,
            1,
            365,
        ),
        "drain_interval_seconds": normalize_i64(
            raw.and_then(|object| object.get("drain_interval_seconds")),
            2,
            1,
            60,
        ),
        "updated_at": raw
            .and_then(|object| object.get("updated_at"))
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(|value| Value::String(value.to_string()))
            .unwrap_or(Value::Null),
    })
}

async fn apply_waf_config_to_gateway(
    state: &AppState,
    config: &Value,
    empty_rules_message: &str,
) -> anyhow::Result<()> {
    if !config
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        let response = state.go_backend.set_waf_config(config).await?;
        let _ = go_response_data(response, "Failed to sync WAF config")?;
        return Ok(());
    }
    let rules_state = read_rules_state(state).await?;
    if !has_any_enabled_rule_files(state, &rules_state, None).await? {
        anyhow::bail!("{empty_rules_message}");
    }
    let response = state.go_backend.reload_waf_rules(config).await?;
    let _ = go_response_data(response, "Failed to load WAF rules")?;
    Ok(())
}

async fn sync_common_auth_location_exemptions_to_gateway(
    state: &AppState,
    waf_config: &Value,
) -> anyhow::Result<()> {
    let runtime = state
        .redis
        .get_string_value("fn_knock:common_auth_locations:runtime")
        .await?
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        .unwrap_or_else(|| json!({}));
    let cidrs = runtime
        .get("cidrs")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let enabled = waf_config
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && waf_config
            .get("common_location_exempt_enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        && runtime
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        && !cidrs.is_empty();
    let payload = json!({
        "enabled": enabled,
        "waf_enabled": enabled,
        "cidrs": if enabled { cidrs } else { Vec::<String>::new() },
        "updated_at": runtime.get("updated_at").cloned().unwrap_or(Value::Null),
    });
    let (status, value) = state
        .go_backend
        .set_common_location_exemptions(&payload)
        .await?;
    if status == reqwest::StatusCode::NOT_FOUND || status == reqwest::StatusCode::NOT_IMPLEMENTED {
        return Ok(());
    }
    if !value
        .get("success")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        anyhow::bail!(
            "{}",
            value
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("Failed to sync common location exemptions")
        );
    }
    Ok(())
}

fn go_response_data(response: Value, fallback: &str) -> anyhow::Result<Value> {
    if !response
        .get("success")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        anyhow::bail!(
            response
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or(fallback)
                .to_string()
        );
    }
    Ok(response.get("data").cloned().unwrap_or(Value::Null))
}

async fn ensure_waf_directories(state: &AppState) -> io::Result<()> {
    fs::create_dir_all(system_dir(state)).await?;
    fs::create_dir_all(custom_dir(state)).await
}

async fn get_manifest_cache_for_details(state: &AppState) -> anyhow::Result<Value> {
    let mut cache = read_manifest_cache(state).await?;
    if cache.get("manifest").is_none_or(Value::is_null) || is_manifest_stale(&cache) {
        if refresh_system_manifest_cache(state).await.is_ok() {
            cache = read_manifest_cache(state).await?;
        } else {
            cache = read_manifest_cache(state).await?;
        }
    }
    Ok(cache)
}

async fn refresh_system_manifest_cache(state: &AppState) -> anyhow::Result<Value> {
    ensure_waf_directories(state).await?;
    let checked_at = time_utils::now_iso();
    let previous = read_manifest_cache(state).await?;
    let result = async {
        let response = state
            .fallback_client
            .get(cache_busted_url(MANIFEST_URL, None)?)
            .header("cache-control", "no-cache, no-store")
            .header("pragma", "no-cache")
            .send()
            .await?;
        if !response.status().is_success() {
            anyhow::bail!("WAF manifest request failed: {}", response.status());
        }
        let manifest = validate_manifest(response.json::<Value>().await?)?;
        let cache = json!({
            "manifest": manifest,
            "cached_at": checked_at,
            "last_checked_at": checked_at,
            "last_error": Value::Null,
        });
        write_json_file(&manifest_cache_path(state), &cache).await?;
        anyhow::Ok(cache)
    }
    .await;
    match result {
        Ok(cache) => Ok(cache),
        Err(error) => {
            let cache = json!({
                "manifest": previous.get("manifest").cloned().unwrap_or(Value::Null),
                "cached_at": previous.get("cached_at").cloned().unwrap_or(Value::Null),
                "last_checked_at": checked_at,
                "last_error": error.to_string(),
            });
            write_json_file(&manifest_cache_path(state), &cache).await?;
            Err(error)
        }
    }
}

async fn sync_system_waf_rules(state: &AppState) -> anyhow::Result<Value> {
    let cache = refresh_system_manifest_cache(state).await?;
    let manifest = cache
        .get("manifest")
        .filter(|value| !value.is_null())
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("WAF manifest is empty"))?;
    sync_system_waf_rules_from_manifest(state, &manifest).await
}

async fn sync_system_waf_rules_from_manifest(
    state: &AppState,
    manifest: &Value,
) -> anyhow::Result<Value> {
    let zip_buffer = download_system_zip(state, manifest).await?;
    let entries = unpack_system_rules_zip(&zip_buffer)?;
    if entries.rule_files.is_empty() {
        anyhow::bail!("WAF system rule bundle contains no .conf files");
    }

    let temp_dir = waf_root_dir(state).join(format!("system.tmp-{}", time_utils::now_ms()));
    let _ = fs::remove_dir_all(&temp_dir).await;
    fs::create_dir_all(&temp_dir).await?;
    for (relative_path, content) in entries.bundle_files {
        let file_path = temp_dir.join(&relative_path);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::write(file_path, content).await?;
    }
    let system_dir = system_dir(state);
    let _ = fs::remove_dir_all(&system_dir).await;
    fs::rename(&temp_dir, &system_dir).await?;

    let mut rules_state = read_rules_state(state).await?;
    let previous = rules_state.system_enabled.clone();
    rules_state.system_enabled = entries
        .rule_files
        .keys()
        .map(|filename| {
            (
                filename.clone(),
                previous
                    .get(filename)
                    .copied()
                    .unwrap_or_else(|| is_system_rule_enabled_by_default(filename)),
            )
        })
        .collect();
    write_rules_state(state, &rules_state).await?;

    write_json_file(
        &system_sync_path(state),
        &json!({
            "zip_file": manifest.get("zipFile").cloned().unwrap_or(Value::Null),
            "zip_hash": manifest.get("zipHash").cloned().unwrap_or(Value::Null),
            "synced_at": time_utils::now_iso(),
            "packaging_time": manifest.get("packagingTime").cloned().unwrap_or(Value::Null),
            "commit_hash": manifest.get("commitHash").cloned().unwrap_or(Value::Null),
            "commit_date": manifest.get("commitDate").cloned().unwrap_or(Value::Null),
        }),
    )
    .await?;

    let config = load_waf_config(state).await?;
    apply_waf_config_to_gateway(
        state,
        &config,
        "Enable WAF after at least one rule is enabled",
    )
    .await?;
    get_waf_details(state).await
}

struct UnpackedWafBundle {
    bundle_files: Vec<(String, Vec<u8>)>,
    rule_files: BTreeMap<String, String>,
}

fn unpack_system_rules_zip(buffer: &[u8]) -> anyhow::Result<UnpackedWafBundle> {
    let mut archive = ZipArchive::new(Cursor::new(buffer))?;
    let mut bundle_files = Vec::new();
    let mut bundle_path_keys = HashSet::new();
    let mut rule_files = BTreeMap::new();
    let mut unpacked_bytes = 0_usize;

    for index in 0..archive.len() {
        let mut file = archive.by_index(index)?;
        if file.is_dir() {
            continue;
        }
        let relative_path = safe_bundle_entry_path(file.name())?;
        let path_key = relative_path.to_ascii_lowercase();
        if !bundle_path_keys.insert(path_key) {
            anyhow::bail!("Duplicate WAF bundle file: {relative_path}");
        }

        let mut content = Vec::new();
        file.read_to_end(&mut content)?;
        unpacked_bytes = unpacked_bytes.saturating_add(content.len());
        if unpacked_bytes > MAX_UNPACKED_ZIP_BYTES {
            anyhow::bail!("WAF system rule bundle is too large after unpacking");
        }

        let filename = relative_path.rsplit('/').next().unwrap_or("").to_string();
        if is_conf_filename(&filename) {
            if relative_path != filename {
                anyhow::bail!("WAF .conf files must be in the bundle root");
            }
            let text = decode_utf8_rule(&content, &filename)?;
            bundle_files.push((relative_path, text.as_bytes().to_vec()));
            rule_files.insert(filename, text);
        } else {
            bundle_files.push((relative_path, content));
        }
    }

    bundle_files.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(UnpackedWafBundle {
        bundle_files,
        rule_files,
    })
}

async fn download_system_zip(state: &AppState, manifest: &Value) -> anyhow::Result<Vec<u8>> {
    let zip_file = manifest
        .get("zipFile")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("WAF manifest missing zip file"))?;
    let expected_hash = manifest
        .get("zipHash")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("WAF manifest missing zip hash"))?
        .to_ascii_lowercase();
    let response = state
        .fallback_client
        .get(cache_busted_url(zip_file, Some(MANIFEST_URL))?)
        .header("cache-control", "no-cache, no-store")
        .header("pragma", "no-cache")
        .send()
        .await?;
    if !response.status().is_success() {
        anyhow::bail!("WAF system rule download failed: {}", response.status());
    }
    let buffer = response.bytes().await?.to_vec();
    if buffer.len() > MAX_ZIP_BYTES {
        anyhow::bail!("WAF system rule zip is too large");
    }
    let actual_hash = hex::encode(Sha256::digest(&buffer));
    if actual_hash != expected_hash {
        anyhow::bail!("WAF system rule zip hash mismatch");
    }
    Ok(buffer)
}

fn safe_bundle_entry_path(value: &str) -> anyhow::Result<String> {
    let normalized = value.replace('\\', "/");
    let segments = normalized.split('/').collect::<Vec<_>>();
    let valid_chars = normalized
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-' | '/'));
    if normalized.is_empty()
        || normalized != normalized.trim()
        || normalized.starts_with('/')
        || normalized.contains("://")
        || segments
            .iter()
            .any(|segment| segment.is_empty() || *segment == "." || *segment == "..")
        || !valid_chars
    {
        anyhow::bail!("Invalid WAF bundle path: {value}");
    }
    Ok(segments.join("/"))
}

fn validate_manifest(mut value: Value) -> anyhow::Result<Value> {
    let Some(object) = value.as_object_mut() else {
        anyhow::bail!("Invalid WAF manifest");
    };
    let zip_file = object
        .get("zipFile")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("WAF manifest missing zip info"))?
        .to_string();
    let zip_hash = object
        .get("zipHash")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("WAF manifest missing zip info"))?
        .to_string();
    object.insert("zipFile".to_string(), Value::String(zip_file));
    object.insert("zipHash".to_string(), Value::String(zip_hash));
    Ok(value)
}

fn is_manifest_stale(cache: &Value) -> bool {
    let checked_ms = cache
        .get("last_checked_at")
        .or_else(|| cache.get("cached_at"))
        .and_then(Value::as_str)
        .and_then(time_utils::parse_iso_ms)
        .unwrap_or(0);
    checked_ms <= 0 || time_utils::now_ms() - checked_ms > MANIFEST_REFRESH_MS
}

async fn read_manifest_cache(state: &AppState) -> anyhow::Result<Value> {
    read_json_file(
        &manifest_cache_path(state),
        json!({
            "manifest": Value::Null,
            "cached_at": Value::Null,
            "last_checked_at": Value::Null,
            "last_error": Value::Null,
        }),
    )
    .await
}

async fn read_system_sync_state(state: &AppState) -> anyhow::Result<Option<Value>> {
    let value = read_json_file(&system_sync_path(state), Value::Null).await?;
    Ok((!value.is_null()).then_some(value))
}

async fn read_rules_state(state: &AppState) -> anyhow::Result<WafRulesState> {
    let state = read_json_file(&rules_state_path(state), default_rules_state()).await?;
    Ok(enforce_required_rule_state(state))
}

async fn write_rules_state(state: &AppState, rules_state: &WafRulesState) -> anyhow::Result<()> {
    let normalized = enforce_required_rule_state(rules_state.clone());
    write_json_file(&rules_state_path(state), &normalized).await
}

async fn read_json_file<T>(path: &FsPath, fallback: T) -> anyhow::Result<T>
where
    T: DeserializeOwned,
{
    match fs::read_to_string(path).await {
        Ok(raw) => Ok(serde_json::from_str::<T>(&raw)?),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(fallback),
        Err(error) => Err(error.into()),
    }
}

async fn write_json_file<T>(path: &FsPath, value: &T) -> anyhow::Result<()>
where
    T: Serialize + ?Sized,
{
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }
    let raw = format!("{}\n", serde_json::to_string_pretty(value)?);
    fs::write(path, raw).await?;
    Ok(())
}

async fn list_rule_files(
    state: &AppState,
    source: &str,
    manifest_cache: &Value,
    rules_state: &WafRulesState,
) -> anyhow::Result<Vec<Value>> {
    let dir = if source == "system" {
        system_dir(state)
    } else {
        custom_dir(state)
    };
    let descriptions = manifest_descriptions(
        manifest_cache
            .get("manifest")
            .filter(|value| !value.is_null()),
    );
    let enabled_map = if source == "system" {
        &rules_state.system_enabled
    } else {
        &rules_state.custom_enabled
    };
    let mut entries = match fs::read_dir(&dir).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    let mut rules = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        let file_type = entry.file_type().await?;
        let filename = entry.file_name().to_string_lossy().to_string();
        if !file_type.is_file() || !is_conf_filename(&filename) {
            continue;
        }
        if source == "system" && filename == INITIALIZATION_RULE_FILENAME {
            continue;
        }
        let metadata = entry.metadata().await?;
        rules.push(json!({
            "source": source,
            "filename": filename,
            "description": descriptions
                .get(&filename)
                .cloned()
                .unwrap_or_else(|| if source == "system" {
                    "System WAF rule".to_string()
                } else {
                    "Custom WAF rule".to_string()
                }),
            "enabled": enabled_map
                .get(&filename)
                .copied()
                .unwrap_or_else(|| if source == "system" {
                    is_system_rule_enabled_by_default(&filename)
                } else {
                    true
                }),
            "size_bytes": metadata.len(),
            "updated_at": system_time_iso(metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH)),
        }));
    }
    rules.sort_by(|left, right| {
        left.get("filename")
            .and_then(Value::as_str)
            .unwrap_or("")
            .cmp(right.get("filename").and_then(Value::as_str).unwrap_or(""))
    });
    Ok(rules)
}

async fn has_any_enabled_rule_files(
    state: &AppState,
    rules_state: &WafRulesState,
    omit: Option<(&str, &str)>,
) -> anyhow::Result<bool> {
    let manifest_cache = get_manifest_cache_for_details(state).await?;
    let system_rules = list_rule_files(state, "system", &manifest_cache, rules_state).await?;
    let custom_rules = list_rule_files(state, "custom", &manifest_cache, rules_state).await?;
    Ok(system_rules.into_iter().chain(custom_rules).any(|rule| {
        let source = rule.get("source").and_then(Value::as_str).unwrap_or("");
        let filename = rule.get("filename").and_then(Value::as_str).unwrap_or("");
        rule.get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            && omit != Some((source, filename))
    }))
}

fn manifest_descriptions(manifest: Option<&Value>) -> BTreeMap<String, String> {
    manifest
        .and_then(|value| value.pointer("/rulesDescription/rules"))
        .and_then(Value::as_array)
        .map(|rules| {
            rules
                .iter()
                .filter_map(|rule| {
                    let filename = rule.get("filename")?.as_str()?.trim();
                    if filename.is_empty() {
                        return None;
                    }
                    Some((
                        filename.to_string(),
                        rule.get("description")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .trim()
                            .to_string(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn default_rules_state() -> WafRulesState {
    let mut system_enabled = BTreeMap::new();
    system_enabled.insert(INITIALIZATION_RULE_FILENAME.to_string(), true);
    WafRulesState {
        system_enabled,
        custom_enabled: BTreeMap::new(),
    }
}

fn enforce_required_rule_state(mut state: WafRulesState) -> WafRulesState {
    state
        .system_enabled
        .insert(INITIALIZATION_RULE_FILENAME.to_string(), true);
    state
}

fn is_system_rule_enabled_by_default(filename: &str) -> bool {
    filename == INITIALIZATION_RULE_FILENAME
        || !DEFAULT_DISABLED_SYSTEM_RULE_FILENAMES.contains(&filename)
}

async fn make_unique_custom_filename(state: &AppState, filename: &str) -> anyhow::Result<String> {
    let ext = FsPath::new(filename)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| format!(".{value}"))
        .unwrap_or_default();
    let base = filename.strip_suffix(&ext).unwrap_or(filename);
    let mut candidate = filename.to_string();
    let mut index = 1;
    loop {
        match fs::metadata(custom_dir(state).join(&candidate)).await {
            Ok(_) => {
                candidate = format!("{base}-{index}{ext}");
                index += 1;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(candidate),
            Err(error) => return Err(error.into()),
        }
    }
}

fn safe_rule_filename(value: &str) -> anyhow::Result<String> {
    let raw = value
        .replace('\\', "/")
        .rsplit('/')
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    if raw.is_empty() || raw == "." || raw == ".." || !is_conf_filename(&raw) {
        anyhow::bail!("Only .conf WAF rule files are supported");
    }
    let safe = raw
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    if safe.is_empty() || !is_conf_filename(&safe) {
        anyhow::bail!("Invalid WAF rule filename");
    }
    Ok(safe)
}

fn normalize_rule_source(source: &str) -> anyhow::Result<&'static str> {
    match source {
        "system" => Ok("system"),
        "custom" => Ok("custom"),
        _ => anyhow::bail!("Invalid WAF rule source"),
    }
}

fn decode_utf8_rule(content: &[u8], filename: &str) -> anyhow::Result<String> {
    if content.len() > MAX_RULE_FILE_BYTES {
        anyhow::bail!("WAF rule file is too large: {filename}");
    }
    let text = String::from_utf8(content.to_vec())
        .map_err(|_| anyhow::anyhow!("WAF rule file is not valid UTF-8: {filename}"))?;
    let text = text.trim_start_matches('\u{feff}').to_string();
    if contains_blocked_directive(&text) {
        anyhow::bail!("WAF rule file contains blocked filesystem directives: {filename}");
    }
    Ok(text)
}

fn read_utf8_rule_text(content: &[u8], filename: &str) -> anyhow::Result<String> {
    if content.len() > MAX_RULE_FILE_BYTES {
        anyhow::bail!("WAF rule file is too large: {filename}");
    }
    let text = String::from_utf8(content.to_vec())
        .map_err(|_| anyhow::anyhow!("WAF rule file is not valid UTF-8: {filename}"))?;
    Ok(text.trim_start_matches('\u{feff}').to_string())
}

fn contains_blocked_directive(text: &str) -> bool {
    text.lines().any(|line| {
        let trimmed = line.trim_start();
        [
            "Include",
            "SecAuditLog",
            "SecDebugLog",
            "SecDataDir",
            "SecTmpDir",
            "SecUploadDir",
        ]
        .iter()
        .any(|directive| starts_with_directive(trimmed, directive))
    })
}

fn starts_with_directive(line: &str, directive: &str) -> bool {
    let Some(prefix) = line.get(..directive.len()) else {
        return false;
    };
    if !prefix.eq_ignore_ascii_case(directive) {
        return false;
    }
    line[directive.len()..]
        .chars()
        .next()
        .is_none_or(char::is_whitespace)
}

fn is_conf_filename(filename: &str) -> bool {
    filename.to_ascii_lowercase().ends_with(".conf")
}

fn normalize_i64(value: Option<&Value>, fallback: i64, min: i64, max: i64) -> i64 {
    let parsed = value
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_str().and_then(|raw| raw.parse::<i64>().ok()))
        })
        .unwrap_or(fallback);
    parsed.clamp(min, max)
}

fn has_any_key(value: &Value, keys: &[&str]) -> bool {
    value
        .as_object()
        .is_some_and(|object| keys.iter().any(|key| object.contains_key(*key)))
}

fn cache_busted_url(input: &str, base: Option<&str>) -> anyhow::Result<String> {
    let mut url = if let Some(base) = base {
        url::Url::parse(base)?.join(input)?
    } else {
        url::Url::parse(input)?
    };
    url.query_pairs_mut().append_pair(
        "t",
        &format!("{}-{}", time_utils::now_ms(), uuid::Uuid::new_v4()),
    );
    Ok(url.to_string())
}

fn system_time_iso(time: SystemTime) -> String {
    let duration = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    time::OffsetDateTime::from_unix_timestamp(duration.as_secs() as i64)
        .ok()
        .and_then(|value| {
            value
                .format(&time::format_description::well_known::Rfc3339)
                .ok()
        })
        .unwrap_or_else(time_utils::now_iso)
}

fn waf_root_dir(state: &AppState) -> PathBuf {
    state.settings.gateway_config_dir.join("waf")
}

fn system_dir(state: &AppState) -> PathBuf {
    waf_root_dir(state).join("system")
}

fn custom_dir(state: &AppState) -> PathBuf {
    waf_root_dir(state).join("custom")
}

fn manifest_cache_path(state: &AppState) -> PathBuf {
    waf_root_dir(state).join("manifest.json")
}

fn system_sync_path(state: &AppState) -> PathBuf {
    waf_root_dir(state).join("system-sync.json")
}

fn rules_state_path(state: &AppState) -> PathBuf {
    waf_root_dir(state).join("rules-state.json")
}

fn rule_file_path(state: &AppState, source: &str, filename: &str) -> PathBuf {
    if source == "system" {
        system_dir(state).join(filename)
    } else {
        custom_dir(state).join(filename)
    }
}

fn is_waf_blocking_event(event: &Value) -> bool {
    let action = event
        .get("action")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_lowercase();
    if matches!(action.as_str(), "block" | "deny") {
        return true;
    }
    if matches!(action.as_str(), "detect" | "log") {
        return false;
    }
    event
        .get("mode")
        .and_then(Value::as_str)
        .is_some_and(|mode| mode.eq_ignore_ascii_case("blocking"))
        && event.get("status").and_then(Value::as_i64).is_some()
}

async fn query_waf_logs(state: &AppState, query: &WafLogQuery) -> anyhow::Result<Value> {
    let date = normalize_date(query.date.as_deref()).map_err(anyhow::Error::msg)?;
    let available_dates = state.redis.list_waf_log_dates(&today()).await?;
    let limit = normalize_limit(query.limit.as_deref());
    let cursor = normalize_cursor(query.cursor.as_deref());

    if let Some(trace_id) = query
        .trace_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let event = get_waf_log_event(state, trace_id).await?;
        let items = event
            .filter(|event| event_matches(event, query))
            .into_iter()
            .collect::<Vec<_>>();
        return Ok(json!({
            "date": date,
            "available_dates": available_dates,
            "cursor": cursor.to_string(),
            "next_cursor": "",
            "has_more": false,
            "limit": limit,
            "total": items.len(),
            "items": items,
        }));
    }

    let page = if has_log_filters(query) {
        query_filtered(state, &date, query, cursor, limit).await?
    } else {
        query_unfiltered(state, &date, cursor, limit).await?
    };

    Ok(json!({
        "date": date,
        "available_dates": available_dates,
        "cursor": cursor.to_string(),
        "next_cursor": page.next_cursor,
        "has_more": page.has_more,
        "limit": limit,
        "total": page.total,
        "items": page.items,
    }))
}

struct WafLogPage {
    items: Vec<Value>,
    next_cursor: String,
    has_more: bool,
    total: i64,
}

async fn query_unfiltered(
    state: &AppState,
    date: &str,
    cursor: i64,
    limit: i64,
) -> anyhow::Result<WafLogPage> {
    let original_total = state.redis.waf_log_date_total(date).await?;
    let mut events = Vec::<Value>::new();
    let mut stale_ids = Vec::<String>::new();
    let mut offset = cursor;

    while events.len() < (limit + 1) as usize {
        let ids = state
            .redis
            .waf_log_ids_desc(
                date,
                offset as isize,
                offset as isize + UNFILTERED_QUERY_SCAN_CHUNK_SIZE - 1,
            )
            .await?;
        if ids.is_empty() {
            break;
        }
        offset += ids.len() as i64;
        let batch = events_by_ids(state, &ids).await?;
        events.extend(batch.events);
        stale_ids.extend(batch.stale_ids);
    }

    state
        .redis
        .remove_waf_log_stale_ids(date, &stale_ids)
        .await?;

    let has_more = events.len() > limit as usize;
    let items = events
        .into_iter()
        .take(limit.max(0) as usize)
        .collect::<Vec<_>>();
    let next_cursor = cursor + items.len() as i64;

    Ok(WafLogPage {
        next_cursor: if has_more {
            next_cursor.to_string()
        } else {
            String::new()
        },
        has_more,
        total: (original_total - stale_ids.len() as i64).max(0),
        items,
    })
}

async fn query_filtered(
    state: &AppState,
    date: &str,
    query: &WafLogQuery,
    cursor: i64,
    limit: i64,
) -> anyhow::Result<WafLogPage> {
    let mut offset = 0_i64;
    let mut matched_total = 0_i64;
    let mut items = Vec::<Value>::new();
    let mut stale_ids = Vec::<String>::new();

    loop {
        let ids = state
            .redis
            .waf_log_ids_desc(
                date,
                offset as isize,
                offset as isize + FILTERED_QUERY_SCAN_CHUNK_SIZE - 1,
            )
            .await?;
        if ids.is_empty() {
            break;
        }
        offset += ids.len() as i64;
        let batch = events_by_ids(state, &ids).await?;
        stale_ids.extend(batch.stale_ids);

        for event in batch.events {
            if !event_matches(&event, query) {
                continue;
            }
            if matched_total >= cursor && items.len() < limit as usize {
                items.push(event);
            }
            matched_total += 1;
        }
    }

    state
        .redis
        .remove_waf_log_stale_ids(date, &stale_ids)
        .await?;
    let next_cursor = cursor + items.len() as i64;
    let has_more = next_cursor < matched_total;
    Ok(WafLogPage {
        next_cursor: if has_more {
            next_cursor.to_string()
        } else {
            String::new()
        },
        has_more,
        total: matched_total,
        items,
    })
}

struct EventBatch {
    events: Vec<Value>,
    stale_ids: Vec<String>,
}

async fn events_by_ids(state: &AppState, ids: &[String]) -> anyhow::Result<EventBatch> {
    let raws = state.redis.waf_log_events_by_ids(ids).await?;
    let mut events = Vec::new();
    let mut stale_ids = Vec::new();
    for (id, raw) in ids.iter().zip(raws) {
        match raw.and_then(sanitize_event) {
            Some(event) => events.push(event),
            None => stale_ids.push(id.clone()),
        }
    }
    Ok(EventBatch { events, stale_ids })
}

async fn get_waf_log_event(state: &AppState, trace_id: &str) -> anyhow::Result<Option<Value>> {
    let trace_id = trace_id.trim();
    if trace_id.is_empty() {
        return Ok(None);
    }
    Ok(state
        .redis
        .get_waf_log_event(trace_id)
        .await?
        .and_then(sanitize_event))
}

fn sanitize_event(mut event: Value) -> Option<Value> {
    if event
        .get("trace_id")
        .and_then(Value::as_str)?
        .trim()
        .is_empty()
    {
        return None;
    }

    let original_rules = event
        .get("rules")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let initialization_rule_ids = original_rules
        .iter()
        .filter(|rule| is_initialization_rule(rule))
        .filter_map(|rule| rule.get("id").and_then(Value::as_i64))
        .collect::<std::collections::HashSet<_>>();
    let rules = original_rules
        .into_iter()
        .filter(|rule| !is_initialization_rule(rule))
        .collect::<Vec<_>>();
    let rule_ids = event.get("rule_ids").and_then(Value::as_array).map(|ids| {
        ids.iter()
            .filter(|id| {
                id.as_i64()
                    .is_none_or(|id| !initialization_rule_ids.contains(&id))
            })
            .cloned()
            .collect::<Vec<_>>()
    });
    let interruption_rule_id = event
        .pointer("/interruption/rule_id")
        .and_then(Value::as_i64);
    let remove_interruption =
        interruption_rule_id.is_some_and(|id| initialization_rule_ids.contains(&id));
    let has_rule_signal = !rules.is_empty() || rule_ids.as_ref().is_some_and(|ids| !ids.is_empty());
    let has_blocking_signal = is_blocking_action(event.get("action"))
        || (event.get("interruption").is_some() && !remove_interruption);
    if !has_rule_signal && !has_blocking_signal {
        return None;
    }

    let object = event.as_object_mut()?;
    if !rules.is_empty() || object.contains_key("rules") {
        object.insert("rules".to_string(), Value::Array(rules));
    }
    if let Some(rule_ids) = rule_ids {
        object.insert("rule_ids".to_string(), Value::Array(rule_ids));
    }
    if remove_interruption {
        object.remove("interruption");
    }
    Some(event)
}

fn event_matches(event: &Value, query: &WafLogQuery) -> bool {
    let host = query.host.as_deref().unwrap_or("").trim().to_lowercase();
    if !host.is_empty()
        && event
            .get("host")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_lowercase()
            != host
    {
        return false;
    }

    let client_ip = query.client_ip.as_deref().unwrap_or("").trim();
    if !client_ip.is_empty() && event.get("client_ip").and_then(Value::as_str) != Some(client_ip) {
        return false;
    }

    let route_type = query.route_type.as_deref().unwrap_or("").trim();
    if !route_type.is_empty() && event.get("route_type").and_then(Value::as_str) != Some(route_type)
    {
        return false;
    }

    let mode = query.mode.as_deref().unwrap_or("").trim();
    if !mode.is_empty() && event.get("mode").and_then(Value::as_str) != Some(mode) {
        return false;
    }

    let raw_rule_id = query.rule_id.as_deref().unwrap_or("").trim();
    if !raw_rule_id.is_empty() {
        let Some(rule_id) = parse_i64_prefix(raw_rule_id.trim_start()) else {
            return false;
        };
        let matches_rule = event
            .get("rule_ids")
            .and_then(Value::as_array)
            .is_some_and(|ids| ids.iter().any(|id| id.as_i64() == Some(rule_id)));
        if !matches_rule {
            return false;
        }
    }

    let search = query.search.as_deref().unwrap_or("").trim().to_lowercase();
    if !search.is_empty() {
        let mut haystack = Vec::new();
        for key in [
            "trace_id",
            "host",
            "path",
            "request_uri",
            "client_ip",
            "route_key",
            "upstream",
            "bundle_id",
        ] {
            if let Some(value) = event.get(key).and_then(Value::as_str) {
                haystack.push(value.to_string());
            }
        }
        if let Some(ids) = event.get("rule_ids").and_then(Value::as_array) {
            haystack.extend(ids.iter().map(|id| id.to_string()));
        }
        if !haystack
            .iter()
            .any(|value| value.to_lowercase().contains(&search))
        {
            return false;
        }
    }

    true
}

fn has_log_filters(query: &WafLogQuery) -> bool {
    [
        query.search.as_deref(),
        query.host.as_deref(),
        query.client_ip.as_deref(),
        query.rule_id.as_deref(),
        query.route_type.as_deref(),
        query.mode.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(|value| !value.trim().is_empty())
}

fn is_initialization_rule(rule: &Value) -> bool {
    rule_basename(rule.get("file")).eq_ignore_ascii_case(INITIALIZATION_RULE_FILENAME)
}

fn rule_basename(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_str)
        .unwrap_or("")
        .replace('\\', "/")
        .rsplit('/')
        .next()
        .unwrap_or("")
        .to_string()
}

fn is_blocking_action(value: Option<&Value>) -> bool {
    matches!(
        value
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_lowercase()
            .as_str(),
        "block" | "deny"
    )
}

fn normalize_date(value: Option<&str>) -> Result<String, &'static str> {
    let raw = value.unwrap_or("").trim();
    if raw.is_empty() {
        return Ok(today());
    }
    if is_date(raw) {
        Ok(raw.to_string())
    } else {
        Err("invalid date, expected YYYY-MM-DD")
    }
}

fn normalize_limit(value: Option<&str>) -> i64 {
    value
        .and_then(|value| parse_i64_prefix(value.trim_start()))
        .filter(|value| *value > 0)
        .unwrap_or(50)
        .min(200)
}

fn normalize_cursor(value: Option<&str>) -> i64 {
    value
        .and_then(|value| parse_i64_prefix(value.trim_start()))
        .filter(|value| *value >= 0)
        .unwrap_or(0)
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

fn is_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
}

fn today() -> String {
    time_utils::local_date_from_ms(time_utils::now_ms())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sanitizes_initialization_rules_like_node() {
        let event = sanitize_event(json!({
            "trace_id": "t1",
            "action": "log",
            "rules": [
                { "id": 901, "file": "/x/REQUEST-901-INITIALIZATION.conf" },
                { "id": 1001, "file": "/x/rule.conf" }
            ],
            "rule_ids": [901, 1001],
            "interruption": { "rule_id": 901 }
        }))
        .unwrap();

        assert_eq!(event["rules"].as_array().unwrap().len(), 1);
        assert_eq!(event["rule_ids"], json!([1001]));
        assert!(event.get("interruption").is_none());
    }

    #[test]
    fn drops_events_without_rule_or_blocking_signal() {
        assert!(
            sanitize_event(json!({
                "trace_id": "t1",
                "action": "log",
                "rules": []
            }))
            .is_none()
        );
        assert!(
            sanitize_event(json!({
                "trace_id": "t1",
                "action": "block"
            }))
            .is_some()
        );
    }

    #[test]
    fn filters_waf_events_by_query() {
        let event = json!({
            "trace_id": "abc",
            "host": "example.com",
            "client_ip": "1.1.1.1",
            "route_type": "host",
            "mode": "blocking",
            "rule_ids": [1001],
            "path": "/login"
        });
        assert!(event_matches(
            &event,
            &WafLogQuery {
                date: None,
                trace_id: None,
                search: Some("login".to_string()),
                host: Some("EXAMPLE.com".to_string()),
                client_ip: Some("1.1.1.1".to_string()),
                rule_id: Some("1001".to_string()),
                route_type: Some("host".to_string()),
                mode: Some("blocking".to_string()),
                cursor: None,
                limit: None,
            }
        ));
    }

    #[test]
    fn waf_query_number_parsers_match_node_parse_int_edges() {
        assert_eq!(normalize_limit(Some("10x")), 10);
        assert_eq!(normalize_limit(Some("  +3.9")), 3);
        assert_eq!(normalize_limit(Some("-1")), 50);
        assert_eq!(normalize_limit(Some("300")), 200);
        assert_eq!(normalize_limit(Some("0x10")), 50);

        assert_eq!(normalize_cursor(Some("12x")), 12);
        assert_eq!(normalize_cursor(Some("  +3.9")), 3);
        assert_eq!(normalize_cursor(Some("-1")), 0);
        assert_eq!(normalize_cursor(Some("0x10")), 0);
    }

    #[test]
    fn waf_event_filters_match_node_unicode_and_rule_id_prefixes() {
        let event = json!({
            "trace_id": "abc",
            "host": "Ä.example",
            "client_ip": "1.1.1.1",
            "route_type": "host",
            "mode": "blocking",
            "rule_ids": [1001],
            "path": "/Älice"
        });

        assert!(event_matches(
            &event,
            &WafLogQuery {
                date: None,
                trace_id: None,
                search: Some("älice".to_string()),
                host: Some("ä.example".to_string()),
                client_ip: None,
                rule_id: Some("1001x".to_string()),
                route_type: None,
                mode: None,
                cursor: None,
                limit: None,
            }
        ));

        assert!(!event_matches(
            &event,
            &WafLogQuery {
                date: None,
                trace_id: None,
                search: None,
                host: None,
                client_ip: None,
                rule_id: Some("nope".to_string()),
                route_type: None,
                mode: None,
                cursor: None,
                limit: None,
            }
        ));
    }

    #[test]
    fn normalizes_waf_rule_filenames_like_node() {
        assert_eq!(
            safe_rule_filename("../custom rule.conf").unwrap(),
            "custom-rule.conf"
        );
        assert!(safe_rule_filename("../secret.txt").is_err());
        assert!(safe_rule_filename("..").is_err());
    }

    #[test]
    fn localizes_waf_route_and_service_errors() {
        let translator = Translator::new("zh-CN");

        assert_eq!(
            waf_text(&translator, "detailsLoadFailed"),
            "读取 WAF 详情失败"
        );
        assert_eq!(
            localize_waf_error(&translator, "WAF manifest is empty"),
            "系统规则清单为空"
        );
        assert_eq!(
            localize_waf_error(&translator, "Duplicate WAF bundle file: REQUEST.conf"),
            "系统规则包内存在重复文件: REQUEST.conf"
        );
        assert_eq!(
            localize_waf_error(&translator, "WAF rule file is too large: custom.conf"),
            "custom.conf 超过 1MB"
        );
        assert_eq!(
            localize_waf_error(&translator, "Invalid WAF rule source"),
            "规则来源不正确"
        );
        assert_eq!(
            localize_waf_error(&translator, "invalid date, expected YYYY-MM-DD"),
            "日期格式不正确，应为 YYYY-MM-DD"
        );
    }

    #[test]
    fn blocks_filesystem_directives_in_uploaded_rules() {
        assert!(contains_blocked_directive("  Include /tmp/*.conf"));
        assert!(contains_blocked_directive("SecAuditLog /tmp/audit.log"));
        assert!(!contains_blocked_directive(
            "SecRule ARGS attack \"id:1001\""
        ));
    }

    #[test]
    fn defaults_high_noise_system_rules_to_disabled() {
        assert!(is_system_rule_enabled_by_default(
            "REQUEST-901-INITIALIZATION.conf"
        ));
        assert!(!is_system_rule_enabled_by_default(
            "REQUEST-942-APPLICATION-ATTACK-SQLI.conf"
        ));
        assert!(is_system_rule_enabled_by_default(
            "REQUEST-949-BLOCKING-EVALUATION.conf"
        ));
    }

    #[test]
    fn validates_waf_bundle_paths() {
        assert_eq!(
            safe_bundle_entry_path("REQUEST-920-PROTOCOL-ENFORCEMENT.conf").unwrap(),
            "REQUEST-920-PROTOCOL-ENFORCEMENT.conf"
        );
        assert!(safe_bundle_entry_path("../evil.conf").is_err());
        assert!(safe_bundle_entry_path("/absolute.conf").is_err());
        assert!(safe_bundle_entry_path("nested//rule.conf").is_err());
    }
}
