use super::*;

pub(super) async fn load_config_section(
    state: &AppState,
    key: &str,
    normalize: fn(Option<&Value>) -> Value,
) -> crate::storage::StorageResult<Value> {
    let config = state.storage.store.get_config().await?;
    Ok(normalize(config.get(key)))
}

pub(super) async fn update_config_section(
    state: &AppState,
    key: &str,
    patch: &Value,
    normalize: fn(Option<&Value>) -> Value,
) -> crate::storage::StorageResult<Value> {
    let mut config = state.storage.store.get_config().await?;
    if !config.is_object() {
        config = app_store::default_config();
    }
    let mut next = normalize(config.get(key));
    merge_object(&mut next, patch);
    next = normalize(Some(&next));
    if let Some(object) = config.as_object_mut() {
        object.insert(key.to_string(), next.clone());
    }
    state.storage.store.save_config(&config).await?;
    Ok(next)
}

pub(super) async fn save_top_level_config_value(
    state: &AppState,
    key: &str,
    value: Value,
) -> crate::storage::StorageResult<()> {
    let mut config = state.storage.store.get_config().await?;
    if !config.is_object() {
        config = app_store::default_config();
    }
    if let Some(object) = config.as_object_mut() {
        object.insert(key.to_string(), value);
    }
    state.storage.store.save_config(&config).await
}

pub(crate) async fn load_protocol_mapping_feature(
    state: &AppState,
    fallback_config: Option<&Value>,
) -> crate::storage::StorageResult<Value> {
    if let Some(value) = state
        .storage
        .store
        .get_json_value(PROTOCOL_MAPPING_FEATURE_KEY)
        .await?
    {
        return Ok(normalize_protocol_mapping_feature(Some(&value)));
    }
    Ok(normalize_protocol_mapping_feature(
        fallback_config.and_then(|config| config.get("protocol_mapping_feature")),
    ))
}

pub(super) async fn save_protocol_mapping_feature(
    state: &AppState,
    value: &Value,
) -> crate::storage::StorageResult<()> {
    let next = normalize_protocol_mapping_feature(Some(value));
    state
        .storage
        .store
        .set_json_value(PROTOCOL_MAPPING_FEATURE_KEY, &next)
        .await
}

/// Loads the standalone captcha settings used by both admin and AUTH routes.
///
/// Captcha settings have never been part of the main `fn_knock:config` object;
/// keep this as the single read path so provider selection cannot diverge.
pub(crate) async fn load_captcha_settings(
    state: &AppState,
) -> crate::storage::StorageResult<Value> {
    let value = match state
        .storage
        .store
        .get_json_value(CAPTCHA_SETTINGS_KEY)
        .await?
    {
        Some(value) => Some(value),
        None => {
            state
                .storage
                .store
                .get_json_value(LEGACY_CAPTCHA_SETTINGS_KEY)
                .await?
        }
    };
    Ok(normalize_captcha_settings(value.as_ref()))
}

pub(super) async fn update_captcha_settings(
    state: &AppState,
    patch: &Value,
) -> crate::storage::StorageResult<Value> {
    let current = load_captcha_settings(state).await?;
    let mut next = current.clone();
    merge_object(&mut next, patch);
    for section in ["pow", "turnstile"] {
        let Some(patch_section) = patch.get(section).and_then(Value::as_object) else {
            continue;
        };
        let mut nested = current
            .get(section)
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        for (key, value) in patch_section {
            if section == "pow" && key == "uncommon_location" {
                let mut uncommon_location = nested
                    .get(key)
                    .and_then(Value::as_object)
                    .cloned()
                    .unwrap_or_default();
                if let Some(patch_uncommon_location) = value.as_object() {
                    for (nested_key, nested_value) in patch_uncommon_location {
                        uncommon_location.insert(nested_key.clone(), nested_value.clone());
                    }
                    nested.insert(key.clone(), Value::Object(uncommon_location));
                    continue;
                }
            }
            nested.insert(key.clone(), value.clone());
        }
        if let Some(object) = next.as_object_mut() {
            object.insert(section.to_string(), Value::Object(nested));
        }
    }
    next = normalize_captcha_settings(Some(&next));
    state
        .storage
        .store
        .set_json_value(CAPTCHA_SETTINGS_KEY, &next)
        .await?;
    Ok(next)
}

