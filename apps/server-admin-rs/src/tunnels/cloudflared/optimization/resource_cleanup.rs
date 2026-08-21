use super::*;

pub(in crate::tunnels::cloudflared) async fn fallback_to_wildcard(
    state: &AppState,
    api: &CloudflareApi,
    managed: &Value,
    ownership: &mut Value,
) -> Result<(), CloudflareApiError> {
    let zone_id = managed.get("zoneId").and_then(Value::as_str).unwrap_or("");
    if zone_id.is_empty() {
        return Ok(());
    }
    let hosts = ownership
        .pointer("/optimization/customHostnames")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let instance_id = managed_instance_id(managed);
    let edge_hostname = ownership
        .pointer("/optimization/edgeDns/name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    for (hostname, value) in hosts {
        if let Some(record_id) = value.get("exactDnsId").and_then(Value::as_str) {
            delete_dns_if_owned(
                api,
                zone_id,
                &tracked_exact_dns_snapshot(
                    &hostname,
                    record_id,
                    &value,
                    ownership,
                    Some(&edge_hostname),
                ),
                &instance_id,
            )
            .await?;
        }
        let mut next = value;
        if next.get("hostnameStatus").is_none()
            && let Some(status) = custom_hostname_activation_status(&next).map(str::to_string)
        {
            ensure_object(&mut next).insert("hostnameStatus".to_string(), json!(status));
        }
        ensure_object(&mut next).remove("exactDnsId");
        ensure_object(&mut next).remove("exactDnsTarget");
        ensure_object(&mut next).insert("status".to_string(), json!("fallback"));
        set_host_state(ownership, &hostname, next);
        save_managed_state(state, ownership).await?;
    }
    let optimization = ensure_nested_object(ownership, &["optimization"]);
    optimization.insert("fallbackActive".to_string(), json!(true));
    optimization.insert("publishSuppressed".to_string(), json!(true));
    optimization.insert("lastFallbackAt".to_string(), json!(time_utils::now_iso()));
    save_managed_state(state, ownership).await
}

pub(in crate::tunnels::cloudflared) async fn cleanup_resources(
    state: &AppState,
    api: &CloudflareApi,
    managed: &Value,
    ownership: &mut Value,
) -> Result<(), CloudflareApiError> {
    let zone_id = managed.get("zoneId").and_then(Value::as_str).unwrap_or("");
    if zone_id.is_empty() {
        return Ok(());
    }
    if let Some(probe) = ownership.pointer("/optimization/capabilityProbe") {
        cleanup_capability_probe(api, zone_id, probe).await?;
    }
    let hosts = ownership
        .pointer("/optimization/customHostnames")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    for (_, host) in hosts {
        if let Some(id) = host.get("exactDnsId").and_then(Value::as_str) {
            ignore_not_found(api.delete_dns_record(zone_id, id).await)?;
        }
        for validation in host
            .get("validationDns")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(id) = validation.get("id").and_then(Value::as_str) {
                ignore_not_found(api.delete_dns_record(zone_id, id).await)?;
            }
        }
        if let Some(id) = host.get("id").and_then(Value::as_str) {
            ignore_not_found(api.delete_custom_hostname(zone_id, id).await)?;
        }
    }
    if let Some(fallback) = ownership.pointer("/optimization/fallbackOrigin").cloned() {
        let expected = fallback.get("origin").and_then(Value::as_str).unwrap_or("");
        let remote = api.get_fallback_origin(zone_id).await?;
        let remote_origin = remote
            .as_ref()
            .and_then(|value| value.get("origin"))
            .and_then(Value::as_str);
        if !expected.is_empty() && remote_origin.is_some() && remote_origin != Some(expected) {
            return Err(CloudflareApiError {
                status: Some(StatusCode::CONFLICT),
                message: "The Cloudflare for SaaS fallback origin changed after preview; refusing to clean it up"
                    .to_string(),
            });
        }
        match fallback.get("ownership").and_then(Value::as_str) {
            Some("dedicated") if remote_origin == Some(expected) => {
                ignore_not_found(api.delete_fallback_origin(zone_id).await)?;
            }
            Some("adopted") if remote_origin == Some(expected) => {
                if let Some(previous) = fallback.get("previousOrigin").and_then(Value::as_str) {
                    if ownership
                        .pointer("/optimization/recoveredOrigins")
                        .and_then(Value::as_object)
                        .is_some_and(|items| items.contains_key(previous))
                    {
                        ignore_not_found(api.delete_fallback_origin(zone_id).await)?;
                    } else {
                        api.update_fallback_origin(zone_id, previous).await?;
                    }
                }
            }
            _ => {}
        }
    }
    for recovered_origin in ownership
        .pointer("/optimization/recoveredOrigins")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|items| items.values())
    {
        if recovered_origin.get("id").and_then(Value::as_str).is_some() {
            delete_dns_if_owned(
                api,
                zone_id,
                recovered_origin,
                &managed_instance_id(managed),
            )
            .await?;
        }
    }
    for path in ["/optimization/originDns/id", "/optimization/edgeDns/id"] {
        if let Some(id) = ownership.pointer(path).and_then(Value::as_str) {
            ignore_not_found(api.delete_dns_record(zone_id, id).await)?;
        }
    }
    ensure_object(ownership).remove("optimization");
    save_managed_state(state, ownership).await
}

