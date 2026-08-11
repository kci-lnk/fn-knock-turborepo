use super::*;

pub(super) async fn export_backup_archive(state: &AppState) -> anyhow::Result<BackupArchive> {
    let _archive_work_guard = state.maintenance.backup_archive_work_lock.lock().await;
    let payload = export_backup_payload(state).await?;
    let exported_at = payload
        .get("exported_at")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let filename = build_backup_filename(&exported_at);
    let payload_bytes = serde_json::to_vec_pretty(&payload)?;
    ensure_backup_export_size(payload_bytes.len())?;
    let buffer = create_password_protected_zip(
        KNOCK_BACKUP_JSON_FILENAME,
        &payload_bytes,
        KNOCK_BACKUP_PASSWORD,
        time_utils::parse_iso_ms(&exported_at).unwrap_or_else(time_utils::now_ms),
    )?;
    ensure_backup_export_size(buffer.len())?;
    Ok(BackupArchive {
        buffer,
        exported_at,
        filename,
    })
}

pub(super) fn ensure_backup_export_size(size: usize) -> anyhow::Result<()> {
    if size > MAX_BACKUP_ARCHIVE_SIZE {
        anyhow::bail!("Backup export is too large");
    }
    Ok(())
}

pub(super) async fn export_backup_payload(state: &AppState) -> anyhow::Result<Value> {
    // Anchor TTL aging before the snapshot starts so time spent reading and
    // packaging the archive can never extend a key's lifetime on restore.
    let exported_at = time_utils::now_iso();
    let mut entries = state
        .storage
        .store
        .export_backup_entries_by_prefix_limited(
            KNOCK_BACKUP_PREFIX,
            MAX_BACKUP_ARCHIVE_SIZE,
            should_export_backup_key,
        )
        .await?;
    for entry in &entries {
        if !is_supported_backup_type(entry.get("type").and_then(Value::as_str)) {
            let key = entry.get("key").and_then(Value::as_str).unwrap_or("");
            anyhow::bail!(
                "Unsupported Redis type for backup: {} ({})",
                entry
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown"),
                key
            );
        }
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
        "exported_at": exported_at,
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
    let (filename, file_path) = unique_backup_destination(&directory, &archive.filename).await;
    let temp_path = directory.join(format!(".manual-backup-{}.tmp", Uuid::new_v4()));
    let write_result = async {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        let mut file = options.open(&temp_path).await?;
        file.write_all(&archive.buffer).await?;
        file.sync_all().await?;
        drop(file);
        fs::rename(&temp_path, &file_path).await?;
        if let Err(error) = sync_backup_directory(&directory).await {
            let _ = fs::remove_file(&file_path).await;
            let _ = sync_backup_directory(&directory).await;
            return Err(error);
        }
        Ok::<(), io::Error>(())
    }
    .await;
    if let Err(error) = write_result {
        let _ = fs::remove_file(&temp_path).await;
        return Err(BackupImportError::internal(error.to_string()));
    }
    let metadata = fs::metadata(&file_path)
        .await
        .map_err(|error| BackupImportError::internal(error.to_string()))?;
    Ok(json!({
        "filename": filename,
        "relativePath": filename,
        "filePath": file_path.to_string_lossy(),
        "size": metadata.len(),
        "exportedAt": archive.exported_at,
    }))
}

pub(super) async fn unique_backup_destination(
    directory: &Path,
    requested_filename: &str,
) -> (String, PathBuf) {
    let requested_path = directory.join(requested_filename);
    if !fs::try_exists(&requested_path).await.unwrap_or(true) {
        return (requested_filename.to_string(), requested_path);
    }
    let path = Path::new(requested_filename);
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("fn-knock-backup");
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("knock");
    let filename = format!("{stem}-{}.{}", Uuid::new_v4(), extension);
    let destination = directory.join(&filename);
    (filename, destination)
}

pub(super) async fn sync_backup_directory(directory: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        fs::File::open(directory).await?.sync_all().await
    }
    #[cfg(not(unix))]
    {
        let _ = directory;
        Ok(())
    }
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

pub(super) fn should_snapshot_backup_import_key(key: &str) -> bool {
    should_export_backup_key(key)
        || matches!(
            key,
            AUTOMATIC_BACKUP_CONFIG_KEY | AUTOMATIC_BACKUP_RUNTIME_KEY
        )
}

pub(super) fn matches_excluded_backup_pattern(key: &str) -> bool {
    matches!(
        key,
        "fn_knock:acme:runtime-lock"
            | "fn_knock:config:host_mappings:generation"
            | "fn_knock:ddns:last_ip"
            | "fn_knock:ddns:last_check"
            | "fn_knock:ddns:logs"
            | "fn_knock:ddns:logs:seq"
    ) || key.ends_with(":lock")
        || key.ends_with(":lease")
        || key.ends_with(":runtime-lock")
        || is_ddns_v2_runtime_key(key)
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
