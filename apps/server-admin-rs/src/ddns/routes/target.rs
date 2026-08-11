use super::*;

pub(super) async fn targets_overview(
    state: &AppState,
    translator: &Translator,
) -> anyhow::Result<Value> {
    let targets = list_targets(state).await?;
    let items = targets
        .iter()
        .map(|target| target_summary(target, translator))
        .collect::<Vec<_>>();
    let primary_target_id = items
        .iter()
        .find(|item| item.get("isPrimary").and_then(Value::as_bool) == Some(true))
        .and_then(|item| item.get("id").and_then(Value::as_str))
        .map(str::to_string);
    let extra_count = items
        .iter()
        .filter(|item| item.get("isPrimary").and_then(Value::as_bool) != Some(true))
        .count();
    let enabled_extra_count = items
        .iter()
        .filter(|item| {
            item.get("isPrimary").and_then(Value::as_bool) != Some(true)
                && item.get("enabled").and_then(Value::as_bool) == Some(true)
        })
        .count();
    Ok(json!({
        "primaryTargetId": primary_target_id,
        "total": items.len(),
        "extraCount": extra_count,
        "enabledExtraCount": enabled_extra_count,
        "items": items
    }))
}

pub(super) async fn target_detail(
    state: &AppState,
    id: &str,
    translator: &Translator,
) -> anyhow::Result<Option<Value>> {
    let target = list_targets(state)
        .await?
        .into_iter()
        .find(|target| target.meta.id == id);
    Ok(target.map(|target| {
        let mut summary = target_summary(&target, translator);
        if let Some(object) = summary.as_object_mut() {
            object.insert("rawName".to_string(), json!(target.meta.name));
            object.insert("config".to_string(), json!(target.config));
        }
        summary
    }))
}

pub(super) async fn create_ddns_target(
    state: &AppState,
    body: TargetBody,
    translator: &Translator,
) -> anyhow::Result<Value> {
    let provider = normalize_provider_name(&body.provider)
        .ok_or_else(|| anyhow::anyhow!("Unknown DDNS provider: {}", body.provider))?;
    let config = normalize_and_validate_config(&provider, body.config.unwrap_or_default())?;
    assert_no_duplicate_target(state, &provider, &config, None).await?;
    ensure_primary_initialized(state).await?;
    let targets = list_targets(state).await?;
    let sort_order = targets
        .iter()
        .map(|target| target.meta.sort_order)
        .max()
        .unwrap_or(0)
        + 1;
    let now = time_utils::now_iso();
    let record = DDNSTargetRecord {
        meta: DDNSTargetMeta {
            id: uuid::Uuid::new_v4().to_string(),
            name: body.name.unwrap_or_default().trim().to_string(),
            is_primary: false,
            enabled: body.enabled.unwrap_or(true),
            provider: Some(provider.clone()),
            created_at: now.clone(),
            updated_at: now,
            sort_order,
        },
        config,
        last_ip: empty_last_ip(),
        selection_anchor: empty_last_ip(),
        last_check: empty_last_check(),
    };
    save_target_record(state, &record).await?;
    Ok(detail_from_record(record, translator))
}

pub(super) async fn update_ddns_target(
    state: &AppState,
    id: &str,
    body: TargetBody,
    translator: &Translator,
) -> anyhow::Result<Value> {
    let provider = normalize_provider_name(&body.provider)
        .ok_or_else(|| anyhow::anyhow!("Unknown DDNS provider: {}", body.provider))?;
    let config = normalize_and_validate_config(&provider, body.config.unwrap_or_default())?;
    let mut target = find_target_or_err(state, id).await?;
    assert_no_duplicate_target(state, &provider, &config, Some(id)).await?;
    let should_reset = comparable_config_key(target.meta.provider.as_deref(), &target.config)
        != comparable_config_key(Some(&provider), &config);
    target.meta.name = body
        .name
        .map(|value| value.trim().to_string())
        .unwrap_or(target.meta.name);
    target.meta.provider = Some(provider.clone());
    target.meta.enabled = if target.meta.is_primary {
        true
    } else {
        body.enabled.unwrap_or(target.meta.enabled)
    };
    target.meta.updated_at = time_utils::now_iso();
    target.config = config;
    write_config_after_runtime_reset(
        should_reset,
        reset_target_runtime_state(state, &target.meta),
        save_target_record(state, &target),
    )
    .await?;
    if target.meta.is_primary {
        save_legacy_config_draft(state, &provider, &target.config).await?;
        mirror_primary_provider(state, Some(&provider)).await?;
    }
    Ok(detail_from_record(
        find_target_or_err(state, id).await?,
        translator,
    ))
}

