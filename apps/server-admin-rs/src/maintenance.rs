use std::{
    io::{self, Write},
    path::{Component, Path, PathBuf},
    process::Stdio,
    sync::atomic::{AtomicBool, Ordering},
    time::SystemTime,
};

use axum::{
    Router,
    body::Body,
    extract::{Json, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use flate2::{Compression, write::DeflateEncoder};
use serde_json::{Value, json};
use tokio::{fs, process::Command};
use uuid::Uuid;

use crate::{
    app_version::{APP_BACKUP_IMPORT_MIN_VERSION, APP_BACKUP_SCHEMA_VERSION, APP_LOCAL_VERSION},
    i18n::Translator,
    redis_store::node_locale_compare_ordering,
    response, runtime_config, ssl,
    state::AppState,
    system_monitor, time_utils,
};

const KNOCK_BACKUP_PREFIX: &str = "fn_knock:";
const KNOCK_BACKUP_EXTENSION: &str = ".knock";
const KNOCK_BACKUP_JSON_FILENAME: &str = "fn-knock-backup.json";
const KNOCK_BACKUP_PASSWORD: &str = "890eced0-4561-4044-8d6b-def83b5c6016";
const OPENWRT_APK_COMMAND: &str = "apk";
const OPENWRT_OPKG_COMMAND: &str = "opkg";
const DEBIAN_APT_GET_PATH: &str = "/usr/bin/apt-get";
const BACKUP_DIRECTORY_NAME: &str = "backup";
const MAX_BACKUP_DIRECTORY_SCAN_DEPTH: usize = 5;
const MAX_BACKUP_DIRECTORY_FILES: usize = 500;
const MAX_BACKUP_ARCHIVE_SIZE: usize = 128 * 1024 * 1024;
const SCAN_COUNT: usize = 200;
const MAINTENANCE_BACKUP_ERROR_MARKER: &str = "__maintenance_backup_error";

static ARCHIVE_COMMANDS_READY: AtomicBool = AtomicBool::new(false);

const BACKUP_EXCLUDED_KEY_PREFIXES: &[&str] = &[
    "fn_knock:acme:job:",
    "fn_knock:acme:logs:",
    "fn_knock:auth_log_data:",
    "fn_knock:auth_logs:",
    "fn_knock:auth_mobility:",
    "fn_knock:backoff:",
    "fn_knock:cidr:",
    "fn_knock:cloudflared:logs",
    "fn_knock:common_auth_locations:runtime",
    "fn_knock:config:backup:",
    "fn_knock:docker_admin:login_backoff:",
    "fn_knock:docker_admin:session:",
    "fn_knock:errors:",
    "fn_knock:events:",
    "fn_knock:fnos-share:session:",
    "fn_knock:fnos-share:validation:",
    "fn_knock:gateway:",
    "fn_knock:ip_location:",
    "fn_knock:lock:",
    "fn_knock:login_backoff:",
    "fn_knock:nonce:",
    "fn_knock:notifications:deliveries:",
    "fn_knock:notifications:runtime:",
    "fn_knock:notifications:triggers:",
    "fn_knock:oidc:invite:",
    "fn_knock:oidc:login_error:",
    "fn_knock:oidc:state:",
    "fn_knock:passkey:bind:",
    "fn_knock:passkey:challenge:",
    "fn_knock:recent_auth_ips:",
    "fn_knock:reverse-proxy:",
    "fn_knock:scanner:blacklist:",
    "fn_knock:scanner:suspicious:",
    "fn_knock:session:",
    "fn_knock:smart-connect:runtime",
    "fn_knock:ssh_security:",
    "fn_knock:terminal:",
    "fn_knock:traffic:",
    "fn_knock:tunnel:runtime",
    "fn_knock:ui:",
    "fn_knock:update:",
    "fn_knock:waf:log:",
    "fn_knock:waf:logs:",
    "fn_knock:waf:stats:",
    "fn_knock:welcome-guide:",
];

pub fn maintenance_routes() -> Router<AppState> {
    Router::new()
        .route("/api/admin/maintenance/backup/export", get(export_backup))
        .route(
            "/api/admin/maintenance/backup/files",
            get(list_backup_files),
        )
        .route(
            "/api/admin/maintenance/backup/export/fnos",
            post(export_backup_to_directory),
        )
        .route("/api/admin/maintenance/backup/import", post(import_backup))
        .route(
            "/api/admin/maintenance/backup/import/fnos",
            post(import_backup_from_directory),
        )
}

#[derive(serde::Deserialize)]
struct ImportBackupBody {
    filename: Option<String>,
    archive_base64: String,
}

#[derive(serde::Deserialize)]
struct ImportBackupFromDirectoryBody {
    path: String,
}

async fn export_backup(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match export_backup_archive(&state).await {
        Ok(archive) => binary_archive_response(archive, &translator),
        Err(error) => {
            tracing::warn!(%error, "failed to export backup archive");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                localize_backup_error_message(&translator, &error.to_string()),
            )
        }
    }
}

async fn list_backup_files(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match list_backup_directory_files().await {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to list backup directory files");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_backup_text(&translator, "readFnosDirectoryFailed"),
            )
        }
    }
}

async fn export_backup_to_directory(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match export_backup_archive_to_directory(&state).await {
        Ok(data) => Json(json!({
            "success": true,
            "data": data,
            "message": admin_backup_text(&translator, "exportFnosSuccess"),
        }))
        .into_response(),
        Err(error) => {
            tracing::warn!(error = %error.message, "failed to export backup archive to share directory");
            response::error(
                error.status,
                localize_backup_error_message(&translator, &error.message),
            )
        }
    }
}

async fn import_backup(
    State(state): State<AppState>,
    Json(body): Json<ImportBackupBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match import_backup_archive(&state, body, &translator).await {
        Ok(data) => import_success_response(data, false, &translator),
        Err(error) => response::error(
            error.status,
            localize_backup_error_message(&translator, &error.message),
        ),
    }
}

async fn import_backup_from_directory(
    State(state): State<AppState>,
    Json(body): Json<ImportBackupFromDirectoryBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match import_backup_archive_from_directory(&state, &body.path, &translator).await {
        Ok(data) => import_success_response(data, true, &translator),
        Err(error) => response::error(
            error.status,
            localize_backup_error_message(&translator, &error.message),
        ),
    }
}

struct BackupArchive {
    buffer: Vec<u8>,
    exported_at: String,
    filename: String,
}

async fn export_backup_archive(state: &AppState) -> anyhow::Result<BackupArchive> {
    let payload = export_backup_payload(state).await?;
    let exported_at = payload
        .get("exported_at")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let filename = build_backup_filename(&exported_at);
    let payload_bytes = serde_json::to_vec_pretty(&payload)?;
    let buffer = create_password_protected_zip(
        KNOCK_BACKUP_JSON_FILENAME,
        &payload_bytes,
        KNOCK_BACKUP_PASSWORD,
        time_utils::parse_iso_ms(&exported_at).unwrap_or_else(time_utils::now_ms),
    )?;
    Ok(BackupArchive {
        buffer,
        exported_at,
        filename,
    })
}

async fn export_backup_payload(state: &AppState) -> anyhow::Result<Value> {
    let keys = state
        .redis
        .scan_keys(KNOCK_BACKUP_PREFIX, SCAN_COUNT)
        .await?;
    let mut entries = Vec::new();
    for key in keys.into_iter().filter(|key| should_export_backup_key(key)) {
        let Some(entry) = state.redis.export_redis_backup_entry(&key).await? else {
            continue;
        };
        if !is_supported_backup_type(entry.get("type").and_then(Value::as_str)) {
            anyhow::bail!(
                "Unsupported Redis type for backup: {} ({})",
                entry
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown"),
                key
            );
        }
        entries.push(entry);
    }
    entries.sort_by(|left, right| {
        node_locale_compare_ordering(
            left.get("key").and_then(Value::as_str).unwrap_or(""),
            right.get("key").and_then(Value::as_str).unwrap_or(""),
        )
    });
    Ok(json!({
        "version": APP_BACKUP_SCHEMA_VERSION,
        "app_version": APP_LOCAL_VERSION,
        "prefix": KNOCK_BACKUP_PREFIX,
        "exported_at": time_utils::now_iso(),
        "entry_count": entries.len(),
        "entries": entries,
    }))
}

async fn import_backup_archive(
    state: &AppState,
    body: ImportBackupBody,
    translator: &Translator,
) -> Result<Value, BackupImportError> {
    let archive_base64 = body.archive_base64.trim();
    if archive_base64.is_empty() {
        return Err(BackupImportError::bad_request(
            "Backup archive content is required",
        ));
    }
    if let Some(filename) = body.filename.as_deref()
        && !filename.trim().is_empty()
        && !is_backup_archive_file(filename)
    {
        return Err(BackupImportError::bad_request(format!(
            "Backup archive filename must end with {KNOCK_BACKUP_EXTENSION}"
        )));
    }
    if !is_node_base64(archive_base64) {
        return Err(BackupImportError::bad_request(
            "Backup archive base64 is invalid",
        ));
    }
    let buffer = STANDARD
        .decode(archive_base64.as_bytes())
        .map_err(|_| BackupImportError::bad_request("Backup archive base64 is invalid"))?;
    import_backup_archive_buffer(state, buffer, translator).await
}

