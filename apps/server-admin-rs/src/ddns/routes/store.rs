use super::*;

pub(super) async fn build_ddns_status(
    state: &AppState,
    translator: &Translator,
) -> anyhow::Result<Value> {
    let enabled = state.store.get_string_value(DDNS_ENABLED).await?.as_deref() == Some("true");
    let settings = parse_settings(
        state
            .store
            .get_string_value(DDNS_SETTINGS)
            .await?
            .as_deref(),
    );
    let targets = list_targets(state).await?;
    let primary = targets
        .iter()
        .find(|target| target.meta.is_primary)
        .or_else(|| targets.first())
        .cloned()
        .unwrap_or_else(default_primary_target);
    let summaries = targets
        .iter()
        .map(|target| target_summary(target, translator))
        .collect::<Vec<_>>();
    let primary_target_id = summaries
        .iter()
        .find(|item| item.get("isPrimary").and_then(Value::as_bool) == Some(true))
        .and_then(|item| item.get("id").and_then(Value::as_str))
        .map(str::to_string);
    let extra_count = summaries
        .iter()
        .filter(|item| item.get("isPrimary").and_then(Value::as_bool) != Some(true))
        .count();
    let enabled_extra_count = summaries
        .iter()
        .filter(|item| {
            item.get("isPrimary").and_then(Value::as_bool) != Some(true)
                && item.get("enabled").and_then(Value::as_bool) == Some(true)
        })
        .count();

    Ok(json!({
        "enabled": enabled,
        "provider": primary.meta.provider,
        "updateIntervalMinutes": settings.get("updateIntervalMinutes").cloned().unwrap_or(json!(10)),
        "publicCheckSources": settings.get("publicCheckSources").cloned().unwrap_or_else(default_public_check_sources),
        "defaultPublicCheckSources": settings.get("defaultPublicCheckSources").cloned().unwrap_or_else(default_public_check_sources),
        "httpTransport": settings.get("httpTransport").cloned().unwrap_or(json!("curl")),
        "updateScope": normalize_update_scope(primary.config.get("update_scope").map(String::as_str)),
        "ipSource": normalize_ip_source(primary.config.get("ip_source").map(String::as_str)),
        "networkInterface": normalize_network_interface(primary.config.get("network_interface").map(String::as_str)),
        "lastIP": primary.last_ip,
        "lastCheck": primary.last_check,
        "primaryTargetId": primary_target_id,
        "extraTargetCount": extra_count,
        "enabledExtraTargetCount": enabled_extra_count,
        "targets": summaries
    }))
}

pub(super) async fn list_targets(state: &AppState) -> anyhow::Result<Vec<DDNSTargetRecord>> {
    let primary_id = state
        .store
        .get_string_value(DDNS_PRIMARY_TARGET_ID)
        .await?
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| PRIMARY_TARGET_ID.to_string());
    let mut ids = BTreeSet::new();
    ids.insert(primary_id.clone());
    for id in state.store.smembers_strings(DDNS_TARGET_IDS).await? {
        let id = id.trim();
        if !id.is_empty() {
            ids.insert(id.to_string());
        }
    }

    let mut targets = Vec::new();
    for id in ids {
        if let Some(target) = read_target(state, &id, id == primary_id).await? {
            targets.push(target);
        }
    }
    if targets.iter().all(|target| !target.meta.is_primary) {
        targets.push(read_legacy_primary_target(state).await?);
    }
    targets.sort_by(compare_targets);
    Ok(targets)
}

pub(super) async fn read_target(
    state: &AppState,
    id: &str,
    primary_hint: bool,
) -> anyhow::Result<Option<DDNSTargetRecord>> {
    let meta_key = target_meta_key(id);
    let meta_hash = state.store.hgetall_string_map(&meta_key).await?;
    if meta_hash.is_empty() {
        if id == PRIMARY_TARGET_ID || primary_hint {
            return Ok(Some(read_legacy_primary_target(state).await?));
        }
        return Ok(None);
    }
    let meta = parse_target_meta(id, &meta_hash, primary_hint);
    let config = state
        .store
        .hgetall_string_map(&target_config_key(id))
        .await?;
    let last_ip = parse_last_ip(
        &state
            .store
            .hgetall_string_map(&target_last_ip_key(id))
            .await?,
    );
    let last_check = parse_last_check(
        &state
            .store
            .hgetall_string_map(&target_last_check_key(id))
            .await?,
    );
    Ok(Some(DDNSTargetRecord {
        meta,
        config,
        last_ip,
        last_check,
    }))
}

