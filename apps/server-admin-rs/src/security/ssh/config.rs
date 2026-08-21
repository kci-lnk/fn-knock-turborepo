use super::*;

pub(crate) async fn migrate_ssh_ipset_on_boot(state: &AppState) -> anyhow::Result<()> {
    let raw = state.storage.store.get_json_value(RUNTIME_KEY).await?;
    let runtime = normalize_runtime(raw);
    let enabled = runtime.get("enabled").and_then(Value::as_bool) == Some(true);
    let policy = policy_from_runtime(&runtime)?.into_current_format();
    let compact = compact_runtime(enabled, &policy, runtime.get("updated_at").cloned());
    state
        .storage
        .store
        .set_json_value(RUNTIME_KEY, &compact)
        .await?;
    state.security.ipsets.publish(
        SSH_ALLOWED_IPSET_KEY,
        (enabled && policy.range_count() > 0).then_some(policy),
    );
    Ok(())
}

pub(super) async fn ssh_security_details(
    state: &AppState,
) -> Result<Value, crate::storage::StorageError> {
    let translator = Translator::from_state(state).await;
    let config = load_config(state).await?;
    let runtime = load_runtime(state).await?;
    let active_block_count = active_blocks(state).await?.len();
    let ports = resolve_ssh_ports();
    let availability = ssh_security_availability(state, &translator);
    Ok(json!({
        "config": config,
        "summary": {
            "configured": config.get("configured_at").is_some_and(|value| !value.is_null()),
            "enabled": config.get("enabled").and_then(Value::as_bool).unwrap_or(false),
            "allowed_cidr_count": runtime.get("source_cidr_count").and_then(Value::as_u64).unwrap_or_default(),
            "allowed_range_count": runtime.get("range_count").and_then(Value::as_u64).unwrap_or_default(),
            "active_block_count": active_block_count,
            "ssh_ports": ports,
            "log_source": availability.log_source,
            "available": availability.available,
            "unavailable_reason": availability.reason,
            "updated_at": config.get("updated_at").cloned().unwrap_or(Value::Null)
        }
    }))
}

pub(super) async fn update_ssh_security_config(
    state: &AppState,
    body: Value,
    translator: &Translator,
) -> Result<Value, SshError> {
    let previous = load_config(state).await?;
    let (config, runtime) = compile_config_patch(state, &body, &previous, translator).await?;
    if config.get("enabled").and_then(Value::as_bool) == Some(true) {
        let availability = ssh_security_availability(state, translator);
        if !availability.available {
            return Err(SshError::Runtime(availability.reason));
        }
    }
    let mut all = state.storage.store.get_config().await?;
    if let Some(object) = all.as_object_mut() {
        object.insert("ssh_security".to_string(), config.clone());
    }
    state.storage.store.save_config(&all).await?;
    state
        .storage
        .store
        .set_json_value(RUNTIME_KEY, &runtime)
        .await?;
    publish_runtime_policy(state, &runtime)
        .map_err(|error| SshError::Runtime(error.to_string()))?;
    apply_ssh_security_config_once(state, &config, &runtime)
        .await
        .map_err(|error| SshError::Runtime(error.to_string()))?;
    state.request_ssh_security_maintenance();
    ssh_security_details(state).await.map_err(SshError::Storage)
}

pub(super) async fn load_config(state: &AppState) -> crate::storage::StorageResult<Value> {
    let config = state.storage.store.get_config().await?;
    Ok(normalize_config(config.get("ssh_security").cloned()))
}

pub(super) async fn load_runtime(state: &AppState) -> crate::storage::StorageResult<Value> {
    Ok(normalize_runtime(
        state.storage.store.get_json_value(RUNTIME_KEY).await?,
    ))
}