async fn import_backup_archive_from_directory(
    state: &AppState,
    relative_path: &str,
    translator: &Translator,
) -> Result<Value, BackupImportError> {
    let file_path = resolve_backup_archive_path(relative_path).await?;
    let metadata = fs::metadata(&file_path)
        .await
        .map_err(|error| match error.kind() {
            io::ErrorKind::NotFound => {
                BackupImportError::new(StatusCode::NOT_FOUND, "Backup file not found")
            }
            io::ErrorKind::PermissionDenied => {
                BackupImportError::new(StatusCode::FORBIDDEN, "Backup file cannot be read")
            }
            _ => BackupImportError::internal(error.to_string()),
        })?;
    if !metadata.is_file() {
        return Err(BackupImportError::bad_request("Backup path must be a file"));
    }
    if !is_backup_archive_file(&file_path.to_string_lossy()) {
        return Err(BackupImportError::bad_request(format!(
            "Backup archive file must end with {KNOCK_BACKUP_EXTENSION}"
        )));
    }
    if metadata.len() as usize > MAX_BACKUP_ARCHIVE_SIZE {
        return Err(BackupImportError::bad_request(
            "Backup directory import archive is too large",
        ));
    }
    let buffer = fs::read(&file_path)
        .await
        .map_err(|error| BackupImportError::internal(error.to_string()))?;
    import_backup_archive_buffer(state, buffer, translator).await
}

async fn import_backup_archive_buffer(
    state: &AppState,
    buffer: Vec<u8>,
    translator: &Translator,
) -> Result<Value, BackupImportError> {
    if buffer.is_empty() {
        return Err(BackupImportError::bad_request("Backup archive is empty"));
    }
    if buffer.len() > MAX_BACKUP_ARCHIVE_SIZE {
        return Err(BackupImportError::bad_request(
            "Backup archive is too large",
        ));
    }

    ensure_archive_commands_ready().await?;
    let payload = extract_backup_payload_from_archive(&buffer).await?;
    let entries = payload
        .get("entries")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let importable_entries = entries
        .into_iter()
        .filter(|entry| {
            entry
                .get("key")
                .and_then(Value::as_str)
                .is_some_and(should_export_backup_key)
        })
        .collect::<Vec<_>>();

    let keys = state
        .redis
        .scan_keys(KNOCK_BACKUP_PREFIX, SCAN_COUNT)
        .await
        .map_err(|error| BackupImportError::internal(error.to_string()))?;
    let cleared_keys = keys.len();
    state
        .redis
        .delete_keys(&keys)
        .await
        .map_err(|error| BackupImportError::internal(error.to_string()))?;
    state
        .redis
        .restore_redis_backup_entries(&importable_entries)
        .await
        .map_err(|error| BackupImportError::internal(error.to_string()))?;

    let (warnings, synced_steps) = sync_runtime_after_import(state, translator).await;
    Ok(json!({
        "cleared_keys": cleared_keys,
        "imported_keys": importable_entries.len(),
        "warnings": warnings,
        "synced_steps": synced_steps
    }))
}

async fn export_backup_archive_to_directory(state: &AppState) -> Result<Value, BackupImportError> {
    let archive = export_backup_archive(state)
        .await
        .map_err(|error| BackupImportError::internal(error.to_string()))?;
    let directory = ensure_backup_directory().await?;
    let file_path = directory.join(&archive.filename);
    fs::write(&file_path, &archive.buffer)
        .await
        .map_err(|error| BackupImportError::internal(error.to_string()))?;
    let metadata = fs::metadata(&file_path)
        .await
        .map_err(|error| BackupImportError::internal(error.to_string()))?;
    Ok(json!({
        "filename": archive.filename,
        "relativePath": archive.filename,
        "filePath": file_path.to_string_lossy(),
        "size": metadata.len(),
        "exportedAt": archive.exported_at,
    }))
}

async fn ensure_archive_commands_ready() -> Result<(), BackupImportError> {
    if ARCHIVE_COMMANDS_READY.load(Ordering::SeqCst) {
        return Ok(());
    }

    install_archive_commands_if_needed().await?;
    ARCHIVE_COMMANDS_READY.store(true, Ordering::SeqCst);
    Ok(())
}

async fn install_archive_commands_if_needed() -> Result<(), BackupImportError> {
    let missing = missing_archive_commands().await?;
    if missing.is_empty() {
        return Ok(());
    }

    let missing_names = missing.join(", ");
    let packages = missing.clone();
    let package_names = packages.join(", ");

    if command_available(OPENWRT_APK_COMMAND, &["--version"]).await? {
        let output = run_command(OPENWRT_APK_COMMAND, &["--update-cache", "add", "unzip"]).await?;
        if !output.status.success() {
            return Err(command_result_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                backup_error_key_message(
                    "packageInstallFailed",
                    &[("packages", package_names.clone())],
                ),
                &output,
            ));
        }
        ensure_no_archive_commands_missing_after_install().await?;
        return Ok(());
    }

    if command_available(OPENWRT_OPKG_COMMAND, &["--version"]).await? {
        let update_output = run_command(OPENWRT_OPKG_COMMAND, &["update"]).await?;
        if !update_output.status.success() {
            return Err(command_result_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                backup_error_key_message("opkgUpdateFailed", &[]),
                &update_output,
            ));
        }

        let install_output = run_command(OPENWRT_OPKG_COMMAND, &["install", "unzip"]).await?;
        if !install_output.status.success() {
            return Err(command_result_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                backup_error_key_message(
                    "packageInstallFailed",
                    &[("packages", package_names.clone())],
                ),
                &install_output,
            ));
        }
        ensure_no_archive_commands_missing_after_install().await?;
        return Ok(());
    }

    if command_available(DEBIAN_APT_GET_PATH, &["--version"]).await? {
        let update_output = run_command(DEBIAN_APT_GET_PATH, &["update"]).await?;
        if !update_output.status.success() {
            return Err(command_result_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                backup_error_key_message("aptUpdateFailed", &[]),
                &update_output,
            ));
        }

        let install_output = run_command(DEBIAN_APT_GET_PATH, &["install", "-y", "unzip"]).await?;
        if !install_output.status.success() {
            return Err(command_result_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                backup_error_key_message(
                    "packageInstallFailed",
                    &[("packages", package_names.clone())],
                ),
                &install_output,
            ));
        }
        ensure_no_archive_commands_missing_after_install().await?;
        return Ok(());
    }

    Err(BackupImportError::internal(backup_error_key_message(
        "commandsMissingNoPackageManager",
        &[("commands", missing_names)],
    )))
}

async fn ensure_no_archive_commands_missing_after_install() -> Result<(), BackupImportError> {
    let remaining = missing_archive_commands().await?;
    if remaining.is_empty() {
        return Ok(());
    }
    Err(BackupImportError::internal(backup_error_key_message(
        "commandsStillMissingAfterInstall",
        &[("commands", remaining.join(", "))],
    )))
}

async fn missing_archive_commands() -> Result<Vec<String>, BackupImportError> {
    let mut missing = Vec::new();
    if !command_available("unzip", &["-v"]).await? {
        missing.push("unzip".to_string());
    }
    Ok(missing)
}

async fn command_available(command: &str, args: &[&str]) -> Result<bool, BackupImportError> {
    match Command::new(command)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
    {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(BackupImportError::internal(backup_error_key_message(
            "commandCheckFailed",
            &[("command", command.to_string())],
        ))),
    }
}

async fn run_command(
    command: &str,
    args: &[&str],
) -> Result<std::process::Output, BackupImportError> {
    Command::new(command)
        .args(args)
        .output()
        .await
        .map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                BackupImportError::internal(backup_error_key_message(
                    "commandMissing",
                    &[("command", command.to_string())],
                ))
            } else {
                BackupImportError::internal(backup_error_key_message(
                    "commandFailed",
                    &[("command", command.to_string())],
                ))
            }
        })
}

