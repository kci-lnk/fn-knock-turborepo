use super::*;

pub(in crate::tunnels::cloudflared) async fn reconcile_resources(
    state: &AppState,
    api: &CloudflareApi,
    managed: &Value,
    ownership: &mut Value,
    force_publish: bool,
    takeover: Option<&HashSet<String>>,
) -> Result<(), CloudflareApiError> {
    if managed.get("optimizationEnabled").and_then(Value::as_bool) != Some(true) {
        return Ok(());
    }
    let zone_id = managed.get("zoneId").and_then(Value::as_str).unwrap_or("");
    let root = managed_root_domain(managed);
    let tunnel_id = managed
        .pointer("/tunnel/id")
        .and_then(Value::as_str)
        .unwrap_or("");
    if zone_id.is_empty() || root.is_empty() || tunnel_id.is_empty() {
        return Err(local_error(
            "Managed Tunnel, Zone, and root domain must be configured first",
        ));
    }
    let suffix = managed_instance_id(managed);
    let origin_hostname = format!("fnknock-origin-{suffix}.{root}");
    let origin_target = format!("{tunnel_id}.cfargotunnel.com");
    let existing_origin_id = ownership
        .pointer("/optimization/originDns/id")
        .and_then(Value::as_str)
        .map(str::to_string);
    let origin_dns = upsert_managed_dns(
        api,
        ManagedDnsRequest {
            zone_id,
            name: &origin_hostname,
            record_type: "CNAME",
            content: &origin_target,
            proxied: true,
            owned_id: existing_origin_id.as_deref(),
            takeover: takeover.is_some_and(|items| items.contains("optimization:origin-dns")),
            instance_id: &suffix,
        },
    )
    .await?;
    ensure_nested_object(ownership, &["optimization"]).insert("originDns".to_string(), origin_dns);
    save_managed_state(state, ownership).await?;

    match ensure_fallback_origin(
        state,
        api,
        zone_id,
        ownership,
        &origin_hostname,
        takeover.is_some_and(|items| items.contains("optimization:fallback-origin")),
    )
    .await?
    {
        FallbackOriginResult::Ready => {}
        FallbackOriginResult::Pending => return Ok(()),
    }

    let selected = ownership.pointer("/optimization/selected").cloned();
    let selected_ip = selected
        .as_ref()
        .and_then(|value| value.get("ip"))
        .and_then(Value::as_str)
        .and_then(|value| Ipv4Addr::from_str(value).ok());
    let edge_hostname = format!("fnknock-edge-{suffix}.{root}");
    match ensure_capability_probe(
        state,
        api,
        managed,
        ownership,
        &origin_hostname,
        selected_ip,
    )
    .await?
    {
        CapabilityProbeResult::Ready => {}
        CapabilityProbeResult::Pending | CapabilityProbeResult::Unsupported => return Ok(()),
    }

    let publish_exact_routes = should_publish_exact_routes(ownership, force_publish);
    if publish_exact_routes && let Some(ip) = selected_ip {
        let ip_text = ip.to_string();
        let current_edge_ip = ownership
            .pointer("/optimization/edgeDns/content")
            .and_then(Value::as_str);
        if current_edge_ip != Some(ip_text.as_str()) {
            validate_candidate_for_active_hostnames(ownership, ip)
                .await
                .map_err(|error| {
                    local_error(format!(
                        "Preferred edge candidate failed pre-publish validation: {error}"
                    ))
                })?;
        }
        let existing_edge_id = ownership
            .pointer("/optimization/edgeDns/id")
            .and_then(Value::as_str)
            .map(str::to_string);
        let edge_dns = upsert_managed_dns(
            api,
            ManagedDnsRequest {
                zone_id,
                name: &edge_hostname,
                record_type: "A",
                content: &ip_text,
                proxied: false,
                owned_id: existing_edge_id.as_deref(),
                takeover: takeover.is_some_and(|items| items.contains("optimization:edge-dns")),
                instance_id: &suffix,
            },
        )
        .await?;
        ensure_nested_object(ownership, &["optimization"]).insert("edgeDns".to_string(), edge_dns);
        save_managed_state(state, ownership).await?;
    }

    let local = state
        .storage
        .store
        .get_config()
        .await
        .map_err(local_error_display)?;
    let hosts =
        reconcile_optimization_host_membership(state, api, zone_id, ownership, &local, &suffix)
            .await?;
    let remote_custom = api.list_custom_hostnames(zone_id, None).await?;
    let recovery_origin = ownership
        .pointer("/optimization/fallbackOrigin/previousOrigin")
        .and_then(Value::as_str)
        .map(str::to_string);
    let mut available = MAX_CUSTOM_HOSTNAMES.saturating_sub(remote_custom.len());
    let mut created_this_run = 0usize;
    for host in hosts {
        let current_owned = ownership
            .pointer(&format!(
                "/optimization/customHostnames/{}",
                json_pointer_escape(&host)
            ))
            .cloned()
            .unwrap_or_else(|| json!({}));
        let owned_id = current_owned.get("id").and_then(Value::as_str);
        let existing = remote_custom.iter().find(|item| {
            item.get("hostname")
                .and_then(Value::as_str)
                .is_some_and(|value| value.eq_ignore_ascii_case(&host))
        });
        let owned_custom_matches = existing.is_some_and(|item| {
            owned_id == item.get("id").and_then(Value::as_str)
                && managed_custom_hostname_matches(
                    item,
                    &host,
                    &current_owned,
                    Some(&origin_hostname),
                )
        });
        let recovery = match existing {
            Some(item) if owned_id != item.get("id").and_then(Value::as_str) => {
                let exact_records = api.list_dns_records(zone_id, Some(&host)).await?;
                let recovered_origin = item
                    .get("custom_origin_server")
                    .and_then(Value::as_str)
                    .and_then(|origin| {
                        ownership
                            .pointer(&format!(
                                "/optimization/recoveredOrigins/{}",
                                json_pointer_escape(origin)
                            ))
                            .cloned()
                    });
                inspect_recoverable_fn_knock_custom_hostname(
                    api,
                    zone_id,
                    root,
                    item,
                    &exact_records,
                    recovery_origin.as_deref(),
                    &suffix,
                    recovered_origin.as_ref(),
                )
                .await?
                .0
            }
            _ => None,
        };
        if let Some(recoverable) = recovery.as_ref() {
            adopt_recoverable_fn_knock_origin(
                state,
                api,
                zone_id,
                ownership,
                recoverable,
                &origin_target,
                &suffix,
            )
            .await?;
        }
        let recovered_lineage = recovery.is_some();
        let custom = match existing {
            Some(item) if owned_custom_matches => item.clone(),
            Some(item) if recovered_lineage => item.clone(),
            Some(item)
                if takeover
                    .is_some_and(|items| items.contains(&format!("custom-hostname:{host}"))) =>
            {
                if let Some(id) = item.get("id").and_then(Value::as_str) {
                    api.delete_custom_hostname(zone_id, id).await?;
                }
                api.create_custom_hostname(zone_id, &host, &origin_hostname)
                    .await?
            }
            Some(item) => {
                set_host_state(
                    ownership,
                    &host,
                    custom_hostname_ownership_conflict(item, &host, root),
                );
                save_managed_state(state, ownership).await?;
                continue;
            }
            None if available == 0 => {
                set_host_state(
                    ownership,
                    &host,
                    json!({
                        "status": "quota",
                        "messageCode": "customHostnameQuotaExhausted",
                        "message": "Custom Hostname quota is exhausted"
                    }),
                );
                save_managed_state(state, ownership).await?;
                continue;
            }
            None if created_this_run >= MAX_CUSTOM_HOSTNAME_CREATES_PER_RECONCILE => {
                set_host_state(
                    ownership,
                    &host,
                    json!({
                        "status": "queued",
                        "messageCode": "certificateRateLimited",
                        "message": "Queued to respect Cloudflare certificate issuance rate limits"
                    }),
                );
                save_managed_state(state, ownership).await?;
                continue;
            }
            None => {
                match api
                    .create_custom_hostname(zone_id, &host, &origin_hostname)
                    .await
                {
                    Ok(custom) => {
                        available = available.saturating_sub(1);
                        created_this_run += 1;
                        custom
                    }
                    Err(error) if is_capability_unsupported_api_error(&error) => {
                        // The included limit is not an entitlement guarantee:
                        // account-specific quotas can be lower or already
                        // exhausted. Keep the wildcard Tunnel serving this and
                        // all remaining hosts instead of aborting reconciliation.
                        available = 0;
                        set_host_state(
                            ownership,
                            &host,
                            json!({
                                "status": "quota",
                                "messageCode": "customHostnameQuotaUnavailable",
                                "messageDetail": error.to_string(),
                                "message": format!("Custom Hostname quota is unavailable: {error}")
                            }),
                        );
                        save_managed_state(state, ownership).await?;
                        continue;
                    }
                    Err(error) => return Err(error),
                }
            }
        };
        let custom_id = custom
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if custom_id.is_empty() {
            continue;
        }
        let mut host_state = current_owned;
        {
            let object = ensure_object(&mut host_state);
            object.remove("message");
            object.remove("messageCode");
            object.remove("messageDetail");
            object.remove("conflictResourceId");
        }
        if let Some(recoverable) = recovery.as_ref() {
            let object = ensure_object(&mut host_state);
            object.insert("ownership".to_string(), json!("recovered"));
            object.insert(
                "recoveredFromInstance".to_string(),
                json!(recoverable.legacy_instance_id),
            );
            object.insert(
                "customOriginServer".to_string(),
                json!(recoverable.origin_hostname),
            );
            object.insert(
                "exactDnsId".to_string(),
                recoverable
                    .exact_dns
                    .get("id")
                    .cloned()
                    .unwrap_or(Value::Null),
            );
            object.insert("exactDnsTarget".to_string(), json!("origin"));
        }
        let exact_route_was_optimized = exact_route_is_optimized(&host_state);
        let status = custom
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("pending");
        let ssl_status = custom
            .pointer("/ssl/status")
            .and_then(Value::as_str)
            .unwrap_or("pending");
        let object = ensure_object(&mut host_state);
        object.insert("id".to_string(), json!(custom_id));
        object.insert("status".to_string(), json!(status));
        object.insert("hostnameStatus".to_string(), json!(status));
        object.insert("sslStatus".to_string(), json!(ssl_status));
        object.insert("hostname".to_string(), json!(host));
        object.insert(
            "customOriginServer".to_string(),
            custom
                .get("custom_origin_server")
                .cloned()
                .unwrap_or_else(|| json!(origin_hostname)),
        );
        set_host_state(ownership, &host, host_state.clone());
        save_managed_state(state, ownership).await?;

        let validation_records = extract_validation_records(&custom);
        let mut validation_ids = Vec::new();
        let mut used_validation_dns_ids = HashSet::new();
        let mut activation_conflict = false;
        for (name, value) in validation_records {
            let existing_id = host_state
                .get("validationDns")
                .and_then(Value::as_array)
                .and_then(|records| {
                    records.iter().find(|record| {
                        record.get("name").and_then(Value::as_str) == Some(name.as_str())
                            && record.get("content").and_then(Value::as_str) == Some(value.as_str())
                            && record
                                .get("id")
                                .and_then(Value::as_str)
                                .is_some_and(|id| !used_validation_dns_ids.contains(id))
                    })
                })
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
                    takeover: recovered_lineage,
                    instance_id: &suffix,
                },
            )
            .await
            {
                Ok(record) => {
                    if let Some(id) = record.get("id").and_then(Value::as_str) {
                        used_validation_dns_ids.insert(id.to_string());
                    }
                    validation_ids.push(record);
                    ensure_object(&mut host_state)
                        .insert("validationDns".to_string(), json!(validation_ids));
                    set_host_state(ownership, &host, host_state.clone());
                    save_managed_state(state, ownership).await?;
                }
                Err(error) if error.status == Some(StatusCode::CONFLICT) => {
                    activation_conflict = true;
                    ensure_object(&mut host_state).insert("status".to_string(), json!("conflict"));
                    let object = ensure_object(&mut host_state);
                    object.insert(
                        "messageCode".to_string(),
                        json!("validationDnsOwnershipConflict"),
                    );
                    object.insert("messageDetail".to_string(), json!(name));
                    object.insert("message".to_string(), json!(error.to_string()));
                }
                Err(error) => return Err(error),
            }
        }
        if !activation_conflict
            && custom_hostname_needs_activation_dns(publish_exact_routes, status, ssl_status)
        {
            // Cloudflare does not support TXT pre-validation when the custom
            // hostname is already in this Cloudflare Zone (Orange-to-Orange).
            // Point the exact hostname at the standard Tunnel first. This
            // activates the Custom Hostname without changing the request path;
            // only switch it to the preferred edge after certificate and SNI
            // validation have completed.
            let activation_targets_edge = publish_exact_routes && exact_route_was_optimized;
            let activation_target = if activation_targets_edge {
                edge_hostname.as_str()
            } else {
                origin_hostname.as_str()
            };
            let exact_id = host_state.get("exactDnsId").and_then(Value::as_str);
            match upsert_managed_dns(
                api,
                ManagedDnsRequest {
                    zone_id,
                    name: &host,
                    record_type: "CNAME",
                    content: activation_target,
                    proxied: false,
                    owned_id: exact_id,
                    takeover: recovered_lineage
                        || takeover.is_some_and(|items| {
                            items.contains(&format!("optimization:dns:{host}"))
                        }),
                    instance_id: &suffix,
                },
            )
            .await
            {
                Ok(record) => {
                    set_exact_dns_route(
                        &mut host_state,
                        &record,
                        if activation_targets_edge {
                            "edge"
                        } else {
                            "origin"
                        },
                    );
                    set_host_state(ownership, &host, host_state.clone());
                    save_managed_state(state, ownership).await?;
                }
                Err(error) if error.status == Some(StatusCode::CONFLICT) => {
                    activation_conflict = true;
                    ensure_object(&mut host_state).insert("status".to_string(), json!("conflict"));
                    let object = ensure_object(&mut host_state);
                    object.insert(
                        "messageCode".to_string(),
                        json!("exactDnsOwnershipConflict"),
                    );
                    object.insert(
                        "conflictResourceId".to_string(),
                        json!(format!("optimization:dns:{host}")),
                    );
                    object.insert("message".to_string(), json!(error.to_string()));
                }
                Err(error) => return Err(error),
            }
        }
        let refreshed = api.get_custom_hostname(zone_id, &custom_id).await?;
        let status = refreshed
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or(status);
        let ssl_status = refreshed
            .pointer("/ssl/status")
            .and_then(Value::as_str)
            .unwrap_or(ssl_status);
        if !activation_conflict {
            ensure_object(&mut host_state).insert("status".to_string(), json!(status));
        }
        update_custom_hostname_activation(&mut host_state, &refreshed);
        if !activation_conflict && status == "active" && ssl_status == "active" {
            if !publish_exact_routes {
                // The exact CNAME is needed while Cloudflare validates an O2O
                // hostname, but an explicit fallback must ultimately use the
                // wildcard Tunnel route. Once hostname and certificate
                // validation are complete, remove only the exact DNS record;
                // retain the active Custom Hostname for TLS/SNI probes and a
                // later zero-downtime optimization recovery.
                if let Some(record_id) = host_state.get("exactDnsId").and_then(Value::as_str) {
                    delete_dns_if_owned(
                        api,
                        zone_id,
                        &tracked_exact_dns_snapshot(
                            &host,
                            record_id,
                            &host_state,
                            ownership,
                            Some(&edge_hostname),
                        ),
                        &suffix,
                    )
                    .await?;
                    let object = ensure_object(&mut host_state);
                    object.remove("exactDnsId");
                    object.remove("exactDnsTarget");
                }
                ensure_object(&mut host_state).insert("status".to_string(), json!("fallback"));
            } else if let Some(ip) = selected_ip {
                match probe_custom_hostname(&host, ip).await {
                    Ok(()) => {
                        let exact_id = host_state.get("exactDnsId").and_then(Value::as_str);
                        match upsert_managed_dns(
                            api,
                            ManagedDnsRequest {
                                zone_id,
                                name: &host,
                                record_type: "CNAME",
                                content: &edge_hostname,
                                proxied: false,
                                owned_id: exact_id,
                                takeover: recovered_lineage
                                    || takeover.is_some_and(|items| {
                                        items.contains(&format!("optimization:dns:{host}"))
                                    }),
                                instance_id: &suffix,
                            },
                        )
                        .await
                        {
                            Ok(record) => {
                                set_exact_dns_route(&mut host_state, &record, "edge");
                                ensure_object(&mut host_state)
                                    .insert("status".to_string(), json!("optimized"));
                                ensure_object(&mut host_state).insert(
                                    "lastVerifiedAt".to_string(),
                                    json!(time_utils::now_iso()),
                                );
                                let object = ensure_object(&mut host_state);
                                object.remove("message");
                                object.remove("messageCode");
                                object.remove("messageDetail");
                                object.remove("conflictResourceId");
                            }
                            Err(error) if error.status == Some(StatusCode::CONFLICT) => {
                                let object = ensure_object(&mut host_state);
                                object.insert("status".to_string(), json!("conflict"));
                                object.insert(
                                    "messageCode".to_string(),
                                    json!("exactDnsOwnershipConflict"),
                                );
                                object.insert(
                                    "conflictResourceId".to_string(),
                                    json!(format!("optimization:dns:{host}")),
                                );
                                object.insert("message".to_string(), json!(error.to_string()));
                            }
                            Err(error) => return Err(error),
                        }
                    }
                    Err(error) => {
                        let mut fallback_conflict = false;
                        if exact_route_is_optimized(&host_state) {
                            let exact_id = host_state.get("exactDnsId").and_then(Value::as_str);
                            match upsert_managed_dns(
                                api,
                                ManagedDnsRequest {
                                    zone_id,
                                    name: &host,
                                    record_type: "CNAME",
                                    content: &origin_hostname,
                                    proxied: false,
                                    owned_id: exact_id,
                                    takeover: recovered_lineage
                                        || takeover.is_some_and(|items| {
                                            items.contains(&format!("optimization:dns:{host}"))
                                        }),
                                    instance_id: &suffix,
                                },
                            )
                            .await
                            {
                                Ok(record) => {
                                    set_exact_dns_route(&mut host_state, &record, "origin");
                                }
                                Err(fallback_error)
                                    if fallback_error.status == Some(StatusCode::CONFLICT) =>
                                {
                                    fallback_conflict = true;
                                    let object = ensure_object(&mut host_state);
                                    object.insert("status".to_string(), json!("conflict"));
                                    object.insert(
                                        "messageCode".to_string(),
                                        json!("exactDnsOwnershipConflict"),
                                    );
                                    object.insert(
                                        "conflictResourceId".to_string(),
                                        json!(format!("optimization:dns:{host}")),
                                    );
                                    object.insert(
                                        "message".to_string(),
                                        json!(fallback_error.to_string()),
                                    );
                                }
                                Err(fallback_error) => return Err(fallback_error),
                            }
                        }
                        if !fallback_conflict {
                            record_preferred_edge_probe_failure(&mut host_state, &error);
                        }
                    }
                }
            } else {
                ensure_object(&mut host_state).insert("status".to_string(), json!("ready"));
            }
        }
        set_host_state(ownership, &host, host_state);
        save_managed_state(state, ownership).await?;
    }
    let any_optimized = ownership
        .pointer("/optimization/customHostnames")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|items| items.values())
        .any(exact_route_is_optimized);
    ensure_nested_object(ownership, &["optimization"]).insert(
        "fallbackActive".to_string(),
        json!(!publish_exact_routes || !any_optimized),
    );
    save_managed_state(state, ownership).await
}

