use super::*;

pub(in super::super) async fn append_cleanup_remote_snapshot(
    api: &CloudflareApi,
    zone_id: &str,
    ownership: &Value,
    instance_id: &str,
    custom_hostnames: &[Value],
    conflicts: &mut Vec<Value>,
    remote_snapshot: &mut Vec<Value>,
) -> Result<(), CloudflareApiError> {
    let fallback_origin = api.get_fallback_origin(zone_id).await?;
    if let Some(owned) = ownership.pointer("/optimization/fallbackOrigin") {
        let expected = owned.get("origin").and_then(Value::as_str);
        let remote_origin = fallback_origin
            .as_ref()
            .and_then(|value| value.get("origin"))
            .and_then(Value::as_str);
        if expected.is_some() && remote_origin.is_some() && expected != remote_origin {
            conflicts.push(json!({
                "id": "optimization:cleanup-fallback-origin",
                "kind": "custom-hostname",
                "target": "Cloudflare for SaaS fallback origin",
                "messageCode": "fallbackOriginChanged",
                "message": "The previously managed fallback origin has been changed by another configuration",
                "takeoverAllowed": false,
            }));
        }
    }
    remote_snapshot.push(json!({ "fallbackOrigin": fallback_origin }));

    let default_custom_origin = ownership
        .pointer("/optimization/originDns/name")
        .and_then(Value::as_str);
    for (hostname, state) in ownership
        .pointer("/optimization/customHostnames")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|items| items.iter())
    {
        let Some(id) = state.get("id").and_then(Value::as_str) else {
            continue;
        };
        let Some(remote) = custom_hostnames
            .iter()
            .find(|item| item.get("id").and_then(Value::as_str) == Some(id))
        else {
            continue;
        };
        if !managed_custom_hostname_matches(remote, hostname, state, default_custom_origin) {
            conflicts.push(json!({
                "id": format!("optimization:cleanup-custom-hostname:{id}"),
                "kind": "custom-hostname",
                "target": hostname,
                "messageCode": "managedCustomHostnameChanged",
                "message": "A previously managed Custom Hostname was changed by another configuration",
                "takeoverAllowed": false,
            }));
        }
    }
    if let Some(probe) = ownership.pointer("/optimization/capabilityProbe")
        && let Some(id) = probe.get("id").and_then(Value::as_str)
        && let Some(remote) = custom_hostnames
            .iter()
            .find(|item| item.get("id").and_then(Value::as_str) == Some(id))
    {
        let hostname = probe.get("hostname").and_then(Value::as_str).unwrap_or("");
        if !managed_custom_hostname_matches(remote, hostname, probe, default_custom_origin) {
            conflicts.push(json!({
                "id": format!("optimization:cleanup-capability-hostname:{id}"),
                "kind": "custom-hostname",
                "target": hostname,
                "messageCode": "capabilityHostnameChanged",
                "message": "The previously managed capability Custom Hostname was changed by another configuration",
                "takeoverAllowed": false,
            }));
        }
    }

    let mut tracked = Vec::new();
    for path in ["/optimization/originDns", "/optimization/edgeDns"] {
        if let Some(record) = ownership.pointer(path) {
            tracked.push(record.clone());
        }
    }
    tracked.extend(
        ownership
            .pointer("/optimization/recoveredOrigins")
            .and_then(Value::as_object)
            .into_iter()
            .flat_map(|items| items.values().cloned()),
    );
    for (hostname, state) in ownership
        .pointer("/optimization/customHostnames")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|items| items.iter())
    {
        if let Some(id) = state.get("exactDnsId").and_then(Value::as_str) {
            tracked.push(json!({
                "id": id,
                "name": hostname,
                "type": "CNAME",
                "content": ownership.pointer("/optimization/edgeDns/name").cloned().unwrap_or(Value::Null),
                "proxied": false,
            }));
        }
        for record in state
            .get("validationDns")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            tracked.push(record.clone());
        }
    }
    if let Some(probe) = ownership.pointer("/optimization/capabilityProbe") {
        if let Some(record) = probe.get("activationDns") {
            tracked.push(record.clone());
        }
        for record in probe
            .get("validationDns")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            tracked.push(record.clone());
        }
    }
    let mut names = tracked
        .iter()
        .filter_map(|record| record.get("name").and_then(Value::as_str))
        .map(str::to_string)
        .collect::<Vec<_>>();
    names.sort();
    names.dedup();
    for name in names {
        let records = api.list_dns_records(zone_id, Some(&name)).await?;
        for owned in tracked
            .iter()
            .filter(|record| record.get("name").and_then(Value::as_str) == Some(name.as_str()))
        {
            let Some(id) = owned.get("id").and_then(Value::as_str) else {
                continue;
            };
            let Some(remote) = records
                .iter()
                .find(|record| record.get("id").and_then(Value::as_str) == Some(id))
            else {
                continue;
            };
            let record_type = owned.get("type").and_then(Value::as_str).unwrap_or("");
            let content = owned.get("content").and_then(Value::as_str);
            let proxied = owned
                .get("proxied")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if record_type.is_empty()
                || !dns_record_owned_for_update(
                    remote,
                    Some(id),
                    instance_id,
                    record_type,
                    content,
                    proxied,
                )
            {
                conflicts.push(json!({
                    "id": format!("optimization:cleanup-dns:{id}"),
                    "kind": "dns",
                    "target": name.clone(),
                    "messageCode": "managedOptimizationDnsChanged",
                    "message": "A previously managed optimization DNS record has been claimed or changed by another configuration",
                    "takeoverAllowed": false,
                }));
            }
        }
        remote_snapshot.push(json!({ "hostname": name, "dnsRecords": records }));
    }
    Ok(())
}
