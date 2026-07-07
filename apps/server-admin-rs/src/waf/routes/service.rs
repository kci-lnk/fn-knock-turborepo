use super::*;

pub(super) async fn get_waf_details(state: &AppState) -> anyhow::Result<Value> {
    ensure_waf_directories(state).await?;
    let config = load_waf_config(state).await?;
    let manifest_cache = get_manifest_cache_for_details(state).await?;
    let synced = read_system_sync_state(state).await?;
    let rules_state = read_rules_state(state).await?;
    let system_rules = list_rule_files(state, "system", &manifest_cache, &rules_state).await?;
    let custom_rules = list_rule_files(state, "custom", &manifest_cache, &rules_state).await?;
    let status = match state
        .go_backend
        .request_json(Method::GET, "/api/waf/status", Option::<&Value>::None)
        .await
    {
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
    let mut full_config = state.redis.get_config().await?;
    if !full_config.is_object() {
        full_config = redis_store::default_config();
    }
    let current = normalize_fixed_waf_config(full_config.get("waf"), state);
    let mut next_raw = current.as_object().cloned().unwrap_or_default();
    if let Some(patch) = patch.as_object() {
        for key in [
            "enabled",
            "system_rules_auto_update_enabled",
            "common_location_exempt_enabled",
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
    let next = normalize_fixed_waf_config(Some(&Value::Object(next_raw)), state);
    if let Some(object) = full_config.as_object_mut() {
        object.insert("waf".to_string(), next.clone());
    }
    state.redis.save_config(&full_config).await?;

    let should_apply_to_gateway = has_any_key(
        patch,
        &["enabled", "paranoia_level", "executing_paranoia_level"],
    );
    if should_apply_to_gateway {
        apply_waf_config_to_gateway(
            state,
            &next,
            "Enable WAF after at least one rule is enabled",
        )
        .await?;
    }
    if should_apply_to_gateway || has_any_key(patch, &["common_location_exempt_enabled"]) {
        sync_common_auth_location_exemptions_to_gateway(state, &next).await?;
    }

    get_waf_details(state).await
}

pub(crate) async fn sync_waf_config_to_gateway(
    state: &AppState,
    config: Option<&Value>,
) -> anyhow::Result<Value> {
    let normalized = normalize_fixed_waf_config(config, state);
    apply_waf_config_to_gateway(
        state,
        &normalized,
        "Enable WAF after at least one rule is enabled",
    )
    .await?;
    sync_common_auth_location_exemptions_to_gateway(state, &normalized).await?;
    Ok(normalized)
}

pub(super) async fn sync_waf_on_boot(state: &AppState) -> anyhow::Result<()> {
    ensure_waf_directories(state).await?;
    let config = load_waf_config(state).await?;
    if config
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false)
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
    sync_waf_config_to_gateway(state, Some(&config)).await?;
    Ok(())
}

pub(super) async fn check_and_sync_system_waf_rules_if_needed(
    state: &AppState,
) -> anyhow::Result<Value> {
    ensure_waf_directories(state).await?;
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
    if !state
        .redis
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

pub(super) async fn waf_drain_interval_seconds(state: &AppState) -> u64 {
    state
        .redis
        .get_config()
        .await
        .ok()
        .and_then(|config| {
            config
                .pointer("/waf/drain_interval_seconds")
                .and_then(Value::as_i64)
        })
        .unwrap_or(2)
        .clamp(1, 3600) as u64
}

pub(super) async fn set_waf_rule_enabled(
    state: &AppState,
    input: WafRuleToggleBody,
) -> anyhow::Result<Value> {
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
    let content = read_utf8_rule_text(
        &fs::read(rule_file_path(state, source, &safe)).await?,
        &safe,
    )?;
    let mut object = rule.as_object().cloned().unwrap_or_default();
    object.insert("content".to_string(), Value::String(content));
    Ok(Value::Object(object))
}

pub(super) async fn upload_custom_waf_rules(
    state: &AppState,
    input: WafUploadBody,
) -> anyhow::Result<Value> {
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
    let config = load_waf_config(state).await?;
    let response = state
        .go_backend
        .drain_waf_events(DEFAULT_DRAIN_LIMIT)
        .await?;
    let data = go_response_data(response, "Failed to drain WAF events")?;
    let raw_events = data
        .get("events")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let events = raw_events
        .into_iter()
        .filter_map(sanitize_event)
        .collect::<Vec<_>>();
    if !events.is_empty() {
        state
            .redis
            .persist_waf_events(
                &events,
                config
                    .get("log_retention_days")
                    .and_then(Value::as_i64)
                    .unwrap_or(7),
            )
            .await?;
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

pub(super) async fn load_waf_config(state: &AppState) -> redis::RedisResult<Value> {
    let config = state.redis.get_config().await?;
    Ok(normalize_fixed_waf_config(config.get("waf"), state))
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
        "disabled_hosts": [],
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
        let response = state.go_backend.set_waf_config(config).await?;
        let _ = go_response_data(response, "Failed to sync WAF config")?;
        return Ok(());
    }
    let rules_state = read_rules_state(state).await?;
    if !has_any_enabled_rule_files(state, &rules_state, None).await? {
        anyhow::bail!("{empty_rules_message}");
    }
    let response = state.go_backend.reload_waf_rules(config).await?;
    let _ = go_response_data(response, "Failed to load WAF rules")?;
    Ok(())
}

pub(super) async fn sync_common_auth_location_exemptions_to_gateway(
    state: &AppState,
    waf_config: &Value,
) -> anyhow::Result<()> {
    let runtime = state
        .redis
        .get_string_value("fn_knock:common_auth_locations:runtime")
        .await?
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        .unwrap_or_else(|| json!({}));
    let cidrs = runtime
        .get("cidrs")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let enabled = waf_config
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && waf_config
            .get("common_location_exempt_enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        && runtime
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        && !cidrs.is_empty();
    let payload = json!({
        "enabled": enabled,
        "waf_enabled": enabled,
        "cidrs": if enabled { cidrs } else { Vec::<String>::new() },
        "updated_at": runtime.get("updated_at").cloned().unwrap_or(Value::Null),
    });
    let (status, value) = state
        .go_backend
        .set_common_location_exemptions(&payload)
        .await?;
    if status == reqwest::StatusCode::NOT_FOUND || status == reqwest::StatusCode::NOT_IMPLEMENTED {
        return Ok(());
    }
    if !value
        .get("success")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        anyhow::bail!(
            "{}",
            value
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("Failed to sync common location exemptions")
        );
    }
    Ok(())
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
