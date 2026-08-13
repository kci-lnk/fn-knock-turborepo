use super::*;

pub(super) async fn load_smart_connect_details(state: &AppState) -> anyhow::Result<Value> {
    let translator = Translator::from_state(state).await;
    let config = state.storage.store.get_config().await?;
    let runtime = state
        .storage
        .store
        .get_json_value(SMART_CONNECT_RUNTIME_KEY)
        .await?
        .map(|value| normalize_smart_connect_runtime(Some(&value)))
        .unwrap_or_else(default_smart_connect_runtime);
    Ok(build_smart_connect_details(
        state,
        &config,
        runtime,
        &translator,
    ))
}

pub(super) async fn sync_smart_connect(state: &AppState, config: &Value) -> Result<Value, String> {
    let translator = Translator::from_state(state).await;
    let smart = normalize_smart_connect_config(config.get("smart_connect"));
    let domains = list_smart_connect_domains(config);
    let available =
        host_firewall_available(state) && config.get("run_type").and_then(Value::as_i64) == Some(3);
    let enabled = smart.get("enabled").and_then(Value::as_bool) == Some(true);
    let selected_ipv4 = smart
        .get("selected_ipv4")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let now = time_utils::now_iso();

    let runtime_result = async {
        if !available || !enabled {
            clear_smart_connect_managed_config(&translator)?;
            return Ok(json!({
                "selected_ipv4": selected_ipv4,
                "synced_domains": [],
                "managed_rule_count": 0,
                "last_sync_at": now,
                "last_sync_error": Value::Null,
            }));
        }
        if selected_ipv4.is_empty() {
            return Err(smart_connect_text(&translator, "selectLocalIp"));
        }
        if !is_private_ipv4(&selected_ipv4) {
            return Err(smart_connect_text(&translator, "selectValidLocalIpv4"));
        }
        let dnsmasq = system_assets::build_dnsmasq_status_with_translator(&translator);
        if dnsmasq.get("installed").and_then(Value::as_bool) != Some(true) {
            return Err(smart_connect_text(&translator, "dnsmasqNotInstalled"));
        }
        if dnsmasq.get("initialized").and_then(Value::as_bool) != Some(true) {
            return Err(smart_connect_text(&translator, "dnsmasqNotInitialized"));
        }
        apply_smart_connect_managed_config(&selected_ipv4, &domains, &translator)?;
        Ok(json!({
            "selected_ipv4": selected_ipv4,
            "synced_domains": domains,
            "managed_rule_count": domains.len(),
            "last_sync_at": now,
            "last_sync_error": Value::Null,
        }))
    }
    .await;

    let runtime = match runtime_result {
        Ok(runtime) => runtime,
        Err(message) => {
            let runtime = json!({
                "selected_ipv4": selected_ipv4,
                "synced_domains": [],
                "managed_rule_count": 0,
                "last_sync_at": Value::Null,
                "last_sync_error": message,
            });
            let _ = state
                .storage
                .store
                .set_json_value(SMART_CONNECT_RUNTIME_KEY, &runtime)
                .await;
            return Err(message);
        }
    };

    state
        .storage
        .store
        .set_json_value(SMART_CONNECT_RUNTIME_KEY, &runtime)
        .await
        .map_err(|error| error.to_string())?;
    Ok(build_smart_connect_details(
        state,
        config,
        runtime,
        &translator,
    ))
}

pub(super) async fn reconcile_smart_connect_for_run_type_change<Sync, SyncFuture>(
    state: &AppState,
    next_config: &mut Value,
    sync_runtime: Sync,
) -> Result<bool, String>
where
    Sync: Fn(AppState, Value) -> SyncFuture,
    SyncFuture: std::future::Future<Output = Result<(), String>>,
{
    let sync_error = match sync_runtime(state.clone(), next_config.clone()).await {
        Ok(()) => return Ok(false),
        Err(error) => error,
    };
    tracing::warn!(
        error = %sync_error,
        "failed to sync smart connect before run type change"
    );

    let mut smart = normalize_smart_connect_config(next_config.get("smart_connect"));
    if smart.get("enabled").and_then(Value::as_bool) != Some(true) {
        return Ok(false);
    }
    ensure_config_object(&mut smart).insert("enabled".to_string(), Value::Bool(false));
    *next_config = state
        .storage
        .store
        .set_config_top_level_value("smart_connect", smart)
        .await
        .map_err(|error| error.to_string())?;

    if let Err(error) = sync_runtime(state.clone(), next_config.clone()).await {
        tracing::warn!(
            %error,
            "failed to clear smart connect runtime after disabling it during run type change"
        );
    }
    Ok(true)
}