pub(super) async fn delete_ddns_target(state: &AppState, id: &str) -> anyhow::Result<()> {
    let target = find_target_or_err(state, id).await?;
    if target.meta.is_primary {
        return Err(anyhow::anyhow!("Primary DDNS target cannot be deleted"));
    }
    state
        .storage
        .store
        .srem_string_member(DDNS_TARGET_IDS, id)
        .await?;
    state
        .storage
        .store
        .delete_keys(&[
            target_meta_key(id),
            target_config_key(id),
            target_last_ip_key(id),
            target_selection_anchor_key(id),
            target_interface_recovery_key(id),
            target_last_check_key(id),
        ])
        .await?;
    Ok(())
}

pub(super) async fn set_ddns_target_enabled(
    state: &AppState,
    id: &str,
    enabled: bool,
) -> anyhow::Result<()> {
    let mut target = find_target_or_err(state, id).await?;
    if target.meta.is_primary && !enabled {
        return Err(anyhow::anyhow!("Primary DDNS target cannot be disabled"));
    }
    if enabled && let Some(provider) = target.meta.provider.as_deref() {
        validated_ddns_domain_targets(provider, &target.config)?;
    }
    target.meta.enabled = if target.meta.is_primary {
        true
    } else {
        enabled
    };
    target.meta.updated_at = time_utils::now_iso();
    save_target_meta(state, &target.meta).await
}

pub(super) async fn set_primary_provider(state: &AppState, provider: &str) -> anyhow::Result<()> {
    let provider = normalize_provider_name(provider)
        .ok_or_else(|| anyhow::anyhow!("Unknown DDNS provider: {provider}"))?;
    let mut primary = primary_target(state).await?;
    if primary.meta.provider.as_deref() == Some(provider.as_str()) {
        validated_ddns_domain_targets(&provider, &primary.config)?;
        mirror_primary_provider(state, Some(&provider)).await?;
        return Ok(());
    }
    let next_config = normalize_and_validate_config(
        &provider,
        read_legacy_config_draft(state, &provider).await?,
    )?;
    assert_no_duplicate_target(state, &provider, &next_config, Some(&primary.meta.id)).await?;
    if let Some(previous) = primary.meta.provider.as_deref()
        && previous != provider
    {
        save_legacy_config_draft(state, previous, &primary.config).await?;
    }
    let should_reset = comparable_config_key(primary.meta.provider.as_deref(), &primary.config)
        != comparable_config_key(Some(&provider), &next_config);
    primary.meta.provider = Some(provider.clone());
    primary.meta.enabled = true;
    primary.meta.updated_at = time_utils::now_iso();
    primary.config = next_config;
    write_config_after_runtime_reset(
        should_reset,
        reset_target_runtime_state(state, &primary.meta),
        save_target_record(state, &primary),
    )
    .await?;
    mirror_primary_provider(state, Some(&provider)).await
}

pub(super) async fn save_primary_config(
    state: &AppState,
    provider: &str,
    config: HashMap<String, String>,
) -> anyhow::Result<()> {
    let provider = normalize_provider_name(provider)
        .ok_or_else(|| anyhow::anyhow!("Unknown DDNS provider: {provider}"))?;
    let normalized = normalize_and_validate_config(&provider, config)?;
    let mut primary = primary_target(state).await?;
    if primary.meta.provider.as_deref() == Some(provider.as_str()) {
        assert_no_duplicate_target(state, &provider, &normalized, Some(&primary.meta.id)).await?;
        let should_reset = comparable_config_key(primary.meta.provider.as_deref(), &primary.config)
            != comparable_config_key(Some(&provider), &normalized);
        primary.config = normalized.clone();
        write_config_after_runtime_reset(
            should_reset,
            reset_target_runtime_state(state, &primary.meta),
            save_target_config(state, &primary.meta, &normalized),
        )
        .await?;
        save_legacy_config_draft(state, &provider, &normalized).await?;
    } else {
        save_legacy_config_draft(state, &provider, &normalized).await?;
    }
    Ok(())
}

