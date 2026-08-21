use super::*;

pub(super) async fn ensure_fallback_origin(
    state: &AppState,
    api: &CloudflareApi,
    zone_id: &str,
    ownership: &mut Value,
    desired_origin: &str,
    takeover: bool,
) -> Result<FallbackOriginResult, CloudflareApiError> {
    let current = ownership.pointer("/optimization/fallbackOrigin").cloned();
    let remote_value = api.get_fallback_origin(zone_id).await?;
    let remote_origin = remote_value
        .as_ref()
        .and_then(|value| value.get("origin"))
        .and_then(Value::as_str);
    let owned_origin = current
        .as_ref()
        .and_then(|value| value.get("origin"))
        .and_then(Value::as_str);
    let owned_kind = current
        .as_ref()
        .and_then(|value| value.get("ownership"))
        .and_then(Value::as_str);

    let (next, ownership_kind, previous_origin) = match remote_origin {
        None => (
            api.update_fallback_origin(zone_id, desired_origin).await?,
            "dedicated",
            None,
        ),
        Some(remote) if remote.eq_ignore_ascii_case(desired_origin) => {
            if let (Some(_), Some(kind)) = (owned_origin, owned_kind) {
                (
                    remote_value
                        .clone()
                        .unwrap_or_else(|| json!({ "origin": remote })),
                    kind,
                    current
                        .as_ref()
                        .and_then(|value| value.get("previousOrigin"))
                        .and_then(Value::as_str),
                )
            } else {
                if !takeover {
                    return Err(CloudflareApiError {
                        status: Some(StatusCode::CONFLICT),
                        message: "The Cloudflare for SaaS fallback origin already exists and is not owned by fn-knock; preview and explicitly confirm takeover"
                            .to_string(),
                    });
                }
                (
                    remote_value
                        .clone()
                        .unwrap_or_else(|| json!({ "origin": remote })),
                    "adopted",
                    None,
                )
            }
        }
        Some(remote) if owned_origin == Some(remote) => {
            if let Some(kind) = owned_kind {
                (
                    api.update_fallback_origin(zone_id, desired_origin).await?,
                    kind,
                    current
                        .as_ref()
                        .and_then(|value| value.get("previousOrigin"))
                        .and_then(Value::as_str),
                )
            } else if takeover {
                (
                    api.update_fallback_origin(zone_id, desired_origin).await?,
                    "adopted",
                    Some(remote),
                )
            } else {
                return Err(CloudflareApiError {
                    status: Some(StatusCode::CONFLICT),
                    message: "A different Cloudflare for SaaS fallback origin exists; preview and explicitly confirm takeover"
                        .to_string(),
                });
            }
        }
        Some(remote) if takeover => (
            api.update_fallback_origin(zone_id, desired_origin).await?,
            "adopted",
            Some(remote),
        ),
        Some(_) => {
            return Err(CloudflareApiError {
                status: Some(StatusCode::CONFLICT),
                message: "A different Cloudflare for SaaS fallback origin exists; preview and explicitly confirm takeover"
                    .to_string(),
            });
        }
    };

    let status = next
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("pending_deployment");
    let errors = next.get("errors").cloned().unwrap_or_else(|| json!([]));
    let mut stored = json!({
        "origin": desired_origin,
        "status": status,
        "errors": errors,
        "ownership": ownership_kind,
        "updatedAt": time_utils::now_iso(),
    });
    if let Some(previous_origin) = previous_origin {
        ensure_object(&mut stored).insert("previousOrigin".to_string(), json!(previous_origin));
    }
    ensure_nested_object(ownership, &["optimization"]).insert("fallbackOrigin".to_string(), stored);
    save_managed_state(state, ownership).await?;

    match status {
        "active" => Ok(FallbackOriginResult::Ready),
        "deployment_timed_out" | "pending_deletion" | "deleted" => Err(local_error(format!(
            "Cloudflare for SaaS fallback origin entered status {status}"
        ))),
        _ => Ok(FallbackOriginResult::Pending),
    }
}