pub(super) async fn load_run_mode_prompt_preferences(
    state: &AppState,
) -> crate::storage::StorageResult<Value> {
    Ok(normalize_run_mode_prompt_preferences(
        state
            .storage
            .store
            .get_json_value(RUN_MODE_PROMPT_PREFERENCES_KEY)
            .await?
            .as_ref(),
    ))
}

pub(super) fn merge_object(target: &mut Value, patch: &Value) {
    let Some(target) = target.as_object_mut() else {
        return;
    };
    if let Some(patch) = patch.as_object() {
        for (key, value) in patch {
            target.insert(key.clone(), value.clone());
        }
    }
}

pub(super) fn merge_runtime(mut config: Value, runtime: Value) -> Value {
    if let Some(object) = config.as_object_mut() {
        object.insert("runtime".to_string(), runtime);
    }
    config
}

pub(super) fn normalize_captcha_settings(value: Option<&Value>) -> Value {
    let provider = if value
        .and_then(|value| value.get("provider"))
        .and_then(Value::as_str)
        == Some("turnstile")
    {
        "turnstile"
    } else {
        "pow"
    };
    let pow = value.and_then(|value| value.get("pow"));
    let base_max_number = normalize_pow_max_number(
        pow.and_then(|value| value.get("base_max_number")),
        POW_DEFAULT_BASE_MAX_NUMBER,
    );
    let uncommon_max_number = normalize_pow_max_number(
        pow.and_then(|value| value.pointer("/uncommon_location/max_number")),
        POW_DEFAULT_UNCOMMON_MAX_NUMBER,
    );
    let uncommon_max_number = if uncommon_max_number < base_max_number {
        POW_DEFAULT_UNCOMMON_MAX_NUMBER.max(base_max_number)
    } else {
        uncommon_max_number
    };
    json!({
        "provider": provider,
        "widget_mode": "normal",
        "pow": {
            "base_max_number": base_max_number,
            "uncommon_location": {
                "enabled": pow
                    .and_then(|value| value.pointer("/uncommon_location/enabled"))
                    .and_then(Value::as_bool)
                    == Some(true),
                "max_number": uncommon_max_number,
            },
        },
        "turnstile": {
            "site_key": value
                .and_then(|value| value.pointer("/turnstile/site_key"))
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or(""),
            "secret_key": value
                .and_then(|value| value.pointer("/turnstile/secret_key"))
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or(""),
        },
    })
}

fn normalize_pow_max_number(value: Option<&Value>, fallback: i64) -> i64 {
    value
        .and_then(Value::as_i64)
        .filter(|value| {
            (POW_MIN_MAX_NUMBER..=POW_MAX_MAX_NUMBER).contains(value)
                && value % POW_MAX_NUMBER_STEP == 0
        })
        .unwrap_or(fallback)
}

pub(crate) fn normalize_wol_feature(value: Option<&Value>) -> Value {
    json!({
        "enabled": bool_field(value, "enabled", false),
    })
}

pub(crate) fn normalize_fnos_share_bypass(value: Option<&Value>) -> Value {
    json!({
        "enabled": bool_field(value, "enabled", false),
        "upstream_timeout_ms": int_field(value, "upstream_timeout_ms", 2500, 500, 15000),
        "validation_cache_ttl_seconds": int_field(value, "validation_cache_ttl_seconds", 30, 5, 300),
        "validation_lock_ttl_seconds": int_field(value, "validation_lock_ttl_seconds", 5, 1, 30),
        "session_ttl_seconds": int_field(value, "session_ttl_seconds", 300, 30, 3600),
    })
}

pub(crate) fn normalize_fnos_port_icon_hijack(value: Option<&Value>) -> Value {
    json!({
        "enabled": bool_field(value, "enabled", false),
        "updated_at": optional_string_field(value, "updated_at").map(Value::String).unwrap_or(Value::Null),
    })
}

pub(super) fn normalize_auto_manage_firewall(value: Option<&Value>) -> bool {
    value.and_then(Value::as_bool) != Some(false)
}

pub(crate) const MAX_FIREWALL_ADDITIONAL_PORTS: usize = 128;

pub(crate) fn normalize_firewall_additional_ports(value: Option<&Value>) -> Vec<i64> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_i64)
        .filter(|port| (1..=65535).contains(port))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .take(MAX_FIREWALL_ADDITIONAL_PORTS)
        .collect()
}

