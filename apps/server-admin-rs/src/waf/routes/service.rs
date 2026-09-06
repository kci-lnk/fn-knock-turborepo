use super::*;

pub(super) async fn get_waf_details(state: &AppState) -> anyhow::Result<Value> {
    ensure_waf_directories(state).await?;
    let config = load_waf_config(state).await?;
    let manifest_cache = get_manifest_cache_for_details(state).await?;
    let synced = read_system_sync_state(state).await?;
    let rules_state = read_rules_state(state).await?;
    let system_rules = list_rule_files(state, "system", &manifest_cache, &rules_state).await?;
    let custom_rules = list_rule_files(state, "custom", &manifest_cache, &rules_state).await?;
    let status = match state.gateway.client.get_waf_status().await {
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

pub(super) async fn apply_waf_config(state: &AppState, patch: &Value) -> anyhow::Result<Value> {
    let mut full_config = state.storage.store.get_config().await?;
    if !full_config.is_object() {
        full_config = store::default_config();
    }
    let current = normalize_waf_config_for_full_config(&full_config, state);
    let mut next_raw = current.as_object().cloned().unwrap_or_default();
    if let Some(patch) = patch.as_object() {
        for key in [
            "enabled",
            "system_rules_auto_update_enabled",
            "common_location_exempt_enabled",
            "private_ip_exempt_enabled",
            "block_behavior",
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
    if let Some(object) = full_config.as_object_mut() {
        object.insert("waf".to_string(), Value::Object(next_raw));
    }
    let next = normalize_waf_config_for_full_config(&full_config, state);
    if let Some(object) = full_config.as_object_mut() {
        object.insert("waf".to_string(), next.clone());
    }
    let should_apply_to_gateway = has_any_key(
        patch,
        &[
            "enabled",
            "paranoia_level",
            "executing_paranoia_level",
            "private_ip_exempt_enabled",
            "block_behavior",
        ],
    );
    let should_sync_common_auth_locations =
        should_apply_to_gateway || has_any_key(patch, &["common_location_exempt_enabled"]);

    // Keep persisted configuration aligned with the gateway runtime. In
    // particular, a failed private-IP-exemption update must not become active
    // only after the next gateway restart.
    if should_apply_to_gateway
        && let Err(error) = apply_waf_config_to_gateway(
            state,
            &next,
            "Enable WAF after at least one rule is enabled",
        )
        .await
    {
        return Err(error);
    }
    if should_sync_common_auth_locations
        && let Err(error) = sync_common_auth_location_exemptions_to_gateway(state, &next).await
    {
        restore_waf_runtime_after_failed_config_update(
            state,
            &current,
            should_apply_to_gateway,
            should_sync_common_auth_locations,
        )
        .await;
        return Err(error);
    }
    if let Err(error) = state.storage.store.save_config(&full_config).await {
        restore_waf_runtime_after_failed_config_update(
            state,
            &current,
            should_apply_to_gateway,
            should_sync_common_auth_locations,
        )
        .await;
        return Err(error.into());
    }

    get_waf_details(state).await
}

async fn restore_waf_runtime_after_failed_config_update(
    state: &AppState,
    previous: &Value,
    restore_waf_config: bool,
    restore_common_auth_locations: bool,
) {
    if restore_waf_config
        && let Err(error) = apply_waf_config_to_gateway(
            state,
            previous,
            "Enable WAF after at least one rule is enabled",
        )
        .await
    {
        tracing::warn!(%error, "failed to restore WAF gateway runtime after rejected config update");
    }
    if restore_common_auth_locations
        && let Err(error) = sync_common_auth_location_exemptions_to_gateway(state, previous).await
    {
        tracing::warn!(%error, "failed to restore common-auth exemptions after rejected WAF config update");
    }
}

pub(crate) async fn sync_waf_config_to_gateway(
    state: &AppState,
    full_config: &Value,
) -> anyhow::Result<Value> {
    let normalized = normalize_waf_config_for_full_config(full_config, state);
    apply_waf_config_to_gateway(
        state,
        &normalized,
        "Enable WAF after at least one rule is enabled",
    )
    .await?;
    sync_common_auth_location_exemptions_to_gateway(state, &normalized).await?;
    Ok(normalized)
}

pub(crate) async fn restore_waf_runtime_after_import(
    state: &AppState,
    full_config: &Value,
) -> anyhow::Result<Value> {
    let normalized = normalize_waf_config_for_full_config(full_config, state);
    if normalized
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        ensure_waf_directories(state).await?;
        let rules_state = read_rules_state(state).await?;
        let has_enabled_rules = has_any_enabled_rule_files(state, &rules_state, None).await?;
        if should_sync_system_rules_for_restore(&normalized, has_enabled_rules) {
            sync_system_waf_rules(state).await?;
        }
    }
    sync_waf_config_to_gateway(state, full_config).await
}

pub(super) fn should_sync_system_rules_for_restore(
    normalized_waf_config: &Value,
    has_enabled_rules: bool,
) -> bool {
    normalized_waf_config
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && !has_enabled_rules
}

pub(super) async fn sync_waf_on_boot(state: &AppState) -> anyhow::Result<()> {
    let full_config = state.storage.store.get_config().await?;
    let config = normalize_waf_config_for_full_config(&full_config, state);
    if apply_recommended_lfi_rule_patch_if_needed(state).await? {
        tracing::info!(
            rule = LFI_RULE_FILENAME,
            "enabled newly recommended WAF rule during upgrade"
        );
    }
    let enabled = config
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if enabled {
        ensure_waf_directories(state).await?;
    }
    if enabled
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
    sync_waf_config_to_gateway(state, &full_config).await?;
    Ok(())
}

pub(super) async fn apply_recommended_lfi_rule_patch_if_needed(
    state: &AppState,
) -> anyhow::Result<bool> {
    if state
        .storage
        .store
        .get_string_value(RECOMMENDED_LFI_RULE_PATCH_FLAG_KEY)
        .await?
        .as_deref()
        == Some("1")
    {
        return Ok(false);
    }

    let _rules_guard = state.security.waf_rules_update_lock.lock().await;
    // The boot WAF task can overlap other startup work. Recheck after taking
    // the rule-state lock so only one caller applies and marks this patch.
    if state
        .storage
        .store
        .get_string_value(RECOMMENDED_LFI_RULE_PATCH_FLAG_KEY)
        .await?
        .as_deref()
        == Some("1")
    {
        return Ok(false);
    }

    let mut rules_state = read_rules_state(state).await?;
    let changed = rules_state.system_enabled.get(LFI_RULE_FILENAME).copied() != Some(true);
    if changed {
        rules_state
            .system_enabled
            .insert(LFI_RULE_FILENAME.to_string(), true);
        write_rules_state(state, &rules_state).await?;
    }
    state
        .storage
        .store
        .set_string_value(RECOMMENDED_LFI_RULE_PATCH_FLAG_KEY, "1")
        .await?;
    Ok(changed)
}

pub(super) async fn check_and_sync_system_waf_rules_if_needed(
    state: &AppState,
) -> anyhow::Result<Value> {
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
    ensure_waf_directories(state).await?;
    if !state
        .storage
        .store
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

pub(super) async fn has_system_rule_files(state: &AppState) -> anyhow::Result<bool> {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct WafDrainSettings {
    pub(super) enabled: bool,
    pub(super) interval_seconds: u64,
    pub(super) retention_days: i64,
}

impl WafDrainSettings {
    fn from_config(config: &Value) -> Self {
        let waf = config.get("waf");
        Self {
            enabled: waf
                .and_then(|value| value.get("enabled"))
                .and_then(Value::as_bool)
                .unwrap_or(false),
            interval_seconds: normalize_i64(
                waf.and_then(|value| value.get("drain_interval_seconds")),
                DEFAULT_WAF_DRAIN_INTERVAL_SECONDS as i64,
                1,
                60,
            ) as u64,
            retention_days: normalize_i64(
                waf.and_then(|value| value.get("log_retention_days")),
                7,
                1,
                365,
            ),
        }
    }
}

pub(super) fn waf_drain_settings(state: &AppState) -> WafDrainSettings {
    // Store initialization reconciles persisted config; writes, migrations and
    // restores publish a revision-ordered snapshot before returning. Only copy
    // the three drain scalars, releasing the Arc before any network/storage
    // await. Polling must not reload/parse megabytes of unrelated host icons or
    // normalize host policies. Full management reads still reconcile legacy
    // and typed documents through get_config().
    WafDrainSettings::from_config(&state.storage.store.config_snapshot())
}

pub(super) fn waf_drain_schedule(state: &AppState) -> Option<u64> {
    let settings = waf_drain_settings(state);
    settings.enabled.then_some(settings.interval_seconds)
}

pub(super) async fn wait_for_waf_drain(
    state: &AppState,
    updates: &mut tokio::sync::watch::Receiver<u64>,
) -> bool {
    loop {
        // Mark the revision seen BEFORE loading the snapshot: a publication
        // racing with that load must remain observable by changed().
        updates.borrow_and_update();
        let schedule = waf_drain_schedule(state);
        let deadline = async {
            match schedule {
                Some(seconds) => tokio_time::sleep(std::time::Duration::from_secs(seconds)).await,
                None => std::future::pending::<()>().await,
            }
        };
        tokio::pin!(deadline);
        loop {
            tokio::select! {
                biased;
                _ = state.shutdown.cancelled() => return false,
                _ = &mut deadline => return true,
                _ = state.waf_event_drain_reload_notify.notified() => break,
                changed = updates.changed() => {
                    if changed.is_err() {
                        return false;
                    }
                    if waf_drain_schedule(state) != schedule {
                        break;
                    }
                    // Icon/policy/retention-only writes must not postpone an
                    // enabled drain indefinitely. Keep the existing deadline.
                }
            }
        }
    }
}

pub(super) async fn set_waf_rule_enabled(
    state: &AppState,
    input: WafRuleToggleBody,
) -> anyhow::Result<Value> {
    let _rules_guard = state.security.waf_rules_update_lock.lock().await;
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

pub(super) async fn set_recommended_system_rules(state: &AppState) -> anyhow::Result<Value> {
    let _rules_guard = state.security.waf_rules_update_lock.lock().await;
    ensure_waf_directories(state).await?;
    let details = get_waf_details(state).await?;
    let existing_names = details
        .pointer("/system/rules")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|rule| rule.get("filename").and_then(Value::as_str))
        .map(str::to_string)
        .collect::<Vec<_>>();

    let previous_state = read_rules_state(state).await?;
    let mut state_file = previous_state.clone();
    apply_recommended_system_rule_state(&mut state_file, existing_names);

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
    let result = async {
        apply_waf_config_to_gateway(state, &config, "Keep at least one WAF rule enabled").await?;
        get_waf_details(state).await
    }
    .await;
    match result {
        Ok(details) => Ok(details),
        Err(error) => {
            let rollback_file = write_rules_state(state, &previous_state).await;
            let rollback_runtime = if rollback_file.is_ok() {
                apply_waf_config_to_gateway(state, &config, "Keep at least one WAF rule enabled")
                    .await
                    .err()
            } else {
                None
            };
            match (rollback_file.err(), rollback_runtime) {
                (None, None) => Err(error),
                (file_error, runtime_error) => Err(anyhow::anyhow!(
                    "{error}; failed to roll back recommended WAF rules{}{}",
                    file_error
                        .map(|value| format!(": rules state: {value}"))
                        .unwrap_or_default(),
                    runtime_error
                        .map(|value| format!(": gateway runtime: {value}"))
                        .unwrap_or_default()
                )),
            }
        }
    }
}

pub(super) fn apply_recommended_system_rule_state(
    state: &mut WafRulesState,
    filenames: impl IntoIterator<Item = String>,
) {
    state.system_enabled = filenames
        .into_iter()
        .map(|filename| {
            let enabled = is_system_rule_enabled_by_default(&filename);
            (filename, enabled)
        })
        .collect();
    state
        .system_enabled
        .insert(INITIALIZATION_RULE_FILENAME.to_string(), true);
}

pub(super) async fn read_waf_rule_file(
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
    let content =
        fs_utils::read_file_limited(&rule_file_path(state, source, &safe), MAX_RULE_FILE_BYTES)
            .await
            .map_err(|error| {
                if error.kind() == io::ErrorKind::InvalidData {
                    anyhow::anyhow!("WAF rule file is too large: {safe}")
                } else {
                    error.into()
                }
            })
            .and_then(|content| read_utf8_rule_text(&content, &safe))?;
    let mut object = rule.as_object().cloned().unwrap_or_default();
    object.insert("content".to_string(), Value::String(content));
    Ok(Value::Object(object))
}

pub(super) async fn upload_custom_waf_rules(
    state: &AppState,
    input: WafUploadBody,
) -> anyhow::Result<Value> {
    let _rules_guard = state.security.waf_rules_update_lock.lock().await;
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

pub(super) async fn delete_custom_waf_rule(
    state: &AppState,
    filename: &str,
) -> anyhow::Result<Value> {
    let _rules_guard = state.security.waf_rules_update_lock.lock().await;
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

pub(super) async fn drain_waf_events_now(state: &AppState) -> anyhow::Result<Value> {
    let operation = state
        .storage
        .store
        .diagnostics()
        .scope("task", "waf.drain_events");
    let result = drain_waf_events_inner(state).await;
    let rows = result
        .as_ref()
        .ok()
        .and_then(|value| value.get("drained"))
        .and_then(Value::as_u64);
    operation.finish(result.is_ok(), rows);
    result
}

async fn drain_waf_events_inner(state: &AppState) -> anyhow::Result<Value> {
    let _drain_guard = state.security.waf_event_drain_lock.lock().await;
    // Read after acquiring the drain lock so a waiter sees settings published
    // while the preceding drain was running.
    let settings = waf_drain_settings(state);
    if !settings.enabled {
        return Ok(json!({
            "drained": 0,
            "remaining": 0,
            "skipped_reason": "waf_disabled",
        }));
    }

    let response = state
        .gateway
        .client
        .lease_waf_events(DEFAULT_DRAIN_LIMIT)
        .await?;
    let data = go_response_data(response, "Failed to drain WAF events")?;
    let lease_id = data
        .get("lease_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let raw_events = data
        .get("events")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    // A missing lease while events are present means the gateway ran in legacy
    // immediate-drain mode (an older data plane, or the backward-compatible
    // path). The events are already removed from the gateway store, so persist
    // them best-effort without a delivery lease instead of failing the drain.
    if !raw_events.is_empty() && lease_id.is_empty() {
        tracing::debug!(
            event_count = raw_events.len(),
            "drained WAF events without a delivery lease (legacy gateway)"
        );
    }
    let leased_event_count = raw_events.len();
    let events = raw_events
        .into_iter()
        .filter_map(sanitize_event)
        .collect::<Vec<_>>();
    if !events.is_empty()
        && let Err(error) = state
            .storage
            .store
            .persist_waf_events(&events, settings.retention_days)
            .await
    {
        if !lease_id.is_empty()
            && let Err(release_error) = state
                .gateway
                .client
                .release_waf_event_lease(&lease_id)
                .await
        {
            tracing::warn!(%release_error, %lease_id, "failed to release WAF event lease after persistence failure");
        }
        return Err(error.into());
    }
    if !lease_id.is_empty() {
        let acknowledgement = state
            .gateway
            .client
            .acknowledge_waf_event_lease(&lease_id)
            .await?;
        let acknowledged = acknowledgement
            .pointer("/data/acknowledged")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        if acknowledged != leased_event_count as i64 {
            anyhow::bail!(
                "Go backend acknowledged {acknowledged} of {} leased WAF events",
                leased_event_count
            );
        }
    }
    if !events.is_empty() {
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

pub(super) async fn load_waf_config(state: &AppState) -> crate::storage::StorageResult<Value> {
    let config = state.storage.store.get_config().await?;
    Ok(normalize_waf_config_for_full_config(&config, state))
}

pub(crate) fn disabled_hosts_for_config(config: &Value) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut hosts = config
        .get("host_mappings")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|mapping| {
            mapping.get("service_role").and_then(Value::as_str) != Some("auth")
                && !mapping
                    .get("target")
                    .and_then(Value::as_str)
                    .is_some_and(crate::gateway::proxy_config::is_auth_host_mapping_target)
                && mapping.get("waf_enabled").and_then(Value::as_bool) == Some(false)
        })
        .filter_map(|mapping| mapping.get("host").and_then(Value::as_str))
        .map(normalize_disabled_host)
        .filter(|host| !host.is_empty() && seen.insert(host.clone()))
        .collect::<Vec<_>>();
    hosts.sort();
    hosts
}

fn normalize_disabled_host(value: &str) -> String {
    let lowered = value.trim().to_ascii_lowercase();
    let authority = lowered
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(&lowered)
        .split('/')
        .next()
        .unwrap_or("")
        .trim_end_matches('.')
        .to_string();
    if authority.starts_with('[') {
        return authority
            .find(']')
            .map(|end| authority[..=end].to_string())
            .unwrap_or(authority);
    }
    match authority.rsplit_once(':') {
        Some((host, _)) if !host.contains(':') => host.trim_end_matches('.').to_string(),
        _ => authority,
    }
}

fn normalize_waf_config_for_full_config(config: &Value, state: &AppState) -> Value {
    let mut raw = config
        .get("waf")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    raw.insert(
        "disabled_hosts".to_string(),
        Value::Array(
            disabled_hosts_for_config(config)
                .into_iter()
                .map(Value::String)
                .collect(),
        ),
    );
    normalize_fixed_waf_config(Some(&Value::Object(raw)), state)
}

pub(super) fn normalize_fixed_waf_config(value: Option<&Value>, state: &AppState) -> Value {
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
        "private_ip_exempt_enabled": raw
            .and_then(|object| object.get("private_ip_exempt_enabled"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        "block_behavior": if raw
            .and_then(|object| object.get("block_behavior"))
            .and_then(Value::as_str)
            == Some("reset_connection")
        {
            "reset_connection"
        } else {
            "error_page"
        },
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
        "disabled_hosts": raw
            .and_then(|object| object.get("disabled_hosts"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
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

pub(super) async fn apply_waf_config_to_gateway(
    state: &AppState,
    config: &Value,
    empty_rules_message: &str,
) -> anyhow::Result<()> {
    if !config
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        let response = state.gateway.client.set_waf_config(config).await?;
        let _ = go_response_data(response, "Failed to sync WAF config")?;
        return Ok(());
    }
    let rules_state = read_rules_state(state).await?;
    if !has_any_enabled_rule_files(state, &rules_state, None).await? {
        anyhow::bail!("{empty_rules_message}");
    }
    let response = state.gateway.client.reload_waf_rules(config).await?;
    let _ = go_response_data(response, "Failed to load WAF rules")?;
    Ok(())
}

pub(super) async fn sync_common_auth_location_exemptions_to_gateway(
    state: &AppState,
    waf_config: &Value,
) -> anyhow::Result<()> {
    crate::common_auth_locations::sync_common_auth_locations_for_waf(state, waf_config).await
}

pub(super) fn go_response_data(response: Value, fallback: &str) -> anyhow::Result<Value> {
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