pub(super) fn custom_hostname_needs_activation_dns(
    publish_exact_routes: bool,
    hostname_status: &str,
    ssl_status: &str,
) -> bool {
    publish_exact_routes || hostname_status != "active" || ssl_status != "active"
}

pub(super) fn custom_hostname_ownership_conflict(
    custom: &Value,
    hostname: &str,
    root: &str,
) -> Value {
    let previous_instance = custom
        .get("custom_origin_server")
        .and_then(Value::as_str)
        .and_then(|origin| fn_knock_origin_instance(origin, root));
    json!({
        "status": "conflict",
        "messageCode": "customHostnameOwnershipConflict",
        "messageDetail": previous_instance,
        "conflictResourceId": format!("custom-hostname:{hostname}"),
        "message": if previous_instance.is_some() {
            "Custom Hostname belongs to an earlier fn-knock instance; explicit takeover is required"
        } else {
            "Custom Hostname is not owned by fn-knock"
        }
    })
}

pub(super) fn set_exact_dns_route(host_state: &mut Value, record: &Value, target: &str) {
    let object = ensure_object(host_state);
    object.insert(
        "exactDnsId".to_string(),
        record.get("id").cloned().unwrap_or(Value::Null),
    );
    object.insert("exactDnsTarget".to_string(), json!(target));
}

