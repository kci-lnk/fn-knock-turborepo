use super::*;

pub(in crate::system::maintenance) async fn automatic_backup_files_payload(
    state: &AppState,
) -> anyhow::Result<Value> {
    let directory = ensure_automatic_backup_directory(state).await?;
    let mut entries = fs::read_dir(&directory).await?;
    let mut files = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        let file_type = entry.file_type().await?;
        if !file_type.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if !is_backup_archive_file(&name) {
            continue;
        }
        let metadata = entry.metadata().await?;
        files.push(json!({
            "name": name,
            "relativePath": entry.file_name().to_string_lossy(),
            "extension": KNOCK_BACKUP_EXTENSION,
            "size": metadata.len(),
            "modifiedAt": time_utils::system_time_iso(
                metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH)
            ),
        }));
    }
    files.sort_by(|left, right| {
        let left_time = left.get("modifiedAt").and_then(Value::as_str).unwrap_or("");
        let right_time = right
            .get("modifiedAt")
            .and_then(Value::as_str)
            .unwrap_or("");
        right_time.cmp(left_time).then_with(|| {
            right
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .cmp(left.get("name").and_then(Value::as_str).unwrap_or(""))
        })
    });
    Ok(json!({
        "directoryPath": directory.to_string_lossy(),
        "available": true,
        "files": files,
    }))
}

pub(in crate::system::maintenance) async fn import_backup_archive_from_automatic_directory(
    state: &AppState,
    relative_path: &str,
    translator: &Translator,
) -> Result<Value, BackupImportError> {
    let file_path = resolve_automatic_backup_archive_path(state, relative_path).await?;
    let buffer = read_backup_archive_file(&file_path).await?;
    import_backup_archive_buffer(state, buffer, translator).await
}

pub(in crate::system::maintenance) fn automatic_backup_directory(state: &AppState) -> PathBuf {
    AUTOMATIC_BACKUP_DIRECTORY
        .iter()
        .fold(state.settings.data_dir.clone(), |path, part| {
            path.join(part)
        })
}

pub(in crate::system::maintenance) async fn ensure_automatic_backup_directory(
    state: &AppState,
) -> io::Result<PathBuf> {
    let directory = automatic_backup_directory(state);
    fs::create_dir_all(&directory).await?;
    Ok(directory)
}

pub(in crate::system::maintenance) async fn resolve_automatic_backup_archive_path(
    state: &AppState,
    relative_path: &str,
) -> Result<PathBuf, BackupImportError> {
    let trimmed = relative_path.trim();
    if trimmed.is_empty()
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || !is_backup_archive_file(trimmed)
        || !matches!(
            Path::new(trimmed)
                .components()
                .collect::<Vec<_>>()
                .as_slice(),
            [Component::Normal(_)]
        )
    {
        return Err(BackupImportError::bad_request("Invalid backup path"));
    }
    let directory = ensure_automatic_backup_directory(state)
        .await
        .map_err(|error| BackupImportError::internal(error.to_string()))?;
    Ok(directory.join(trimmed))
}

pub(in crate::system::maintenance) async fn write_automatic_backup_archive(
    state: &AppState,
) -> anyhow::Result<Value> {
    let directory = ensure_automatic_backup_directory(state).await?;
    cleanup_automatic_backup_temp_files(&directory).await?;
    let archive = export_backup_archive(state).await?;
    let (filename, final_path) = unique_backup_destination(&directory, &archive.filename).await;
    let temp_path = directory.join(format!(
        "{AUTOMATIC_BACKUP_TEMP_PREFIX}{}.tmp",
        Uuid::new_v4()
    ));
    let write_result = async {
        let mut file = fs::File::create(&temp_path).await?;
        for chunk in archive.buffer.chunks() {
            file.write_all(chunk).await?;
        }
        file.sync_all().await?;
        drop(file);
        fs::rename(&temp_path, &final_path).await?;
        if let Err(error) = sync_backup_directory(&directory).await {
            let _ = fs::remove_file(&final_path).await;
            let _ = sync_backup_directory(&directory).await;
            return Err(error.into());
        }
        let metadata = fs::metadata(&final_path).await?;
        Ok::<Value, anyhow::Error>(json!({
            "filename": filename,
            "relativePath": filename,
            "filePath": final_path.to_string_lossy(),
            "size": metadata.len(),
            "exportedAt": archive.exported_at,
        }))
    }
    .await;
    if write_result.is_err() {
        let _ = fs::remove_file(temp_path).await;
    }
    write_result
}

pub(in crate::system::maintenance) async fn cleanup_automatic_backup_temp_files(
    directory: &Path,
) -> io::Result<()> {
    let mut entries = fs::read_dir(directory).await?;
    while let Some(entry) = entries.next_entry().await? {
        let file_type = entry.file_type().await?;
        let name = entry.file_name().to_string_lossy().to_string();
        if file_type.is_file()
            && name.starts_with(AUTOMATIC_BACKUP_TEMP_PREFIX)
            && name.ends_with(".tmp")
        {
            let _ = fs::remove_file(entry.path()).await;
        }
    }
    Ok(())
}

pub(in crate::system::maintenance) async fn prune_automatic_backup_directory(
    state: &AppState,
    retention_days: i64,
) -> io::Result<()> {
    let directory = ensure_automatic_backup_directory(state).await?;
    let pinned = backup_email::pinned_files(state)
        .await
        .map_err(io::Error::other)?;
    let cutoff = SystemTime::now()
        .checked_sub(Duration::from_secs(retention_days as u64 * 24 * 3600))
        .unwrap_or(SystemTime::UNIX_EPOCH);
    let mut entries = fs::read_dir(&directory).await?;
    while let Some(entry) = entries.next_entry().await? {
        let file_type = entry.file_type().await?;
        if !file_type.is_file() || !is_backup_archive_file(&entry.file_name().to_string_lossy()) {
            continue;
        }
        let metadata = entry.metadata().await?;
        if !pinned.contains(&entry.file_name().to_string_lossy().to_string())
            && metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH) < cutoff
            && let Err(error) = fs::remove_file(entry.path()).await
        {
            tracing::warn!(%error, path = %entry.path().display(), "failed to prune automatic backup");
        }
    }
    Ok(())
}