async fn extract_backup_payload_from_archive(buffer: &[u8]) -> Result<Value, BackupImportError> {
    let temp_dir = std::env::temp_dir().join(format!("fn-knock-backup-{}", Uuid::new_v4()));
    fs::create_dir_all(&temp_dir)
        .await
        .map_err(|error| BackupImportError::internal(error.to_string()))?;
    let archive_path = temp_dir.join(format!("import{KNOCK_BACKUP_EXTENSION}"));
    let result = async {
        fs::write(&archive_path, buffer)
            .await
            .map_err(|error| BackupImportError::internal(error.to_string()))?;
        let output = Command::new("unzip")
            .arg("-qq")
            .arg("-P")
            .arg(KNOCK_BACKUP_PASSWORD)
            .arg("-p")
            .arg(&archive_path)
            .arg(KNOCK_BACKUP_JSON_FILENAME)
            .output()
            .await
            .map_err(|error| {
                if error.kind() == io::ErrorKind::NotFound {
                    BackupImportError::internal(backup_error_key_message(
                        "commandMissing",
                        &[("command", "unzip".to_string())],
                    ))
                } else {
                    BackupImportError::internal(backup_error_key_message(
                        "commandFailed",
                        &[("command", "unzip".to_string())],
                    ))
                }
            })?;
        if !output.status.success() {
            let detail = format!(
                "{}\n{}",
                String::from_utf8_lossy(&output.stderr),
                String::from_utf8_lossy(&output.stdout)
            )
            .to_ascii_lowercase();
            if detail.contains("filename not matched") {
                return Err(BackupImportError::bad_request(format!(
                    "Backup archive is missing {KNOCK_BACKUP_JSON_FILENAME}"
                )));
            }
            if detail.contains("incorrect password") || detail.contains("wrong password") {
                return Err(BackupImportError::bad_request(
                    "Backup archive password is invalid",
                ));
            }
            return Err(command_result_error(
                StatusCode::BAD_REQUEST,
                backup_error_key_message("readArchiveFailed", &[]),
                &output,
            ));
        }
        let raw = String::from_utf8(output.stdout)
            .map_err(|_| BackupImportError::bad_request("Backup payload is not valid UTF-8"))?;
        parse_backup_payload(&raw)
    }
    .await;
    let _ = fs::remove_dir_all(temp_dir).await;
    result
}

fn parse_backup_payload(raw: &str) -> Result<Value, BackupImportError> {
    let payload: Value = serde_json::from_str(raw)
        .map_err(|_| BackupImportError::bad_request("Backup JSON payload is invalid"))?;
    let Some(object) = payload.as_object() else {
        return Err(BackupImportError::bad_request(
            "Backup payload must be an object",
        ));
    };
    if object.get("version").and_then(Value::as_i64) != Some(APP_BACKUP_SCHEMA_VERSION) {
        return Err(BackupImportError::bad_request(format!(
            "Unsupported backup schema version. Expected {APP_BACKUP_SCHEMA_VERSION}"
        )));
    }
    if object.get("prefix").and_then(Value::as_str) != Some(KNOCK_BACKUP_PREFIX) {
        return Err(BackupImportError::bad_request(format!(
            "Unsupported backup prefix. Expected {KNOCK_BACKUP_PREFIX}"
        )));
    }
    let app_version = object
        .get("app_version")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| BackupImportError::bad_request("Backup app version is missing"))?;
    if !backup_app_version_supported(app_version) {
        return Err(BackupImportError::bad_request(format!(
            "Backup app version {app_version} is unsupported. Supported range is {APP_BACKUP_IMPORT_MIN_VERSION} ~ {APP_LOCAL_VERSION}"
        )));
    }
    let exported_at = object
        .get("exported_at")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| BackupImportError::bad_request("Backup exported_at is missing"))?;
    let raw_entries = object
        .get("entries")
        .and_then(Value::as_array)
        .ok_or_else(|| BackupImportError::bad_request("Backup entries are missing"))?;
    let mut entries = Vec::with_capacity(raw_entries.len());
    let mut keys = std::collections::BTreeSet::new();
    for (index, entry) in raw_entries.iter().enumerate() {
        let normalized = parse_backup_entry(entry, index)?;
        let key = normalized
            .get("key")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if !keys.insert(key) {
            return Err(BackupImportError::bad_request(
                "Backup contains duplicated Redis keys",
            ));
        }
        entries.push(normalized);
    }
    Ok(json!({
        "version": APP_BACKUP_SCHEMA_VERSION,
        "app_version": app_version,
        "prefix": KNOCK_BACKUP_PREFIX,
        "exported_at": exported_at,
        "entry_count": entries.len(),
        "entries": entries
    }))
}

fn parse_backup_entry(entry: &Value, index: usize) -> Result<Value, BackupImportError> {
    let Some(object) = entry.as_object() else {
        return Err(BackupImportError::bad_request(format!(
            "entries[{index}] must be an object"
        )));
    };
    let key = object
        .get("key")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if !key.starts_with(KNOCK_BACKUP_PREFIX) {
        return Err(BackupImportError::bad_request(format!(
            "entries[{index}].key must start with {KNOCK_BACKUP_PREFIX}"
        )));
    }
    let value_type = object
        .get("type")
        .and_then(Value::as_str)
        .filter(|value| is_supported_backup_type(Some(value)))
        .ok_or_else(|| {
            BackupImportError::bad_request(format!("entries[{index}].type is unsupported"))
        })?;
    let ttl_ms = match object.get("ttl_ms") {
        None | Some(Value::Null) => Value::Null,
        Some(value) => {
            let ttl = js_number_from_json(value)
                .filter(|value| *value > 0.0)
                .ok_or_else(|| {
                    BackupImportError::bad_request(format!("entries[{index}].ttl_ms is invalid"))
                })?;
            let ttl = ttl.floor();
            if ttl > i64::MAX as f64 {
                return Err(BackupImportError::bad_request(format!(
                    "entries[{index}].ttl_ms is invalid"
                )));
            }
            json!(ttl as i64)
        }
    };
    let value = match value_type {
        "string" => json!(object.get("value").and_then(Value::as_str).ok_or_else(|| {
            BackupImportError::bad_request(format!("entries[{index}].value must be a string"))
        })?),
        "hash" => json!(parse_backup_hash_value(object.get("value"), index)?),
        "list" | "set" => json!(parse_backup_string_array(
            object.get("value"),
            index,
            "value"
        )?),
        "zset" => json!(parse_backup_zset_value(object.get("value"), index)?),
        "stream" => json!(parse_backup_stream_value(object.get("value"), index)?),
        _ => unreachable!(),
    };
    Ok(json!({
        "key": key,
        "type": value_type,
        "ttl_ms": ttl_ms,
        "value": value
    }))
}

fn parse_backup_hash_value(
    value: Option<&Value>,
    index: usize,
) -> Result<serde_json::Map<String, Value>, BackupImportError> {
    let Some(object) = value.and_then(Value::as_object) else {
        return Err(BackupImportError::bad_request(format!(
            "entries[{index}].value must be an object"
        )));
    };
    let mut output = serde_json::Map::new();
    for (field, field_value) in object {
        let Some(text) = field_value.as_str() else {
            return Err(BackupImportError::bad_request(format!(
                "entries[{index}].value.{field} must be a string"
            )));
        };
        output.insert(field.clone(), json!(text));
    }
    Ok(output)
}

fn parse_backup_string_array(
    value: Option<&Value>,
    index: usize,
    label: &str,
) -> Result<Vec<String>, BackupImportError> {
    let Some(items) = value.and_then(Value::as_array) else {
        return Err(BackupImportError::bad_request(format!(
            "entries[{index}].{label} must be an array"
        )));
    };
    let mut output = Vec::with_capacity(items.len());
    for item in items {
        let Some(text) = item.as_str() else {
            return Err(BackupImportError::bad_request(format!(
                "entries[{index}].{label} must contain only strings"
            )));
        };
        output.push(text.to_string());
    }
    Ok(output)
}

fn parse_backup_zset_value(
    value: Option<&Value>,
    index: usize,
) -> Result<Vec<Value>, BackupImportError> {
    let Some(items) = value.and_then(Value::as_array) else {
        return Err(BackupImportError::bad_request(format!(
            "entries[{index}].value must be an array"
        )));
    };
    let mut output = Vec::with_capacity(items.len());
    for (item_index, item) in items.iter().enumerate() {
        let Some(member) = item.get("member").and_then(Value::as_str) else {
            return Err(BackupImportError::bad_request(format!(
                "entries[{index}].value[{item_index}].member must be a string"
            )));
        };
        let score = item
            .get("score")
            .and_then(js_number_from_json)
            .filter(|value| value.is_finite())
            .ok_or_else(|| {
                BackupImportError::bad_request(format!(
                    "entries[{index}].value[{item_index}].score is invalid"
                ))
            })?;
        output.push(json!({ "member": member, "score": score }));
    }
    Ok(output)
}

fn parse_backup_stream_value(
    value: Option<&Value>,
    index: usize,
) -> Result<Vec<Value>, BackupImportError> {
    let Some(items) = value.and_then(Value::as_array) else {
        return Err(BackupImportError::bad_request(format!(
            "entries[{index}].value must be an array"
        )));
    };
    let mut output = Vec::with_capacity(items.len());
    for (item_index, item) in items.iter().enumerate() {
        let Some(id) = item.get("id").and_then(Value::as_str) else {
            return Err(BackupImportError::bad_request(format!(
                "entries[{index}].value[{item_index}].id must be a string"
            )));
        };
        let fields = parse_backup_string_array(
            item.get("fields"),
            index,
            &format!("value[{item_index}].fields"),
        )?;
        if fields.is_empty() || fields.len() % 2 != 0 {
            return Err(BackupImportError::bad_request(format!(
                "entries[{index}].value[{item_index}].fields is invalid"
            )));
        }
        output.push(json!({ "id": id, "fields": fields }));
    }
    Ok(output)
}