pub(super) async fn read_legacy_primary_target(
    state: &AppState,
) -> anyhow::Result<DDNSTargetRecord> {
    let provider = state
        .store
        .get_string_value(DDNS_LEGACY_PROVIDER)
        .await?
        .and_then(|value| normalize_provider_name(&value));
    let config = if let Some(provider) = provider.as_deref() {
        state
            .store
            .hgetall_string_map(&(DDNS_LEGACY_CONFIG_PREFIX.to_string() + provider))
            .await?
    } else {
        HashMap::new()
    };
    let last_ip = parse_last_ip(&state.store.hgetall_string_map(DDNS_LEGACY_LAST_IP).await?);
    let last_check = parse_last_check(
        &state
            .store
            .hgetall_string_map(DDNS_LEGACY_LAST_CHECK)
            .await?,
    );
    let now = time_utils::now_iso();
    Ok(DDNSTargetRecord {
        meta: DDNSTargetMeta {
            id: PRIMARY_TARGET_ID.to_string(),
            name: PRIMARY_TARGET_NAME.to_string(),
            is_primary: true,
            enabled: true,
            provider,
            created_at: now.clone(),
            updated_at: now,
            sort_order: 0,
        },
        config,
        last_ip,
        last_check,
    })
}

pub(super) fn default_primary_target() -> DDNSTargetRecord {
    let now = time_utils::now_iso();
    DDNSTargetRecord {
        meta: DDNSTargetMeta {
            id: PRIMARY_TARGET_ID.to_string(),
            name: PRIMARY_TARGET_NAME.to_string(),
            is_primary: true,
            enabled: true,
            provider: None,
            created_at: now.clone(),
            updated_at: now,
            sort_order: 0,
        },
        config: HashMap::new(),
        last_ip: empty_last_ip(),
        last_check: empty_last_check(),
    }
}

pub(super) fn parse_target_meta(
    id: &str,
    data: &HashMap<String, String>,
    primary_hint: bool,
) -> DDNSTargetMeta {
    let now = time_utils::now_iso();
    let is_primary = data.get("is_primary").map(String::as_str) == Some("true")
        || id == PRIMARY_TARGET_ID
        || primary_hint;
    DDNSTargetMeta {
        id: id.to_string(),
        name: data
            .get("name")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| {
                if is_primary {
                    PRIMARY_TARGET_NAME.to_string()
                } else {
                    String::new()
                }
            }),
        is_primary,
        enabled: if is_primary {
            true
        } else {
            data.get("enabled").map(String::as_str) != Some("false")
        },
        provider: data
            .get("provider")
            .and_then(|value| normalize_provider_name(value)),
        created_at: data
            .get("created_at")
            .cloned()
            .unwrap_or_else(|| now.clone()),
        updated_at: data
            .get("updated_at")
            .or_else(|| data.get("created_at"))
            .cloned()
            .unwrap_or(now),
        sort_order: data
            .get("sort_order")
            .and_then(|value| value.parse::<i64>().ok())
            .unwrap_or(if is_primary { 0 } else { 1 }),
    }
}

pub(super) fn target_summary(target: &DDNSTargetRecord, translator: &Translator) -> Value {
    let provider_label = provider_label(target.meta.provider.as_deref(), translator);
    let domain_summary =
        domain_summary(target.meta.provider.as_deref(), &target.config, translator);
    let name = if !target.meta.name.trim().is_empty() {
        target.meta.name.trim().to_string()
    } else if target.meta.is_primary {
        ddns_text(translator, "primaryDomainName", &[])
    } else if !domain_summary.is_empty() {
        domain_summary.clone()
    } else {
        provider_label.clone()
    };

    json!({
        "id": target.meta.id,
        "name": name,
        "isPrimary": target.meta.is_primary,
        "enabled": if target.meta.is_primary { true } else { target.meta.enabled },
        "provider": target.meta.provider,
        "updateScope": normalize_update_scope(target.config.get("update_scope").map(String::as_str)),
        "providerLabel": provider_label,
        "domainSummary": domain_summary,
        "createdAt": target.meta.created_at,
        "updatedAt": target.meta.updated_at,
        "sortOrder": target.meta.sort_order,
        "lastIP": target.last_ip,
        "lastCheck": target.last_check
    })
}

pub(super) fn compare_targets(
    left: &DDNSTargetRecord,
    right: &DDNSTargetRecord,
) -> std::cmp::Ordering {
    match (left.meta.is_primary, right.meta.is_primary) {
        (true, false) => return std::cmp::Ordering::Less,
        (false, true) => return std::cmp::Ordering::Greater,
        _ => {}
    }
    left.meta
        .sort_order
        .cmp(&right.meta.sort_order)
        .then_with(|| left.meta.created_at.cmp(&right.meta.created_at))
        .then_with(|| left.meta.id.cmp(&right.meta.id))
}