pub(super) async fn primary_target(state: &AppState) -> anyhow::Result<DDNSTargetRecord> {
    ensure_primary_initialized(state).await?;
    list_targets(state)
        .await?
        .into_iter()
        .find(|target| target.meta.is_primary)
        .ok_or_else(|| anyhow::anyhow!("Failed to initialize primary DDNS target"))
}

pub(super) async fn ensure_primary_initialized(state: &AppState) -> anyhow::Result<()> {
    let primary_id = state
        .storage
        .store
        .get_string_value(DDNS_PRIMARY_TARGET_ID)
        .await?
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| PRIMARY_TARGET_ID.to_string());
    if !state
        .storage
        .store
        .hgetall_string_map(&target_meta_key(&primary_id))
        .await?
        .is_empty()
    {
        state
            .storage
            .store
            .sadd_string_member(DDNS_TARGET_IDS, &primary_id)
            .await?;
        return Ok(());
    }
    let legacy = read_legacy_primary_target(state).await?;
    save_target_record(state, &legacy).await?;
    let mut anchor = HashMap::new();
    for field in ["ipv4", "ipv6", "updated_at"] {
        if let Some(value) = legacy.selection_anchor.get(field).and_then(Value::as_str) {
            anchor.insert(field.to_string(), value.to_string());
        }
    }
    if !anchor.is_empty() {
        state
            .storage
            .store
            .replace_hash_string_map(&target_selection_anchor_key(&legacy.meta.id), &anchor)
            .await?;
    }
    Ok(())
}

pub(super) async fn find_target_or_err(
    state: &AppState,
    id: &str,
) -> anyhow::Result<DDNSTargetRecord> {
    ensure_primary_initialized(state).await?;
    list_targets(state)
        .await?
        .into_iter()
        .find(|target| target.meta.id == id)
        .ok_or_else(|| anyhow::anyhow!("DDNS target not found"))
}

pub(super) async fn save_target_record(
    state: &AppState,
    record: &DDNSTargetRecord,
) -> anyhow::Result<()> {
    save_target_meta(state, &record.meta).await?;
    save_target_config(state, &record.meta, &record.config).await
}

pub(super) async fn save_target_meta(
    state: &AppState,
    meta: &DDNSTargetMeta,
) -> anyhow::Result<()> {
    let mut payload = HashMap::new();
    payload.insert("name".to_string(), meta.name.trim().to_string());
    payload.insert(
        "is_primary".to_string(),
        if meta.is_primary { "true" } else { "false" }.to_string(),
    );
    payload.insert(
        "enabled".to_string(),
        if meta.enabled { "true" } else { "false" }.to_string(),
    );
    payload.insert(
        "provider".to_string(),
        meta.provider.clone().unwrap_or_default(),
    );
    payload.insert("created_at".to_string(), meta.created_at.clone());
    payload.insert("updated_at".to_string(), meta.updated_at.clone());
    payload.insert("sort_order".to_string(), meta.sort_order.to_string());
    state
        .storage
        .store
        .replace_hash_string_map(&target_meta_key(&meta.id), &payload)
        .await?;
    state
        .storage
        .store
        .sadd_string_member(DDNS_TARGET_IDS, &meta.id)
        .await?;
    if meta.is_primary {
        state
            .storage
            .store
            .set_string_value(DDNS_PRIMARY_TARGET_ID, &meta.id)
            .await?;
    }
    Ok(())
}

pub(super) async fn save_target_config(
    state: &AppState,
    meta: &DDNSTargetMeta,
    config: &HashMap<String, String>,
) -> anyhow::Result<()> {
    let provider = meta.provider.as_deref();
    let prepared = prepare_config_for_storage(provider, normalize_config_map(provider, config));
    state
        .storage
        .store
        .replace_hash_string_map(&target_config_key(&meta.id), &prepared)
        .await?;
    if meta.is_primary
        && let Some(provider) = provider
    {
        save_legacy_config_draft(state, provider, &prepared).await?;
    }
    Ok(())
}

