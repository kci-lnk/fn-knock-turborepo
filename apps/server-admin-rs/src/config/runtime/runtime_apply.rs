use super::*;

pub(super) fn log_go_value_result(result: Result<Value, String>, operation: &'static str) {
    match result {
        Ok(value) => {
            if let Err(error) = ensure_go_success(value) {
                tracing::warn!(%error, operation, "go backend call failed during run type apply");
            }
        }
        Err(error) => {
            tracing::warn!(%error, operation, "go backend request failed during run type apply");
        }
    }
}

pub(super) fn log_go_status_value_result(
    result: Result<(reqwest::StatusCode, Value), String>,
    operation: &'static str,
) {
    match result {
        Ok((status, value)) => {
            if !status.is_success() {
                let error = go_response_message(&value, &format!("go backend returned {status}"));
                tracing::warn!(%error, operation, "go backend call failed during run type apply");
                return;
            }
            if let Err(error) = ensure_go_success(value) {
                tracing::warn!(%error, operation, "go backend call failed during run type apply");
            }
        }
        Err(error) => {
            tracing::warn!(%error, operation, "go backend request failed during run type apply");
        }
    }
}

pub(crate) async fn apply_run_type_config(
    state: &AppState,
    config: &Value,
    run_type: i64,
) -> Result<(), String> {
    log_go_value_result(
        state
            .go_backend
            .set_auth_config(&build_gateway_auth_config(config))
            .await
            .map_err(|error| error.to_string()),
        "sync auth gateway config",
    );
    let default_throttle = json!({
        "enabled": true,
        "requests_per_second": 100,
        "burst": 200,
        "block_seconds": 30,
    });
    let throttle = config
        .get("reverse_proxy_throttle")
        .unwrap_or(&default_throttle);
    log_go_value_result(
        state
            .go_backend
            .set_reverse_proxy_throttle(throttle)
            .await
            .map_err(|error| error.to_string()),
        "sync reverse proxy throttle",
    );
    let default_crawler = json!({ "enabled": false });
    let crawler = config
        .get("gateway_crawler_blocker")
        .unwrap_or(&default_crawler);
    log_go_value_result(
        state
            .go_backend
            .set_crawler_blocker_config(crawler)
            .await
            .map_err(|error| error.to_string()),
        "sync crawler blocker config",
    );
    whitelist::sync_reverse_proxy_trusted_ips(state).await;
    if let Err(error) = gateway_settings::sync_gateway_visibility_runtime_from_store(state).await {
        tracing::warn!(
            %error,
            "failed to sync gateway visibility runtime during run type apply"
        );
    }
    if let Err(error) =
        gateway_settings::sync_gateway_target_runtime_for_config(state, config, false).await
    {
        tracing::warn!(
            %error,
            "failed to sync gateway target runtime during run type apply"
        );
    }

    let protocol_mapping_feature = load_protocol_mapping_feature(state, Some(config))
        .await
        .map_err(|error| error.to_string())?;
    let protocol_mapping_enabled = run_type == 3
        && protocol_mapping_feature
            .get("enabled")
            .and_then(Value::as_bool)
            == Some(true);

    if run_type == 1 {
        log_go_value_result(
            state
                .go_backend
                .set_proxy_protocol_force(true)
                .await
                .map_err(|error| error.to_string()),
            "enable proxy protocol force",
        );
        log_go_value_result(
            state
                .go_backend
                .flush_stream_rules()
                .await
                .map_err(|error| error.to_string()),
            "flush stream rules",
        );

        if is_reverse_proxy_subdomain_mode(config) {
            log_go_value_result(
                state
                    .go_backend
                    .flush_rules()
                    .await
                    .map_err(|error| error.to_string()),
                "flush path rules",
            );
            sync_host_rules(state, config).await;
            log_go_status_value_result(
                state
                    .go_backend
                    .set_default_route("/__select__")
                    .await
                    .map_err(|error| error.to_string()),
                "sync disabled default route",
            );
            return Ok(());
        }

        log_go_value_result(
            state
                .go_backend
                .flush_host_rules()
                .await
                .map_err(|error| error.to_string()),
            "flush host rules",
        );
        sync_path_rules(state, config).await;
        sync_default_route(state, config).await;
        return Ok(());
    }

    log_go_value_result(
        state
            .go_backend
            .set_proxy_protocol_force(false)
            .await
            .map_err(|error| error.to_string()),
        "disable proxy protocol force",
    );

    if run_type == 3 {
        log_go_value_result(
            state
                .go_backend
                .flush_rules()
                .await
                .map_err(|error| error.to_string()),
            "flush path rules",
        );
        sync_host_rules(state, config).await;
        if protocol_mapping_enabled {
            sync_stream_rules(state, config).await;
        } else {
            log_go_value_result(
                state
                    .go_backend
                    .flush_stream_rules()
                    .await
                    .map_err(|error| error.to_string()),
                "flush stream rules",
            );
        }
        sync_default_route(state, config).await;
        maybe_apply_host_firewall(state, config, run_type, protocol_mapping_enabled).await?;
        return Ok(());
    }

    log_go_value_result(
        state
            .go_backend
            .flush_host_rules()
            .await
            .map_err(|error| error.to_string()),
        "flush host rules",
    );
    log_go_value_result(
        state
            .go_backend
            .flush_stream_rules()
            .await
            .map_err(|error| error.to_string()),
        "flush stream rules",
    );
    sync_path_rules(state, config).await;
    sync_default_route(state, config).await;
    if run_type == 0 {
        sync_auth_entry_route(state).await;
    }
    maybe_apply_host_firewall(state, config, run_type, protocol_mapping_enabled).await
}

