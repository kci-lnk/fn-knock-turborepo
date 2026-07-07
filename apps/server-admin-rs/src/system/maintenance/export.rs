use super::*;

pub(super) async fn export_backup_archive(state: &AppState) -> anyhow::Result<BackupArchive> {
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

pub(super) async fn export_backup_payload(state: &AppState) -> anyhow::Result<Value> {
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

pub(super) async fn export_backup_archive_to_directory(
    state: &AppState,
) -> Result<Value, BackupImportError> {
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

pub(super) fn binary_archive_response(archive: BackupArchive, translator: &Translator) -> Response {
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

pub(super) fn build_backup_filename(exported_at: &str) -> String {
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

pub(super) fn should_export_backup_key(key: &str) -> bool {
    !BACKUP_EXCLUDED_KEY_PREFIXES
        .iter()
        .any(|prefix| key.starts_with(prefix))
        && !matches_excluded_backup_pattern(key)
}

pub(super) fn matches_excluded_backup_pattern(key: &str) -> bool {
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

pub(super) fn is_ddns_v2_runtime_key(key: &str) -> bool {
    let parts = key.split(':').collect::<Vec<_>>();
    parts.len() == 6
        && parts[0] == "fn_knock"
        && parts[1] == "ddns"
        && parts[2] == "v2"
        && parts[3] == "target"
        && matches!(parts[5], "last_ip" | "last_check")
}

pub(super) fn is_frpc_v2_runtime_key(key: &str) -> bool {
    let parts = key.split(':').collect::<Vec<_>>();
    parts.len() >= 6
        && parts[0] == "fn_knock"
        && parts[1] == "frpc"
        && parts[2] == "v2"
        && parts[3] == "instance"
        && matches!(&parts[5..], ["runtime"] | ["logs"] | ["logs", "seq"])
}

pub(super) fn is_supported_backup_type(value: Option<&str>) -> bool {
    matches!(
        value,
        Some("string" | "hash" | "list" | "set" | "zset" | "stream")
    )
}