pub(super) async fn save_legacy_config_draft(
    state: &AppState,
    provider: &str,
    config: &HashMap<String, String>,
) -> anyhow::Result<()> {
    let prepared =
        prepare_config_for_storage(Some(provider), normalize_config_map(Some(provider), config));
    state
        .storage
        .store
        .replace_hash_string_map(
            &(DDNS_LEGACY_CONFIG_PREFIX.to_string() + provider),
            &prepared,
        )
        .await?;
    Ok(())
}

pub(super) async fn read_legacy_config_draft(
    state: &AppState,
    provider: &str,
) -> anyhow::Result<HashMap<String, String>> {
    let raw = state
        .storage
        .store
        .hgetall_string_map(&(DDNS_LEGACY_CONFIG_PREFIX.to_string() + provider))
        .await?;
    Ok(normalize_config(provider, raw))
}

pub(super) async fn mirror_primary_provider(
    state: &AppState,
    provider: Option<&str>,
) -> anyhow::Result<()> {
    if let Some(provider) = provider.filter(|value| !value.trim().is_empty()) {
        state
            .storage
            .store
            .set_string_value(DDNS_LEGACY_PROVIDER, provider)
            .await?;
    } else {
        state.storage.store.delete_key(DDNS_LEGACY_PROVIDER).await?;
    }
    Ok(())
}

pub(super) async fn reset_target_runtime_state(
    state: &AppState,
    meta: &DDNSTargetMeta,
) -> anyhow::Result<()> {
    state
        .storage
        .store
        .replace_hash_string_map(&target_last_ip_key(&meta.id), &HashMap::new())
        .await?;
    state
        .storage
        .store
        .replace_hash_string_map(&target_last_check_key(&meta.id), &HashMap::new())
        .await?;
    state
        .storage
        .store
        .replace_hash_string_map(&target_interface_recovery_key(&meta.id), &HashMap::new())
        .await?;
    if meta.is_primary {
        state
            .storage
            .store
            .replace_hash_string_map(DDNS_LEGACY_LAST_IP, &HashMap::new())
            .await?;
        state
            .storage
            .store
            .replace_hash_string_map(DDNS_LEGACY_LAST_CHECK, &HashMap::new())
            .await?;
    }
    Ok(())
}

pub(super) async fn write_config_after_runtime_reset<T, Reset, Write>(
    should_reset: bool,
    reset: Reset,
    write: Write,
) -> anyhow::Result<T>
where
    Reset: Future<Output = anyhow::Result<()>>,
    Write: Future<Output = anyhow::Result<T>>,
{
    if should_reset {
        reset.await?;
    }
    write.await
}

pub(super) async fn assert_no_duplicate_target(
    state: &AppState,
    provider: &str,
    config: &HashMap<String, String>,
    except_id: Option<&str>,
) -> anyhow::Result<()> {
    let next = duplicate_key(provider, config);
    let next_domains = ddns_domain_target_set(provider, config);
    if next.is_empty() && next_domains.is_none() {
        return Ok(());
    }
    for target in list_targets(state).await? {
        if except_id == Some(target.meta.id.as_str()) {
            continue;
        }
        let target_provider = target.meta.provider.as_deref().unwrap_or("");
        if target_provider != provider {
            continue;
        }
        let domains_overlap = next_domains.as_ref().is_some_and(|next_domains| {
            ddns_domain_target_set(target_provider, &target.config)
                .is_some_and(|existing| !next_domains.is_disjoint(&existing))
        });
        let legacy_duplicate =
            !next.is_empty() && duplicate_key(target_provider, &target.config) == next;
        if domains_overlap || legacy_duplicate {
            return Err(anyhow::anyhow!("Duplicate DDNS target"));
        }
    }
    Ok(())
}

pub(super) fn detail_from_record(record: DDNSTargetRecord, translator: &Translator) -> Value {
    let mut summary = target_summary(&record, translator);
    if let Some(object) = summary.as_object_mut() {
        object.insert("rawName".to_string(), json!(record.meta.name));
        object.insert("config".to_string(), json!(record.config));
    }
    summary
}

pub(super) async fn ddns_error_response_from_state(
    state: &AppState,
    error: anyhow::Error,
) -> Response {
    let translator = Translator::from_state(state).await;
    ddns_error_response(&translator, error)
}
