use super::*;

pub(super) async fn load_config_section(
    state: &AppState,
    key: &str,
    normalize: fn(Option<&Value>) -> Value,
) -> redis::RedisResult<Value> {
    let config = state.redis.get_config().await?;
    Ok(normalize(config.get(key)))
}

pub(super) async fn update_config_section(
    state: &AppState,
    key: &str,
    patch: &Value,
    normalize: fn(Option<&Value>) -> Value,
) -> redis::RedisResult<Value> {
    let mut config = state.redis.get_config().await?;
    if !config.is_object() {
        config = redis_store::default_config();
    }
    let mut next = normalize(config.get(key));
    merge_object(&mut next, patch);
    next = normalize(Some(&next));
    if let Some(object) = config.as_object_mut() {
        object.insert(key.to_string(), next.clone());
    }
    state.redis.save_config(&config).await?;
    Ok(next)
}

pub(super) async fn save_top_level_config_value(
    state: &AppState,
    key: &str,
    value: Value,
) -> redis::RedisResult<()> {
    let mut config = state.redis.get_config().await?;
    if !config.is_object() {
        config = redis_store::default_config();
    }
    if let Some(object) = config.as_object_mut() {
        object.insert(key.to_string(), value);
    }
    state.redis.save_config(&config).await
}

pub(crate) async fn load_protocol_mapping_feature(
    state: &AppState,
    fallback_config: Option<&Value>,
) -> redis::RedisResult<Value> {
    if let Some(value) = state
        .redis
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
) -> redis::RedisResult<()> {
    let next = normalize_protocol_mapping_feature(Some(value));
    state
        .redis
        .set_json_value(PROTOCOL_MAPPING_FEATURE_KEY, &next)
        .await
}

pub(super) async fn load_captcha_settings(state: &AppState) -> redis::RedisResult<Value> {
    let value = match state.redis.get_json_value(CAPTCHA_SETTINGS_KEY).await? {
        Some(value) => Some(value),
        None => {
            state
                .redis
                .get_json_value(LEGACY_CAPTCHA_SETTINGS_KEY)
                .await?
        }
    };
    Ok(normalize_captcha_settings(value.as_ref()))
}

pub(super) async fn update_captcha_settings(
    state: &AppState,
    patch: &Value,
) -> redis::RedisResult<Value> {
    let current = load_captcha_settings(state).await?;
    let mut next = current.clone();
    merge_object(&mut next, patch);
    if let Some(patch_turnstile) = patch.get("turnstile").and_then(Value::as_object) {
        let mut turnstile = current
            .get("turnstile")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        for (key, value) in patch_turnstile {
            turnstile.insert(key.clone(), value.clone());
        }
        if let Some(object) = next.as_object_mut() {
            object.insert("turnstile".to_string(), Value::Object(turnstile));
        }
    }
    next = normalize_captcha_settings(Some(&next));
    state
        .redis
        .set_json_value(CAPTCHA_SETTINGS_KEY, &next)
        .await?;
    Ok(next)
}

pub(super) async fn load_run_mode_prompt_preferences(
    state: &AppState,
) -> redis::RedisResult<Value> {
    Ok(normalize_run_mode_prompt_preferences(
        state
            .redis
            .get_json_value(RUN_MODE_PROMPT_PREFERENCES_KEY)
            .await?
            .as_ref(),
    ))
}

pub(super) async fn load_welcome_guide_status(state: &AppState) -> redis::RedisResult<Value> {
    let raw = state
        .redis
        .get_string_value(WELCOME_GUIDE_STATUS_KEY)
        .await?;
    Ok(match raw.as_deref() {
        None => json!({ "completed": false, "completed_at": Value::Null }),
        Some("1") | Some("true") => json!({ "completed": true, "completed_at": Value::Null }),
        Some(value) => serde_json::from_str::<Value>(value)
            .ok()
            .map(|value| {
                json!({
                    "completed": value.get("completed").and_then(Value::as_bool).unwrap_or(false),
                    "completed_at": value
                        .get("completed_at")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(|value| Value::String(value.to_string()))
                        .unwrap_or(Value::Null),
                })
            })
            .unwrap_or_else(|| json!({ "completed": false, "completed_at": Value::Null })),
    })
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
    json!({
        "provider": provider,
        "widget_mode": "normal",
        "pow": {},
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

pub(crate) fn normalize_terminal_feature(value: Option<&Value>) -> Value {
    json!({
        "enabled": bool_field(value, "enabled", false),
        "default_cwd": normalize_terminal_default_cwd(
            value
                .and_then(|value| value.get("default_cwd"))
                .and_then(Value::as_str)
        ),
        "max_sessions": int_field(value, "max_sessions", 3, 1, 12),
        "idle_timeout_seconds": int_field(value, "idle_timeout_seconds", 24 * 60 * 60, 60, 7 * 24 * 60 * 60),
        "resume_backend": "tmux",
        "allow_mobile_toolbar": bool_field(value, "allow_mobile_toolbar", true),
        "dangerously_run_as_current_user": bool_field(value, "dangerously_run_as_current_user", true),
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

pub(super) fn parse_i64_prefix(value: &str) -> Option<i64> {
    crate::node_compat::parse_i64_prefix(value)
}

pub(super) fn ensure_config_object(value: &mut Value) -> &mut Map<String, Value> {
    if !value.is_object() {
        *value = redis_store::default_config();
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
    json!({ "enabled": bool_field(value, "enabled", false) })
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