pub(super) async fn ensure_capability_probe(
    state: &AppState,
    api: &CloudflareApi,
    managed: &Value,
    ownership: &mut Value,
    origin_hostname: &str,
    selected_ip: Option<Ipv4Addr>,
) -> Result<CapabilityProbeResult, CloudflareApiError> {
    let existing_status = ownership
        .pointer("/optimization/capabilityProbe/status")
        .and_then(Value::as_str)
        .map(str::to_string);
    if existing_status.as_deref() == Some("compatible") {
        let tested_ip = ownership
            .pointer("/optimization/capabilityProbe/testedIp")
            .and_then(Value::as_str);
        let selected_ip_text = selected_ip.map(|ip| ip.to_string());
        let candidate_changed_without_business_probe = selected_ip_text.is_some()
            && tested_ip != selected_ip_text.as_deref()
            && active_probe_hostname(ownership).is_none();
        if !candidate_changed_without_business_probe {
            return Ok(CapabilityProbeResult::Ready);
        }
        if let Some(optimization) = ownership
            .pointer_mut("/optimization")
            .and_then(Value::as_object_mut)
        {
            optimization.remove("capabilityProbe");
        }
        save_managed_state(state, ownership).await?;
    }
    if existing_status.as_deref() == Some("unsupported") {
        let definitive = ownership
            .pointer("/optimization/capabilityProbe")
            .is_some_and(capability_probe_is_definitively_unsupported);
        if definitive {
            return Ok(CapabilityProbeResult::Unsupported);
        }
        // Older releases classified any candidate route failure as a product
        // capability failure and persisted `unsupported` without a reason
        // code. That state is safe to retry; definitive entitlement failures
        // always carry a reasonCode and remain disabled.
        if let Some(optimization) = ownership
            .pointer_mut("/optimization")
            .and_then(Value::as_object_mut)
        {
            optimization.remove("capabilityProbe");
        }
        save_managed_state(state, ownership).await?;
    }
    let zone_id = managed.get("zoneId").and_then(Value::as_str).unwrap_or("");
    let root = managed_root_domain(managed);
    let suffix = managed_instance_id(managed);
    let hostname = format!("fnknock-probe-{suffix}.{root}");
    let current = ownership
        .pointer("/optimization/capabilityProbe")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let owned_id = current.get("id").and_then(Value::as_str);
    let custom = if let Some(id) = owned_id {
        match api.get_custom_hostname(zone_id, id).await {
            Ok(value) => value,
            Err(error) if error.status == Some(StatusCode::NOT_FOUND) => {
                create_capability_hostname(
                    state,
                    api,
                    managed,
                    ownership,
                    &hostname,
                    origin_hostname,
                )
                .await?
            }
            Err(error) => return Err(error),
        }
    } else {
        let existing = api.list_custom_hostnames(zone_id, Some(&hostname)).await?;
        if existing.is_empty() {
            match create_capability_hostname(
                state,
                api,
                managed,
                ownership,
                &hostname,
                origin_hostname,
            )
            .await
            {
                Ok(value) => value,
                Err(error) if is_capability_unsupported_api_error(&error) => {
                    disable_unsupported_optimization(
                        state,
                        managed,
                        ownership,
                        &error.to_string(),
                        Some(CLOUDFLARE_SAAS_REQUIRED_ERROR_CODE),
                    )
                    .await?;
                    return Ok(CapabilityProbeResult::Unsupported);
                }
                Err(error) => return Err(error),
            }
        } else {
            return Err(CloudflareApiError {
                status: Some(StatusCode::CONFLICT),
                message: "The isolated optimization probe hostname already exists and is not owned by fn-knock"
                    .to_string(),
            });
        }
    };
    let custom_id = custom.get("id").and_then(Value::as_str).unwrap_or("");
    if custom_id.is_empty() {
        return Err(local_error(
            "Cloudflare did not return an ID for the optimization capability probe",
        ));
    }
    let current = ownership
        .pointer("/optimization/capabilityProbe")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let mut validation_dns = Vec::new();
    for (name, value) in extract_validation_records(&custom) {
        let existing_id = current
            .get("validationDns")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .find(|record| record.get("name").and_then(Value::as_str) == Some(name.as_str()))
            .and_then(|record| record.get("id"))
            .and_then(Value::as_str);
        match upsert_managed_dns(
            api,
            ManagedDnsRequest {
                zone_id,
                name: &name,
                record_type: "TXT",
                content: &value,
                proxied: false,
                owned_id: existing_id,
                takeover: false,
                instance_id: &suffix,
            },
        )
        .await
        {
            Ok(record) => {
                validation_dns.push(record);
                let probe = ensure_nested_object(ownership, &["optimization", "capabilityProbe"]);
                probe.insert("id".to_string(), json!(custom_id));
                probe.insert("hostname".to_string(), json!(hostname));
                probe.insert("validationDns".to_string(), json!(validation_dns.clone()));
                save_managed_state(state, ownership).await?;
            }
            Err(error) if is_capability_unsupported_api_error(&error) => {
                let mut cleanup = current.clone();
                let cleanup_object = ensure_object(&mut cleanup);
                cleanup_object.insert("id".to_string(), json!(custom_id));
                cleanup_object.insert("validationDns".to_string(), json!(validation_dns));
                cleanup_capability_probe(api, zone_id, &cleanup).await?;
                disable_unsupported_optimization(
                    state,
                    managed,
                    ownership,
                    &error.to_string(),
                    Some(CLOUDFLARE_SAAS_REQUIRED_ERROR_CODE),
                )
                .await?;
                return Ok(CapabilityProbeResult::Unsupported);
            }
            Err(error) => return Err(error),
        }
    }
    let activation_dns = upsert_managed_dns(
        api,
        ManagedDnsRequest {
            zone_id,
            name: &hostname,
            record_type: "CNAME",
            content: origin_hostname,
            proxied: false,
            owned_id: current.pointer("/activationDns/id").and_then(Value::as_str),
            takeover: false,
            instance_id: &suffix,
        },
    )
    .await?;
    {
        let probe = ensure_nested_object(ownership, &["optimization", "capabilityProbe"]);
        probe.insert("id".to_string(), json!(custom_id));
        probe.insert("hostname".to_string(), json!(hostname));
        probe.insert("activationDns".to_string(), activation_dns.clone());
        probe.insert("validationDns".to_string(), json!(validation_dns.clone()));
    }
    save_managed_state(state, ownership).await?;
    let refreshed = api.get_custom_hostname(zone_id, custom_id).await?;
    let status = refreshed
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("pending");
    let ssl_status = refreshed
        .pointer("/ssl/status")
        .and_then(Value::as_str)
        .unwrap_or("pending");
    let verification_errors = refreshed
        .get("verification_errors")
        .cloned()
        .unwrap_or_else(|| json!([]));
    let verification_message = cloudflare_error_list_message(&verification_errors);
    let probe_state = json!({
        "id": custom_id,
        "hostname": hostname,
        "status": if status == "active" && ssl_status == "active" && selected_ip.is_none() {
            "awaiting-candidate"
        } else {
            "pending"
        },
        "hostnameStatus": status,
        "sslStatus": ssl_status,
        "activationDns": activation_dns,
        "validationDns": validation_dns,
        "verificationErrors": verification_errors,
        "message": verification_message,
        "updatedAt": time_utils::now_iso(),
    });
    ensure_nested_object(ownership, &["optimization"])
        .insert("capabilityProbe".to_string(), probe_state.clone());
    save_managed_state(state, ownership).await?;
    if status != "active" || ssl_status != "active" {
        return Ok(CapabilityProbeResult::Pending);
    }
    let Some(ip) = selected_ip else {
        return Ok(CapabilityProbeResult::Pending);
    };
    match probe_custom_hostname(&hostname, ip).await {
        Ok(()) => {
            cleanup_capability_probe(api, zone_id, &probe_state).await?;
            ensure_nested_object(ownership, &["optimization"]).insert(
                "capabilityProbe".to_string(),
                json!({
                    "hostname": hostname,
                    "status": "compatible",
                    "testedIp": ip,
                    "testedAt": time_utils::now_iso(),
                }),
            );
            save_managed_state(state, ownership).await?;
            Ok(CapabilityProbeResult::Ready)
        }
        Err(error) => {
            // The Custom Hostname and certificate are already active here.
            // A failed request to one candidate IP is a route-level result,
            // not evidence that Cloudflare for SaaS is unsupported. Retain the
            // isolated hostname so the user can retry this scan or apply a
            // different candidate without reprovisioning the capability probe.
            let failed_probe = capability_probe_failure_state(&probe_state, &error);
            ensure_nested_object(ownership, &["optimization"])
                .insert("capabilityProbe".to_string(), failed_probe);
            save_managed_state(state, ownership).await?;
            Err(local_error(format!(
                "Preferred edge candidate failed capability validation: {error}"
            )))
        }
    }
}

