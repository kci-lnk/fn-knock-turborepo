use super::*;

pub(in crate::system::maintenance) async fn automatic_backup_details(
    state: &AppState,
) -> anyhow::Result<Value> {
    let config = load_automatic_backup_config(state).await?;
    let runtime = load_automatic_backup_runtime(state).await?;
    backup_email::decorate(
        state,
        automatic_backup_details_value(state, config, runtime),
    )
    .await
}

pub(in crate::system::maintenance) fn automatic_backup_details_value(
    state: &AppState,
    config: Value,
    runtime: Value,
) -> Value {
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

pub(in crate::system::maintenance) async fn save_automatic_backup_config(
    state: &AppState,
    body: UpdateAutomaticBackupBody,
) -> Result<Value, BackupImportError> {
    validate_automatic_backup_config(&body)?;
    let guard = state.maintenance.automatic_backup_lock.lock().await;
    let mut email = backup_email::load(state)
        .await
        .map_err(|error| BackupImportError::internal(error.to_string()))?;
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
    let old_secret = email.secret_id.clone();
    let staged_secret = if let Some(update) = body.email {
        backup_email::prepare_update(state, &mut email, update, body.enabled)?
    } else {
        backup_email::cancel_pending(&mut email, body.enabled, false);
        None
    };
    let email_value = serde_json::to_value(&email)
        .map_err(|error| BackupImportError::internal(error.to_string()))?;
    let saved = state
        .storage
        .store
        .set_json_values_atomically(&[
            (backup_email::EMAIL_KEY, &email_value),
            (AUTOMATIC_BACKUP_CONFIG_KEY, &config),
            (AUTOMATIC_BACKUP_RUNTIME_KEY, &runtime),
        ])
        .await
        .map_err(|error| BackupImportError::internal(error.to_string()));
    if let Err(error) = saved {
        backup_email::discard_secret(state, staged_secret.as_deref());
        return Err(error);
    }
    if old_secret != email.secret_id {
        backup_email::discard_secret(state, old_secret.as_deref());
    }
    drop(guard);
    state.maintenance.automatic_backup_notify.notify_one();

    backup_email::decorate(
        state,
        automatic_backup_details_value(state, config, runtime),
    )
    .await
    .map_err(|error| BackupImportError::internal(error.to_string()))
}

pub(in crate::system::maintenance) async fn preserved_automatic_backup_entries(
    state: &AppState,
) -> Result<Vec<Value>, BackupImportError> {
    let mut entries = Vec::new();
    for key in [
        AUTOMATIC_BACKUP_CONFIG_KEY,
        AUTOMATIC_BACKUP_RUNTIME_KEY,
        backup_email::EMAIL_KEY,
    ] {
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

pub(in crate::system::maintenance) async fn load_automatic_backup_config(
    state: &AppState,
) -> anyhow::Result<Value> {
    let value = state
        .storage
        .store
        .get_json_value(AUTOMATIC_BACKUP_CONFIG_KEY)
        .await?;
    Ok(normalize_automatic_backup_config(value.as_ref()))
}

pub(in crate::system::maintenance) fn normalize_automatic_backup_config(
    value: Option<&Value>,
) -> Value {
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

pub(in crate::system::maintenance) fn validate_automatic_backup_config(
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

pub(in crate::system::maintenance) fn normalized_integer(
    value: Option<&Value>,
    fallback: i64,
    min: i64,
    max: i64,
) -> i64 {
    value
        .and_then(Value::as_i64)
        .filter(|value| (min..=max).contains(value))
        .unwrap_or(fallback)
}

pub(in crate::system::maintenance) async fn load_automatic_backup_runtime(
    state: &AppState,
) -> anyhow::Result<Value> {
    let value = state
        .storage
        .store
        .get_json_value(AUTOMATIC_BACKUP_RUNTIME_KEY)
        .await?;
    Ok(normalize_automatic_backup_runtime(value.as_ref()))
}

pub(in crate::system::maintenance) fn normalize_automatic_backup_runtime(
    value: Option<&Value>,
) -> Value {
    let raw = value.and_then(Value::as_object);
    json!({
        "last_attempt_at": normalized_timestamp(raw.and_then(|value| value.get("last_attempt_at"))),
        "last_success_at": normalized_timestamp(raw.and_then(|value| value.get("last_success_at"))),
        "last_error": raw.and_then(|value| value.get("last_error")).and_then(Value::as_str).filter(|value| !value.trim().is_empty()),
        "last_filename": raw.and_then(|value| value.get("last_filename")).and_then(Value::as_str).filter(|value| is_backup_archive_file(value)),
        "next_backup_at": normalized_timestamp(raw.and_then(|value| value.get("next_backup_at"))),
    })
}

pub(in crate::system::maintenance) fn normalized_timestamp(value: Option<&Value>) -> Option<&str> {
    value
        .and_then(Value::as_str)
        .filter(|value| time_utils::parse_iso_ms(value).is_some())
}

pub(in crate::system::maintenance) async fn save_automatic_backup_runtime(
    state: &AppState,
    runtime: &Value,
) -> anyhow::Result<()> {
    state
        .storage
        .store
        .set_json_value(AUTOMATIC_BACKUP_RUNTIME_KEY, runtime)
        .await
        .map_err(Into::into)
}
