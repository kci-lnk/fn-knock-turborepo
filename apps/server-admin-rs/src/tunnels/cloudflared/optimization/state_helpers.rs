use std::collections::HashMap;

use serde_json::{Map, Value, json};

pub(super) fn parse_trace(value: &str) -> HashMap<String, String> {
    value
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.trim().to_string(), value.trim().to_string()))
        .collect()
}

pub(super) fn scan_validation_hostname(ownership: &Value) -> Option<String> {
    if ownership
        .pointer("/optimization/customHostnames")
        .and_then(Value::as_object)
        .is_some_and(|items| {
            items
                .values()
                .any(|state| state.get("status").and_then(Value::as_str) == Some("conflict"))
        })
    {
        return None;
    }
    optimized_health_hostname(ownership).or_else(|| {
        ownership
            .pointer("/optimization/customHostnames")
            .and_then(Value::as_object)
            .and_then(|items| {
                items.iter().find_map(|(hostname, state)| {
                    scan_business_hostname_is_ready(state).then(|| hostname.clone())
                })
            })
            .or_else(|| {
                let probe = ownership.pointer("/optimization/capabilityProbe")?;
                capability_probe_hostname_is_ready(probe)
                    .then(|| {
                        probe
                            .get("hostname")
                            .and_then(Value::as_str)
                            .map(str::to_string)
                    })
                    .flatten()
            })
    })
}

pub(super) fn scan_business_hostname_is_ready(state: &Value) -> bool {
    custom_hostname_can_validate_candidates(state)
}

pub(super) fn custom_hostname_activation_status(state: &Value) -> Option<&str> {
    state
        .get("hostnameStatus")
        .and_then(Value::as_str)
        .or_else(|| {
            let legacy_status = state.get("status").and_then(Value::as_str)?;
            Some(
                if matches!(
                    legacy_status,
                    "ready" | "optimized" | "probe-failed" | "fallback"
                ) {
                    "active"
                } else {
                    legacy_status
                },
            )
        })
}

pub(super) fn custom_hostname_can_validate_candidates(state: &Value) -> bool {
    // Candidate probes override DNS and connect directly to the supplied edge
    // IP. Exact DNS is therefore not required during fallback recovery, but a
    // tracked active hostname and certificate remain mandatory for TLS/SNI.
    let management_status = state.get("status").and_then(Value::as_str);
    management_status != Some("conflict")
        && state
            .get("id")
            .and_then(Value::as_str)
            .is_some_and(|id| !id.is_empty())
        && custom_hostname_activation_status(state) == Some("active")
        && state.get("sslStatus").and_then(Value::as_str) == Some("active")
}

pub(super) fn capability_probe_hostname_is_ready(probe: &Value) -> bool {
    let cloudflare_ready = probe.get("hostnameStatus").and_then(Value::as_str) == Some("active")
        && probe.get("sslStatus").and_then(Value::as_str) == Some("active");
    let activation_dns_ready = probe
        .pointer("/activationDns/id")
        .and_then(Value::as_str)
        .is_some_and(|id| !id.is_empty());
    activation_dns_ready && cloudflare_ready
}

pub(super) fn extract_validation_records(custom: &Value) -> Vec<(String, String)> {
    let mut output = Vec::new();
    if let Some(record) = custom.get("ownership_verification") {
        let name = record
            .get("name")
            .or_else(|| record.get("txt_name"))
            .and_then(Value::as_str);
        let value = record
            .get("value")
            .or_else(|| record.get("txt_value"))
            .or_else(|| record.get("txt_record"))
            .and_then(Value::as_str);
        if let (Some(name), Some(value)) = (name, value) {
            output.push((name.to_string(), value.to_string()));
        }
    }
    for record in custom
        .pointer("/ssl/validation_records")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let name = record
            .get("txt_name")
            .or_else(|| record.get("name"))
            .and_then(Value::as_str);
        let value = record
            .get("txt_value")
            .or_else(|| record.get("txt_record"))
            .or_else(|| record.get("value"))
            .and_then(Value::as_str);
        if let (Some(name), Some(value)) = (name, value) {
            output.push((name.to_string(), value.to_string()));
        }
    }
    output.sort();
    output.dedup();
    output
}

pub(super) fn set_host_state(ownership: &mut Value, hostname: &str, value: Value) {
    ensure_nested_object(ownership, &["optimization", "customHostnames"])
        .insert(hostname.to_string(), value);
}

pub(super) fn ensure_nested_object<'a>(
    value: &'a mut Value,
    path: &[&str],
) -> &'a mut Map<String, Value> {
    let mut current = value;
    for segment in path {
        let object = ensure_object(current);
        current = object
            .entry((*segment).to_string())
            .or_insert_with(|| json!({}));
    }
    ensure_object(current)
}

pub(super) fn ensure_object(value: &mut Value) -> &mut Map<String, Value> {
    if !value.is_object() {
        *value = json!({});
    }
    value
        .as_object_mut()
        .expect("value was normalized to object")
}

pub(super) fn preview_operation(
    id: &str,
    kind: &str,
    action: &str,
    target: &str,
    owned: bool,
) -> Value {
    json!({ "id": id, "kind": kind, "action": action, "target": target, "owned": owned })
}