pub(super) fn capability_probe_failure_state(probe_state: &Value, error: &str) -> Value {
    let mut failed = probe_state.clone();
    record_preferred_edge_probe_failure(&mut failed, error);
    failed
}

pub(super) fn capability_probe_is_definitively_unsupported(probe: &Value) -> bool {
    probe.get("status").and_then(Value::as_str) == Some("unsupported")
        && probe.get("reasonCode").and_then(Value::as_str)
            == Some(CLOUDFLARE_SAAS_REQUIRED_ERROR_CODE)
}

pub(super) async fn create_capability_hostname(
    state: &AppState,
    api: &CloudflareApi,
    managed: &Value,
    ownership: &mut Value,
    hostname: &str,
    origin_hostname: &str,
) -> Result<Value, CloudflareApiError> {
    let zone_id = managed.get("zoneId").and_then(Value::as_str).unwrap_or("");
    let custom = api
        .create_custom_hostname(zone_id, hostname, origin_hostname)
        .await?;
    ensure_nested_object(ownership, &["optimization"]).insert(
        "capabilityProbe".to_string(),
        json!({
            "id": custom.get("id").cloned().unwrap_or(Value::Null),
            "hostname": hostname,
            "status": "pending",
            "createdAt": time_utils::now_iso(),
        }),
    );
    save_managed_state(state, ownership).await?;
    Ok(custom)
}

