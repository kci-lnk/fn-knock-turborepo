use super::*;
use std::time::{Duration, SystemTime};

pub(super) const AUTOMATIC_BACKUP_TEMP_PREFIX: &str = ".automatic-backup-";

pub(super) fn spawn_automatic_backup_task(state: AppState) {
    state.spawn_background(
        "automatic-backup",
        automatic_backup_scheduler(state.clone()),
    );
}

pub(super) async fn automatic_backup_details(state: &AppState) -> anyhow::Result<Value> {
    let config = load_automatic_backup_config(state).await?;
    let runtime = load_automatic_backup_runtime(state).await?;
    Ok(automatic_backup_details_value(state, config, runtime))
}

fn automatic_backup_details_value(state: &AppState, config: Value, runtime: Value) -> Value {
    json!({
        "config": config,
        "status": {
            "directory_path": automatic_backup_directory(state).to_string_lossy(),
            "last_attempt_at": runtime.get("last_attempt_at").cloned().unwrap_or(Value::Null),
            "last_success_at": runtime.get("last_success_at").cloned().unwrap_or(Value::Null),
            "last_error": runtime.get("last_error").cloned().unwrap_or(Value::Null),
            "last_filename": runtime.get("last_filename").cloned().unwrap_or(Value::Null),
            "next_backup_at": runtime.get("next_backup_at").cloned().unwrap_or(Value::Null),
        }
    })
}

pub(super) async fn save_automatic_backup_config(
    state: &AppState,
    body: UpdateAutomaticBackupBody,
) -> Result<Value, BackupImportError> {
    validate_automatic_backup_config(&body)?;
    let guard = state.maintenance.automatic_backup_lock.lock().await;
    let previous = load_automatic_backup_config(state)
        .await
        .map_err(|error| BackupImportError::internal(error.to_string()))?;
    let previous_enabled = previous
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let previous_interval = previous
        .get("interval_hours")
        .and_then(Value::as_i64)
        .unwrap_or(AUTOMATIC_BACKUP_DEFAULT_INTERVAL_HOURS);
    let now = time_utils::now_iso();
    let config = json!({
        "enabled": body.enabled,
        "interval_hours": body.interval_hours,
        "retention_days": body.retention_days,
        "updated_at": now,
    });
    let mut runtime = load_automatic_backup_runtime(state)
        .await
        .map_err(|error| BackupImportError::internal(error.to_string()))?;
    let next_backup_at = if !body.enabled {
        Value::Null
    } else if !previous_enabled {
        Value::String(now)
    } else if previous_interval != body.interval_hours {
        let now_ms = time_utils::now_ms();
        Value::String(
            if runtime.get("last_error").and_then(Value::as_str).is_some() {
                next_backup_after_failure(
                    runtime.get("next_backup_at").and_then(Value::as_str),
                    body.interval_hours,
                    now_ms,
                )
            } else {
                next_backup_after_last_success(
                    runtime.get("last_success_at").and_then(Value::as_str),
                    body.interval_hours,
                    now_ms,
                )
            },
        )
    } else {
        runtime
            .get("next_backup_at")
            .and_then(Value::as_str)
            .filter(|value| time_utils::parse_iso_ms(value).is_some())
            .map(|value| Value::String(value.to_string()))
            .unwrap_or_else(|| Value::String(time_utils::now_iso()))
    };
    runtime["next_backup_at"] = next_backup_at;
    state
        .storage
        .store
        .set_json_values_atomically(&[
            (AUTOMATIC_BACKUP_CONFIG_KEY, &config),
            (AUTOMATIC_BACKUP_RUNTIME_KEY, &runtime),
        ])
        .await
        .map_err(|error| BackupImportError::internal(error.to_string()))?;
    drop(guard);
    state.maintenance.automatic_backup_notify.notify_one();

    Ok(automatic_backup_details_value(state, config, runtime))
}

