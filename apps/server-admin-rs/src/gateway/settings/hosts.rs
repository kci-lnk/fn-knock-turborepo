use super::*;

pub(super) fn visible_host_mappings(mappings: &[Value]) -> Vec<Value> {
    mappings
        .iter()
        .filter(|mapping| !is_auth_service_mapping(mapping))
        .cloned()
        .collect()
}

pub(super) fn is_auth_service_mapping(mapping: &Value) -> bool {
    mapping
        .get("target")
        .and_then(Value::as_str)
        .is_some_and(is_auth_service_target)
}

pub(super) fn is_auth_service_target(target: &str) -> bool {
    is_http_proxy_target_url(target)
        && parse_target_port(target).is_some_and(|port| port == resolve_auth_service_port())
}

pub(super) fn is_http_proxy_target_url(target: &str) -> bool {
    Url::parse(target.trim()).ok().is_some_and(|url| {
        matches!(url.scheme(), "http" | "https" | "ws" | "wss") && url.host_str().is_some()
    })
}

pub(super) use crate::proxy_utils::parse_target_port_i64 as parse_target_port;

pub(super) fn resolve_auth_service_port() -> i64 {
    std::env::var("AUTH_PORT")
        .ok()
        .and_then(|value| value.trim().parse::<i64>().ok())
        .filter(|port| *port > 0)
        .unwrap_or(7997)
}

pub(super) fn build_gateway_proxy_header_items(hosts: &[Value], config: &Value) -> Vec<Value> {
    let disabled = disabled_hosts_set(config);
    hosts
        .iter()
        .map(|mapping| {
            let host = mapping.get("host").and_then(Value::as_str).unwrap_or("");
            json!({
                "host": host,
                "target": mapping.get("target").and_then(Value::as_str).unwrap_or("").trim(),
                "title": mapping.get("title").and_then(Value::as_str).unwrap_or("").trim(),
                "send_proxy_headers": !disabled.contains(&normalize_host(host)),
            })
        })
        .collect()
}

pub(super) fn build_gateway_host_response_items(hosts: &[Value], config: &Value) -> Vec<Value> {
    let disabled = disabled_hosts_set(config);
    hosts
        .iter()
        .map(|mapping| {
            let host = mapping.get("host").and_then(Value::as_str).unwrap_or("");
            json!({
                "host": host,
                "target": mapping.get("target").and_then(Value::as_str).unwrap_or("").trim(),
                "title": mapping.get("title").and_then(Value::as_str).unwrap_or("").trim(),
                "preserve_host": !disabled.contains(&normalize_host(host)),
            })
        })
        .collect()
}

pub(super) fn disabled_hosts_set(config: &Value) -> std::collections::HashSet<String> {
    config
        .get("disabled_hosts")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(normalize_host)
                .filter(|host| !host.is_empty())
                .collect()
        })
        .unwrap_or_default()
}
