use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) async fn inspect_auxiliary_dns(
    api: &CloudflareApi,
    zone_id: &str,
    name: &str,
    owned_id: Option<&str>,
    logical_id: &str,
    instance_id: &str,
    record_type: &str,
    content: Option<&str>,
    proxied: bool,
    operations: &mut Vec<Value>,
    conflicts: &mut Vec<Value>,
    remote_snapshot: &mut Vec<Value>,
) -> Result<(), CloudflareApiError> {
    let records = api.list_dns_records(zone_id, Some(name)).await?;
    remote_snapshot.push(json!({
        "hostname": name,
        "dnsRecords": records.clone(),
    }));
    if records.is_empty() {
        operations.push(preview_operation(logical_id, "dns", "create", name, false));
        return Ok(());
    }
    let record = owned_id
        .and_then(|id| {
            records
                .iter()
                .find(|record| record.get("id").and_then(Value::as_str) == Some(id))
        })
        .or_else(|| {
            records
                .iter()
                .find(|record| is_managed_dns(record, instance_id))
        })
        .unwrap_or(&records[0]);
    if records.len() == 1
        && dns_record_owned_for_update(record, owned_id, instance_id, record_type, content, proxied)
    {
        operations.push(preview_operation(logical_id, "dns", "update", name, true));
    } else {
        let single_record = records.len() == 1;
        conflicts.push(json!({
            "id": logical_id,
            "kind": "dns",
            "target": name,
            "messageCode": if single_record { "optimizationDnsConflict" } else { "multipleOptimizationDnsConflict" },
            "message": if single_record {
                "An unowned DNS record already uses the optimization hostname"
            } else {
                "Multiple DNS records already use the optimization hostname"
            },
            "takeoverAllowed": single_record,
            "details": dns_conflict_details(
                &records,
                instance_id,
                record_type,
                content.unwrap_or(""),
                proxied,
            ),
        }));
    }
    Ok(())
}

pub(super) fn recoverable_fn_knock_custom_hostname_from_snapshot(
    custom: &Value,
    exact_records: &[Value],
    origin_records: &[Value],
    recovery_origin: Option<&str>,
    root: &str,
    current_instance_id: &str,
    recovered_origin: Option<&Value>,
) -> Option<RecoverableCustomHostname> {
    let origin_hostname = custom
        .get("custom_origin_server")
        .and_then(Value::as_str)?
        .trim()
        .trim_end_matches('.')
        .to_ascii_lowercase();
    let expected_recovery_origin = recovery_origin?
        .trim()
        .trim_end_matches('.')
        .to_ascii_lowercase();
    if origin_hostname != expected_recovery_origin {
        return None;
    }
    let legacy_instance_id = fn_knock_origin_instance(&origin_hostname, root)?;
    let hostname = custom.get("hostname").and_then(Value::as_str)?;
    let expected_edge = format!("fnknock-edge-{legacy_instance_id}.{root}");
    let exact_dns = exact_records.iter().find(|record| {
        record
            .get("name")
            .and_then(Value::as_str)
            .is_some_and(|value| value.eq_ignore_ascii_case(hostname))
            && record.get("type").and_then(Value::as_str) == Some("CNAME")
            && record
                .get("content")
                .and_then(Value::as_str)
                .is_some_and(|value| {
                    value.eq_ignore_ascii_case(&expected_edge)
                        || value.eq_ignore_ascii_case(&origin_hostname)
                })
            && record.get("proxied").and_then(Value::as_bool) == Some(false)
            && is_managed_dns(record, &legacy_instance_id)
    })?;
    let origin_dns = origin_records.iter().find(|record| {
        let tunnel_target = record
            .get("content")
            .and_then(Value::as_str)
            .map(|value| value.trim().trim_end_matches('.'));
        record
            .get("name")
            .and_then(Value::as_str)
            .is_some_and(|value| value.eq_ignore_ascii_case(&origin_hostname))
            && record.get("type").and_then(Value::as_str) == Some("CNAME")
            && tunnel_target.is_some_and(|value| {
                value
                    .strip_suffix(".cfargotunnel.com")
                    .is_some_and(|id| uuid::Uuid::parse_str(id).is_ok())
            })
            && record.get("proxied").and_then(Value::as_bool) == Some(true)
            && (is_managed_dns(record, &legacy_instance_id)
                || recovered_origin.is_some_and(|saved| {
                    saved.get("recoveredFromInstance").and_then(Value::as_str)
                        == Some(legacy_instance_id.as_str())
                        && saved.get("id").and_then(Value::as_str)
                            == record.get("id").and_then(Value::as_str)
                        && saved.get("name").and_then(Value::as_str)
                            == record.get("name").and_then(Value::as_str)
                        && saved.get("type").and_then(Value::as_str)
                            == record.get("type").and_then(Value::as_str)
                        && saved.get("content").and_then(Value::as_str)
                            == record.get("content").and_then(Value::as_str)
                        && saved.get("proxied").and_then(Value::as_bool)
                            == record.get("proxied").and_then(Value::as_bool)
                        && is_managed_dns(record, current_instance_id)
                }))
    })?;
    Some(RecoverableCustomHostname {
        legacy_instance_id,
        origin_hostname,
        origin_dns: origin_dns.clone(),
        exact_dns: exact_dns.clone(),
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn inspect_recoverable_fn_knock_custom_hostname(
    api: &CloudflareApi,
    zone_id: &str,
    root: &str,
    custom: &Value,
    exact_records: &[Value],
    recovery_origin: Option<&str>,
    current_instance_id: &str,
    recovered_origin: Option<&Value>,
) -> Result<(Option<RecoverableCustomHostname>, Vec<Value>), CloudflareApiError> {
    let Some(origin_hostname) = custom
        .get("custom_origin_server")
        .and_then(Value::as_str)
        .filter(|value| fn_knock_origin_instance(value, root).is_some())
    else {
        return Ok((None, Vec::new()));
    };
    let origin_records = api.list_dns_records(zone_id, Some(origin_hostname)).await?;
    let recoverable = recoverable_fn_knock_custom_hostname_from_snapshot(
        custom,
        exact_records,
        &origin_records,
        recovery_origin,
        root,
        current_instance_id,
        recovered_origin,
    );
    Ok((recoverable, origin_records))
}

pub(super) async fn adopt_recoverable_fn_knock_origin(
    state: &AppState,
    api: &CloudflareApi,
    zone_id: &str,
    ownership: &mut Value,
    recoverable: &RecoverableCustomHostname,
    origin_target: &str,
    instance_id: &str,
) -> Result<(), CloudflareApiError> {
    let origin_dns = upsert_managed_dns(
        api,
        ManagedDnsRequest {
            zone_id,
            name: &recoverable.origin_hostname,
            record_type: "CNAME",
            content: origin_target,
            proxied: true,
            owned_id: recoverable.origin_dns.get("id").and_then(Value::as_str),
            takeover: true,
            instance_id,
        },
    )
    .await?;
    let mut recovered_origin = origin_dns;
    ensure_object(&mut recovered_origin).insert(
        "recoveredFromInstance".to_string(),
        json!(recoverable.legacy_instance_id),
    );
    ensure_nested_object(ownership, &["optimization", "recoveredOrigins"])
        .insert(recoverable.origin_hostname.clone(), recovered_origin);
    save_managed_state(state, ownership).await
}