async fn sync_runtime_after_import(
    state: &AppState,
    translator: &Translator,
) -> (Vec<String>, Vec<String>) {
    let mut warnings = Vec::new();
    let mut synced_steps = Vec::new();
    let config = match state.redis.get_config().await {
        Ok(config) => config,
        Err(error) => {
            warnings.push(format!(
                "{}: {error}",
                maintenance_backup_text(translator, "syncSteps.runModeGatewayRoutes")
            ));
            return (warnings, synced_steps);
        }
    };
    let run_type = config.get("run_type").and_then(Value::as_i64).unwrap_or(3);

    let run_mode_label = maintenance_backup_text(translator, "syncSteps.runModeGatewayRoutes");
    match runtime_config::apply_run_type_config(state, &config, run_type).await {
        Ok(()) => synced_steps.push(run_mode_label),
        Err(error) => warnings.push(format!(
            "{}: {}",
            run_mode_label,
            localize_backup_error_message(translator, &error)
        )),
    }

    if run_type == 0 {
        let whitelist_label = maintenance_backup_text(translator, "syncSteps.directModeWhitelist");
        match sync_direct_mode_whitelist_after_import(state).await {
            Ok(()) => synced_steps.push(whitelist_label),
            Err(error) => warnings.push(format!("{whitelist_label}: {error}")),
        }
    }

    let gateway_logging_label = maintenance_backup_text(translator, "syncSteps.gatewayLogging");
    let gateway_logging = config.get("gateway_logging").cloned().unwrap_or_else(|| {
        json!({
            "enabled": true,
            "max_days": 7
        })
    });
    match state
        .go_backend
        .request_json_with_status(
            axum::http::Method::POST,
            "/api/logging",
            Some(&gateway_logging),
        )
        .await
    {
        Ok((status, value))
            if status.is_success()
                && value.get("success").and_then(Value::as_bool) != Some(false) =>
        {
            synced_steps.push(gateway_logging_label);
        }
        Ok((status, value)) => warnings.push(format!(
            "{}: {}",
            gateway_logging_label,
            value
                .get("message")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| status.to_string())
        )),
        Err(error) => warnings.push(format!("{gateway_logging_label}: {error}")),
    }

    let ssl_label = maintenance_backup_text(translator, "syncSteps.sslDeployment");
    match ssl::sync_ssl_deployment_to_gateway(state, Some(&config)).await {
        Ok(()) => synced_steps.push(ssl_label),
        Err(error) => warnings.push(format!("{ssl_label}: {error}")),
    }

    let cleanup_label = maintenance_backup_text(translator, "syncSteps.legacyAuthLogCleanup");
    match crate::cleanup_legacy_auth_log_storage(state).await {
        Ok(()) => synced_steps.push(cleanup_label),
        Err(error) => warnings.push(format!("{cleanup_label}: {error}")),
    }

    let monitor_label = maintenance_backup_text(translator, "syncSteps.systemResourceMonitorReset");
    system_monitor::reset_states(state).await;
    synced_steps.push(monitor_label);

    (warnings, synced_steps)
}

async fn sync_direct_mode_whitelist_after_import(state: &AppState) -> anyhow::Result<()> {
    let records = state.redis.list_whitelist_active_concrete_targets().await?;
    for record in records {
        let value = state.go_backend.allow_ip(&record.target).await?;
        if value.get("success").and_then(Value::as_bool) == Some(false) {
            anyhow::bail!(
                "{}",
                value
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("allow ip failed")
            );
        }
    }
    Ok(())
}

fn import_success_response(data: Value, from_directory: bool, translator: &Translator) -> Response {
    let has_warnings = data
        .get("warnings")
        .and_then(Value::as_array)
        .is_some_and(|warnings| !warnings.is_empty());
    let key = match (from_directory, has_warnings) {
        (true, true) => "importFnosSuccessWithWarnings",
        (true, false) => "importFnosSuccess",
        (false, true) => "importSuccessWithWarnings",
        (false, false) => "importSuccess",
    };
    axum::Json(json!({
        "success": true,
        "message": admin_backup_text(translator, key),
        "data": data
    }))
    .into_response()
}

fn admin_backup_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.admin.backup.{key}"))
}

fn maintenance_backup_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.maintenanceBackup.{key}"))
}

fn maintenance_backup_text_params(
    translator: &Translator,
    key: &str,
    params: &[(&str, String)],
) -> String {
    translator.t_params(&format!("server.maintenanceBackup.{key}"), params)
}

fn backup_import_version_range() -> String {
    format!("{APP_BACKUP_IMPORT_MIN_VERSION} ~ {APP_LOCAL_VERSION}")
}

fn backup_error_key_message(key: &str, params: &[(&str, String)]) -> String {
    let mut params_object = serde_json::Map::new();
    for (key, value) in params {
        params_object.insert((*key).to_string(), Value::String(value.clone()));
    }
    json!({
        MAINTENANCE_BACKUP_ERROR_MARKER: true,
        "kind": "key",
        "key": key,
        "params": params_object,
    })
    .to_string()
}

fn backup_command_error_message(message: String, code: i32, detail: Option<String>) -> String {
    json!({
        MAINTENANCE_BACKUP_ERROR_MARKER: true,
        "kind": "command_error",
        "message": message,
        "code": code,
        "detail": detail.unwrap_or_default(),
    })
    .to_string()
}

fn command_result_error(
    status: StatusCode,
    message: String,
    output: &std::process::Output,
) -> BackupImportError {
    BackupImportError::new(
        status,
        backup_command_error_message(
            message,
            output.status.code().unwrap_or(-1),
            summarize_command_failure(&output.stdout, &output.stderr),
        ),
    )
}

fn localize_backup_error_message(translator: &Translator, message: &str) -> String {
    let normalized = message.trim();
    if let Some(localized) = localize_structured_backup_error(translator, normalized) {
        return localized;
    }
    if let Some(localized) = localize_backup_entry_error(translator, normalized) {
        return localized;
    }
    match normalized {
        "Backup share directory is not configured" => {
            maintenance_backup_text(translator, "shareDirectoryMissing")
        }
        "Invalid backup path" => maintenance_backup_text(translator, "invalidBackupPath"),
        "Backup file not found" => {
            maintenance_backup_text(translator, "directoryImportFileNotFound")
        }
        "Backup file cannot be read" => {
            maintenance_backup_text(translator, "directoryImportFileUnreadable")
        }
        "Backup path must be a file" => {
            maintenance_backup_text(translator, "directoryImportFileOnly")
        }
        "Backup archive content is required" => {
            maintenance_backup_text(translator, "archiveContentMissing")
        }
        "Backup archive base64 is invalid" => {
            maintenance_backup_text(translator, "archiveBase64Invalid")
        }
        "Backup archive is empty" => maintenance_backup_text(translator, "archiveEmpty"),
        "Backup archive is too large" => maintenance_backup_text(translator, "archiveTooLarge"),
        "Backup directory import archive is too large" => {
            maintenance_backup_text(translator, "directoryImportTooLarge")
        }
        "Backup archive password is invalid" => {
            maintenance_backup_text(translator, "archivePasswordInvalid")
        }
        "Backup payload is not valid UTF-8" => {
            maintenance_backup_text(translator, "payloadUtf8Invalid")
        }
        "Backup JSON payload is invalid" => maintenance_backup_text(translator, "jsonParseFailed"),
        "Backup payload must be an object" => {
            maintenance_backup_text(translator, "payloadObjectInvalid")
        }
        "Backup app version is missing" => maintenance_backup_text(translator, "missingAppVersion"),
        "Backup exported_at is missing" => maintenance_backup_text(translator, "missingExportedAt"),
        "Backup entries are missing" => maintenance_backup_text(translator, "missingEntries"),
        "Backup contains duplicated Redis keys" => {
            maintenance_backup_text(translator, "duplicateRedisKey")
        }
        _ if normalized.starts_with("Backup archive filename must end with") => {
            maintenance_backup_text_params(
                translator,
                "invalidBackupExtension",
                &[("extension", KNOCK_BACKUP_EXTENSION.to_string())],
            )
        }
        _ if normalized.starts_with("Backup archive file must end with") => {
            maintenance_backup_text_params(
                translator,
                "directoryImportExtensionOnly",
                &[("extension", KNOCK_BACKUP_EXTENSION.to_string())],
            )
        }
        _ if normalized.starts_with("Backup archive is missing ") => {
            maintenance_backup_text_params(
                translator,
                "archiveMissingPayload",
                &[("filename", KNOCK_BACKUP_JSON_FILENAME.to_string())],
            )
        }
        _ if normalized.starts_with("Unsupported Redis type for backup: ") => {
            let detail = normalized
                .strip_prefix("Unsupported Redis type for backup: ")
                .unwrap_or_default();
            let (data_type, key) = parse_type_and_key_detail(detail);
            maintenance_backup_text_params(
                translator,
                "unsupportedRedisExportType",
                &[("type", data_type), ("key", key)],
            )
        }
        _ if normalized.starts_with("Unsupported backup schema version.") => {
            maintenance_backup_text_params(
                translator,
                "unsupportedSchemaVersion",
                &[("version", APP_BACKUP_SCHEMA_VERSION.to_string())],
            )
        }
        _ if normalized.starts_with("Unsupported backup prefix.") => {
            maintenance_backup_text_params(
                translator,
                "unsupportedPrefix",
                &[("prefix", KNOCK_BACKUP_PREFIX.to_string())],
            )
        }
        _ if normalized.starts_with("Backup app version ")
            && normalized.contains(" is unsupported.") =>
        {
            let app_version = normalized
                .strip_prefix("Backup app version ")
                .and_then(|rest| rest.split_once(" is unsupported."))
                .map(|(version, _)| version.trim().to_string())
                .filter(|version| !version.is_empty())
                .unwrap_or_else(|| "unknown".to_string());
            maintenance_backup_text_params(
                translator,
                "appVersionUnsupported",
                &[
                    ("currentVersion", APP_LOCAL_VERSION.to_string()),
                    ("range", backup_import_version_range()),
                    ("appVersion", app_version),
                ],
            )
        }
        _ if normalized.starts_with("Failed to read backup archive") => {
            maintenance_backup_text(translator, "readArchiveFailed")
        }
        _ => normalized.to_string(),
    }
}