pub(super) fn parse_firewall_additional_ports(body: &Value) -> Result<Vec<i64>, &'static str> {
    let Some(items) = body.get("ports").and_then(Value::as_array) else {
        return Err("portsArrayRequired");
    };
    let mut ports = BTreeSet::new();
    for item in items {
        let Some(port) = item.as_i64() else {
            return Err("portIntegerRequired");
        };
        if !(1..=65535).contains(&port) {
            return Err("portOutOfRange");
        }
        ports.insert(port);
    }
    if ports.len() > MAX_FIREWALL_ADDITIONAL_PORTS {
        return Err("tooManyPorts");
    }
    Ok(ports.into_iter().collect())
}

pub(super) fn normalize_run_mode_prompt_preferences(value: Option<&Value>) -> Value {
    json!({
        "directToReverseProxy": bool_field(value, "directToReverseProxy", false),
        "reverseProxyToDirect": bool_field(value, "reverseProxyToDirect", false),
        "switchToSubdomain": bool_field(value, "switchToSubdomain", false),
        "subdomainToReverseProxy": bool_field(value, "subdomainToReverseProxy", false),
    })
}

pub(super) fn bool_field(value: Option<&Value>, key: &str, fallback: bool) -> bool {
    value
        .and_then(|value| value.get(key))
        .and_then(Value::as_bool)
        .unwrap_or(fallback)
}

pub(super) fn bool_field_alias(
    value: Option<&Value>,
    snake_key: &str,
    camel_key: &str,
    fallback: bool,
) -> bool {
    value
        .and_then(|value| {
            value
                .get(snake_key)
                .and_then(Value::as_bool)
                .or_else(|| value.get(camel_key).and_then(Value::as_bool))
        })
        .unwrap_or(fallback)
}

