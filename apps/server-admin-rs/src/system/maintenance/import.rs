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

    #[cfg(not(windows))]
    ensure_archive_commands_ready().await?;
    let payload = extract_backup_payload_from_archive(&buffer).await?;
    let entries = payload
        .get("entries")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut importable_entries = entries
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
    let _automatic_backup_guard = state.automatic_backup_lock.lock().await;
    importable_entries.extend(preserved_automatic_backup_entries(state).await?);

    // Backup replacement clears the complete fn_knock: prefix. Serialize it
    // with Host Mapping config/runtime updates using a lease whose own key is
    // deliberately outside that prefix, and retain it until runtime sync has
    // consumed the restored config.
    let _host_mappings_guard = state.host_mappings_update_lock.lock().await;
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

    let previous_keys = state
        .store
        .scan_keys(KNOCK_BACKUP_PREFIX, SCAN_COUNT)
        .await
        .map_err(|error| BackupImportError::internal(error.to_string()))?;
    let mut previous_entries = Vec::with_capacity(previous_keys.len());
    for key in previous_keys {
        if let Some(entry) = state
            .store
            .export_backup_entry(&key)
            .await
            .map_err(|error| BackupImportError::internal(error.to_string()))?
        {
            previous_entries.push(entry);
        }
    }

    let cleared_keys = state
        .store
        .replace_backup_entries_by_prefix(KNOCK_BACKUP_PREFIX, &importable_entries, SCAN_COUNT)
        .await
        .map_err(|error| BackupImportError::internal(error.to_string()))?;

    if let Err(migration_error) = migrate_compiled_ipsets_after_import(state).await {
        let rollback_result = state
            .store
            .replace_backup_entries_by_prefix(KNOCK_BACKUP_PREFIX, &previous_entries, SCAN_COUNT)
            .await;
        let runtime_restore_result = if rollback_result.is_ok() {
            migrate_compiled_ipsets_after_import(state).await
        } else {
            Ok(())
        };
        let ownership_result = host_mappings_lease.ensure_owned().await;
        let release_result = host_mappings_lease.release().await;
        ownership_result.map_err(|error| BackupImportError::internal(error.to_string()))?;
        release_result.map_err(|error| BackupImportError::internal(error.to_string()))?;
        let rollback_detail = rollback_result
            .err()
            .map(|error| format!("; storage rollback failed: {error}"))
            .unwrap_or_default();
        let runtime_detail = runtime_restore_result
            .err()
            .map(|error| format!("; runtime rollback failed: {error}"))
            .unwrap_or_default();
        return Err(BackupImportError::internal(format!(
            "compiled IP set migration failed: {migration_error}{rollback_detail}{runtime_detail}"
        )));
    }

    let (warnings, synced_steps) = sync_runtime_after_import(state, translator).await;
    let ownership_result = host_mappings_lease.ensure_owned().await;
    let release_result = host_mappings_lease.release().await;
    ownership_result.map_err(|error| BackupImportError::internal(error.to_string()))?;
    release_result.map_err(|error| BackupImportError::internal(error.to_string()))?;
    Ok(json!({
        "cleared_keys": cleared_keys,
        "imported_keys": imported_keys,
        "warnings": warnings,
        "synced_steps": synced_steps
    }))
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

#[cfg(not(windows))]
pub(super) async fn extract_backup_payload_from_archive(
    buffer: &[u8],
) -> Result<Value, BackupImportError> {
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

#[cfg(windows)]
pub(super) async fn extract_backup_payload_from_archive(
    buffer: &[u8],
) -> Result<Value, BackupImportError> {
    let buffer = buffer.to_vec();
    tokio::task::spawn_blocking(move || {
        let raw = read_backup_json_from_archive_native(&buffer)?;
        parse_backup_payload(&raw)
    })
    .await
    .map_err(|error| BackupImportError::internal(error.to_string()))?
}

#[cfg(any(windows, test))]
pub(super) fn read_backup_json_from_archive_native(
    buffer: &[u8],
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
    if file.size() > MAX_BACKUP_ARCHIVE_SIZE as u64 {
        return Err(BackupImportError::bad_request(
            "Backup JSON payload is too large",
        ));
    }
    let mut raw = Vec::new();
    file.take((MAX_BACKUP_ARCHIVE_SIZE + 1) as u64)
        .read_to_end(&mut raw)
        .map_err(|_| {
            BackupImportError::bad_request(backup_error_key_message("readArchiveFailed", &[]))
        })?;
    if raw.len() > MAX_BACKUP_ARCHIVE_SIZE {
        return Err(BackupImportError::bad_request(
            "Backup JSON payload is too large",
        ));
    }
    String::from_utf8(raw)
        .map_err(|_| BackupImportError::bad_request("Backup payload is not valid UTF-8"))
}

pub(super) fn parse_backup_payload(raw: &str) -> Result<Value, BackupImportError> {
    let payload: Value = serde_json::from_str(raw)
        .map_err(|_| BackupImportError::bad_request("Backup JSON payload is invalid"))?;
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