pub(super) async fn sync_smart_connect_on_boot(
    state: &AppState,
    config: &Value,
) -> Result<(), String> {
    let smart = normalize_smart_connect_config(config.get("smart_connect"));
    let available =
        host_firewall_available(state) && config.get("run_type").and_then(Value::as_i64) == Some(3);
    let enabled = smart.get("enabled").and_then(Value::as_bool) == Some(true);
    if available && enabled {
        sync_smart_connect(state, config).await?;
        return Ok(());
    }

    let selected_ipv4 = smart
        .get("selected_ipv4")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if Path::new(SMART_CONNECT_MANAGED_CONF_PATH).exists() {
        fs::remove_file(SMART_CONNECT_MANAGED_CONF_PATH).map_err(|error| error.to_string())?;
        let translator = Translator::from_state(state).await;
        restart_dnsmasq_service(&translator)?;
    }
    let runtime = json!({
        "selected_ipv4": selected_ipv4,
        "synced_domains": [],
        "managed_rule_count": 0,
        "last_sync_at": time_utils::now_iso(),
        "last_sync_error": Value::Null,
    });
    state
        .storage
        .store
        .set_json_value(SMART_CONNECT_RUNTIME_KEY, &runtime)
        .await
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub(crate) fn schedule_smart_connect_sync_after_host_mappings_change(
    state: AppState,
    config: Value,
) {
    let smart = normalize_smart_connect_config(config.get("smart_connect"));
    if config.get("run_type").and_then(Value::as_i64) != Some(3)
        || smart.get("enabled").and_then(Value::as_bool) != Some(true)
    {
        return;
    }

    let task_state = state.clone();
    state.spawn_background("smart-connect-sync", async move {
        let latest_config = match task_state.storage.store.get_config().await {
            Ok(config) => config,
            Err(error) => {
                tracing::warn!(
                    %error,
                    "failed to load config for smart connect background sync after host mappings change"
                );
                config
            }
        };
        if let Err(message) = sync_smart_connect(&task_state, &latest_config).await {
            tracing::warn!(
                %message,
                "failed to sync smart connect after host mappings change"
            );
        }
    });
}

pub(super) fn build_smart_connect_details(
    state: &AppState,
    config: &Value,
    runtime: Value,
    translator: &Translator,
) -> Value {
    let smart = normalize_smart_connect_config(config.get("smart_connect"));
    let available =
        host_runtime_available(state) && config.get("run_type").and_then(Value::as_i64) == Some(3);
    let reason = if available {
        String::new()
    } else if !host_runtime_available(state) {
        capability_blocked_text(state, "smart_connect_available", translator)
    } else {
        let mode = smart_connect_run_type_label(
            translator,
            config.get("run_type").and_then(Value::as_i64).unwrap_or(3),
        );
        smart_connect_text_params(translator, "unavailableReason", &[("mode", mode)])
    };
    json!({
        "config": smart,
        "availability": {
            "available": available,
            "reason": reason,
        },
        "dnsmasq": merge_dnsmasq_runtime(
            system_assets::build_dnsmasq_status_with_translator(translator),
            runtime
        ),
        "domains": list_smart_connect_domains(config),
        "local_ip_options": list_private_ipv4_candidates(),
    })
}

pub(super) fn merge_dnsmasq_runtime(mut status: Value, runtime: Value) -> Value {
    if let Some(object) = status.as_object_mut() {
        object.insert("runtime".to_string(), runtime);
    }
    status
}

pub(super) fn smart_connect_run_type_label(translator: &Translator, run_type: i64) -> String {
    match run_type {
        0 => smart_connect_text(translator, "runTypes.direct"),
        1 => smart_connect_text(translator, "runTypes.reverseProxy"),
        3 => smart_connect_text(translator, "runTypes.subdomain"),
        _ => smart_connect_text(translator, "currentMode"),
    }
}

pub(super) fn list_smart_connect_domains(config: &Value) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut auth_hosts = Vec::new();
    let mut app_hosts = Vec::new();
    if let Some(mappings) = config.get("host_mappings").and_then(Value::as_array) {
        for mapping in mappings {
            let host = normalize_host(mapping.get("host").and_then(Value::as_str).unwrap_or(""));
            if host.is_empty() || !seen.insert(host.clone()) {
                continue;
            }
            if mapping.get("service_role").and_then(Value::as_str) == Some("auth") {
                auth_hosts.push(host);
            } else {
                app_hosts.push(host);
            }
        }
    }
    auth_hosts.extend(app_hosts);
    auth_hosts
}

pub(super) fn normalize_host(value: &str) -> String {
    let lower = value.trim().to_lowercase();
    let without_scheme = strip_alpha_scheme(&lower);
    without_scheme
        .split('/')
        .next()
        .unwrap_or("")
        .trim_end_matches('.')
        .to_string()
}

pub(super) fn strip_alpha_scheme(value: &str) -> &str {
    let Some((scheme, rest)) = value.split_once("://") else {
        return value;
    };
    if !scheme.is_empty() && scheme.chars().all(|ch| ch.is_ascii_alphabetic()) {
        rest
    } else {
        value
    }
}

