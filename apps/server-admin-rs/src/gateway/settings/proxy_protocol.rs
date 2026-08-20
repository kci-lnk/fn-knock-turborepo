use super::*;

pub(super) fn default_gateway_proxy_protocol() -> Value {
    json!({ "enabled": false, "trusted_sources": [] })
}

pub(super) fn managed_frp_proxy_protocol_enabled(config: &Value) -> bool {
    config.get("run_type").and_then(Value::as_i64) == Some(1)
}

pub(super) fn normalize_gateway_proxy_protocol(value: &Value) -> Result<Value, String> {
    let enabled = value
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let raw_sources = value
        .get("trusted_sources")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut sources = BTreeSet::new();

    for raw in raw_sources {
        let source = raw
            .as_str()
            .ok_or_else(|| "Each trusted PROXY source must be a string".to_string())?
            .trim();
        if source.is_empty() {
            continue;
        }
        if let Ok(address) = source.parse::<IpAddr>() {
            let canonical = match address {
                IpAddr::V6(address) => address
                    .to_ipv4_mapped()
                    .map(IpAddr::V4)
                    .unwrap_or(IpAddr::V6(address)),
                address => address,
            };
            sources.insert(canonical.to_string());
            continue;
        }
        let network = source
            .parse::<IpNet>()
            .map_err(|_| format!("Trusted PROXY source {source:?} must be an IP address or CIDR"))?
            .trunc();
        if network.prefix_len() == 0 {
            return Err(format!(
                "Trusted PROXY source {source:?} must not cover every address"
            ));
        }
        sources.insert(network.to_string());
    }

    if enabled && sources.is_empty() {
        return Err(
            "At least one trusted source is required when PROXY protocol is enabled".to_string(),
        );
    }

    Ok(json!({
        "enabled": enabled,
        "trusted_sources": sources.into_iter().collect::<Vec<_>>(),
    }))
}

pub(super) fn gateway_proxy_protocol_from_body(body: &Value) -> Result<Value, String> {
    let object = body
        .as_object()
        .ok_or_else(|| "Gateway PROXY protocol payload must be an object".to_string())?;
    if !object.get("enabled").is_some_and(Value::is_boolean) {
        return Err("PROXY protocol enabled must be a boolean".to_string());
    }
    if !object.get("trusted_sources").is_some_and(Value::is_array) {
        return Err("PROXY protocol trusted_sources must be an array".to_string());
    }
    normalize_gateway_proxy_protocol(body)
}

pub(super) fn build_gateway_proxy_protocol_response(config: &Value) -> Result<Value, String> {
    let external = normalize_gateway_proxy_protocol(
        config
            .get("gateway_proxy_protocol")
            .unwrap_or(&default_gateway_proxy_protocol()),
    )?;
    let managed = managed_frp_proxy_protocol_enabled(config);
    let enabled = external
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    Ok(json!({
        "enabled": enabled,
        "trusted_sources": external.get("trusted_sources").cloned().unwrap_or_else(|| json!([])),
        "managed_frp_enabled": managed,
        "effective_enabled": managed || enabled,
    }))
}

pub(super) async fn sync_gateway_proxy_protocol_runtime(
    state: &AppState,
    config: &Value,
) -> Result<(), String> {
    let normalized = normalize_gateway_proxy_protocol(
        config
            .get("gateway_proxy_protocol")
            .unwrap_or(&default_gateway_proxy_protocol()),
    )?;
    let expected_enabled = normalized
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let expected_sources = normalized
        .get("trusted_sources")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let (applied_enabled, applied_sources) = state
        .gateway
        .client
        .set_gateway_proxy_protocol_config(expected_enabled, expected_sources.clone())
        .await
        .map_err(|error| error.to_string())?;
    if applied_enabled != expected_enabled || applied_sources != expected_sources {
        return Err(format!(
            "Go gateway did not apply PROXY protocol config (enabled={expected_enabled}, trusted_sources={expected_sources:?})"
        ));
    }
    Ok(())
}