pub(super) async fn sync_path_rules(state: &AppState, config: &Value) {
    log_go_value_result(
        state
            .go_backend
            .set_rules(
                config
                    .get("proxy_mappings")
                    .unwrap_or(&Value::Array(Vec::new())),
            )
            .await
            .map_err(|error| error.to_string()),
        "sync path rules",
    );
}

pub(super) async fn sync_host_rules(state: &AppState, config: &Value) {
    let mappings = config
        .get("host_mappings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    log_go_value_result(
        state
            .go_backend
            .set_host_rules(&build_host_rules_payload(&mappings))
            .await
            .map_err(|error| error.to_string()),
        "sync host rules",
    );
}

pub(super) async fn sync_stream_rules(state: &AppState, config: &Value) {
    log_go_value_result(
        state
            .go_backend
            .set_stream_rules(
                config
                    .get("stream_mappings")
                    .unwrap_or(&Value::Array(Vec::new())),
            )
            .await
            .map_err(|error| error.to_string()),
        "sync stream rules",
    );
}

pub(super) async fn sync_auth_entry_route(state: &AppState) {
    log_go_value_result(
        state
            .go_backend
            .set_rules(&auth_entry_route_payload(state.settings.auth_port))
            .await
            .map_err(|error| error.to_string()),
        "sync auth entry route",
    );
    log_go_status_value_result(
        state
            .go_backend
            .set_default_route("/auth")
            .await
            .map_err(|error| error.to_string()),
        "sync auth default route",
    );
}

pub(super) fn auth_entry_route_payload(auth_port: u16) -> Value {
    json!([{
        "path": "/auth",
        "target": format!("http://127.0.0.1:{auth_port}"),
        "rewrite_html": false,
        "use_auth": false,
        "use_root_mode": false,
        "strip_path": false,
    }])
}

pub(super) async fn sync_default_route(state: &AppState, config: &Value) {
    let route = config
        .get("default_route")
        .and_then(Value::as_str)
        .unwrap_or("/__select__");
    log_go_status_value_result(
        state
            .go_backend
            .set_default_route(route)
            .await
            .map_err(|error| error.to_string()),
        "sync default route",
    );
}

pub(super) async fn maybe_apply_host_firewall(
    state: &AppState,
    config: &Value,
    run_type: i64,
    protocol_mapping_enabled: bool,
) -> Result<(), String> {
    if !host_firewall_available(state) {
        return Ok(());
    }
    if run_type != 0 && !normalize_auto_manage_firewall(config.get("auto_manage_firewall")) {
        return Ok(());
    }
    if run_type == 1 {
        clear_legacy_gateway_redirects(state, gateway_port(), false).await?;
        log_go_value_result(
            state
                .go_backend
                .clean_iptables()
                .await
                .map_err(|error| error.to_string()),
            "clean iptables",
        );
        return Ok(());
    }
    let payload = json!({
        "chain_name": "FN-KNOCK-FW",
        "parent_chain": ["INPUT", "DOCKER-USER"],
        "exempt_ports": exempt_ports(config, protocol_mapping_enabled, run_type),
    });
    if run_type == 3 {
        log_go_value_result(
            state
                .go_backend
                .init_iptables(&payload)
                .await
                .map_err(|error| error.to_string()),
            "init default firewall",
        );
        clear_legacy_gateway_redirects(state, gateway_port(), false).await?;
        return Ok(());
    }

    clear_legacy_gateway_redirects(state, gateway_port(), false).await?;
    log_go_value_result(
        state
            .go_backend
            .init_iptables(&payload)
            .await
            .map_err(|error| error.to_string()),
        "init default firewall",
    );
    sync_active_whitelist_targets(state, false).await?;
    Ok(())
}