pub(super) fn optional_string_field(value: Option<&Value>, key: &str) -> Option<String> {
    value
        .and_then(|value| value.get(key))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(super) fn optional_string_field_alias(
    value: Option<&Value>,
    snake_key: &str,
    camel_key: &str,
) -> Option<String> {
    value
        .and_then(|value| {
            value
                .get(snake_key)
                .and_then(Value::as_str)
                .or_else(|| value.get(camel_key).and_then(Value::as_str))
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(super) fn int_field(
    value: Option<&Value>,
    key: &str,
    fallback: i64,
    min: i64,
    max: i64,
) -> i64 {
    value
        .and_then(|value| value.get(key))
        .and_then(parse_int_field_value)
        .unwrap_or(fallback)
        .clamp(min, max)
}

pub(super) fn parse_int_field_value(value: &Value) -> Option<i64> {
    crate::node_compat::parse_i64_from_json_like_node(value)
}

pub(super) fn ensure_config_object(value: &mut Value) -> &mut Map<String, Value> {
    if !value.is_object() {
        *value = app_store::default_config();
    }
    value.as_object_mut().expect("config is object")
}

pub(super) fn normalize_run_type(value: Option<&Value>) -> Option<i64> {
    match value.and_then(Value::as_i64) {
        Some(0) => Some(0),
        Some(1) => Some(1),
        Some(3) => Some(3),
        _ => None,
    }
}

pub(crate) fn normalize_protocol_mapping_feature(value: Option<&Value>) -> Value {
    normalize_protocol_mapping_feature_strict(value).unwrap_or_else(|_| {
        let mut normalized = json!({
            "enabled": bool_field(value, "enabled", false),
            "availability": Value::Null,
        });
        preserve_protocol_mapping_runtime_issue(&mut normalized, value);
        normalized
    })
}

pub(super) fn normalize_protocol_mapping_feature_strict(
    value: Option<&Value>,
) -> Result<Value, crate::daily_availability::DailyAvailabilityError> {
    let availability = crate::daily_availability::normalize_daily_availability(
        value.and_then(|value| value.get("availability")),
    )?;
    let mut normalized = json!({
        "enabled": bool_field(value, "enabled", false),
        "availability": availability,
    });
    preserve_protocol_mapping_runtime_issue(&mut normalized, value);
    Ok(normalized)
}

fn preserve_protocol_mapping_runtime_issue(normalized: &mut Value, source: Option<&Value>) {
    let Some(issue) = normalize_protocol_mapping_runtime_issue(
        source.and_then(|value| value.get("runtime_issue")),
    ) else {
        return;
    };
    ensure_object(normalized).insert("runtime_issue".to_string(), issue);
}

fn normalize_protocol_mapping_runtime_issue(value: Option<&Value>) -> Option<Value> {
    let value = value?.as_object()?;
    let message = value.get("message")?.as_str()?.trim();
    if message.is_empty() {
        return None;
    }
    let code = match value.get("code").and_then(Value::as_str) {
        Some("local_port_loop") => "local_port_loop",
        Some("listen_port_in_use") => "listen_port_in_use",
        _ => "runtime_sync_failed",
    };
    let protocol = value
        .get("protocol")
        .and_then(Value::as_str)
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .filter(|protocol| matches!(protocol.as_str(), "tcp" | "udp"))
        .map(Value::String)
        .unwrap_or(Value::Null);
    let listen_port = value
        .get("listen_port")
        .and_then(Value::as_u64)
        .filter(|port| (1..=65_535).contains(port))
        .map(Value::from)
        .unwrap_or(Value::Null);
    let target = value
        .get("target")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|target| !target.is_empty())
        .map(|target| Value::String(target.chars().take(512).collect()))
        .unwrap_or(Value::Null);
    Some(json!({
        "code": code,
        "message": message.chars().take(2_000).collect::<String>(),
        "protocol": protocol,
        "listen_port": listen_port,
        "target": target,
    }))
}

pub(super) fn normalize_smart_connect_config(value: Option<&Value>) -> Value {
    json!({
        "enabled": bool_field(value, "enabled", false),
        "selected_ipv4": optional_string_field(value, "selected_ipv4").unwrap_or_default(),
    })
}

pub(super) fn default_smart_connect_runtime() -> Value {
    json!({
        "selected_ipv4": "",
        "synced_domains": [],
        "managed_rule_count": 0,
        "last_sync_at": Value::Null,
        "last_sync_error": Value::Null,
    })
}

pub(super) fn normalize_smart_connect_runtime(value: Option<&Value>) -> Value {
    let raw = value.unwrap_or(&Value::Null);
    json!({
        "selected_ipv4": raw.get("selected_ipv4").and_then(Value::as_str).unwrap_or("").trim(),
        "synced_domains": string_array(raw.get("synced_domains")),
        "managed_rule_count": raw.get("managed_rule_count").and_then(Value::as_i64).unwrap_or(0).max(0),
        "last_sync_at": raw.get("last_sync_at").and_then(Value::as_str).map(|value| Value::String(value.trim().to_string())).unwrap_or(Value::Null),
        "last_sync_error": raw.get("last_sync_error").and_then(Value::as_str).map(|value| Value::String(value.trim().to_string())).unwrap_or(Value::Null),
    })
}

pub(super) fn normalize_gateway_logging(value: Option<&Value>) -> Value {
    json!({
        "enabled": bool_field(value, "enabled", false),
        "record_localhost": bool_field(value, "record_localhost", false),
        "max_days": int_field(value, "max_days", 7, 1, JS_MAX_SAFE_INTEGER_I64),
    })
}

pub(crate) fn normalize_fnos_network_tuning(value: Option<&Value>) -> Value {
    json!({
        "bbr_enabled": bool_field_alias(value, "bbr_enabled", "bbrEnabled", false),
        "mtu_probing_enabled": bool_field_alias(value, "mtu_probing_enabled", "mtuProbingEnabled", false),
        "previous_tcp_congestion_control": optional_string_field_alias(value, "previous_tcp_congestion_control", "previousTcpCongestionControl").map(Value::String).unwrap_or(Value::Null),
        "previous_default_qdisc": optional_string_field_alias(value, "previous_default_qdisc", "previousDefaultQdisc").map(Value::String).unwrap_or(Value::Null),
        "previous_tcp_mtu_probing": optional_string_field_alias(value, "previous_tcp_mtu_probing", "previousTcpMtuProbing").map(Value::String).unwrap_or(Value::Null),
        "updated_at": optional_string_field_alias(value, "updated_at", "updatedAt").map(Value::String).unwrap_or(Value::Null),
        "last_error": optional_string_field_alias(value, "last_error", "lastError").map(Value::String).unwrap_or(Value::Null),
    })
}