pub(super) async fn automatic_backup_files_payload(state: &AppState) -> anyhow::Result<Value> {
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

pub(super) async fn import_backup_archive_from_automatic_directory(
    state: &AppState,
    relative_path: &str,
    translator: &Translator,
) -> Result<Value, BackupImportError> {
    let file_path = resolve_automatic_backup_archive_path(state, relative_path).await?;
    let buffer = read_backup_archive_file(&file_path).await?;
    import_backup_archive_buffer(state, buffer, translator).await
}

pub(super) async fn preserved_automatic_backup_entries(
    state: &AppState,
) -> Result<Vec<Value>, BackupImportError> {
    let mut entries = Vec::new();
    for key in [AUTOMATIC_BACKUP_CONFIG_KEY, AUTOMATIC_BACKUP_RUNTIME_KEY] {
        if let Some(entry) = state
            .storage
            .store
            .export_backup_entry(key)
            .await
            .map_err(|error| BackupImportError::internal(error.to_string()))?
        {
            entries.push(entry);
        }
    }
    Ok(entries)
}

pub(super) fn automatic_backup_directory(state: &AppState) -> PathBuf {
    AUTOMATIC_BACKUP_DIRECTORY
        .iter()
        .fold(state.settings.data_dir.clone(), |path, part| {
            path.join(part)
        })
}

async fn ensure_automatic_backup_directory(state: &AppState) -> io::Result<PathBuf> {
    let directory = automatic_backup_directory(state);
    fs::create_dir_all(&directory).await?;
    Ok(directory)
}

pub(super) async fn resolve_automatic_backup_archive_path(
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

pub(super) async fn load_automatic_backup_config(state: &AppState) -> anyhow::Result<Value> {
    let value = state
        .storage
        .store
        .get_json_value(AUTOMATIC_BACKUP_CONFIG_KEY)
        .await?;
    Ok(normalize_automatic_backup_config(value.as_ref()))
}

pub(super) fn normalize_automatic_backup_config(value: Option<&Value>) -> Value {
    let raw = value.and_then(Value::as_object);
    json!({
        "enabled": raw.and_then(|value| value.get("enabled")).and_then(Value::as_bool).unwrap_or(false),
        "interval_hours": normalized_integer(
            raw.and_then(|value| value.get("interval_hours")),
            AUTOMATIC_BACKUP_DEFAULT_INTERVAL_HOURS,
            AUTOMATIC_BACKUP_MIN_INTERVAL_HOURS,
            AUTOMATIC_BACKUP_MAX_INTERVAL_HOURS,
        ),
        "retention_days": normalized_integer(
            raw.and_then(|value| value.get("retention_days")),
            AUTOMATIC_BACKUP_DEFAULT_RETENTION_DAYS,
            AUTOMATIC_BACKUP_MIN_RETENTION_DAYS,
            AUTOMATIC_BACKUP_MAX_RETENTION_DAYS,
        ),
        "updated_at": raw
            .and_then(|value| value.get("updated_at"))
            .and_then(Value::as_str)
            .filter(|value| time_utils::parse_iso_ms(value).is_some()),
    })
}

pub(super) fn validate_automatic_backup_config(
    body: &UpdateAutomaticBackupBody,
) -> Result<(), BackupImportError> {
    if !(AUTOMATIC_BACKUP_MIN_INTERVAL_HOURS..=AUTOMATIC_BACKUP_MAX_INTERVAL_HOURS)
        .contains(&body.interval_hours)
    {
        return Err(BackupImportError::bad_request(
            "Automatic backup interval is invalid",
        ));
    }
    if !(AUTOMATIC_BACKUP_MIN_RETENTION_DAYS..=AUTOMATIC_BACKUP_MAX_RETENTION_DAYS)
        .contains(&body.retention_days)
    {
        return Err(BackupImportError::bad_request(
            "Automatic backup retention is invalid",
        ));
    }
    Ok(())
}

fn normalized_integer(value: Option<&Value>, fallback: i64, min: i64, max: i64) -> i64 {
    value
        .and_then(Value::as_i64)
        .filter(|value| (min..=max).contains(value))
        .unwrap_or(fallback)
}

pub(super) async fn load_automatic_backup_runtime(state: &AppState) -> anyhow::Result<Value> {
    let value = state
        .storage
        .store
        .get_json_value(AUTOMATIC_BACKUP_RUNTIME_KEY)
        .await?;
    Ok(normalize_automatic_backup_runtime(value.as_ref()))
}

fn normalize_automatic_backup_runtime(value: Option<&Value>) -> Value {
    let raw = value.and_then(Value::as_object);
    json!({
        "last_attempt_at": normalized_timestamp(raw.and_then(|value| value.get("last_attempt_at"))),
        "last_success_at": normalized_timestamp(raw.and_then(|value| value.get("last_success_at"))),
        "last_error": raw.and_then(|value| value.get("last_error")).and_then(Value::as_str).filter(|value| !value.trim().is_empty()),
        "last_filename": raw.and_then(|value| value.get("last_filename")).and_then(Value::as_str).filter(|value| is_backup_archive_file(value)),
        "next_backup_at": normalized_timestamp(raw.and_then(|value| value.get("next_backup_at"))),
    })
}

fn normalized_timestamp(value: Option<&Value>) -> Option<&str> {
    value
        .and_then(Value::as_str)
        .filter(|value| time_utils::parse_iso_ms(value).is_some())
}

async fn save_automatic_backup_runtime(state: &AppState, runtime: &Value) -> anyhow::Result<()> {
    state
        .storage
        .store
        .set_json_value(AUTOMATIC_BACKUP_RUNTIME_KEY, runtime)
        .await
        .map_err(Into::into)
}

pub(super) async fn automatic_backup_scheduler(state: AppState) {
    match ensure_automatic_backup_directory(&state).await {
        Ok(directory) => {
            if let Err(error) = cleanup_automatic_backup_temp_files(&directory).await {
                tracing::warn!(%error, "failed to clean stale automatic backup files on startup");
            }
        }
        Err(error) => {
            tracing::warn!(%error, "failed to initialize automatic backup directory");
        }
    }
    loop {
        if state.shutdown.is_cancelled() {
            return;
        }
        let config = match load_automatic_backup_config(&state).await {
            Ok(config) => config,
            Err(error) => {
                tracing::warn!(%error, "failed to load automatic backup config");
                wait_for_automatic_backup_wakeup(&state, AUTOMATIC_BACKUP_RECHECK_SECONDS).await;
                continue;
            }
        };
        if config.get("enabled").and_then(Value::as_bool) != Some(true) {
            tokio::select! {
                _ = state.shutdown.cancelled() => return,
                _ = state.maintenance.automatic_backup_notify.notified() => {}
            }
            continue;
        }
        let runtime = match load_automatic_backup_runtime(&state).await {
            Ok(runtime) => runtime,
            Err(error) => {
                tracing::warn!(%error, "failed to load automatic backup runtime");
                wait_for_automatic_backup_wakeup(&state, AUTOMATIC_BACKUP_RECHECK_SECONDS).await;
                continue;
            }
        };
        let next_ms = runtime
            .get("next_backup_at")
            .and_then(Value::as_str)
            .and_then(time_utils::parse_iso_ms)
            .unwrap_or_else(time_utils::now_ms);
        let remaining_ms = next_ms.saturating_sub(time_utils::now_ms());
        if remaining_ms > 0 {
            wait_for_automatic_backup_wakeup(
                &state,
                ((remaining_ms as u64).saturating_add(999) / 1000)
                    .min(AUTOMATIC_BACKUP_RECHECK_SECONDS),
            )
            .await;
            continue;
        }
        if let Err(error) = run_automatic_backup_once(&state).await {
            tracing::warn!(%error, "automatic backup attempt failed");
        }
    }
}

async fn wait_for_automatic_backup_wakeup(state: &AppState, seconds: u64) {
    tokio::select! {
        _ = state.shutdown.cancelled() => {}
        _ = state.maintenance.automatic_backup_notify.notified() => {}
        _ = tokio::time::sleep(Duration::from_secs(seconds.max(1))) => {}
    }
}

pub(super) async fn run_automatic_backup_once(state: &AppState) -> anyhow::Result<Value> {
    let _guard = state.maintenance.automatic_backup_lock.lock().await;
    let config = load_automatic_backup_config(state).await?;
    if config.get("enabled").and_then(Value::as_bool) != Some(true) {
        return Ok(Value::Null);
    }
    let interval_hours = config
        .get("interval_hours")
        .and_then(Value::as_i64)
        .unwrap_or(AUTOMATIC_BACKUP_DEFAULT_INTERVAL_HOURS);
    let retention_days = config
        .get("retention_days")
        .and_then(Value::as_i64)
        .unwrap_or(AUTOMATIC_BACKUP_DEFAULT_RETENTION_DAYS);
    let attempt_at = time_utils::now_iso();
    let mut runtime = load_automatic_backup_runtime(state).await?;
    runtime["last_attempt_at"] = Value::String(attempt_at);

    let result = write_automatic_backup_archive(state).await;
    match result {
        Ok(data) => {
            let mut failure_runtime = runtime.clone();
            let completed_at = time_utils::now_iso();
            let filename = data
                .get("filename")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            runtime["last_success_at"] = Value::String(completed_at);
            runtime["last_error"] = Value::Null;
            runtime["last_filename"] = Value::String(filename.clone());
            runtime["next_backup_at"] =
                Value::String(time_utils::iso_after_seconds(interval_hours * 3600));
            if let Err(error) = save_automatic_backup_runtime(state, &runtime).await {
                let uncommitted_path = automatic_backup_directory(state).join(&filename);
                if let Err(remove_error) = fs::remove_file(&uncommitted_path).await
                    && remove_error.kind() != io::ErrorKind::NotFound
                {
                    tracing::warn!(%remove_error, path = %uncommitted_path.display(), "failed to remove uncommitted automatic backup");
                }
                let retry_seconds = (interval_hours * 3600).min(3600);
                failure_runtime["last_error"] = Value::String(error.to_string());
                failure_runtime["next_backup_at"] =
                    Value::String(time_utils::iso_after_seconds(retry_seconds));
                if let Err(save_error) =
                    save_automatic_backup_runtime(state, &failure_runtime).await
                {
                    tracing::warn!(%save_error, "failed to persist automatic backup bookkeeping failure");
                }
                return Err(error);
            }
            if let Err(error) = prune_automatic_backup_directory(state, retention_days).await {
                tracing::warn!(%error, "failed to prune expired automatic backups");
            }
            Ok(data)
        }
        Err(error) => {
            let retry_seconds = (interval_hours * 3600).min(3600);
            runtime["last_error"] = Value::String(error.to_string());
            runtime["next_backup_at"] = Value::String(time_utils::iso_after_seconds(retry_seconds));
            if let Err(save_error) = save_automatic_backup_runtime(state, &runtime).await {
                tracing::warn!(%save_error, "failed to persist automatic backup failure");
            }
            Err(error)
        }
    }
}

async fn write_automatic_backup_archive(state: &AppState) -> anyhow::Result<Value> {
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

async fn cleanup_automatic_backup_temp_files(directory: &Path) -> io::Result<()> {
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

pub(super) async fn prune_automatic_backup_directory(
    state: &AppState,
    retention_days: i64,
) -> io::Result<()> {
    let directory = ensure_automatic_backup_directory(state).await?;
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
        if metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH) < cutoff
            && let Err(error) = fs::remove_file(entry.path()).await
        {
            tracing::warn!(%error, path = %entry.path().display(), "failed to prune automatic backup");
        }
    }
    Ok(())
}

pub(super) fn next_backup_after_last_success(
    last_success_at: Option<&str>,
    interval_hours: i64,
    now_ms: i64,
) -> String {
    let next_ms = last_success_at
        .and_then(time_utils::parse_iso_ms)
        .and_then(|last| last.checked_add(interval_hours * 3600 * 1000))
        .filter(|next| *next > now_ms)
        .unwrap_or(now_ms);
    time_utils::iso_from_ms(next_ms)
}

pub(super) fn next_backup_after_failure(
    current_next_backup_at: Option<&str>,
    interval_hours: i64,
    now_ms: i64,
) -> String {
    let retry_seconds = interval_hours.saturating_mul(3600).min(3600);
    let retry_cap_ms = now_ms.saturating_add(retry_seconds.saturating_mul(1000));
    let next_ms = current_next_backup_at
        .and_then(time_utils::parse_iso_ms)
        .map(|next| next.min(retry_cap_ms))
        .unwrap_or(retry_cap_ms);
    time_utils::iso_from_ms(next_ms)
}