fn localize_structured_backup_error(translator: &Translator, message: &str) -> Option<String> {
    let parsed: Value = serde_json::from_str(message).ok()?;
    let object = parsed.as_object()?;
    if object
        .get(MAINTENANCE_BACKUP_ERROR_MARKER)
        .and_then(Value::as_bool)
        != Some(true)
    {
        return None;
    }

    match object.get("kind").and_then(Value::as_str) {
        Some("key") => {
            let key = object.get("key").and_then(Value::as_str)?;
            let params_object = object
                .get("params")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            let params = params_object
                .iter()
                .map(|(key, value)| (key.as_str(), backup_param_value_to_string(value)))
                .collect::<Vec<_>>();
            Some(maintenance_backup_text_params(translator, key, &params))
        }
        Some("command_error") => {
            let raw_message = object.get("message").and_then(Value::as_str).unwrap_or("");
            let message = if raw_message.trim().is_empty() {
                maintenance_backup_text(translator, "unknownError")
            } else {
                localize_backup_error_message(translator, raw_message)
            };
            let code = object
                .get("code")
                .and_then(Value::as_i64)
                .unwrap_or(-1)
                .to_string();
            let detail = object
                .get("detail")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_string();
            if detail.is_empty() {
                Some(maintenance_backup_text_params(
                    translator,
                    "commandError",
                    &[("message", message), ("code", code)],
                ))
            } else {
                Some(maintenance_backup_text_params(
                    translator,
                    "commandErrorWithDetail",
                    &[("message", message), ("code", code), ("detail", detail)],
                ))
            }
        }
        _ => None,
    }
}

fn backup_param_value_to_string(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Null => String::new(),
        value => value.to_string(),
    }
}

fn localize_backup_entry_error(translator: &Translator, message: &str) -> Option<String> {
    let rest = message.strip_prefix("entries[")?;
    let (index, suffix) = rest.split_once(']')?;
    if index.is_empty() {
        return None;
    }
    let value_label = format!("entries[{index}].value");
    match suffix {
        " must be an object" => {
            return Some(maintenance_backup_text_params(
                translator,
                "entryObjectRequired",
                &[("index", index.to_string())],
            ));
        }
        ".type is unsupported" => {
            return Some(maintenance_backup_text_params(
                translator,
                "entryTypeUnsupported",
                &[("index", index.to_string())],
            ));
        }
        ".ttl_ms is invalid" => {
            return Some(maintenance_backup_text_params(
                translator,
                "entryTtlInvalid",
                &[("index", index.to_string())],
            ));
        }
        ".value must be a string" => {
            return Some(maintenance_backup_text_params(
                translator,
                "entryValueStringRequired",
                &[("index", index.to_string())],
            ));
        }
        ".value must be an object" => {
            return Some(maintenance_backup_text_params(
                translator,
                "objectRequired",
                &[("label", value_label)],
            ));
        }
        ".value must be an array" => {
            return Some(maintenance_backup_text_params(
                translator,
                "arrayRequired",
                &[("label", value_label)],
            ));
        }
        ".value must contain only strings" => {
            return Some(maintenance_backup_text_params(
                translator,
                "stringArrayOnlyStrings",
                &[("label", value_label)],
            ));
        }
        _ => {}
    }

    if suffix.starts_with(".key must start with ") {
        return Some(maintenance_backup_text_params(
            translator,
            "entryKeyPrefixRequired",
            &[
                ("index", index.to_string()),
                ("prefix", KNOCK_BACKUP_PREFIX.to_string()),
            ],
        ));
    }

    if let Some(field) = suffix
        .strip_prefix(".value.")
        .and_then(|rest| rest.strip_suffix(" must be a string"))
    {
        return Some(maintenance_backup_text_params(
            translator,
            "fieldStringRequired",
            &[("label", value_label), ("field", field.to_string())],
        ));
    }

    let rest = suffix.strip_prefix(".value[")?;
    let (item_index, item_suffix) = rest.split_once(']')?;
    if item_index.is_empty() {
        return None;
    }
    match item_suffix {
        ".member must be a string" => Some(maintenance_backup_text_params(
            translator,
            "zsetMemberRequired",
            &[("label", value_label), ("index", item_index.to_string())],
        )),
        ".score is invalid" => Some(maintenance_backup_text_params(
            translator,
            "zsetScoreRequired",
            &[("label", value_label), ("index", item_index.to_string())],
        )),
        ".id must be a string" => Some(maintenance_backup_text_params(
            translator,
            "streamIdRequired",
            &[("label", value_label), ("index", item_index.to_string())],
        )),
        ".fields must be an array" => Some(maintenance_backup_text_params(
            translator,
            "stringArrayRequired",
            &[(
                "label",
                format!("entries[{index}].value[{item_index}].fields"),
            )],
        )),
        ".fields must contain only strings" => Some(maintenance_backup_text_params(
            translator,
            "stringArrayOnlyStrings",
            &[(
                "label",
                format!("entries[{index}].value[{item_index}].fields"),
            )],
        )),
        ".fields is invalid" => Some(maintenance_backup_text_params(
            translator,
            "streamFieldsInvalid",
            &[("label", value_label), ("index", item_index.to_string())],
        )),
        _ => None,
    }
}

fn parse_type_and_key_detail(detail: &str) -> (String, String) {
    let Some((data_type, rest)) = detail.trim().split_once(" (") else {
        return (detail.trim().to_string(), "unknown".to_string());
    };
    (
        data_type.trim().to_string(),
        rest.trim_end_matches(')').trim().to_string(),
    )
}

async fn resolve_backup_archive_path(relative_path: &str) -> Result<PathBuf, BackupImportError> {
    let directory = ensure_backup_directory().await?;
    resolve_backup_archive_path_like_node(&directory, relative_path)
}

fn resolve_backup_archive_path_like_node(
    directory: &Path,
    relative_path: &str,
) -> Result<PathBuf, BackupImportError> {
    let sanitized = relative_path.replace('\\', "/").trim().to_string();
    if sanitized.is_empty() || sanitized.starts_with('/') {
        return Err(BackupImportError::bad_request("Invalid backup path"));
    }
    let normalized_root = normalize_path_like_node(directory);
    let resolved = normalize_path_like_node(&normalized_root.join(&sanitized));
    if !resolved.starts_with(&normalized_root) || resolved == normalized_root {
        return Err(BackupImportError::bad_request("Invalid backup path"));
    }
    Ok(resolved)
}

fn normalize_path_like_node(path: &Path) -> PathBuf {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    };
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    normalized
}

fn is_backup_archive_file(value: &str) -> bool {
    Path::new(value)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            format!(".{}", extension.to_ascii_lowercase()) == KNOCK_BACKUP_EXTENSION
        })
}

fn is_node_base64(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.is_empty() || bytes.len() % 4 != 0 {
        return false;
    }

    let padding = bytes.iter().rev().take_while(|byte| **byte == b'=').count();
    if padding > 2 {
        return false;
    }

    let content_len = bytes.len() - padding;
    let expected_remainder = match padding {
        0 => 0,
        1 => 3,
        2 => 2,
        _ => unreachable!(),
    };
    if content_len % 4 != expected_remainder {
        return false;
    }

    bytes[..content_len]
        .iter()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(*byte, b'+' | b'/'))
        && bytes[content_len..].iter().all(|byte| *byte == b'=')
}