pub(crate) fn normalize_config(value: Option<Value>) -> Value {
    let raw = value
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    json!({
        "enabled": raw.get("enabled").and_then(Value::as_bool).unwrap_or(false),
        "window_minutes": int_field(&raw, "window_minutes", 10, 1, 24 * 60),
        "failed_login_threshold": int_field(&raw, "failed_login_threshold", 5, 1, 1000),
        "block_duration_value": int_field(&raw, "block_duration_value", 1, 1, 365),
        "block_duration_unit": normalize_duration_unit(raw.get("block_duration_unit").and_then(Value::as_str)),
        "allowed_regions": normalize_allowed_regions(raw.get("allowed_regions")),
        "custom_cidrs": normalize_cidrs(raw.get("custom_cidrs")),
        "configured_at": normalize_timestamp(raw.get("configured_at")),
        "updated_at": normalize_timestamp(raw.get("updated_at"))
    })
}

pub(super) fn normalize_runtime(value: Option<Value>) -> Value {
    let raw = value
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    let mut runtime = json!({
        "enabled": raw.get("enabled").and_then(Value::as_bool).unwrap_or(false),
        "allowed_cidrs": normalize_cidrs(raw.get("allowed_cidrs")),
        "policy_id": raw.get("policy_id").cloned().unwrap_or(Value::Null),
        "source_cidr_count": raw.get("source_cidr_count").cloned().unwrap_or_else(|| json!(0)),
        "range_count": raw.get("range_count").cloned().unwrap_or_else(|| json!(0)),
        "policy": raw.get("policy").cloned().unwrap_or(Value::Null),
        "updated_at": normalize_timestamp(raw.get("updated_at"))
    });
    if runtime.get("policy").is_some_and(Value::is_null)
        && runtime
            .get("allowed_cidrs")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty)
    {
        runtime
            .as_object_mut()
            .expect("normalized runtime object")
            .remove("allowed_cidrs");
    }
    runtime
}