pub(super) async fn rollback_config_protocol_feature_and_runtime(
    state: &AppState,
    previous_config: &Value,
    previous_protocol_mapping_feature: &Value,
    run_type: i64,
) {
    if let Err(error) = state.store.save_config(previous_config).await {
        tracing::warn!(%error, "failed to rollback runtime config");
        return;
    }
    if let Err(error) =
        save_protocol_mapping_feature(state, previous_protocol_mapping_feature).await
    {
        tracing::warn!(%error, "failed to rollback protocol mapping feature");
        return;
    }
    if let Err(error) = sync_smart_connect(state, previous_config).await {
        tracing::warn!(%error, "failed to rollback smart connect runtime");
        return;
    }
    if let Err(error) = apply_run_type_config(state, previous_config, run_type).await {
        tracing::warn!(%error, "failed to rollback runtime state");
    }
}

pub(super) async fn reset_firewall_for_run_type(
    state: &AppState,
    run_type: i64,
) -> Result<Value, String> {
    let config = state
        .store
        .get_config()
        .await
        .map_err(|error| error.to_string())?;
    let protocol_mapping_feature = load_protocol_mapping_feature(state, Some(&config))
        .await
        .map_err(|error| error.to_string())?;
    let protocol_mapping_enabled = run_type == 3
        && protocol_mapping_feature
            .get("enabled")
            .and_then(Value::as_bool)
            == Some(true);
    clear_legacy_gateway_redirects(state, gateway_port(), true).await?;
    ensure_go_success(
        state
            .go_backend
            .clean_iptables()
            .await
            .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;

    if run_type != 1 {
        let payload = json!({
            "chain_name": "FN-KNOCK-FW",
            "parent_chain": ["INPUT", "DOCKER-USER"],
            "exempt_ports": exempt_ports(&config, protocol_mapping_enabled, run_type),
        });
        ensure_go_success(
            state
                .go_backend
                .init_iptables(&payload)
                .await
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
    }

    let whitelist_synced = if run_type == 0 {
        sync_active_whitelist_targets(state, true).await?
    } else {
        0
    };

    Ok(json!({
        "runType": run_type,
        "gatewayPort": gateway_port(),
        "exemptPorts": if run_type == 1 {
            Vec::<String>::new()
        } else {
            exempt_ports(&config, protocol_mapping_enabled, run_type)
        },
        "whitelistSynced": whitelist_synced,
    }))
}

pub(super) async fn clear_legacy_gateway_redirects(
    state: &AppState,
    target_port: i64,
    strict: bool,
) -> Result<(), String> {
    let translator = Translator::from_state(state).await;
    for listen_port in LEGACY_REDIRECTED_HTTP_PORTS {
        let fallback = firewall_text_params(
            &translator,
            "clearLegacyTcpRedirectFailed",
            &[
                ("listenPort", listen_port.to_string()),
                ("targetPort", target_port.to_string()),
            ],
        );
        let result = match state
            .go_backend
            .clear_tcp_redirect(listen_port, target_port)
            .await
        {
            Ok((status, value)) => {
                ensure_go_success_with_acceptable_codes(status, value, &[404], &fallback)
            }
            Err(_) => Err(fallback),
        };
        if strict {
            result?;
        } else if let Err(error) = result {
            tracing::warn!(
                %error,
                listen_port,
                target_port,
                "failed to clear legacy TCP redirect"
            );
        }
    }
    Ok(())
}

pub(super) async fn sync_active_whitelist_targets(
    state: &AppState,
    strict: bool,
) -> Result<usize, String> {
    let targets = state
        .store
        .list_whitelist_active_concrete_targets()
        .await
        .map_err(|error| error.to_string())?;
    let mut concrete_targets = Vec::new();
    for target in targets {
        let value = target.target.trim();
        if !value.is_empty() {
            concrete_targets.push(value.to_string());
        }
    }

    let translator = Translator::from_state(state).await;
    for target in &concrete_targets {
        let fallback = firewall_text_params(
            &translator,
            "syncWhitelistTargetFailed",
            &[("target", target.to_string())],
        );
        let result = match state.go_backend.allow_ip(target).await {
            Ok(value) => ensure_go_success(value).map_err(|_| fallback.clone()),
            Err(_) => Err(fallback),
        };
        if strict {
            result?;
        } else if let Err(error) = result {
            tracing::warn!(%error, %target, "failed to sync whitelist target to Go backend");
        }
    }

    Ok(concrete_targets.len())
}

pub(super) fn ensure_go_success_with_acceptable_codes(
    status: reqwest::StatusCode,
    value: Value,
    acceptable_codes: &[u16],
    fallback: &str,
) -> Result<(), String> {
    let code = if status.is_success() {
        go_response_code(&value).unwrap_or_else(|| status.as_u16())
    } else {
        status.as_u16()
    };
    if acceptable_codes.contains(&code) {
        return Ok(());
    }
    if status.is_success()
        && value
            .get("success")
            .and_then(Value::as_bool)
            .unwrap_or(true)
    {
        return Ok(());
    }
    Err(go_response_message(&value, fallback))
}

pub(super) fn go_response_code(value: &Value) -> Option<u16> {
    value
        .get("code")
        .and_then(Value::as_u64)
        .and_then(|code| u16::try_from(code).ok())
}

pub(super) fn firewall_reset_success_message(
    translator: &Translator,
    data: &Value,
    run_type: i64,
) -> String {
    let whitelist_message = if run_type == 0 {
        admin_text_params(
            translator,
            "firewall.whitelistSynced",
            &[(
                "count",
                data.get("whitelistSynced")
                    .and_then(Value::as_i64)
                    .unwrap_or(0)
                    .to_string(),
            )],
        )
    } else {
        String::new()
    };
    let exempt_ports_message = if run_type == 0 || run_type == 3 {
        let ports = data
            .get("exemptPorts")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        admin_text_params(translator, "firewall.exemptPorts", &[("ports", ports)])
    } else {
        String::new()
    };
    admin_text_params(
        translator,
        "firewall.resetSuccess",
        &[
            ("runType", admin_run_type_label(translator, run_type)),
            ("whitelistMessage", whitelist_message),
            ("exemptPortsMessage", exempt_ports_message),
        ],
    )
}

pub(super) fn admin_run_type_label(translator: &Translator, run_type: i64) -> String {
    match run_type {
        0 => admin_text(translator, "runTypes.direct"),
        1 => admin_text(translator, "runTypes.reverseProxy"),
        3 => admin_text(translator, "runTypes.subdomain"),
        _ => run_type.to_string(),
    }
}

pub(super) fn gateway_port() -> i64 {
    gateway_port_from_env(std::env::var("GO_REPROXY_PORT").ok())
}

pub(super) fn gateway_port_from_env(value: Option<String>) -> i64 {
    let raw = value
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "7999".to_string());
    crate::node_compat::parse_i64_prefix(raw.trim_start())
        .filter(|port| *port > 0)
        .unwrap_or(7999)
}

pub(super) fn exempt_ports(
    config: &Value,
    protocol_mapping_enabled: bool,
    run_type: i64,
) -> Vec<String> {
    let mut ports = BTreeSet::new();
    ports.insert(gateway_port().to_string());
    if run_type == 3
        && protocol_mapping_enabled
        && let Some(mappings) = config.get("stream_mappings").and_then(Value::as_array)
    {
        for mapping in mappings {
            if let Some(port) = mapping.get("listen_port").and_then(Value::as_i64)
                && (1..=65535).contains(&port)
            {
                ports.insert(port.to_string());
            }
        }
    }
    let smart = normalize_smart_connect_config(config.get("smart_connect"));
    if run_type == 3
        && smart.get("enabled").and_then(Value::as_bool) == Some(true)
        && smart
            .get("selected_ipv4")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty())
    {
        ports.insert(SMART_CONNECT_DNS_PORT.to_string());
    }
    ports.into_iter().collect()
}