fn js_number_from_json(value: &Value) -> Option<f64> {
    let number = match value {
        Value::Number(number) => number.as_f64()?,
        Value::String(value) => js_number_from_string(value)?,
        Value::Bool(value) => {
            if *value {
                1.0
            } else {
                0.0
            }
        }
        Value::Null => 0.0,
        Value::Array(values) => js_number_from_string(&js_array_to_string(values))?,
        Value::Object(_) => return None,
    };
    number.is_finite().then_some(number)
}

fn js_array_to_string(values: &[Value]) -> String {
    values
        .iter()
        .map(js_value_to_array_string)
        .collect::<Vec<_>>()
        .join(",")
}

fn js_value_to_array_string(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Array(values) => js_array_to_string(values),
        Value::Object(_) => "[object Object]".to_string(),
    }
}

fn js_number_from_string(value: &str) -> Option<f64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Some(0.0);
    }

    let radix_value = if let Some(rest) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        Some(u128::from_str_radix(rest, 16).ok()? as f64)
    } else if let Some(rest) = trimmed
        .strip_prefix("0b")
        .or_else(|| trimmed.strip_prefix("0B"))
    {
        Some(u128::from_str_radix(rest, 2).ok()? as f64)
    } else if let Some(rest) = trimmed
        .strip_prefix("0o")
        .or_else(|| trimmed.strip_prefix("0O"))
    {
        Some(u128::from_str_radix(rest, 8).ok()? as f64)
    } else {
        None
    };

    match radix_value {
        Some(value) => Some(value),
        None => trimmed.parse::<f64>().ok(),
    }
}

fn backup_app_version_supported(version: &str) -> bool {
    compare_version(version, APP_BACKUP_IMPORT_MIN_VERSION) >= 0
        && compare_version(version, APP_LOCAL_VERSION) <= 0
}

fn compare_version(left: &str, right: &str) -> i8 {
    let left_parts = version_parts(left);
    let right_parts = version_parts(right);
    let max_len = left_parts.len().max(right_parts.len()).max(3);
    for index in 0..max_len {
        let left = *left_parts.get(index).unwrap_or(&0);
        let right = *right_parts.get(index).unwrap_or(&0);
        if left > right {
            return 1;
        }
        if left < right {
            return -1;
        }
    }
    0
}

fn version_parts(value: &str) -> Vec<i64> {
    value
        .trim()
        .split('.')
        .map(|part| {
            let digits = part
                .chars()
                .skip_while(|ch| !ch.is_ascii_digit())
                .take_while(|ch| ch.is_ascii_digit())
                .collect::<String>();
            digits.parse::<i64>().unwrap_or(0)
        })
        .collect()
}

fn summarize_command_failure(stdout: &[u8], stderr: &[u8]) -> Option<String> {
    let stderr = String::from_utf8_lossy(stderr);
    let stdout = String::from_utf8_lossy(stdout);
    let detail = if stderr.is_empty() {
        stdout.as_ref()
    } else {
        stderr.as_ref()
    };
    let summary = detail
        .trim()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .rev()
        .take(3)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join(" | ");
    (!summary.is_empty()).then_some(summary)
}

#[derive(Debug)]
struct BackupImportError {
    status: StatusCode,
    message: String,
}

impl BackupImportError {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, message)
    }

    fn internal(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, message)
    }
}

async fn list_backup_directory_files() -> anyhow::Result<Value> {
    let Some(directory) = configured_share_directory().map(|path| path.join(BACKUP_DIRECTORY_NAME))
    else {
        return Ok(json!({
            "shareName": "fn-knock / backup",
            "available": false,
            "files": [],
        }));
    };
    fs::create_dir_all(&directory).await?;
    let mut files = Vec::new();
    collect_backup_directory_files(&directory, &directory, &mut files, 0).await?;
    files.sort_by(|left, right| {
        let left_time = left.get("modifiedAt").and_then(Value::as_str).unwrap_or("");
        let right_time = right
            .get("modifiedAt")
            .and_then(Value::as_str)
            .unwrap_or("");
        right_time.cmp(left_time).then_with(|| {
            left.get("relativePath")
                .and_then(Value::as_str)
                .unwrap_or("")
                .cmp(
                    right
                        .get("relativePath")
                        .and_then(Value::as_str)
                        .unwrap_or(""),
                )
        })
    });
    Ok(json!({
        "shareName": "fn-knock / backup",
        "available": true,
        "files": files,
    }))
}

async fn collect_backup_directory_files(
    current: &Path,
    root: &Path,
    bucket: &mut Vec<Value>,
    depth: usize,
) -> io::Result<()> {
    if bucket.len() >= MAX_BACKUP_DIRECTORY_FILES {
        return Ok(());
    }
    let mut entries = fs::read_dir(current).await?;
    while let Some(entry) = entries.next_entry().await? {
        if bucket.len() >= MAX_BACKUP_DIRECTORY_FILES {
            return Ok(());
        }
        let file_type = entry.file_type().await?;
        let path = entry.path();
        if file_type.is_dir() {
            if depth < MAX_BACKUP_DIRECTORY_SCAN_DEPTH {
                Box::pin(collect_backup_directory_files(
                    &path,
                    root,
                    bucket,
                    depth + 1,
                ))
                .await?;
            }
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if !file_type.is_file() || !name.to_ascii_lowercase().ends_with(KNOCK_BACKUP_EXTENSION) {
            continue;
        }
        let metadata = entry.metadata().await?;
        bucket.push(json!({
            "name": name,
            "relativePath": path.strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/"),
            "extension": KNOCK_BACKUP_EXTENSION,
            "size": metadata.len(),
            "modifiedAt": system_time_iso(metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH)),
        }));
    }
    Ok(())
}

async fn ensure_backup_directory() -> Result<PathBuf, BackupImportError> {
    let Some(directory) = configured_share_directory().map(|path| path.join(BACKUP_DIRECTORY_NAME))
    else {
        return Err(BackupImportError::new(
            StatusCode::NOT_FOUND,
            "Backup share directory is not configured",
        ));
    };
    fs::create_dir_all(&directory)
        .await
        .map_err(|error| BackupImportError::internal(error.to_string()))?;
    Ok(directory)
}

fn binary_archive_response(archive: BackupArchive, translator: &Translator) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", archive.filename),
        )
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from(archive.buffer))
        .unwrap_or_else(|_| {
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                maintenance_backup_text(translator, "buildResponseFailed"),
            )
        })
}

fn configured_share_directory() -> Option<PathBuf> {
    std::env::var("FN_KNOCK_ROOT_SHARE_DIR")
        .or_else(|_| std::env::var("FN_KNOCK_CERT_SHARE_DIR"))
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            let raw = std::env::var("TRIM_DATA_SHARE_PATHS").ok()?;
            raw.split(':')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .min_by_key(|value| value.len())
                .map(PathBuf::from)
        })
}

fn build_backup_filename(exported_at: &str) -> String {
    let normalized = if exported_at.trim().is_empty() {
        time_utils::now_iso()
    } else {
        exported_at.to_string()
    };
    format!(
        "fn-knock-backup-{}{}",
        normalized.replace([':', '.'], "-"),
        KNOCK_BACKUP_EXTENSION
    )
}

fn should_export_backup_key(key: &str) -> bool {
    !BACKUP_EXCLUDED_KEY_PREFIXES
        .iter()
        .any(|prefix| key.starts_with(prefix))
        && !matches_excluded_backup_pattern(key)
}

fn matches_excluded_backup_pattern(key: &str) -> bool {
    matches!(
        key,
        "fn_knock:acme:runtime-lock"
            | "fn_knock:ddns:last_ip"
            | "fn_knock:ddns:last_check"
            | "fn_knock:ddns:logs"
            | "fn_knock:ddns:logs:seq"
    ) || is_ddns_v2_runtime_key(key)
        || is_frpc_v2_runtime_key(key)
}

fn is_ddns_v2_runtime_key(key: &str) -> bool {
    let parts = key.split(':').collect::<Vec<_>>();
    parts.len() == 6
        && parts[0] == "fn_knock"
        && parts[1] == "ddns"
        && parts[2] == "v2"
        && parts[3] == "target"
        && matches!(parts[5], "last_ip" | "last_check")
}

fn is_frpc_v2_runtime_key(key: &str) -> bool {
    let parts = key.split(':').collect::<Vec<_>>();
    parts.len() >= 6
        && parts[0] == "fn_knock"
        && parts[1] == "frpc"
        && parts[2] == "v2"
        && parts[3] == "instance"
        && matches!(&parts[5..], ["runtime"] | ["logs"] | ["logs", "seq"])
}