pub(super) async fn update_gateway_proxy_protocol_inner(
    state: &AppState,
    body: &Value,
) -> Result<Value, String> {
    let normalized = gateway_proxy_protocol_from_body(body)?;
    // Share the gateway runtime transaction with startup and host-rule syncs:
    // all of them can reconfigure the same Go listener and must not interleave
    // persistence, application, or rollback.
    proxy_config::with_host_mappings_runtime_transaction(state, move |state| async move {
        update_gateway_proxy_protocol_locked(&state, normalized).await
    })
    .await?;

    let current = state
        .storage
        .store
        .get_config()
        .await
        .map_err(|error| error.to_string())?;
    build_gateway_proxy_protocol_response(&current)
}

async fn update_gateway_proxy_protocol_locked(
    state: &AppState,
    normalized: Value,
) -> Result<(), String> {
    let previous = state
        .storage
        .store
        .get_config()
        .await
        .map_err(|error| error.to_string())?;
    let mut next = previous.clone();
    ensure_object(&mut next).insert("gateway_proxy_protocol".to_string(), normalized);

    state
        .storage
        .store
        .save_config(&next)
        .await
        .map_err(|error| error.to_string())?;
    if let Err(error) = sync_gateway_proxy_protocol_runtime(state, &next).await {
        let storage_rollback_error =
            state
                .storage
                .store
                .save_config(&previous)
                .await
                .err()
                .map(|rollback_error| {
                    format!("failed to restore previous PROXY protocol config: {rollback_error}")
                });
        let runtime_rollback_error = sync_gateway_proxy_protocol_runtime(state, &previous)
            .await
            .err()
            .map(|rollback_error| {
                format!("failed to restore previous PROXY protocol runtime: {rollback_error}")
            });
        let rollback_failures = [storage_rollback_error, runtime_rollback_error]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        if rollback_failures.is_empty() {
            return Err(error);
        }
        return Err(format!("{error}; {}", rollback_failures.join("; ")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_and_deduplicates_trusted_sources() {
        let value = json!({
            "enabled": true,
            "trusted_sources": [
                " 192.0.2.10 ",
                "192.0.2.10",
                "::ffff:192.0.2.10",
                "10.0.0.9/24",
                "2001:db8::1/64"
            ]
        });
        assert_eq!(
            normalize_gateway_proxy_protocol(&value).unwrap(),
            json!({
                "enabled": true,
                "trusted_sources": ["10.0.0.0/24", "192.0.2.10", "2001:db8::/64"]
            })
        );
    }

    #[test]
    fn rejects_missing_dns_and_world_sources() {
        for value in [
            json!({"enabled": true, "trusted_sources": []}),
            json!({"enabled": true, "trusted_sources": ["proxy.example.com"]}),
            json!({"enabled": true, "trusted_sources": ["0.0.0.0/0"]}),
            json!({"enabled": true, "trusted_sources": ["::/0"]}),
        ] {
            assert!(normalize_gateway_proxy_protocol(&value).is_err(), "{value}");
        }
    }

    #[test]
    fn response_distinguishes_managed_and_external_enablement() {
        let managed = build_gateway_proxy_protocol_response(&json!({
            "run_type": 1,
            "gateway_proxy_protocol": {"enabled": false, "trusted_sources": []}
        }))
        .unwrap();
        assert_eq!(managed["managed_frp_enabled"], true);
        assert_eq!(managed["effective_enabled"], true);

        let external = build_gateway_proxy_protocol_response(&json!({
            "run_type": 0,
            "gateway_proxy_protocol": {
                "enabled": true,
                "trusted_sources": ["192.0.2.10"]
            }
        }))
        .unwrap();
        assert_eq!(external["managed_frp_enabled"], false);
        assert_eq!(external["effective_enabled"], true);
    }
}