pub(super) async fn cleanup_removed_hosts(
    state: &AppState,
    api: &CloudflareApi,
    zone_id: &str,
    ownership: &mut Value,
    configured: &HashSet<String>,
    instance_id: &str,
) -> Result<(), CloudflareApiError> {
    let default_custom_origin = ownership
        .pointer("/optimization/originDns/name")
        .and_then(Value::as_str)
        .map(str::to_string);
    let current = ownership
        .pointer("/optimization/customHostnames")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    for (hostname, host) in current {
        if configured.contains(&hostname) {
            continue;
        }
        if let Some(id) = host.get("exactDnsId").and_then(Value::as_str) {
            delete_dns_if_owned(
                api,
                zone_id,
                &tracked_exact_dns_snapshot(&hostname, id, &host, ownership, None),
                instance_id,
            )
            .await?;
        }
        for record in host
            .get("validationDns")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if record.get("id").and_then(Value::as_str).is_some() {
                delete_dns_if_owned(api, zone_id, record, instance_id).await?;
            }
        }
        if let Some(id) = host.get("id").and_then(Value::as_str) {
            match api.get_custom_hostname(zone_id, id).await {
                Ok(remote)
                    if managed_custom_hostname_matches(
                        &remote,
                        &hostname,
                        &host,
                        default_custom_origin.as_deref(),
                    ) =>
                {
                    ignore_not_found(api.delete_custom_hostname(zone_id, id).await)?;
                }
                Ok(_) => {
                    return Err(CloudflareApiError {
                        status: Some(StatusCode::CONFLICT),
                        message: format!(
                            "Custom Hostname {hostname} changed outside fn-knock; refusing automatic deletion"
                        ),
                    });
                }
                Err(error) if error.status == Some(StatusCode::NOT_FOUND) => {}
                Err(error) => return Err(error),
            }
        }
        if let Some(items) = ownership
            .pointer_mut("/optimization/customHostnames")
            .and_then(Value::as_object_mut)
        {
            items.remove(&hostname);
        }
        save_managed_state(state, ownership).await?;
    }
    Ok(())
}

pub(super) async fn reconcile_optimization_host_membership(
    state: &AppState,
    api: &CloudflareApi,
    zone_id: &str,
    ownership: &mut Value,
    config: &Value,
    instance_id: &str,
) -> Result<Vec<String>, CloudflareApiError> {
    let configured = configured_hosts(config);
    let settings = load_domain_settings(state).await?;
    let (managed_hosts, _) = partition_optimization_hosts(configured.clone(), &settings);

    // An explicit external-hostname choice is a request to relinquish, not to
    // delete unconditionally. Retry that safe path before applying the stricter
    // cleanup policy used for hostnames removed from the application config.
    for hostname in &settings.external_hostnames {
        let cleanup_error = if ownership
            .pointer(&format!(
                "/optimization/customHostnames/{}",
                json_pointer_escape(hostname)
            ))
            .is_some()
        {
            relinquish_optimization_host(state, api, zone_id, ownership, hostname, instance_id)
                .await
                .err()
        } else {
            None
        };
        if let Some(error) = cleanup_error {
            tracing::warn!(%error, %hostname, "external optimization hostname cleanup remains pending");
        }
    }

    let mut configured_set = configured.into_iter().collect::<HashSet<_>>();
    configured_set.extend(settings.external_hostnames.iter().cloned());
    cleanup_removed_hosts(state, api, zone_id, ownership, &configured_set, instance_id).await?;
    Ok(managed_hosts)
}

