use super::*;

pub(super) async fn ssh_security_details(state: &AppState) -> Result<Value, redis::RedisError> {
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
            "allowed_cidr_count": runtime.get("allowed_cidrs").and_then(Value::as_array).map(|items| items.len()).unwrap_or_default(),
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
    let mut all = state.redis.get_config().await?;
    if let Some(object) = all.as_object_mut() {
        object.insert("ssh_security".to_string(), config.clone());
    }
    state.redis.save_config(&all).await?;
    state.redis.set_json_value(RUNTIME_KEY, &runtime).await?;
    apply_ssh_security_config_once(state, &config, &runtime)
        .await
        .map_err(|error| SshError::Runtime(error.to_string()))?;
    ssh_security_details(state).await.map_err(SshError::Redis)
}

pub(super) async fn load_config(state: &AppState) -> redis::RedisResult<Value> {
    let config = state.redis.get_config().await?;
    Ok(normalize_config(config.get("ssh_security").cloned()))
}

pub(super) async fn load_runtime(state: &AppState) -> redis::RedisResult<Value> {
    Ok(normalize_runtime(
        state.redis.get_json_value(RUNTIME_KEY).await?,
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
    json!({
        "enabled": raw.get("enabled").and_then(Value::as_bool).unwrap_or(false),
        "allowed_cidrs": normalize_cidrs(raw.get("allowed_cidrs")),
        "updated_at": normalize_timestamp(raw.get("updated_at"))
    })
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
    let allowed_regions = resolve_allowed_regions(state, &raw, previous).await?;
    let custom_cidrs = if raw.contains_key("custom_cidrs") {
        validate_cidrs(raw.get("custom_cidrs"), translator)?;
        normalize_cidrs(raw.get("custom_cidrs"))
    } else {
        normalize_cidrs(previous.get("custom_cidrs"))
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
    let runtime = build_runtime_from_config(&config, allowed_regions.cidrs);
    Ok((config, runtime))
}

pub(super) async fn resolve_allowed_regions(
    state: &AppState,
    raw: &Map<String, Value>,
    previous: &Value,
) -> Result<ResolvedAllowedRegions, SshError> {
    let source = if raw.contains_key("allowed_regions") {
        raw.get("allowed_regions")
    } else {
        previous.get("allowed_regions")
    };
    let Some(items) = source.and_then(Value::as_array) else {
        return Ok(ResolvedAllowedRegions {
            selections: json!([]),
            cidrs: Vec::new(),
        });
    };

    let mut seen = HashSet::new();
    let mut selections = Vec::new();
    let mut cidrs = Vec::new();
    for item in items {
        let province = item
            .get("province")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        if province.is_empty() {
            continue;
        }
        let query_city = item
            .get("query_city")
            .or_else(|| item.get("queryCity"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let key = format!("{province}::{}", query_city.unwrap_or(""));
        if !seen.insert(key) {
            continue;
        }
        let lookup = scanner::lookup_cidr_region(state, province, query_city)
            .await
            .map_err(SshError::BadRequest)?;
        selections.push(lookup.selection);
        cidrs.extend(lookup.cidrs);
    }
    cidrs = normalize_cidr_strings(cidrs);
    Ok(ResolvedAllowedRegions {
        selections: Value::Array(selections),
        cidrs,
    })
}

pub(super) fn build_runtime_from_config(
    config: &Value,
    mut resolved_region_cidrs: Vec<String>,
) -> Value {
    let custom_cidrs = config
        .get("custom_cidrs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string);
    resolved_region_cidrs.extend(custom_cidrs);
    let allowed_cidrs = normalize_cidr_strings(resolved_region_cidrs);
    json!({
        "enabled": config.get("enabled").and_then(Value::as_bool).unwrap_or(false),
        "allowed_cidrs": if config.get("enabled").and_then(Value::as_bool).unwrap_or(false) {
            Value::Array(allowed_cidrs.into_iter().map(Value::String).collect())
        } else {
            json!([])
        },
        "updated_at": time_utils::now_iso()
    })
}
