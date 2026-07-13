use super::*;
use ipnet::IpNet;

pub(super) async fn sync_go_rules(state: &AppState, rules: &Value) -> Result<(), String> {
    ensure_go_success(
        state
            .go_backend
            .set_rules(rules)
            .await
            .map_err(|error| error.to_string())?,
    )
}

pub(crate) async fn sync_go_host_rules_locked(
    state: &AppState,
    rules: &Value,
) -> Result<(), String> {
    let response = state
        .go_backend
        .set_host_rules(rules)
        .await
        .map_err(|error| error.to_string())?;
    ensure_go_success(response.clone())?;
    ensure_go_host_protocol_modes_applied(rules, &response)
}

pub(crate) async fn flush_go_host_rules_locked(state: &AppState) -> Result<(), String> {
    ensure_go_success(
        state
            .go_backend
            .flush_host_rules()
            .await
            .map_err(|error| error.to_string())?,
    )
}

pub(super) fn host_rules_payload_for_config(config: &Value) -> Option<Value> {
    if !is_any_subdomain_routing_mode(config) {
        return None;
    }
    let mappings = config
        .get("host_mappings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    Some(build_host_rules_payload(&mappings))
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
    let requested_visibilities = host_visibilities_by_host(requested)?;
    let echoed_visibilities = host_visibilities_by_host(echoed_payload)?;
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

fn host_visibilities_by_host(
    value: &Value,
) -> Result<HashMap<String, (String, Vec<String>)>, String> {
    let items = value
        .as_array()
        .ok_or_else(|| "Host-rules visibility payload must be an array".to_string())?;
    let mut visibilities = HashMap::with_capacity(items.len());
    for item in items {
        let host = normalize_host_value(item.get("host").and_then(Value::as_str).unwrap_or(""));
        let visibility = item.get("visibility");
        let mode = if visibility
            .and_then(|value| value.get("mode"))
            .and_then(Value::as_str)
            == Some("custom")
        {
            "custom"
        } else {
            "inherit"
        };
        let mut seen = HashSet::new();
        let cidrs = visibility
            .and_then(|value| value.get("cidrs"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .filter_map(canonical_host_visibility_cidr)
            .filter(|cidr| seen.insert(cidr.clone()))
            .collect();
        visibilities.insert(host, (mode.to_string(), cidrs));
    }
    Ok(visibilities)
}

fn canonical_host_visibility_cidr(value: &str) -> Option<String> {
    let network = value.trim().parse::<IpNet>().ok()?;
    Some(match network {
        IpNet::V4(network) => format!("{}/{}", network.network(), network.prefix_len()),
        IpNet::V6(network) => format!("{}/{}", network.network(), network.prefix_len()),
    })
}

fn host_protocol_modes_by_host(
    value: &Value,
    context: &str,
) -> Result<HashMap<String, String>, String> {
    let items = value
        .as_array()
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
            .go_backend
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
