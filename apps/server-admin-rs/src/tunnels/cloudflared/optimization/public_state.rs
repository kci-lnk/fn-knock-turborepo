use super::*;

pub(in crate::tunnels::cloudflared) async fn public_state(
    state: &AppState,
    managed: &Value,
    ownership: &Value,
) -> Value {
    let runtime = load_runtime(state).await;
    let domain_settings = load_domain_settings(state).await.unwrap_or_default();
    let external_hostnames = domain_settings
        .external_hostnames
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let (sources, source_settings_error) = match load_source_settings(state).await {
        Ok(value) => (value, None),
        Err(error) => (
            OptimizationSourceSettings {
                official_ranges: false,
                builtin_ids: Vec::new(),
                custom_hostnames: Vec::new(),
            },
            Some(error.to_string()),
        ),
    };
    let local = state
        .storage
        .store
        .get_config()
        .await
        .unwrap_or_else(|_| json!({}));
    let host_states = ownership
        .pointer("/optimization/customHostnames")
        .and_then(Value::as_object);
    let domains = configured_hosts(&local)
        .into_iter()
        .map(|host| {
            let current = host_states.and_then(|items| items.get(&host));
            let external = external_hostnames.contains(host.as_str());
            json!({
                "hostname": host,
                "managementMode": if external { "external" } else { "optimize" },
                "status": if external {
                    json!("external")
                } else {
                    current.and_then(|value| value.get("status")).cloned().unwrap_or_else(|| json!("fallback"))
                },
                "hostnameStatus": current
                    .and_then(custom_hostname_activation_status)
                    .map(Value::from)
                    .unwrap_or(Value::Null),
                "sslStatus": current.and_then(|value| value.get("sslStatus")).cloned().unwrap_or(Value::Null),
                "customHostnameId": current.and_then(|value| value.get("id")).cloned().unwrap_or(Value::Null),
                "optimized": !external && current.is_some_and(exact_route_is_optimized),
                "actionRequired": !external && current.and_then(|value| value.get("status")).and_then(Value::as_str) == Some("conflict"),
                "cleanupPending": external && current.is_some(),
                "conflictResourceId": current.and_then(|value| value.get("conflictResourceId")).cloned().unwrap_or(Value::Null),
                "messageCode": current.and_then(|value| value.get("messageCode")).cloned().unwrap_or(Value::Null),
                "messageDetail": current.and_then(|value| value.get("messageDetail")).cloned().unwrap_or(Value::Null),
                "message": if external {
                    Value::Null
                } else {
                    current.and_then(|value| value.get("message")).cloned().unwrap_or(Value::Null)
                },
            })
        })
        .collect::<Vec<_>>();
    let latest_jobs = {
        let jobs = state.tunnel.cloudflared_scan_jobs.read().await;
        let mut values = jobs.values().cloned().collect::<Vec<_>>();
        values.sort_by(|left, right| {
            right
                .get("createdAt")
                .and_then(Value::as_str)
                .cmp(&left.get("createdAt").and_then(Value::as_str))
        });
        values.into_iter().take(5).collect::<Vec<_>>()
    };
    let mut public_sources = public_source_settings(&sources);
    ensure_object(&mut public_sources).insert(
        "error".to_string(),
        source_settings_error
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    let scan_ready = scan_validation_hostname(ownership).is_some();
    let scan_readiness_error_code = (!scan_ready)
        .then(|| scan_validation_hostname_error(ownership))
        .and_then(|error| optimization_scan_error_code(&error))
        .map(Value::from)
        .unwrap_or(Value::Null);
    json!({
        "enabled": managed.get("optimizationEnabled").and_then(Value::as_bool).unwrap_or(false),
        "beta": true,
        "ipv4Only": true,
        "selected": ownership.pointer("/optimization/selected").cloned().unwrap_or(Value::Null),
        "fallbackActive": ownership.pointer("/optimization/fallbackActive").and_then(Value::as_bool).unwrap_or(true),
        "publishSuppressed": ownership.pointer("/optimization/publishSuppressed").and_then(Value::as_bool).unwrap_or(false),
        "originHostname": ownership.pointer("/optimization/originDns/name").cloned().unwrap_or(Value::Null),
        "edgeHostname": ownership.pointer("/optimization/edgeDns/name").cloned().unwrap_or(Value::Null),
        "fallbackOrigin": ownership.pointer("/optimization/fallbackOrigin").cloned().unwrap_or(Value::Null),
        "capabilityProbe": ownership.pointer("/optimization/capabilityProbe").cloned().unwrap_or(Value::Null),
        "scanReady": scan_ready,
        "scanReadinessErrorCode": scan_readiness_error_code,
        "candidateSources": public_sources,
        "vantage": runtime.get("lastVantage").cloned().unwrap_or(Value::Null),
        "sourceWarnings": runtime.get("lastSourceWarnings").cloned().unwrap_or_else(|| json!([])),
        "resolverDiagnostics": runtime.get("lastResolverDiagnostics").cloned().unwrap_or_else(|| json!([])),
        "resolutionPath": runtime.get("lastResolutionPath").cloned().unwrap_or(Value::Null),
        "domains": domains,
        "schedule": {
            "fullScanIntervalDays": 7,
            "healthCheckIntervalMinutes": 15,
            "nextFullScanAt": runtime.get("nextFullScanAt").cloned().unwrap_or(Value::Null),
            "lastFullScanAt": runtime.get("lastFullScanAt").cloned().unwrap_or(Value::Null),
            "lastHealthAt": runtime.get("lastHealthAt").cloned().unwrap_or(Value::Null),
            "healthFailures": runtime.get("healthFailures").cloned().unwrap_or_else(|| json!(0)),
            "lastSwitchReason": runtime.get("lastSwitchReason").cloned().unwrap_or(Value::Null),
            "lastError": runtime.get("lastError").cloned().unwrap_or(Value::Null),
        },
        "scans": latest_jobs,
    })
}
