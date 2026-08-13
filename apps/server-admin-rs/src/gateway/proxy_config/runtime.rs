use super::*;

pub(super) async fn sync_go_rules(state: &AppState, rules: &Value) -> Result<(), String> {
    ensure_go_success(
        state
            .gateway
            .client
            .set_rules(rules)
            .await
            .map_err(|error| error.to_string())?,
    )
}

pub(crate) async fn sync_go_host_rules_locked(
    state: &AppState,
    rules: &Value,
) -> Result<(), String> {
    state.set_gateway_config_synced(false);
    state.runtime_health.operational_log(
        "INFO",
        "config_sync",
        "apply_started",
        "host_rules_pending",
        Map::new(),
    );
    let result = async {
        let response = state
            .gateway
            .client
            .set_host_rules(rules)
            .await
            .map_err(|error| error.to_string())?;
        ensure_go_success(response.clone())?;
        ensure_go_host_protocol_modes_applied(rules, &response)?;
        Ok(())
    }
    .await;
    if result.is_ok() {
        state.set_gateway_config_synced(true);
        state.runtime_health.operational_log(
            "INFO",
            "config_sync",
            "apply_completed",
            "host_rules_applied",
            Map::new(),
        );
    } else {
        state.runtime_health.operational_log(
            "ERROR",
            "config_sync",
            "apply_failed",
            "host_rules_rejected",
            Map::new(),
        );
    }
    result
}

pub(crate) async fn flush_go_host_rules_locked(state: &AppState) -> Result<(), String> {
    state.set_gateway_config_synced(false);
    let result = state
        .gateway
        .client
        .flush_host_rules()
        .await
        .map_err(|error| error.to_string())
        .and_then(ensure_go_success);
    if result.is_ok() {
        state.set_gateway_config_synced(true);
        state.runtime_health.operational_log(
            "INFO",
            "config_sync",
            "apply_completed",
            "host_rules_flushed",
            Map::new(),
        );
    } else {
        state.runtime_health.operational_log(
            "ERROR",
            "config_sync",
            "apply_failed",
            "host_rules_flush_failed",
            Map::new(),
        );
    }
    result
}

pub(super) fn host_rules_payload_for_config(config: &Value) -> Option<Value> {
    if !is_any_subdomain_routing_mode(config) {
        return None;
    }
    Some(build_host_rules_payload_for_config(config))
}

pub(crate) async fn sync_go_host_rules_for_config_locked(
    state: &AppState,
    config: &Value,
) -> Result<(), String> {
    match host_rules_payload_for_config(config) {
        Some(rules) => sync_go_host_rules_locked(state, &rules).await,
        None => flush_go_host_rules_locked(state).await,
    }
}

