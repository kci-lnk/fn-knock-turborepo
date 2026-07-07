use super::*;

pub(super) async fn apply_boot_config_migrations(
    state: &AppState,
    config: &mut Value,
) -> redis::RedisResult<Vec<&'static str>> {
    let mut applied = Vec::new();
    let mut config_changed = false;
    let mut mark_throttle_patch_done = false;
    let mut mark_resource_alerts_patch_done = false;

    if state
        .redis
        .get_string_value(LEGACY_REVERSE_PROXY_THROTTLE_PATCH_FLAG_KEY)
        .await?
        .as_deref()
        != Some("1")
    {
        if legacy_reverse_proxy_throttle_matches(config.get("reverse_proxy_throttle")) {
            let mut throttle = config
                .get("reverse_proxy_throttle")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            throttle.insert("requests_per_second".to_string(), json!(100));
            throttle.insert("burst".to_string(), json!(200));
            throttle.insert("block_seconds".to_string(), json!(30));
            ensure_config_object(config).insert(
                "reverse_proxy_throttle".to_string(),
                Value::Object(throttle),
            );
            config_changed = true;
            applied.push("legacy_reverse_proxy_throttle");
        }
        mark_throttle_patch_done = true;
    }

    if state
        .redis
        .get_string_value(LEGACY_EVENT_SYSTEM_RESOURCE_ALERTS_PATCH_FLAG_KEY)
        .await?
        .as_deref()
        != Some("1")
    {
        if legacy_resource_alert_rules_match(config) {
            set_event_resource_alert_enabled(config, "cpu_alert");
            set_event_resource_alert_enabled(config, "memory_alert");
            config_changed = true;
            applied.push("legacy_event_system_resource_alerts");
        }
        mark_resource_alerts_patch_done = true;
    }

    if config_changed {
        state.redis.save_config(config).await?;
    }
    if mark_throttle_patch_done {
        state
            .redis
            .set_string_value(LEGACY_REVERSE_PROXY_THROTTLE_PATCH_FLAG_KEY, "1")
            .await?;
    }
    if mark_resource_alerts_patch_done {
        state
            .redis
            .set_string_value(LEGACY_EVENT_SYSTEM_RESOURCE_ALERTS_PATCH_FLAG_KEY, "1")
            .await?;
    }
    Ok(applied)
}