pub(super) fn record_preferred_edge_probe_failure(state: &mut Value, error: &str) {
    let object = ensure_object(state);
    object.insert("status".to_string(), json!("probe-failed"));
    object.insert("messageCode".to_string(), json!("preferredEdgeProbeFailed"));
    object.insert("messageDetail".to_string(), json!(error));
    object.insert("message".to_string(), json!(error));
    object.insert(
        "lastProbeFailedAt".to_string(),
        json!(time_utils::now_iso()),
    );
}

pub(super) fn update_custom_hostname_activation(host_state: &mut Value, remote: &Value) -> bool {
    let hostname_status = remote
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("pending");
    let ssl_status = remote
        .pointer("/ssl/status")
        .and_then(Value::as_str)
        .unwrap_or("pending");
    let changed = host_state.get("hostnameStatus").and_then(Value::as_str) != Some(hostname_status)
        || host_state.get("sslStatus").and_then(Value::as_str) != Some(ssl_status);
    let object = ensure_object(host_state);
    object.insert("hostnameStatus".to_string(), json!(hostname_status));
    object.insert("sslStatus".to_string(), json!(ssl_status));
    changed
}

pub(super) fn active_probe_hostname(ownership: &Value) -> Option<String> {
    active_probe_hostnames(ownership).into_iter().next()
}

pub(super) fn active_probe_hostnames(ownership: &Value) -> Vec<String> {
    ownership
        .pointer("/optimization/customHostnames")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|items| items.iter())
        .filter(|(_, state)| custom_hostname_can_validate_candidates(state))
        .map(|(hostname, _)| hostname.clone())
        .collect()
}

pub(super) async fn validate_candidate_for_active_hostnames(
    ownership: &Value,
    ip: Ipv4Addr,
) -> Result<(), String> {
    let hostnames = active_probe_hostnames(ownership);
    for chunk in hostnames.chunks(SNI_VALIDATION_CONCURRENCY) {
        let mut probes = JoinSet::new();
        for hostname in chunk.iter().cloned() {
            probes.spawn(async move {
                let result = probe_custom_hostname(&hostname, ip).await;
                (hostname, result)
            });
        }
        while let Some(result) = probes.join_next().await {
            match result {
                Ok((_, Ok(()))) => {}
                Ok((hostname, Err(error))) => {
                    probes.abort_all();
                    return Err(format!("{hostname}: {error}"));
                }
                Err(error) => {
                    probes.abort_all();
                    return Err(format!("SNI validation task failed: {error}"));
                }
            }
        }
    }
    Ok(())
}