pub(super) fn is_managed_dns(record: &Value, instance_id: &str) -> bool {
    let expected_comment = format!("Managed by fn-knock ({instance_id})");
    let expected_tag = format!("fn-knock-instance:{instance_id}");
    record
        .get("comment")
        .and_then(Value::as_str)
        .is_some_and(|value| value == expected_comment)
        || record
            .get("tags")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .any(|value| value.as_str() == Some(expected_tag.as_str()))
}

pub(super) fn dns_record_owner_kind(record: &Value, instance_id: &str) -> &'static str {
    if is_managed_dns(record, instance_id) {
        return "current-instance";
    }
    let fn_knock_comment = record
        .get("comment")
        .and_then(Value::as_str)
        .is_some_and(|value| value.starts_with("Managed by fn-knock ("));
    let fn_knock_tag = record
        .get("tags")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .any(|value| value.starts_with("fn-knock-instance:"));
    if fn_knock_comment || fn_knock_tag {
        "other-fn-knock-instance"
    } else {
        "external"
    }
}

pub(super) fn dns_conflict_details(
    records: &[Value],
    instance_id: &str,
    desired_type: &str,
    desired_content: &str,
    desired_proxied: bool,
) -> Value {
    json!({
        "records": records.iter().map(|record| json!({
            "type": record.get("type").cloned().unwrap_or(Value::Null),
            "content": record.get("content").cloned().unwrap_or(Value::Null),
            "proxied": record.get("proxied").cloned().unwrap_or(Value::Null),
            "ownerKind": dns_record_owner_kind(record, instance_id),
        })).collect::<Vec<_>>(),
        "desired": {
            "type": desired_type,
            "content": desired_content,
            "proxied": desired_proxied,
        },
    })
}

pub(super) fn managed_custom_hostname_matches(
    remote: &Value,
    hostname: &str,
    owned: &Value,
    default_origin: Option<&str>,
) -> bool {
    let expected_origin = owned
        .get("customOriginServer")
        .and_then(Value::as_str)
        .or(default_origin);
    owned
        .get("id")
        .and_then(Value::as_str)
        .is_some_and(|id| remote.get("id").and_then(Value::as_str) == Some(id))
        && remote
            .get("hostname")
            .and_then(Value::as_str)
            .is_some_and(|value| value.eq_ignore_ascii_case(hostname))
        && expected_origin.is_some_and(|expected| {
            remote
                .get("custom_origin_server")
                .and_then(Value::as_str)
                .is_some_and(|value| value.eq_ignore_ascii_case(expected))
        })
}

pub(super) fn fn_knock_origin_instance(origin: &str, root: &str) -> Option<String> {
    let normalized_origin = origin.trim().trim_end_matches('.').to_ascii_lowercase();
    let normalized_root = root.trim().trim_end_matches('.').to_ascii_lowercase();
    let label = normalized_origin.strip_suffix(&format!(".{normalized_root}"))?;
    let instance_id = label.strip_prefix("fnknock-origin-")?;
    (instance_id.len() == 12 && instance_id.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| instance_id.to_string())
}

pub(super) fn should_publish_exact_routes(ownership: &Value, force_publish: bool) -> bool {
    force_publish
        || ownership
            .pointer("/optimization/publishSuppressed")
            .and_then(Value::as_bool)
            != Some(true)
}

pub(super) fn exact_route_is_optimized(state: &Value) -> bool {
    state
        .get("exactDnsId")
        .and_then(Value::as_str)
        .is_some_and(|id| !id.is_empty())
        && (state.get("exactDnsTarget").and_then(Value::as_str) == Some("edge")
            || (state.get("exactDnsTarget").is_none()
                && state.get("status").and_then(Value::as_str) == Some("optimized")))
}

pub(super) fn optimized_health_hostname(ownership: &Value) -> Option<String> {
    ownership
        .pointer("/optimization/customHostnames")
        .and_then(Value::as_object)
        .and_then(|items| {
            items.iter().find_map(|(hostname, state)| {
                (exact_route_is_optimized(state)
                    && state.get("sslStatus").and_then(Value::as_str) == Some("active"))
                .then(|| hostname.clone())
            })
        })
}

pub(super) fn legacy_publish_suppression(ownership: &Value, runtime: &Value) -> bool {
    ownership
        .pointer("/optimization/fallbackActive")
        .and_then(Value::as_bool)
        == Some(true)
        && matches!(
            runtime.get("lastSwitchReason").and_then(Value::as_str),
            Some("manual-fallback" | "health-fallback")
        )
}

pub(super) fn cloudflare_error_list_message(errors: &Value) -> Option<String> {
    let messages = errors
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| {
            item.as_str()
                .or_else(|| item.get("message").and_then(Value::as_str))
                .map(str::trim)
                .filter(|value| !value.is_empty())
        })
        .collect::<Vec<_>>();
    (!messages.is_empty()).then(|| messages.join("; "))
}

pub(super) fn scan_job_active(job: &Value) -> bool {
    matches!(
        job.get("status").and_then(Value::as_str),
        Some("queued" | "running")
    ) && job.get("cancelRequested").and_then(Value::as_bool) != Some(true)
}

pub(super) fn json_pointer_escape(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}