pub(super) async fn cleanup_capability_probe(
    api: &CloudflareApi,
    zone_id: &str,
    probe: &Value,
) -> Result<(), CloudflareApiError> {
    if let Some(id) = probe.pointer("/activationDns/id").and_then(Value::as_str) {
        ignore_not_found(api.delete_dns_record(zone_id, id).await)?;
    }
    for record in probe
        .get("validationDns")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if let Some(id) = record.get("id").and_then(Value::as_str) {
            ignore_not_found(api.delete_dns_record(zone_id, id).await)?;
        }
    }
    if let Some(id) = probe.get("id").and_then(Value::as_str) {
        ignore_not_found(api.delete_custom_hostname(zone_id, id).await)?;
    }
    Ok(())
}

pub(super) async fn disable_unsupported_optimization(
    state: &AppState,
    managed: &Value,
    ownership: &mut Value,
    reason: &str,
    reason_code: Option<&str>,
) -> Result<(), CloudflareApiError> {
    let mut capability_probe = json!({
        "status": "unsupported",
        "message": reason,
        "testedAt": time_utils::now_iso(),
    });
    if let Some(reason_code) = reason_code {
        ensure_object(&mut capability_probe).insert("reasonCode".to_string(), json!(reason_code));
    }
    ensure_nested_object(ownership, &["optimization"])
        .insert("capabilityProbe".to_string(), capability_probe);
    ensure_nested_object(ownership, &["optimization"])
        .insert("fallbackActive".to_string(), json!(true));
    ensure_nested_object(ownership, &["optimization"])
        .insert("publishSuppressed".to_string(), json!(true));
    save_managed_state(state, ownership).await?;
    let mut next_managed = managed.clone();
    ensure_object(&mut next_managed).insert("optimizationEnabled".to_string(), json!(false));
    save_managed_config(state, &next_managed).await
}
