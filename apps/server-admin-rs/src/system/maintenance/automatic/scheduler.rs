use super::*;

pub(in crate::system::maintenance) fn spawn_automatic_backup_task(state: AppState) {
    state.spawn_background(
        "automatic-backup",
        automatic_backup_scheduler(state.clone()),
    );
}

pub(in crate::system::maintenance) async fn automatic_backup_scheduler(state: AppState) {
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

pub(in crate::system::maintenance) async fn wait_for_automatic_backup_wakeup(
    state: &AppState,
    seconds: u64,
) {
    tokio::select! {
        _ = state.shutdown.cancelled() => {}
        _ = state.maintenance.automatic_backup_notify.notified() => {}
        _ = tokio::time::sleep(Duration::from_secs(seconds.max(1))) => {}
    }
}

pub(in crate::system::maintenance) async fn run_automatic_backup_once(
    state: &AppState,
) -> anyhow::Result<Value> {
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
            let commit = async {
                let mut email = backup_email::load(state).await?;
                backup_email::enqueue(
                    &mut email,
                    &filename,
                    data["exportedAt"].as_str().unwrap_or_default(),
                );
                let value = serde_json::to_value(email)?;
                state
                    .storage
                    .store
                    .set_json_values_atomically(&[
                        (AUTOMATIC_BACKUP_RUNTIME_KEY, &runtime),
                        (backup_email::EMAIL_KEY, &value),
                    ])
                    .await?;
                Ok::<(), anyhow::Error>(())
            }
            .await;
            if let Err(error) = commit {
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

pub(in crate::system::maintenance) fn next_backup_after_last_success(
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

pub(in crate::system::maintenance) fn next_backup_after_failure(
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