fn is_supported_backup_type(value: Option<&str>) -> bool {
    matches!(
        value,
        Some("string" | "hash" | "list" | "set" | "zset" | "stream")
    )
}

fn create_password_protected_zip(
    file_name: &str,
    content: &[u8],
    password: &str,
    modified_at_ms: i64,
) -> anyhow::Result<Vec<u8>> {
    let file_name_bytes = file_name.as_bytes();
    let crc = crc32_buffer(content);
    let compressed = deflate_raw(content)?;
    let mut encryptor = ZipCryptoEncryptor::new(password);
    let mut encryption_header = rand::random::<[u8; 12]>();
    encryption_header[11] = (crc >> 24) as u8;

    let mut encrypted_data = Vec::with_capacity(12 + compressed.len());
    encrypted_data.extend(encryptor.encrypt(&encryption_header));
    encrypted_data.extend(encryptor.encrypt(&compressed));

    let compressed_size = encrypted_data.len() as u32;
    let uncompressed_size = content.len() as u32;
    let (dos_time, dos_date) = dos_datetime(modified_at_ms);
    let flags = 0x0001_u16;
    let compression_method = 8_u16;

    let mut local_header = Vec::new();
    write_u32(&mut local_header, 0x04034b50);
    write_u16(&mut local_header, 20);
    write_u16(&mut local_header, flags);
    write_u16(&mut local_header, compression_method);
    write_u16(&mut local_header, dos_time);
    write_u16(&mut local_header, dos_date);
    write_u32(&mut local_header, crc);
    write_u32(&mut local_header, compressed_size);
    write_u32(&mut local_header, uncompressed_size);
    write_u16(&mut local_header, file_name_bytes.len() as u16);
    write_u16(&mut local_header, 0);
    local_header.extend_from_slice(file_name_bytes);

    let central_directory_offset = (local_header.len() + encrypted_data.len()) as u32;
    let mut central_directory = Vec::new();
    write_u32(&mut central_directory, 0x02014b50);
    write_u16(&mut central_directory, 20);
    write_u16(&mut central_directory, 20);
    write_u16(&mut central_directory, flags);
    write_u16(&mut central_directory, compression_method);
    write_u16(&mut central_directory, dos_time);
    write_u16(&mut central_directory, dos_date);
    write_u32(&mut central_directory, crc);
    write_u32(&mut central_directory, compressed_size);
    write_u32(&mut central_directory, uncompressed_size);
    write_u16(&mut central_directory, file_name_bytes.len() as u16);
    write_u16(&mut central_directory, 0);
    write_u16(&mut central_directory, 0);
    write_u16(&mut central_directory, 0);
    write_u16(&mut central_directory, 0);
    write_u32(&mut central_directory, 0);
    write_u32(&mut central_directory, 0);
    central_directory.extend_from_slice(file_name_bytes);

    let mut end = Vec::new();
    write_u32(&mut end, 0x06054b50);
    write_u16(&mut end, 0);
    write_u16(&mut end, 0);
    write_u16(&mut end, 1);
    write_u16(&mut end, 1);
    write_u32(&mut end, central_directory.len() as u32);
    write_u32(&mut end, central_directory_offset);
    write_u16(&mut end, 0);

    let mut output = Vec::with_capacity(
        local_header.len() + encrypted_data.len() + central_directory.len() + end.len(),
    );
    output.extend(local_header);
    output.extend(encrypted_data);
    output.extend(central_directory);
    output.extend(end);
    Ok(output)
}

struct ZipCryptoEncryptor {
    key0: u32,
    key1: u32,
    key2: u32,
}

impl ZipCryptoEncryptor {
    fn new(password: &str) -> Self {
        let mut this = Self {
            key0: 0x12345678,
            key1: 0x23456789,
            key2: 0x34567890,
        };
        for byte in password.as_bytes() {
            this.update_keys(*byte);
        }
        this
    }

    fn encrypt(&mut self, plain: &[u8]) -> Vec<u8> {
        plain
            .iter()
            .map(|byte| {
                let encrypted = *byte ^ self.decrypt_byte();
                self.update_keys(*byte);
                encrypted
            })
            .collect()
    }

    fn update_keys(&mut self, byte: u8) {
        self.key0 = crc32_update(self.key0, byte);
        self.key1 = self
            .key1
            .wrapping_add(self.key0 & 0xff)
            .wrapping_mul(134775813)
            .wrapping_add(1);
        self.key2 = crc32_update(self.key2, (self.key1 >> 24) as u8);
    }

    fn decrypt_byte(&self) -> u8 {
        let temp = (self.key2 | 2) & 0xffff;
        (((temp.wrapping_mul(temp ^ 1)) >> 8) & 0xff) as u8
    }
}

fn deflate_raw(content: &[u8]) -> io::Result<Vec<u8>> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(content)?;
    encoder.finish()
}

fn crc32_buffer(buffer: &[u8]) -> u32 {
    let mut crc = 0xffffffff_u32;
    for byte in buffer {
        crc = crc32_update(crc, *byte);
    }
    crc ^ 0xffffffff
}

fn crc32_update(crc: u32, byte: u8) -> u32 {
    let mut value = (crc ^ u32::from(byte)) & 0xff;
    for _ in 0..8 {
        value = if value & 1 != 0 {
            0xedb88320 ^ (value >> 1)
        } else {
            value >> 1
        };
    }
    value ^ (crc >> 8)
}

fn dos_datetime(ms: i64) -> (u16, u16) {
    let timestamp = ms.div_euclid(1000);
    let utc = time::OffsetDateTime::from_unix_timestamp(timestamp)
        .unwrap_or_else(|_| time::OffsetDateTime::UNIX_EPOCH);
    let local = time::UtcOffset::current_local_offset()
        .map(|offset| utc.to_offset(offset))
        .unwrap_or(utc);
    let year = local.year().clamp(1980, 2107);
    let month = u8::from(local.month()) as u16;
    let day = local.day() as u16;
    let hours = local.hour() as u16;
    let minutes = local.minute() as u16;
    let seconds = (local.second() / 2) as u16;
    let time = ((hours & 0x1f) << 11) | ((minutes & 0x3f) << 5) | (seconds & 0x1f);
    let date = (((year - 1980) as u16) << 9) | ((month & 0xf) << 5) | (day & 0x1f);
    (time, date)
}