pub(super) fn tracked_exact_dns_snapshot(
    hostname: &str,
    id: &str,
    host: &Value,
    ownership: &Value,
    legacy_edge_hostname: Option<&str>,
) -> Value {
    let target_path = if host.get("exactDnsTarget").and_then(Value::as_str) == Some("origin") {
        "/optimization/originDns/name"
    } else {
        "/optimization/edgeDns/name"
    };
    let content = ownership
        .pointer(target_path)
        .cloned()
        .or_else(|| legacy_edge_hostname.map(|value| json!(value)))
        .unwrap_or(Value::Null);
    json!({
        "id": id,
        "name": hostname,
        "type": "CNAME",
        "content": content,
        "proxied": false,
    })
}

pub(super) fn host_has_tracked_remote_resources(host: &Value) -> bool {
    host.get("id").and_then(Value::as_str).is_some()
        || host.get("exactDnsId").and_then(Value::as_str).is_some()
        || host
            .get("validationDns")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .any(|record| record.get("id").and_then(Value::as_str).is_some())
}

pub(super) async fn forget_optimization_host_state(
    state: &AppState,
    ownership: &mut Value,
    hostname: &str,
) -> Result<(), CloudflareApiError> {
    if let Some(items) = ownership
        .pointer_mut("/optimization/customHostnames")
        .and_then(Value::as_object_mut)
    {
        items.remove(hostname);
    }
    save_managed_state(state, ownership).await
}

pub(super) async fn relinquish_optimization_host(
    state: &AppState,
    api: &CloudflareApi,
    zone_id: &str,
    ownership: &mut Value,
    hostname: &str,
    instance_id: &str,
) -> Result<(), CloudflareApiError> {
    let Some(host) = ownership
        .pointer(&format!(
            "/optimization/customHostnames/{}",
            json_pointer_escape(hostname)
        ))
        .cloned()
    else {
        return Ok(());
    };
    let default_custom_origin = ownership
        .pointer("/optimization/originDns/name")
        .and_then(Value::as_str)
        .map(str::to_string);

    if let Some(id) = host.get("exactDnsId").and_then(Value::as_str) {
        let owned = tracked_exact_dns_snapshot(hostname, id, &host, ownership, None);
        if let Err(error) = delete_dns_if_owned(api, zone_id, &owned, instance_id).await {
            if error.status == Some(StatusCode::CONFLICT) {
                tracing::warn!(%error, %hostname, "retaining externally changed exact DNS while relinquishing optimization hostname");
            } else {
                return Err(error);
            }
        }
    }
    for record in host
        .get("validationDns")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if record.get("id").and_then(Value::as_str).is_none() {
            continue;
        }
        if let Err(error) = delete_dns_if_owned(api, zone_id, record, instance_id).await {
            if error.status == Some(StatusCode::CONFLICT) {
                tracing::warn!(%error, %hostname, "retaining externally changed validation DNS while relinquishing optimization hostname");
            } else {
                return Err(error);
            }
        }
    }
    if let Some(id) = host.get("id").and_then(Value::as_str) {
        match api.get_custom_hostname(zone_id, id).await {
            Ok(remote)
                if managed_custom_hostname_matches(
                    &remote,
                    hostname,
                    &host,
                    default_custom_origin.as_deref(),
                ) =>
            {
                ignore_not_found(api.delete_custom_hostname(zone_id, id).await)?;
            }
            Ok(_) => {
                tracing::warn!(%hostname, "retaining externally changed Custom Hostname while relinquishing ownership");
            }
            Err(error) if error.status == Some(StatusCode::NOT_FOUND) => {}
            Err(error) => return Err(error),
        }
    }
    forget_optimization_host_state(state, ownership, hostname).await
}
