use super::*;

pub(super) async fn import_backup_archive(
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

pub(super) async fn import_backup_archive_from_directory(
    state: &AppState,
    relative_path: &str,
    translator: &Translator,
) -> Result<Value, BackupImportError> {
    let file_path = resolve_backup_archive_path(relative_path).await?;
    if !is_backup_archive_file(&file_path.to_string_lossy()) {
        return Err(BackupImportError::bad_request(format!(
            "Backup archive file must end with {KNOCK_BACKUP_EXTENSION}"
        )));
    }
    let buffer = read_backup_archive_file(&file_path).await?;
    import_backup_archive_buffer(state, buffer, translator).await
}

pub(super) async fn read_backup_archive_file(
    file_path: &Path,
) -> Result<Vec<u8>, BackupImportError> {
    let file = open_backup_archive_without_following_links(file_path).await?;
    let metadata = file
        .metadata()
        .await
        .map_err(|error| BackupImportError::internal(error.to_string()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(BackupImportError::bad_request("Backup path must be a file"));
    }
    if metadata.len() > MAX_BACKUP_ARCHIVE_SIZE as u64 {
        return Err(BackupImportError::bad_request(
            "Backup directory import archive is too large",
        ));
    }
    fs_utils::read_open_file_limited(file, MAX_BACKUP_ARCHIVE_SIZE)
        .await
        .map_err(|error| {
            if error.kind() == io::ErrorKind::InvalidData {
                BackupImportError::bad_request("Backup directory import archive is too large")
            } else {
                BackupImportError::internal(error.to_string())
            }
        })
}

async fn open_backup_archive_without_following_links(
    file_path: &Path,
) -> Result<fs::File, BackupImportError> {
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW);
    #[cfg(windows)]
    options.custom_flags(windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT);
    options.open(file_path).await.map_err(|error| {
        #[cfg(unix)]
        if error.raw_os_error() == Some(libc::ELOOP) {
            return BackupImportError::bad_request("Backup path must be a file");
        }
        match error.kind() {
            io::ErrorKind::NotFound => {
                BackupImportError::new(StatusCode::NOT_FOUND, "Backup file not found")
            }
            io::ErrorKind::PermissionDenied => {
                BackupImportError::new(StatusCode::FORBIDDEN, "Backup file cannot be read")
            }
            _ => BackupImportError::internal(error.to_string()),
        }
    })
}

pub(super) async fn import_backup_archive_buffer(
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

    let mut payload = {
        let _archive_work_guard = state.maintenance.backup_archive_work_lock.lock().await;
        extract_backup_payload_from_archive(buffer).await?
    };
    let restored_credentials = import_protected_credentials(
        payload
            .as_object_mut()
            .and_then(|object| object.remove("protected_credentials")),
    )?;
    let elapsed_ms = payload
        .get("exported_at")
        .and_then(Value::as_str)
        .and_then(time_utils::parse_iso_ms)
        .map(|exported_at| time_utils::now_ms().saturating_sub(exported_at).max(0))
        // Legacy parsers accepted any non-empty exported_at string. Keep such
        // archives importable, but fail closed by discarding expiring state.
        .unwrap_or(i64::MAX);
    let entries = payload
        .as_object_mut()
        .and_then(|object| object.remove("entries"))
        .and_then(|entries| match entries {
            Value::Array(entries) => Some(entries),
            _ => None,
        })
        .unwrap_or_default();
    let mut importable_entries = age_backup_entries(entries, elapsed_ms)
        .into_iter()
        .filter(|entry| {
            entry
                .get("key")
                .and_then(Value::as_str)
                .is_some_and(should_export_backup_key)
        })
        .collect::<Vec<_>>();
    let imported_keys = importable_entries.len();

    // Automatic backup settings describe this installation rather than the
    // imported snapshot. Preserve them inside the same atomic prefix
    // replacement so a restore cannot silently disable the scheduler.
    let _automatic_backup_guard = state.maintenance.automatic_backup_lock.lock().await;
    importable_entries.extend(preserved_automatic_backup_entries(state).await?);

    // Backup replacement clears the complete fn_knock: prefix. Serialize it
    // with Host Mapping config/runtime updates using a lease whose own key is
    // deliberately outside that prefix, and retain it until runtime sync has
    // consumed the restored config.
    let _host_mappings_guard = state.gateway.host_mappings_update_lock.lock().await;
    let host_mappings_lease = proxy_config::acquire_host_mappings_transaction_lease(state)
        .await
        .map_err(|error| BackupImportError::internal(error.to_string()))?
        .ok_or_else(|| {
            BackupImportError::internal("Host mappings transaction is busy during backup import")
        })?;
    host_mappings_lease
        .ensure_owned()
        .await
        .map_err(|error| BackupImportError::internal(error.to_string()))?;

    let previous_snapshot_at_ms = time_utils::now_ms();
    let previous_entries = state
        .storage
        .store
        .export_backup_entries_by_prefix_limited(
            KNOCK_BACKUP_PREFIX,
            MAX_BACKUP_ARCHIVE_SIZE,
            should_snapshot_backup_import_key,
        )
        .await
        .map_err(|error| BackupImportError::internal(error.to_string()))?;
    let previous_config =
        backup_config_from_entries(&previous_entries).unwrap_or_else(|| json!({}));
    let previous_credentials = snapshot_current_credentials(state).await?;

    let cleared_keys = state
        .storage
        .store
        .replace_backup_entries_by_prefix(KNOCK_BACKUP_PREFIX, &importable_entries, SCAN_COUNT)
        .await
        .map_err(|error| BackupImportError::internal(error.to_string()))?;

    if let Err(migration_error) = migrate_compiled_ipsets_after_import(state).await {
        let rollback_detail =
            rollback_backup_import_storage(state, &previous_entries, previous_snapshot_at_ms).await;
        let ownership_result = host_mappings_lease.ensure_owned().await;
        let release_result = host_mappings_lease.release().await;
        ownership_result.map_err(|error| BackupImportError::internal(error.to_string()))?;
        release_result.map_err(|error| BackupImportError::internal(error.to_string()))?;
        return Err(BackupImportError::internal(format!(
            "compiled IP set migration failed: {migration_error}{rollback_detail}"
        )));
    }

    if let Err(migration_error) =
        runtime_config::migrate_and_constrain_config_after_import(state).await
    {
        let rollback_detail =
            rollback_backup_import_storage(state, &previous_entries, previous_snapshot_at_ms).await;
        let ownership_result = host_mappings_lease.ensure_owned().await;
        let release_result = host_mappings_lease.release().await;
        ownership_result.map_err(|error| BackupImportError::internal(error.to_string()))?;
        release_result.map_err(|error| BackupImportError::internal(error.to_string()))?;
        return Err(BackupImportError::internal(format!(
            "backup config migration failed: {migration_error}{rollback_detail}"
        )));
    }

    // Terminal sessions are process-local runtime state and must never span a
    // target metadata restore. Closing them before secret cleanup also ensures
    // no live actor keeps using credentials that no longer belong to the
    // restored installation state.
    state.terminal.shutdown_all().await;

    // Credentials belong to this installation. Clear them only after every
    // fatal storage step has succeeded, and restore storage if credential
    // cleanup itself cannot be completed atomically.
    if let Err(error) = cloudflared::clear_credentials_after_backup_restore(state).await {
        let rollback_detail =
            rollback_backup_import_storage(state, &previous_entries, previous_snapshot_at_ms).await;
        let ownership_result = host_mappings_lease.ensure_owned().await;
        let release_result = host_mappings_lease.release().await;
        ownership_result.map_err(|error| BackupImportError::internal(error.to_string()))?;
        release_result.map_err(|error| BackupImportError::internal(error.to_string()))?;
        return Err(BackupImportError::internal(format!(
            "Cloudflared credentials could not be cleared after backup restore: {error}{rollback_detail}"
        )));
    }

    if let Err(error) = restore_credential_snapshot(state, &restored_credentials).await {
        let rollback_detail = rollback_backup_import_with_credentials(
            state,
            &previous_entries,
            previous_snapshot_at_ms,
            &previous_credentials,
        )
        .await;
        let ownership_result = host_mappings_lease.ensure_owned().await;
        let release_result = host_mappings_lease.release().await;
        ownership_result.map_err(|error| BackupImportError::internal(error.to_string()))?;
        release_result.map_err(|error| BackupImportError::internal(error.to_string()))?;
        return Err(BackupImportError::internal(format!(
            "Terminal and WoL credentials could not be restored from backup: {error}{rollback_detail}"
        )));
    }

    if let Err(error) = panel_sync::clear_credentials_after_backup_restore(state).await {
        let rollback_detail = rollback_backup_import_with_credentials(
            state,
            &previous_entries,
            previous_snapshot_at_ms,
            &previous_credentials,
        )
        .await;
        let ownership_result = host_mappings_lease.ensure_owned().await;
        let release_result = host_mappings_lease.release().await;
        ownership_result.map_err(|error| BackupImportError::internal(error.to_string()))?;
        release_result.map_err(|error| BackupImportError::internal(error.to_string()))?;
        return Err(BackupImportError::internal(format!(
            "Panel sync credentials could not be cleared after backup restore: {error}{rollback_detail}"
        )));
    }

    let (mut warnings, synced_steps) =
        sync_runtime_after_import(state, translator, &previous_config).await;
    let ownership_result = host_mappings_lease.ensure_owned().await;
    let release_result = host_mappings_lease.release().await;
    let transaction_label = maintenance_backup_text(translator, "syncSteps.transactionFinalize");
    if let Err(error) = ownership_result {
        warnings.push(format!("{transaction_label}: {error}"));
    }
    if let Err(error) = release_result {
        warnings.push(format!("{transaction_label}: {error}"));
    }
    Ok(json!({
        "cleared_keys": cleared_keys,
        "imported_keys": imported_keys,
        "warnings": warnings,
        "synced_steps": synced_steps
    }))
}

pub(super) fn backup_config_from_entries(entries: &[Value]) -> Option<Value> {
    entries
        .iter()
        .find(|entry| entry.get("key").and_then(Value::as_str) == Some("fn_knock:config"))
        .and_then(|entry| entry.get("value").and_then(Value::as_str))
        .and_then(|value| serde_json::from_str(value).ok())
}

async fn rollback_backup_import_storage(
    state: &AppState,
    previous_entries: &[Value],
    snapshot_at_ms: i64,
) -> String {
    let elapsed_ms = time_utils::now_ms().saturating_sub(snapshot_at_ms).max(0);
    let previous_entries = age_backup_entries(previous_entries.to_vec(), elapsed_ms);
    let rollback_result = state
        .storage
        .store
        .replace_backup_entries_by_prefix(KNOCK_BACKUP_PREFIX, &previous_entries, SCAN_COUNT)
        .await;
    let runtime_restore_result = if rollback_result.is_ok() {
        migrate_compiled_ipsets_after_import(state).await
    } else {
        Ok(())
    };
    let rollback_detail = rollback_result
        .err()
        .map(|error| format!("; storage rollback failed: {error}"))
        .unwrap_or_default();
    let runtime_detail = runtime_restore_result
        .err()
        .map(|error| format!("; runtime rollback failed: {error}"))
        .unwrap_or_default();
    format!("{rollback_detail}{runtime_detail}")
}

async fn rollback_backup_import_with_credentials(
    state: &AppState,
    previous_entries: &[Value],
    snapshot_at_ms: i64,
    previous_credentials: &CredentialBackupPayload,
) -> String {
    let mut detail = rollback_backup_import_storage(state, previous_entries, snapshot_at_ms).await;
    if !detail.contains("storage rollback failed")
        && let Err(error) = restore_credential_snapshot(state, previous_credentials).await
    {
        detail.push_str(&format!("; credential rollback failed: {error}"));
    }
    detail
}

pub(super) fn age_backup_entries(entries: Vec<Value>, elapsed_ms: i64) -> Vec<Value> {
    entries
        .into_iter()
        .filter_map(|entry| age_backup_entry_ttl(entry, elapsed_ms))
        .collect()
}

pub(super) fn age_backup_entry_ttl(mut entry: Value, elapsed_ms: i64) -> Option<Value> {
    let Some(ttl_ms) = entry.get("ttl_ms").and_then(Value::as_i64) else {
        return Some(entry);
    };
    let remaining_ms = ttl_ms.saturating_sub(elapsed_ms);
    if remaining_ms <= 0 {
        return None;
    }
    entry["ttl_ms"] = json!(remaining_ms);
    Some(entry)
}

async fn migrate_compiled_ipsets_after_import(state: &AppState) -> Result<(), String> {
    crate::cidr::migrate_cidr_query_caches_on_boot(state)
        .await
        .map_err(|error| error.to_string())?;
    gateway_settings::migrate_visibility_policies_locked(state).await?;
    scanner::migrate_scanner_cidr_ipset_on_boot(state)
        .await
        .map_err(|error| error.to_string())?;
    common_auth_locations::migrate_common_auth_location_ipset_in_storage(state)
        .await
        .map_err(|error| error.to_string())?;
    whitelist::migrate_whitelist_ipsets_in_storage(state)
        .await
        .map_err(|error| error.to_string())?;
    ssh_security::migrate_ssh_ipset_on_boot(state)
        .await
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub(super) async fn extract_backup_payload_from_archive(
    buffer: Vec<u8>,
) -> Result<Value, BackupImportError> {
    tokio::task::spawn_blocking(move || parse_backup_payload_from_archive_native(&buffer))
        .await
        .map_err(|error| BackupImportError::internal(error.to_string()))?
}

pub(super) fn parse_backup_payload_from_archive_native(
    buffer: &[u8],
) -> Result<Value, BackupImportError> {
    let mut archive = ::zip::ZipArchive::new(Cursor::new(buffer)).map_err(|_| {
        BackupImportError::bad_request(backup_error_key_message("readArchiveFailed", &[]))
    })?;
    let file = archive
        .by_name_decrypt(KNOCK_BACKUP_JSON_FILENAME, KNOCK_BACKUP_PASSWORD.as_bytes())
        .map_err(|error| match error {
            ::zip::result::ZipError::FileNotFound => BackupImportError::bad_request(format!(
                "Backup archive is missing {KNOCK_BACKUP_JSON_FILENAME}"
            )),
            ::zip::result::ZipError::InvalidPassword => {
                BackupImportError::bad_request("Backup archive password is invalid")
            }
            _ => BackupImportError::bad_request(backup_error_key_message("readArchiveFailed", &[])),
        })?;
    if file.size() > MAX_BACKUP_ARCHIVE_SIZE as u64 {
        return Err(BackupImportError::bad_request(
            "Backup JSON payload is too large",
        ));
    }
    let payload: Value =
        serde_json::from_reader(file.take((MAX_BACKUP_ARCHIVE_SIZE + 1) as u64))
            .map_err(|_| BackupImportError::bad_request("Backup JSON payload is invalid"))?;
    normalize_backup_payload(payload)
}

#[cfg(test)]
pub(super) fn read_backup_json_from_archive_native(
    buffer: &[u8],
) -> Result<String, BackupImportError> {
    read_backup_json_from_archive_native_with_limit(buffer, MAX_BACKUP_ARCHIVE_SIZE)
}

#[cfg(test)]
pub(super) fn read_backup_json_from_archive_native_with_limit(
    buffer: &[u8],
    limit: usize,
) -> Result<String, BackupImportError> {
    let mut archive = ::zip::ZipArchive::new(Cursor::new(buffer)).map_err(|_| {
        BackupImportError::bad_request(backup_error_key_message("readArchiveFailed", &[]))
    })?;
    let file = archive
        .by_name_decrypt(KNOCK_BACKUP_JSON_FILENAME, KNOCK_BACKUP_PASSWORD.as_bytes())
        .map_err(|error| match error {
            ::zip::result::ZipError::FileNotFound => BackupImportError::bad_request(format!(
                "Backup archive is missing {KNOCK_BACKUP_JSON_FILENAME}"
            )),
            ::zip::result::ZipError::InvalidPassword => {
                BackupImportError::bad_request("Backup archive password is invalid")
            }
            _ => BackupImportError::bad_request(backup_error_key_message("readArchiveFailed", &[])),
        })?;
    if file.size() > limit as u64 {
        return Err(BackupImportError::bad_request(
            "Backup JSON payload is too large",
        ));
    }

    let mut raw = Vec::new();
    file.take((limit + 1) as u64)
        .read_to_end(&mut raw)
        .map_err(|_| {
            BackupImportError::bad_request(backup_error_key_message("readArchiveFailed", &[]))
        })?;
    if raw.len() > limit {
        return Err(BackupImportError::bad_request(
            "Backup JSON payload is too large",
        ));
    }
    String::from_utf8(raw)
        .map_err(|_| BackupImportError::bad_request("Backup payload is not valid UTF-8"))
}

#[cfg(test)]
pub(super) fn parse_backup_payload(raw: &str) -> Result<Value, BackupImportError> {
    let payload: Value = serde_json::from_str(raw)
        .map_err(|_| BackupImportError::bad_request("Backup JSON payload is invalid"))?;
    normalize_backup_payload(payload)
}

pub(super) fn normalize_backup_payload(mut payload: Value) -> Result<Value, BackupImportError> {
    let Some(object) = payload.as_object() else {
        return Err(BackupImportError::bad_request(
            "Backup payload must be an object",
        ));
    };
    let schema_version = object
        .get("version")
        .or_else(|| object.get("backupSchemaVersion"))
        .and_then(js_number_from_json)
        .filter(|value| value.is_finite())
        .map(|value| value.trunc() as i64);
    if schema_version != Some(APP_BACKUP_SCHEMA_VERSION) {
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
        .map(str::to_string)
        .ok_or_else(|| BackupImportError::bad_request("Backup app version is missing"))?;
    if !backup_app_version_supported(&app_version) {
        return Err(BackupImportError::bad_request(format!(
            "Backup app version {app_version} is unsupported. Supported range is {APP_BACKUP_IMPORT_MIN_VERSION} ~ {APP_LOCAL_VERSION}"
        )));
    }
    let exported_at = object
        .get("exported_at")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| BackupImportError::bad_request("Backup exported_at is missing"))?;
    let protected_credentials = payload
        .as_object()
        .and_then(|object| object.get("protected_credentials"))
        .cloned();
    let mut entries = payload
        .as_object_mut()
        .and_then(|object| object.remove("entries"))
        .and_then(|entries| match entries {
            Value::Array(entries) => Some(entries),
            _ => None,
        })
        .ok_or_else(|| BackupImportError::bad_request("Backup entries are missing"))?;
    let mut keys = std::collections::BTreeSet::new();
    for (index, entry) in entries.iter_mut().enumerate() {
        let raw = std::mem::take(entry);
        let normalized = parse_backup_entry(&raw, index)?;
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
        *entry = normalized;
    }
    let mut normalized = json!({
        "version": APP_BACKUP_SCHEMA_VERSION,
        "app_version": app_version,
        "prefix": KNOCK_BACKUP_PREFIX,
        "exported_at": exported_at,
        "entry_count": entries.len(),
        "entries": entries
    });
    if let Some(protected_credentials) = protected_credentials {
        normalized["protected_credentials"] = protected_credentials;
    }
    Ok(normalized)
}

pub(super) fn parse_backup_entry(entry: &Value, index: usize) -> Result<Value, BackupImportError> {
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

pub(super) fn parse_backup_hash_value(
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

pub(super) fn parse_backup_string_array(
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

pub(super) fn parse_backup_zset_value(
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

pub(super) fn parse_backup_stream_value(
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
