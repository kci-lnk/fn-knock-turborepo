use super::*;

#[allow(clippy::too_many_arguments)]
pub(in crate::tunnels::cloudflared) async fn append_preview(
    api: &CloudflareApi,
    zone_id: &str,
    root: &str,
    instance: &str,
    hosts: &[String],
    ownership: &Value,
    custom_hostnames: &[Value],
    operations: &mut Vec<Value>,
    conflicts: &mut Vec<Value>,
    remote_snapshot: &mut Vec<Value>,
) -> Result<(), CloudflareApiError> {
    let origin = format!("fnknock-origin-{instance}.{root}");
    let origin_target = ownership
        .pointer("/tunnel/id")
        .and_then(Value::as_str)
        .map(|id| format!("{id}.cfargotunnel.com"));
    inspect_auxiliary_dns(
        api,
        zone_id,
        &origin,
        ownership
            .pointer("/optimization/originDns/id")
            .and_then(Value::as_str),
        "optimization:origin-dns",
        instance,
        "CNAME",
        origin_target.as_deref(),
        true,
        operations,
        conflicts,
        remote_snapshot,
    )
    .await?;
    let remote_fallback = api.get_fallback_origin(zone_id).await?;
    remote_snapshot.push(json!({ "fallbackOrigin": remote_fallback.clone() }));
    let owned_fallback = ownership.pointer("/optimization/fallbackOrigin");
    let remote_origin = remote_fallback
        .as_ref()
        .and_then(|value| value.get("origin"))
        .and_then(Value::as_str);
    let owned_origin = owned_fallback
        .and_then(|value| value.get("origin"))
        .and_then(Value::as_str);
    let recovery_origin = owned_fallback
        .and_then(|value| value.get("previousOrigin"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            remote_origin
                .filter(|remote| !remote.eq_ignore_ascii_case(&origin))
                .map(str::to_string)
        });
    match remote_origin {
        None => operations.push(preview_operation(
            "optimization:fallback-origin",
            "custom-hostname",
            "create",
            &origin,
            false,
        )),
        Some(remote) if remote.eq_ignore_ascii_case(&origin) && owned_origin == Some(remote) => {
            operations.push(preview_operation(
                "optimization:fallback-origin",
                "custom-hostname",
                "keep",
                &origin,
                true,
            ));
        }
        Some(remote)
            if owned_origin == Some(remote)
                && owned_fallback
                    .and_then(|value| value.get("ownership"))
                    .and_then(Value::as_str)
                    .is_some() =>
        {
            operations.push(preview_operation(
                "optimization:fallback-origin",
                "custom-hostname",
                "update",
                &origin,
                true,
            ));
        }
        Some(_) => conflicts.push(json!({
            "id": "optimization:fallback-origin",
            "kind": "custom-hostname",
            "target": "Cloudflare for SaaS fallback origin",
            "messageCode": "unownedFallbackOrigin",
            "message": "A Zone-wide fallback origin already exists and is not owned by fn-knock",
            "takeoverAllowed": true,
        })),
    }
    let capability_status = ownership
        .pointer("/optimization/capabilityProbe/status")
        .and_then(Value::as_str);
    operations.push(preview_operation(
        "optimization:capability-probe",
        "custom-hostname",
        if capability_status == Some("compatible") {
            "keep"
        } else {
            "probe"
        },
        &format!("fnknock-probe-{instance}.{root}"),
        capability_status == Some("compatible"),
    ));
    if capability_status != Some("compatible") {
        let probe_hostname = format!("fnknock-probe-{instance}.{root}");
        inspect_auxiliary_dns(
            api,
            zone_id,
            &probe_hostname,
            ownership
                .pointer("/optimization/capabilityProbe/activationDns/id")
                .and_then(Value::as_str),
            "optimization:capability-probe-dns",
            instance,
            "CNAME",
            Some(origin.as_str()),
            false,
            operations,
            conflicts,
            remote_snapshot,
        )
        .await?;
    }
    if ownership.pointer("/optimization/selected/ip").is_some() {
        let edge = format!("fnknock-edge-{instance}.{root}");
        inspect_auxiliary_dns(
            api,
            zone_id,
            &edge,
            ownership
                .pointer("/optimization/edgeDns/id")
                .and_then(Value::as_str),
            "optimization:edge-dns",
            instance,
            "A",
            ownership
                .pointer("/optimization/selected/ip")
                .and_then(Value::as_str),
            false,
            operations,
            conflicts,
            remote_snapshot,
        )
        .await?;
    }
    let owned = ownership
        .pointer("/optimization/customHostnames")
        .and_then(Value::as_object);
    let total = custom_hostnames.len();
    let mut remaining = MAX_CUSTOM_HOSTNAMES.saturating_sub(total);
    for host in hosts {
        let existing = custom_hostnames.iter().find(|item| {
            item.get("hostname")
                .and_then(Value::as_str)
                .is_some_and(|value| value.eq_ignore_ascii_case(host))
        });
        let owned_id = owned
            .and_then(|items| items.get(host))
            .and_then(|value| value.get("id"))
            .and_then(Value::as_str);
        let exact_records = api.list_dns_records(zone_id, Some(host)).await?;
        remote_snapshot.push(json!({
            "hostname": host,
            "dnsRecords": exact_records.clone(),
        }));
        let (recoverable, recovery_origin_records) = match existing {
            Some(item) if owned_id != item.get("id").and_then(Value::as_str) => {
                let recovered_origin = item
                    .get("custom_origin_server")
                    .and_then(Value::as_str)
                    .and_then(|origin| {
                        ownership.pointer(&format!(
                            "/optimization/recoveredOrigins/{}",
                            json_pointer_escape(origin)
                        ))
                    });
                inspect_recoverable_fn_knock_custom_hostname(
                    api,
                    zone_id,
                    root,
                    item,
                    &exact_records,
                    recovery_origin.as_deref(),
                    instance,
                    recovered_origin,
                )
                .await?
            }
            _ => (None, Vec::new()),
        };
        if !recovery_origin_records.is_empty() {
            remote_snapshot.push(json!({
                "hostname": recoverable
                    .as_ref()
                    .map(|value| value.origin_hostname.clone())
                    .or_else(|| existing
                        .and_then(|item| item.get("custom_origin_server"))
                        .and_then(Value::as_str)
                        .map(str::to_string)),
                "dnsRecords": recovery_origin_records,
            }));
        }
        let owned_state = owned.and_then(|items| items.get(host));
        let owned_custom_matches = existing.is_some_and(|item| {
            owned_state.is_some_and(|state| {
                owned_id == item.get("id").and_then(Value::as_str)
                    && managed_custom_hostname_matches(item, host, state, Some(&origin))
            })
        });
        match existing {
            Some(_) if owned_custom_matches => {
                operations.push(preview_operation(
                    &format!("custom-hostname:{host}"),
                    "custom-hostname",
                    "keep",
                    host,
                    true,
                ));
            }
            Some(_) if recoverable.is_some() => operations.push(preview_operation(
                &format!("custom-hostname:{host}"),
                "custom-hostname",
                "recover",
                host,
                true,
            )),
            Some(_) => conflicts.push(json!({
                "id": format!("custom-hostname:{host}"),
                "kind": "custom-hostname",
                "target": host,
                "messageCode": "unownedCustomHostname",
                "message": "An unowned Cloudflare for SaaS Custom Hostname already exists",
                "takeoverAllowed": true,
            })),
            None if remaining > 0 => {
                operations.push(preview_operation(
                    &format!("custom-hostname:{host}"),
                    "custom-hostname",
                    "create",
                    host,
                    false,
                ));
                remaining -= 1;
            }
            None => operations.push(preview_operation(
                &format!("custom-hostname:{host}"),
                "custom-hostname",
                "fallback",
                host,
                false,
            )),
        }
        if recoverable.is_some() {
            operations.push(preview_operation(
                &format!("optimization:dns:{host}"),
                "dns",
                "recover",
                host,
                true,
            ));
        } else if ownership.pointer("/optimization/selected/ip").is_some() {
            let exact_owned_id = owned
                .and_then(|items| items.get(host))
                .and_then(|value| value.get("exactDnsId"))
                .and_then(Value::as_str);
            let exact_record = exact_owned_id
                .and_then(|id| {
                    exact_records
                        .iter()
                        .find(|record| record.get("id").and_then(Value::as_str) == Some(id))
                })
                .or_else(|| {
                    exact_records
                        .iter()
                        .find(|record| is_managed_dns(record, instance))
                })
                .or_else(|| {
                    exact_records.iter().find(|record| {
                        matches!(
                            record.get("type").and_then(Value::as_str),
                            Some("A" | "AAAA" | "CNAME")
                        )
                    })
                })
                .or_else(|| exact_records.first());
            if let Some(record) = exact_record {
                let edge = format!("fnknock-edge-{instance}.{root}");
                let exact_owned = dns_record_owned_for_update(
                    record,
                    exact_owned_id,
                    instance,
                    "CNAME",
                    Some(&edge),
                    false,
                );
                if exact_owned && exact_records.len() == 1 {
                    operations.push(preview_operation(
                        &format!("optimization:dns:{host}"),
                        "dns",
                        "update",
                        host,
                        true,
                    ));
                } else {
                    let single_record = exact_records.len() == 1;
                    conflicts.push(json!({
                        "id": format!("optimization:dns:{host}"),
                        "kind": "dns",
                        "target": host,
                        "messageCode": if single_record { "exactDnsConflict" } else { "multipleExactDnsConflict" },
                        "message": if single_record {
                            "An unowned exact DNS record prevents optimization"
                        } else {
                            "Multiple exact DNS records must be resolved before optimization"
                        },
                        "takeoverAllowed": single_record,
                        "details": dns_conflict_details(&exact_records, instance, "CNAME", &edge, false),
                    }));
                }
            } else {
                operations.push(preview_operation(
                    &format!("optimization:dns:{host}"),
                    "dns",
                    "create",
                    host,
                    false,
                ));
            }
        }
    }
    Ok(())
}