pub(super) fn list_private_ipv4_candidates() -> Vec<Value> {
    let Ok(items) = get_if_addrs::get_if_addrs() else {
        return Vec::new();
    };
    let mut seen = BTreeSet::new();
    let mut output = Vec::new();
    for item in items {
        if item.is_loopback() || is_excluded_interface(&item.name) {
            continue;
        }
        let get_if_addrs::IfAddr::V4(v4) = item.addr else {
            continue;
        };
        let address = v4.ip.to_string();
        if !is_private_ipv4(&address) || !seen.insert(address.clone()) {
            continue;
        }
        let netmask = v4.netmask.to_string();
        let prefix = ipv4_netmask_to_prefix(v4.netmask);
        output.push(json!({
            "label": format!("{} ({})", address, item.name),
            "value": address,
            "interface": item.name,
            "netmask": netmask,
            "prefix": prefix,
        }));
    }
    output.sort_by(|left, right| {
        let left_key = format!(
            "{}\0{}",
            left.get("interface").and_then(Value::as_str).unwrap_or(""),
            left.get("value").and_then(Value::as_str).unwrap_or("")
        );
        let right_key = format!(
            "{}\0{}",
            right.get("interface").and_then(Value::as_str).unwrap_or(""),
            right.get("value").and_then(Value::as_str).unwrap_or("")
        );
        left_key.cmp(&right_key)
    });
    output
}

pub(super) fn is_excluded_interface(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower == "lo"
        || lower.starts_with("docker")
        || lower.starts_with("br-")
        || lower.starts_with("veth")
        || lower.starts_with("tailscale")
        || lower.starts_with("zt")
        || lower.starts_with("tun")
        || lower.starts_with("tap")
        || lower.starts_with("wg")
}

pub(super) fn ipv4_netmask_to_prefix(mask: Ipv4Addr) -> Option<u8> {
    let mask = u32::from(mask);
    let mut prefix = 0;
    let mut seen_zero = false;
    for bit in (0..32).rev() {
        let one = (mask & (1 << bit)) != 0;
        if one && seen_zero {
            return None;
        }
        if one {
            prefix += 1;
        } else {
            seen_zero = true;
        }
    }
    Some(prefix)
}

pub(super) fn is_private_ipv4(value: &str) -> bool {
    let Ok(ip) = value.parse::<Ipv4Addr>() else {
        return false;
    };
    let [a, b, _, _] = ip.octets();
    a == 10 || (a == 172 && (16..=31).contains(&b)) || (a == 192 && b == 168)
}

pub(super) fn apply_smart_connect_managed_config(
    selected_ipv4: &str,
    domains: &[String],
    translator: &Translator,
) -> Result<(), String> {
    let content = build_smart_connect_managed_config(selected_ipv4, domains);
    let path = Path::new(SMART_CONNECT_MANAGED_CONF_PATH);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let tmp = format!("{}.tmp", SMART_CONNECT_MANAGED_CONF_PATH);
    fs::write(&tmp, content).map_err(|error| error.to_string())?;
    fs::rename(&tmp, SMART_CONNECT_MANAGED_CONF_PATH).map_err(|error| error.to_string())?;
    restart_dnsmasq_service(translator)
}

pub(super) fn clear_smart_connect_managed_config(translator: &Translator) -> Result<(), String> {
    if Path::new(SMART_CONNECT_MANAGED_CONF_PATH).exists() {
        fs::remove_file(SMART_CONNECT_MANAGED_CONF_PATH).map_err(|error| error.to_string())?;
        restart_dnsmasq_service(translator)?;
    }
    Ok(())
}

pub(super) fn build_smart_connect_managed_config(
    selected_ipv4: &str,
    domains: &[String],
) -> String {
    let normalized_ipv4 = selected_ipv4.trim();
    let mut normalized_domains = Vec::new();
    for domain in domains {
        let domain = domain.trim().to_lowercase();
        if !domain.is_empty() && !normalized_domains.contains(&domain) {
            normalized_domains.push(domain);
        }
    }
    let mut listen_addresses = vec!["127.0.0.1".to_string()];
    if !normalized_ipv4.is_empty() && !listen_addresses.iter().any(|item| item == normalized_ipv4) {
        listen_addresses.push(normalized_ipv4.to_string());
    }
    let mut lines = vec![
        "# Managed by fn-knock smart connect. Do not edit manually.".to_string(),
        format!("local-ttl={SMART_CONNECT_LOCAL_TTL_SECONDS}"),
        format!("listen-address={}", listen_addresses.join(",")),
        "bind-interfaces".to_string(),
    ];
    for domain in normalized_domains {
        lines.push(format!("address=/{domain}/{normalized_ipv4}"));
        lines.push(format!("local=/{domain}/"));
    }
    lines.push(String::new());
    lines.join("\n")
}

pub(super) fn restart_dnsmasq_service(translator: &Translator) -> Result<(), String> {
    if Command::new("systemctl")
        .args(["restart", "dnsmasq"])
        .status()
        .is_ok_and(|status| status.success())
    {
        return Ok(());
    }
    if Command::new("service")
        .args(["dnsmasq", "restart"])
        .status()
        .is_ok_and(|status| status.success())
    {
        return Ok(());
    }
    Err(smart_connect_text(translator, "syncFailed"))
}