pub(super) async fn compile_config_patch(
    state: &AppState,
    body: &Value,
    previous: &Value,
    translator: &Translator,
) -> Result<(Value, Value), SshError> {
    let raw = body.as_object().cloned().unwrap_or_default();
    let now = time_utils::now_iso();
    let enabled = raw
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or_else(|| {
            previous
                .get("enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        });
    let policy_inputs_changed =
        raw.contains_key("allowed_regions") || raw.contains_key("custom_cidrs");
    let custom_cidrs = if raw.contains_key("custom_cidrs") {
        validate_cidrs(raw.get("custom_cidrs"), translator)?;
        normalize_cidrs(raw.get("custom_cidrs"))
    } else {
        normalize_cidrs(previous.get("custom_cidrs"))
    };
    let previous_runtime = load_runtime(state).await?;
    let previous_policy = policy_from_runtime(&previous_runtime)
        .map_err(|error| SshError::Runtime(error.to_string()))?;
    let has_semantic_allowlist = previous
        .get("allowed_regions")
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty())
        || custom_cidrs
            .as_array()
            .is_some_and(|items| !items.is_empty());
    let reuse_policy =
        !policy_inputs_changed && (!has_semantic_allowlist || previous_policy.range_count() > 0);
    let allowed_regions = if reuse_policy {
        ResolvedAllowedRegions {
            selections: previous
                .get("allowed_regions")
                .cloned()
                .unwrap_or_else(|| json!([])),
            policy: previous_policy.clone(),
        }
    } else {
        resolve_allowed_regions(state, &raw, previous, translator).await?
    };
    let config = json!({
        "enabled": enabled,
        "window_minutes": int_field_or_previous(&raw, previous, "window_minutes", 10, 1, 24 * 60),
        "failed_login_threshold": int_field_or_previous(&raw, previous, "failed_login_threshold", 5, 1, 1000),
        "block_duration_value": int_field_or_previous(&raw, previous, "block_duration_value", 1, 1, 365),
        "block_duration_unit": raw.get("block_duration_unit").and_then(Value::as_str).map(|value| normalize_duration_unit(Some(value))).unwrap_or_else(|| previous.get("block_duration_unit").and_then(Value::as_str).unwrap_or("day").to_string()),
        "allowed_regions": allowed_regions.selections,
        "custom_cidrs": custom_cidrs,
        "configured_at": previous.get("configured_at").cloned().filter(|value| !value.is_null()).unwrap_or_else(|| Value::String(now.clone())),
        "updated_at": now
    });
    let runtime = if reuse_policy {
        compact_runtime(
            enabled,
            &previous_policy,
            Some(Value::String(time_utils::now_iso())),
        )
    } else {
        build_runtime_from_config(&config, allowed_regions.policy).map_err(SshError::BadRequest)?
    };
    Ok((config, runtime))
}

pub(super) async fn resolve_allowed_regions(
    state: &AppState,
    raw: &Map<String, Value>,
    previous: &Value,
    translator: &Translator,
) -> Result<ResolvedAllowedRegions, SshError> {
    let source = if raw.contains_key("allowed_regions") {
        raw.get("allowed_regions")
    } else {
        previous.get("allowed_regions")
    };
    let Some(items) = source.and_then(Value::as_array) else {
        return Ok(ResolvedAllowedRegions {
            selections: json!([]),
            policy: compile_ip_set(std::iter::empty::<&str>()).map_err(SshError::BadRequest)?,
        });
    };

    let mut seen = HashSet::new();
    let mut selections = Vec::new();
    let mut policies = Vec::new();
    for item in items {
        let Some(query) = parse_allowed_region(item, translator)? else {
            continue;
        };
        let key = query.key();
        if !seen.insert(key) {
            continue;
        }
        let lookup = crate::cidr::lookup_region(state, &query)
            .await
            .map_err(|error| {
                SshError::BadRequest(crate::cidr::localize_error(translator, &error.to_string()))
            })?;
        selections.push(serde_json::to_value(lookup.selection).unwrap_or(Value::Null));
        policies.push(lookup.policy);
    }
    Ok(ResolvedAllowedRegions {
        selections: Value::Array(selections),
        policy: crate::cidr::union_ip_sets(policies.iter()),
    })
}

pub(super) fn parse_allowed_region(
    item: &Value,
    translator: &Translator,
) -> Result<Option<CidrRegionQuery>, SshError> {
    let province = item
        .get("province")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if province.is_empty() {
        return Ok(None);
    }
    let query_city = item
        .get("query_city")
        .or_else(|| item.get("queryCity"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let operator = CidrOperator::parse_value(item.get("operator")).map_err(|message| {
        SshError::BadRequest(crate::cidr::localize_error(translator, &message))
    })?;
    Ok(Some(CidrRegionQuery::new(province, query_city, operator)))
}

pub(super) fn build_runtime_from_config(
    config: &Value,
    resolved_region_policy: CompiledIpSet,
) -> Result<Value, String> {
    let custom_cidrs = config
        .get("custom_cidrs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<Vec<_>>();
    let custom_policy = compile_ip_set(&custom_cidrs)?;
    let policy = crate::cidr::union_ip_sets([&resolved_region_policy, &custom_policy]);
    Ok(compact_runtime(
        config
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        &policy,
        Some(Value::String(time_utils::now_iso())),
    ))
}

pub(super) fn policy_from_runtime(runtime: &Value) -> anyhow::Result<CompiledIpSet> {
    CompiledIpSet::from_runtime_or_legacy_cidrs(runtime, "allowed_cidrs")
        .map_err(anyhow::Error::msg)
}

pub(super) fn compact_runtime(
    enabled: bool,
    policy: &CompiledIpSet,
    updated_at: Option<Value>,
) -> Value {
    let has_policy = policy.range_count() > 0;
    let mut runtime = json!({
        "enabled": enabled,
        "updated_at": updated_at.unwrap_or_else(|| Value::String(time_utils::now_iso())),
    });
    CompiledIpSet::apply_runtime_envelope(&mut runtime, has_policy.then_some(policy));
    runtime
}

fn publish_runtime_policy(state: &AppState, runtime: &Value) -> anyhow::Result<()> {
    let enabled = runtime.get("enabled").and_then(Value::as_bool) == Some(true);
    let policy = policy_from_runtime(runtime)?;
    state.security.ipsets.publish(
        SSH_ALLOWED_IPSET_KEY,
        (enabled && policy.range_count() > 0).then_some(policy),
    );
    Ok(())
}