pub(super) fn ensure_go_host_protocol_modes_applied(
    requested: &Value,
    response: &Value,
) -> Result<(), String> {
    let echoed_payload = response
        .get("data")
        .ok_or_else(|| "Go backend host-rules response is missing data".to_string())?;
    let requested_modes = host_protocol_modes_by_host(requested, "Host-rules request")?;
    let echoed_modes = host_protocol_modes_by_host(echoed_payload, "Go backend response")?;
    let requested_target_path_modes =
        host_target_path_modes_by_host(requested, "Host-rules request")?;
    let echoed_target_path_modes =
        host_target_path_modes_by_host(echoed_payload, "Go backend response")?;
    let requested_visibilities = host_visibilities_by_host(requested)?;
    let echoed_visibilities = host_visibilities_by_host(echoed_payload)?;
    let requested_advanced_auth = host_advanced_auth_by_host(requested)?;
    let echoed_advanced_auth = host_advanced_auth_by_host(echoed_payload)?;
    let requested_groups = host_groups_by_host(requested)?;
    let echoed_groups = host_groups_by_host(echoed_payload)?;
    for (host, requested_mode) in &requested_modes {
        let Some(echoed_mode) = echoed_modes.get(host) else {
            return Err(format!(
                "Go backend did not apply host mapping {host}; upgrade the gateway backend"
            ));
        };
        if echoed_mode != requested_mode {
            return Err(format!(
                "Go backend did not apply HTTPS protocol mode {requested_mode} for {host} (reported {echoed_mode}); upgrade the gateway backend"
            ));
        }
        let requested_target_path_mode = requested_target_path_modes
            .get(host)
            .expect("target path mode map follows the validated host map");
        let echoed_target_path_mode = echoed_target_path_modes
            .get(host)
            .expect("target path mode map follows the validated host map");
        if echoed_target_path_mode != requested_target_path_mode {
            return Err(format!(
                "Go backend did not apply target path mode {requested_target_path_mode} for {host} (reported {echoed_target_path_mode}); upgrade the gateway backend"
            ));
        }
        if echoed_groups.get(host) != requested_groups.get(host) {
            return Err(format!(
                "Go backend did not apply host rule group for {host}; upgrade the gateway backend"
            ));
        }
        let requested_visibility = requested_visibilities
            .get(host)
            .expect("visibility map follows the validated host map");
        let echoed_visibility = echoed_visibilities
            .get(host)
            .expect("visibility map follows the validated host map");
        if echoed_visibility.0 != requested_visibility.0
            || (requested_visibility.0 == "custom" && echoed_visibility.1 != requested_visibility.1)
        {
            return Err(format!(
                "Go backend did not apply host visibility for {host}; upgrade the gateway backend"
            ));
        }
        if let Some(requested_policy) = requested_advanced_auth.get(host)
            && requested_policy
                .get("enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            && echoed_advanced_auth
                .get(host)
                .map(gateway_advanced_auth_projection)
                != Some(gateway_advanced_auth_projection(requested_policy))
        {
            return Err(format!(
                "Go backend did not apply advanced authentication for {host}; upgrade the gateway backend"
            ));
        }
    }
    let mut unexpected_hosts = echoed_modes
        .keys()
        .filter(|host| !requested_modes.contains_key(*host))
        .collect::<Vec<_>>();
    unexpected_hosts.sort_unstable();
    if let Some(host) = unexpected_hosts.first() {
        return Err(format!(
            "Go backend retained unexpected host mapping {host}; upgrade the gateway backend"
        ));
    }
    Ok(())
}

fn host_rule_items(value: &Value) -> Option<&Vec<Value>> {
    value
        .get("items")
        .and_then(Value::as_array)
        .or_else(|| value.as_array())
}

fn host_groups_by_host(value: &Value) -> Result<HashMap<String, (String, String)>, String> {
    let items =
        host_rule_items(value).ok_or_else(|| "Host-rules payload must be an array".to_string())?;
    let mut groups = HashMap::with_capacity(items.len());
    for item in items {
        let host = normalize_host_value(item.get("host").and_then(Value::as_str).unwrap_or(""));
        if host.is_empty() {
            return Err("Host-rules payload contains an empty host".to_string());
        }
        groups.insert(
            host,
            (
                item.get("group_id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                item.get("group_name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
            ),
        );
    }
    Ok(groups)
}

/// The protobuf deliberately carries only fields the gateway evaluates.  A
/// persisted control-plane policy also contains CIDR selector metadata
/// (`selections`, source fingerprint and compile time), which the gateway
/// cannot echo.  Compare this stable projection instead of rejecting every
/// region-backed policy during the runtime transaction.
fn gateway_advanced_auth_projection(value: &Value) -> Value {
    let Some(object) = value.as_object() else {
        return Value::Null;
    };
    let groups = object
        .get("groups")
        .and_then(Value::as_array)
        .map(|groups| {
            groups
                .iter()
                .filter_map(Value::as_object)
                .map(|group| {
                    let conditions = group
                        .get("conditions")
                        .and_then(Value::as_array)
                        .map(|conditions| {
                            conditions
                                .iter()
                                .filter_map(Value::as_object)
                                .map(|condition| {
                                    json!({
                                        "id": condition.get("id").cloned().unwrap_or(Value::String(String::new())),
                                        "target": condition.get("target").cloned().unwrap_or(Value::String(String::new())),
                                        "operator": condition.get("operator").cloned().unwrap_or(Value::String(String::new())),
                                        "name": condition.get("name").cloned().unwrap_or(Value::String(String::new())),
                                        "policy_id": condition.get("policy_id").cloned().unwrap_or(Value::String(String::new())),
                                        // Go normalizes empty slices to nil in
                                        // protobuf and emits JSON null; treat
                                        // null and [] as the same semantic
                                        // value for an omitted field.
                                        "values": gateway_list_value(condition.get("values")),
                                        "cidrs": gateway_list_value(condition.get("cidrs")),
                                    })
                                })
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    json!({
                        "id": group.get("id").cloned().unwrap_or(Value::String(String::new())),
                        "conditions": conditions,
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    json!({
        "enabled": object.get("enabled").cloned().unwrap_or(Value::Bool(false)),
        "idle_ttl_seconds": object.get("idle_ttl_seconds").cloned().unwrap_or(Value::from(0)),
        "max_lifetime_seconds": object.get("max_lifetime_seconds").cloned().unwrap_or(Value::from(0)),
        "policy_version": object.get("policy_version").cloned().unwrap_or(Value::String(String::new())),
        "groups": groups,
    })
}

fn gateway_list_value(value: Option<&Value>) -> Value {
    match value {
        Some(Value::Array(values)) if values.is_empty() => Value::Null,
        Some(value) => value.clone(),
        None => Value::Null,
    }
}

fn host_advanced_auth_by_host(value: &Value) -> Result<HashMap<String, Value>, String> {
    let items = host_rule_items(value)
        .ok_or_else(|| "Host-rules advanced-auth payload must be an array".to_string())?;
    let mut policies = HashMap::with_capacity(items.len());
    for item in items {
        let host = normalize_host_value(item.get("host").and_then(Value::as_str).unwrap_or(""));
        let policy = item.get("advanced_auth").cloned().unwrap_or(Value::Null);
        policies.insert(host, policy);
    }
    Ok(policies)
}

fn host_visibilities_by_host(value: &Value) -> Result<HashMap<String, (String, String)>, String> {
    let items = host_rule_items(value)
        .ok_or_else(|| "Host-rules visibility payload must be an array".to_string())?;
    let mut visibilities = HashMap::with_capacity(items.len());
    for item in items {
        let host = normalize_host_value(item.get("host").and_then(Value::as_str).unwrap_or(""));
        let visibility = item.get("visibility");
        let mode = match visibility
            .and_then(|value| value.get("mode"))
            .and_then(Value::as_str)
        {
            Some("custom") => "custom",
            Some("disabled") => "disabled",
            _ => "inherit",
        };
        let policy_id = visibility
            .and_then(|value| value.get("policy_id"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        visibilities.insert(host, (mode.to_string(), policy_id));
    }
    Ok(visibilities)
}

fn host_protocol_modes_by_host(
    value: &Value,
    context: &str,
) -> Result<HashMap<String, String>, String> {
    let items = host_rule_items(value)
        .ok_or_else(|| format!("{context} host-rules payload must be an array"))?;
    let mut modes = HashMap::with_capacity(items.len());
    for item in items {
        let host = normalize_host_value(item.get("host").and_then(Value::as_str).unwrap_or(""));
        if host.is_empty() {
            return Err(format!("{context} contains a host rule without a host"));
        }
        let mode = normalize_protocol_mode(item.get("protocol_mode"));
        if modes.insert(host.clone(), mode).is_some() {
            return Err(format!(
                "{context} contains duplicate canonical host mapping {host}"
            ));
        }
    }
    Ok(modes)
}

fn host_target_path_modes_by_host(
    value: &Value,
    context: &str,
) -> Result<HashMap<String, String>, String> {
    let items = host_rule_items(value)
        .ok_or_else(|| format!("{context} host-rules payload must be an array"))?;
    let mut modes = HashMap::with_capacity(items.len());
    for item in items {
        let host = normalize_host_value(item.get("host").and_then(Value::as_str).unwrap_or(""));
        if host.is_empty() {
            return Err(format!("{context} contains a host rule without a host"));
        }
        let mode = normalize_target_path_mode(item.get("target_path_mode"));
        if modes.insert(host.clone(), mode).is_some() {
            return Err(format!(
                "{context} contains duplicate canonical host mapping {host}"
            ));
        }
    }
    Ok(modes)
}

pub(super) async fn sync_stream_mappings_runtime(
    state: &AppState,
    config: &Value,
) -> Result<(), String> {
    let run_type = config.get("run_type").and_then(Value::as_i64).unwrap_or(3);
    runtime_config::apply_run_type_config(state, config, run_type).await
}

pub(super) async fn sync_go_auth_config(state: &AppState, config: &Value) -> Result<(), String> {
    let auth_config = build_gateway_auth_config(config);
    ensure_go_success(
        state
            .gateway
            .client
            .set_auth_config(&auth_config)
            .await
            .map_err(|error| error.to_string())?,
    )
}

pub(super) async fn sync_host_mappings_runtime(
    state: &AppState,
    previous_config: &Value,
    _mappings: &[Value],
) -> Result<(), String> {
    // A non-Host config writer does not take the HostRules lease and may have
    // committed run_type/submode settings after the Host section CAS. Re-read
    // while the caller still owns the lease so the runtime always follows the
    // latest complete persisted config.
    let current_config = state
        .storage
        .store
        .get_config()
        .await
        .map_err(|error| error.to_string())?;
    sync_go_host_rules_for_config_locked(state, &current_config).await?;
    sync_go_auth_config(state, &current_config).await?;
    gateway_settings::sync_gateway_target_runtime_for_config(state, &current_config, true, true)
        .await?;
    if waf::disabled_hosts_for_config(previous_config)
        != waf::disabled_hosts_for_config(&current_config)
    {
        waf::sync_waf_config_to_gateway(state, &current_config)
            .await
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}
