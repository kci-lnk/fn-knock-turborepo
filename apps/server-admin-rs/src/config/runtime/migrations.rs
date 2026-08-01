use super::*;

pub(super) async fn apply_boot_config_migrations(
    state: &AppState,
    config: &mut Value,
) -> crate::storage::StorageResult<Vec<&'static str>> {
    let mut applied = Vec::new();
    let mut config_changed = false;
    let mut mark_throttle_patch_done = false;
    let mut mark_resource_alerts_patch_done = false;

    if ensure_runtime_event_config(config) {
        config_changed = true;
        applied.push("runtime_health_events");
    }

    if state
        .store
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
        .store
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
        state.store.save_config(config).await?;
    }
    if mark_throttle_patch_done {
        state
            .store
            .set_string_value(LEGACY_REVERSE_PROXY_THROTTLE_PATCH_FLAG_KEY, "1")
            .await?;
    }
    if mark_resource_alerts_patch_done {
        state
            .store
            .set_string_value(LEGACY_EVENT_SYSTEM_RESOURCE_ALERTS_PATCH_FLAG_KEY, "1")
            .await?;
    }
    Ok(applied)
}

fn ensure_runtime_event_config(config: &mut Value) -> bool {
    let event_system = ensure_config_object(config)
        .entry("event_system".to_string())
        .or_insert_with(|| json!({ "enabled": true, "retention_days": 30, "rules": {} }));
    if !event_system.is_object() {
        *event_system = json!({ "enabled": true, "retention_days": 30, "rules": {} });
    }
    let event_object = event_system
        .as_object_mut()
        .expect("event system is object");
    let mut changed = false;
    let max_records = event_object
        .get("max_records")
        .and_then(Value::as_i64)
        .unwrap_or(10_000)
        .clamp(1_000, 50_000);
    if event_object.get("max_records").and_then(Value::as_i64) != Some(max_records) {
        event_object.insert("max_records".to_string(), json!(max_records));
        changed = true;
    }
    let rules = event_object
        .entry("rules".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !rules.is_object() {
        *rules = Value::Object(Map::new());
        changed = true;
    }
    let rules = rules.as_object_mut().expect("rules is object");
    for key in ["runtime_lifecycle", "runtime_health"] {
        if !rules.contains_key(key) {
            rules.insert(key.to_string(), json!({ "enabled": true }));
            changed = true;
        }
    }
    changed
}

pub(super) async fn apply_runtime_constraints_on_boot(
    state: &AppState,
    config: &mut Value,
) -> crate::storage::StorageResult<Vec<String>> {
    let mut corrected = Vec::new();
    let target = deployment_target(state);
    let host_runtime = host_runtime_available(state);
    let host_firewall = host_firewall_available(state);
    let capabilities =
        runtime_profile::get_runtime_capabilities(&runtime_profile::get_runtime_profile(state));

    if target == "fpk-lite" {
        let auth_target = crate::proxy_utils::default_auth_service_target();
        if retarget_fpk_lite_auth_service(config, &auth_target) {
            corrected.push(format!("subdomain auth service -> {auth_target}"));
        }
    }

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
    if !capabilities.terminal_available
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
    if !capabilities.auto_https_available
        && auto_https.get("enabled").and_then(Value::as_bool) == Some(true)
    {
        let mut next = auto_https;
        if let Some(object) = next.as_object_mut() {
            object.insert("enabled".to_string(), Value::Bool(false));
        }
        ensure_config_object(config).insert("auto_https".to_string(), next);
        corrected.push("auto_https.enabled -> false".to_string());
    }

    let fnos_network_tuning = normalize_fnos_network_tuning(config.get("fnos_network_tuning"));
    if !capabilities.fnos_network_tuning_available
        && (fnos_network_tuning
            .get("bbr_enabled")
            .and_then(Value::as_bool)
            == Some(true)
            || fnos_network_tuning
                .get("mtu_probing_enabled")
                .and_then(Value::as_bool)
                == Some(true))
    {
        let mut next = fnos_network_tuning;
        if let Some(object) = next.as_object_mut() {
            object.insert("bbr_enabled".to_string(), Value::Bool(false));
            object.insert("mtu_probing_enabled".to_string(), Value::Bool(false));
        }
        ensure_config_object(config).insert("fnos_network_tuning".to_string(), next);
        corrected.push("fnos_network_tuning -> disabled".to_string());
    }

    let fnos_connect_waf = normalize_fnos_connect_waf(config.get("fnos_connect_waf"));
    if !capabilities.fnos_connect_waf_available
        && fnos_connect_waf.get("enabled").and_then(Value::as_bool) == Some(true)
    {
        let mut next = fnos_connect_waf;
        ensure_config_object(&mut next).insert("enabled".to_string(), Value::Bool(false));
        ensure_config_object(config).insert("fnos_connect_waf".to_string(), next);
        corrected.push("fnos_connect_waf.enabled -> false".to_string());
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
        state.store.save_config(config).await?;
    }
    Ok(corrected)
}

pub(super) fn retarget_fpk_lite_auth_service(config: &mut Value, auth_target: &str) -> bool {
    let Some(auth_port) = crate::proxy_utils::parse_target_port_u16(auth_target) else {
        return false;
    };
    if auth_port == 7997 {
        return false;
    }

    let mut changed = false;
    let auth_host = config
        .pointer("/subdomain_mode/auth_host")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("")
        .to_ascii_lowercase();

    if let Some(subdomain_mode) = config
        .get_mut("subdomain_mode")
        .and_then(Value::as_object_mut)
        && subdomain_mode
            .get("auth_target")
            .and_then(Value::as_str)
            .is_some_and(is_legacy_default_auth_service_target)
    {
        subdomain_mode.insert("auth_target".to_string(), json!(auth_target));
        changed = true;
    }

    if let Some(mappings) = config
        .get_mut("host_mappings")
        .and_then(Value::as_array_mut)
    {
        for mapping in mappings {
            let Some(object) = mapping.as_object_mut() else {
                continue;
            };
            let is_auth_mapping = object.get("service_role").and_then(Value::as_str)
                == Some("auth")
                || (!auth_host.is_empty()
                    && object
                        .get("host")
                        .and_then(Value::as_str)
                        .is_some_and(|host| host.trim().eq_ignore_ascii_case(&auth_host)));
            if is_auth_mapping
                && object
                    .get("target")
                    .and_then(Value::as_str)
                    .is_some_and(is_legacy_default_auth_service_target)
            {
                object.insert("target".to_string(), json!(auth_target));
                changed = true;
            }
        }
    }

    changed
}

fn is_legacy_default_auth_service_target(target: &str) -> bool {
    let Ok(url) = url::Url::parse(target.trim()) else {
        return false;
    };
    url.scheme() == "http"
        && matches!(url.host_str(), Some("127.0.0.1" | "localhost"))
        && url.port_or_known_default() == Some(7997)
        && matches!(url.path(), "" | "/")
        && url.query().is_none()
        && url.fragment().is_none()
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

#[cfg(test)]
mod runtime_event_config_tests {
    use super::*;

    #[test]
    fn runtime_event_record_limit_is_defaulted_and_clamped() {
        for (input, expected) in [
            (Value::Null, 10_000),
            (json!(999), 1_000),
            (json!(50_001), 50_000),
            (json!("invalid"), 10_000),
        ] {
            let mut config = json!({
                "event_system": {
                    "max_records": input,
                    "rules": {}
                }
            });
            assert!(ensure_runtime_event_config(&mut config));
            assert_eq!(
                config.pointer("/event_system/max_records"),
                Some(&json!(expected))
            );
            assert_eq!(
                config.pointer("/event_system/rules/runtime_lifecycle/enabled"),
                Some(&Value::Bool(true))
            );
            assert_eq!(
                config.pointer("/event_system/rules/runtime_health/enabled"),
                Some(&Value::Bool(true))
            );
        }
    }
}