pub(super) async fn apply_runtime_constraints_on_boot(
    state: &AppState,
    config: &mut Value,
) -> redis::RedisResult<Vec<String>> {
    let mut corrected = Vec::new();
    let target = deployment_target(state);
    let host_runtime = host_runtime_available(state);
    let host_firewall = host_firewall_available(state);

    if !host_runtime && config.get("run_type").and_then(Value::as_i64) == Some(0) {
        ensure_config_object(config).insert("run_type".to_string(), json!(3));
        corrected.push("run_type=0 -> run_type=3".to_string());
    }

    let smart = normalize_smart_connect_config(config.get("smart_connect"));
    if !host_runtime && smart.get("enabled").and_then(Value::as_bool) == Some(true) {
        let mut next = smart;
        if let Some(object) = next.as_object_mut() {
            object.insert("enabled".to_string(), Value::Bool(false));
        }
        ensure_config_object(config).insert("smart_connect".to_string(), next);
        corrected.push("smart_connect.enabled -> false".to_string());
    }

    let terminal = normalize_terminal_feature(config.get("terminal_feature"));
    if matches!(target.as_str(), "docker" | "openwrt")
        && terminal.get("enabled").and_then(Value::as_bool) == Some(true)
    {
        let mut next = terminal;
        if let Some(object) = next.as_object_mut() {
            object.insert("enabled".to_string(), Value::Bool(false));
        }
        ensure_config_object(config).insert("terminal_feature".to_string(), next);
        corrected.push("terminal_feature.enabled -> false".to_string());
    }

    let auto_https = auto_https::normalize_auto_https_config(config.get("auto_https"));
    if matches!(target.as_str(), "docker" | "openwrt")
        && auto_https.get("enabled").and_then(Value::as_bool) == Some(true)
    {
        let mut next = auto_https;
        if let Some(object) = next.as_object_mut() {
            object.insert("enabled".to_string(), Value::Bool(false));
        }
        ensure_config_object(config).insert("auto_https".to_string(), next);
        corrected.push("auto_https.enabled -> false".to_string());
    }

    let ssh_security = crate::ssh_security::normalize_config(config.get("ssh_security").cloned());
    if (!host_firewall || target == "openwrt")
        && ssh_security.get("enabled").and_then(Value::as_bool) == Some(true)
    {
        let mut next = ssh_security;
        if let Some(object) = next.as_object_mut() {
            object.insert("enabled".to_string(), Value::Bool(false));
        }
        ensure_config_object(config).insert("ssh_security".to_string(), next);
        corrected.push("ssh_security.enabled -> false".to_string());
    }

    let auto_manage_firewall =
        config.get("auto_manage_firewall").and_then(Value::as_bool) != Some(false);
    if !host_firewall {
        if config.get("auto_manage_firewall").and_then(Value::as_bool) != Some(false) {
            corrected.push("auto_manage_firewall -> false".to_string());
        }
        ensure_config_object(config).insert("auto_manage_firewall".to_string(), Value::Bool(false));
    } else if config.get("auto_manage_firewall").and_then(Value::as_bool)
        != Some(auto_manage_firewall)
    {
        ensure_config_object(config).insert(
            "auto_manage_firewall".to_string(),
            json!(auto_manage_firewall),
        );
        corrected.push(format!("auto_manage_firewall -> {auto_manage_firewall}"));
    }

    if !corrected.is_empty() {
        state.redis.save_config(config).await?;
    }
    Ok(corrected)
}

pub(super) fn legacy_reverse_proxy_throttle_matches(value: Option<&Value>) -> bool {
    int_field(value, "requests_per_second", 100, 1, 10_000) == 20
        && int_field(value, "burst", 200, 1, 100_000) == 50
        && int_field(value, "block_seconds", 30, 1, 86_400) == 30
}

pub(super) fn legacy_resource_alert_rules_match(config: &Value) -> bool {
    let rules = config
        .pointer("/event_system/rules")
        .unwrap_or(&Value::Null);
    resource_rule_matches(rules.get("cpu_alert"), false, 85, 70, 15, 120)
        && resource_rule_matches(rules.get("memory_alert"), false, 90, 75, 15, 120)
}

pub(super) fn resource_rule_matches(
    value: Option<&Value>,
    enabled: bool,
    threshold: i64,
    recover: i64,
    sample_interval: i64,
    sustain: i64,
) -> bool {
    bool_field(value, "enabled", true) == enabled
        && int_field(value, "threshold_percent", threshold, 1, 100) == threshold
        && int_field(value, "recover_percent", recover, 0, 100) == recover
        && int_field(value, "sample_interval_seconds", sample_interval, 1, 3600) == sample_interval
        && int_field(value, "sustain_seconds", sustain, 1, 86_400) == sustain
}

pub(super) fn set_event_resource_alert_enabled(config: &mut Value, key: &str) {
    let object = ensure_config_object(config);
    let event_system = object
        .entry("event_system".to_string())
        .or_insert_with(|| json!({ "enabled": true, "retention_days": 30, "rules": {} }));
    if !event_system.is_object() {
        *event_system = json!({ "enabled": true, "retention_days": 30, "rules": {} });
    }
    let event_object = event_system
        .as_object_mut()
        .expect("event system is object");
    let rules = event_object
        .entry("rules".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !rules.is_object() {
        *rules = Value::Object(Map::new());
    }
    let rules_object = rules.as_object_mut().expect("rules is object");
    let rule = rules_object
        .entry(key.to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !rule.is_object() {
        *rule = Value::Object(Map::new());
    }
    rule.as_object_mut()
        .expect("rule is object")
        .insert("enabled".to_string(), Value::Bool(true));
}