fn write_u16(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn write_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_backup_keys_like_node() {
        assert!(should_export_backup_key("fn_knock:config"));
        for prefix in BACKUP_EXCLUDED_KEY_PREFIXES {
            assert!(
                !should_export_backup_key(&format!("{prefix}sample")),
                "expected prefix {prefix} to be excluded"
            );
        }
        for key in [
            "fn_knock:acme:runtime-lock",
            "fn_knock:ddns:last_ip",
            "fn_knock:ddns:last_check",
            "fn_knock:ddns:logs",
            "fn_knock:ddns:logs:seq",
            "fn_knock:ddns:v2:target:home:last_ip",
            "fn_knock:ddns:v2:target:home:last_check",
            "fn_knock:frpc:v2:instance:main:runtime",
            "fn_knock:frpc:v2:instance:main:logs",
            "fn_knock:frpc:v2:instance:main:logs:seq",
        ] {
            assert!(
                !should_export_backup_key(key),
                "expected {key} to be excluded"
            );
        }
        assert!(should_export_backup_key(
            "fn_knock:ddns:v2:target:home:config"
        ));
        assert!(should_export_backup_key(
            "fn_knock:frpc:v2:instance:main:config"
        ));
    }

    #[test]
    fn builds_node_compatible_backup_filename() {
        assert_eq!(
            build_backup_filename("2026-07-05T01:02:03.456Z"),
            "fn-knock-backup-2026-07-05T01-02-03-456Z.knock"
        );
    }

    #[test]
    fn writes_encrypted_zip_headers() {
        let zip = create_password_protected_zip(
            KNOCK_BACKUP_JSON_FILENAME,
            br#"{"ok":true}"#,
            KNOCK_BACKUP_PASSWORD,
            1_704_067_200_000,
        )
        .unwrap();
        assert_eq!(&zip[0..4], &[0x50, 0x4b, 0x03, 0x04]);
        assert!(
            zip.windows(KNOCK_BACKUP_JSON_FILENAME.len())
                .any(|window| window == KNOCK_BACKUP_JSON_FILENAME.as_bytes())
        );
        assert!(
            zip.windows(4)
                .any(|window| window == [0x50, 0x4b, 0x05, 0x06])
        );
    }

    #[test]
    fn parses_import_payload_with_supported_redis_types() {
        let payload = json!({
            "version": APP_BACKUP_SCHEMA_VERSION,
            "app_version": APP_LOCAL_VERSION,
            "prefix": KNOCK_BACKUP_PREFIX,
            "exported_at": "2026-07-05T00:00:00Z",
            "entries": [
                {"key":"fn_knock:string","type":"string","ttl_ms":1000,"value":"v"},
                {"key":"fn_knock:hash","type":"hash","ttl_ms":null,"value":{"a":"b"}},
                {"key":"fn_knock:list","type":"list","ttl_ms":null,"value":["a","b"]},
                {"key":"fn_knock:set","type":"set","ttl_ms":null,"value":["a"]},
                {"key":"fn_knock:zset","type":"zset","ttl_ms":null,"value":[{"member":"a","score":1.5}]},
                {"key":"fn_knock:stream","type":"stream","ttl_ms":null,"value":[{"id":"1-0","fields":["a","b"]}]}
            ]
        });
        let parsed = parse_backup_payload(&payload.to_string()).unwrap();
        assert_eq!(parsed["entry_count"], json!(6));
        assert_eq!(parsed["entries"][0]["ttl_ms"], json!(1000));
    }

    #[test]
    fn parses_import_payload_number_coercions_like_node() {
        let payload = json!({
            "version": APP_BACKUP_SCHEMA_VERSION,
            "app_version": APP_LOCAL_VERSION,
            "prefix": KNOCK_BACKUP_PREFIX,
            "exported_at": " ",
            "entries": [
                {"key":"fn_knock:string","type":"string","ttl_ms":"1000.9","value":"v"},
                {"key":"fn_knock:hash","type":"hash","ttl_ms":0.5,"value":{"a":"b"}},
                {"key":"fn_knock:list","type":"list","ttl_ms":true,"value":["a"]},
                {"key":"fn_knock:zset","type":"zset","ttl_ms":null,"value":[
                    {"member":"string-score","score":"1.5"},
                    {"member":"null-score","score":null},
                    {"member":"bool-score","score":true},
                    {"member":"array-score","score":["2.75"]}
                ]}
            ]
        });

        let parsed = parse_backup_payload(&payload.to_string()).unwrap();

        assert_eq!(parsed["entries"][0]["ttl_ms"], json!(1000));
        assert_eq!(parsed["entries"][1]["ttl_ms"], json!(0));
        assert_eq!(parsed["entries"][2]["ttl_ms"], json!(1));
        assert_eq!(parsed["entries"][3]["value"][0]["score"], json!(1.5));
        assert_eq!(parsed["entries"][3]["value"][1]["score"], json!(0.0));
        assert_eq!(parsed["entries"][3]["value"][2]["score"], json!(1.0));
        assert_eq!(parsed["entries"][3]["value"][3]["score"], json!(2.75));
        assert_eq!(parsed["exported_at"], json!(" "));
    }

    #[test]
    fn rejects_import_payload_invalid_number_coercions_like_node() {
        let invalid_ttl = json!({
            "version": APP_BACKUP_SCHEMA_VERSION,
            "app_version": APP_LOCAL_VERSION,
            "prefix": KNOCK_BACKUP_PREFIX,
            "exported_at": "2026-07-05T00:00:00Z",
            "entries": [
                {"key":"fn_knock:string","type":"string","ttl_ms":false,"value":"v"}
            ]
        });
        assert!(parse_backup_payload(&invalid_ttl.to_string()).is_err());

        let invalid_score = json!({
            "version": APP_BACKUP_SCHEMA_VERSION,
            "app_version": APP_LOCAL_VERSION,
            "prefix": KNOCK_BACKUP_PREFIX,
            "exported_at": "2026-07-05T00:00:00Z",
            "entries": [
                {"key":"fn_knock:zset","type":"zset","ttl_ms":null,"value":[{"member":"a","score":{}}]}
            ]
        });
        assert!(parse_backup_payload(&invalid_score.to_string()).is_err());
    }

    #[test]
    fn validates_base64_like_node_regex() {
        assert!(is_node_base64("Zm9v"));
        assert!(is_node_base64("Zm8="));
        assert!(is_node_base64("Zg=="));
        assert!(!is_node_base64(""));
        assert!(!is_node_base64("Zg"));
        assert!(!is_node_base64("Z==="));
        assert!(!is_node_base64("Zm9v\n"));
        assert!(!is_node_base64("Zm9v-"));
    }

    #[test]
    fn rejects_duplicate_import_keys() {
        let payload = json!({
            "version": APP_BACKUP_SCHEMA_VERSION,
            "app_version": APP_LOCAL_VERSION,
            "prefix": KNOCK_BACKUP_PREFIX,
            "exported_at": "2026-07-05T00:00:00Z",
            "entries": [
                {"key":"fn_knock:dup","type":"string","ttl_ms":null,"value":"a"},
                {"key":"fn_knock:dup","type":"string","ttl_ms":null,"value":"b"}
            ]
        });
        let error = parse_backup_payload(&payload.to_string()).unwrap_err();
        assert_eq!(error.status, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn validates_backup_version_range_like_node() {
        assert!(backup_app_version_supported(APP_BACKUP_IMPORT_MIN_VERSION));
        assert!(backup_app_version_supported(APP_LOCAL_VERSION));
        assert!(!backup_app_version_supported("1.3.9"));
        assert!(!backup_app_version_supported("99.0.0"));
    }

    #[test]
    fn detects_backup_archive_extension_case_insensitively() {
        assert!(is_backup_archive_file("backup.KNOCK"));
        assert!(!is_backup_archive_file("backup.zip"));
    }

    #[test]
    fn resolves_backup_archive_paths_like_node() {
        let root = Path::new("/share/backup");

        assert_eq!(
            resolve_backup_archive_path_like_node(root, "sub/../file.knock")
                .unwrap()
                .as_path(),
            Path::new("/share/backup/file.knock")
        );
        assert_eq!(
            resolve_backup_archive_path_like_node(root, "./nested/file.knock")
                .unwrap()
                .as_path(),
            Path::new("/share/backup/nested/file.knock")
        );

        for value in ["", "   ", "/", "..", "../file.knock", "a/../.."] {
            let error = resolve_backup_archive_path_like_node(root, value).unwrap_err();
            assert_eq!(error.status, StatusCode::BAD_REQUEST);
            assert_eq!(error.message, "Invalid backup path");
        }
    }

    #[test]
    fn summarizes_command_failures_like_node() {
        assert_eq!(
            summarize_command_failure(b"out1\nout2\nout3\nout4", b"err1\nerr2\nerr3\nerr4")
                .as_deref(),
            Some("err2 | err3 | err4")
        );
        assert_eq!(
            summarize_command_failure(b"out1\nout2\nout3\nout4", b"").as_deref(),
            Some("out2 | out3 | out4")
        );
        assert_eq!(summarize_command_failure(b"stdout", b"   "), None);
    }

    #[test]
    fn localizes_backup_error_messages() {
        let translator = Translator::new("zh-CN");

        assert_eq!(
            localize_backup_error_message(&translator, "Backup JSON payload is invalid"),
            "备份文件 JSON 无法解析"
        );
        assert_eq!(
            localize_backup_error_message(&translator, "Backup archive file must end with .knock"),
            "仅支持导入 .knock 备份文件"
        );
        assert_eq!(
            localize_backup_error_message(
                &translator,
                "Unsupported Redis type for backup: bitmap (fn_knock:sample)"
            ),
            "不支持导出的 Redis 数据类型: bitmap (fn_knock:sample)"
        );
        assert_eq!(
            localize_backup_error_message(&translator, "entries[2].value[3].fields is invalid"),
            "entries[2].value[3].fields 必须是偶数长度且非空的字符串数组"
        );
        assert_eq!(
            localize_backup_error_message(&translator, "Backup file not found"),
            "未找到要导入的备份文件"
        );
        assert_eq!(
            localize_backup_error_message(
                &translator,
                "Backup directory import archive is too large"
            ),
            "备份文件过大，无法从飞牛目录导入"
        );
        assert_eq!(
            localize_backup_error_message(
                &translator,
                &backup_error_key_message("commandMissing", &[("command", "unzip".to_string())])
            ),
            "系统环境缺少 unzip 命令"
        );
        let command_error = backup_command_error_message(
            backup_error_key_message("readArchiveFailed", &[]),
            9,
            Some("cannot find fn-knock-backup.json".to_string()),
        );
        assert_eq!(
            localize_backup_error_message(&translator, &command_error),
            "读取 .knock 备份归档失败（退出码: 9）: cannot find fn-knock-backup.json"
        );
    }

    #[test]
    fn localizes_runtime_sync_step_labels() {
        let zh = Translator::new("zh-CN");
        let en = Translator::new("en");

        assert_eq!(
            maintenance_backup_text(&zh, "syncSteps.runModeGatewayRoutes"),
            "运行模式与网关路由"
        );
        assert_eq!(
            maintenance_backup_text(&zh, "syncSteps.directModeWhitelist"),
            "直连模式白名单"
        );
        assert_eq!(
            maintenance_backup_text(&zh, "syncSteps.systemResourceMonitorReset"),
            "系统资源监控状态重置"
        );
        assert_eq!(
            maintenance_backup_text(&en, "syncSteps.runModeGatewayRoutes"),
            "Run mode and gateway routes"
        );
    }
}
