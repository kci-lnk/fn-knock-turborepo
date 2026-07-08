use super::*;

pub(super) fn string_array(value: Option<&Value>) -> Vec<Value> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| Value::String(value.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

pub(super) use crate::proxy_utils::{
    is_any_subdomain_routing_mode, is_reverse_proxy_subdomain_mode,
};

pub(super) fn config_array_len(config: &Value, key: &str) -> usize {
    config
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or_default()
}

pub(super) fn proxy_protocol_force_payload(value: &Value, fallback: bool) -> Value {
    let force = value
        .pointer("/data/proxy_protocol_force")
        .and_then(Value::as_bool)
        .or_else(|| value.get("proxy_protocol_force").and_then(Value::as_bool))
        .unwrap_or(fallback);
    json!({ "proxy_protocol_force": force })
}

pub(super) fn go_response_message(value: &Value, fallback: &str) -> String {
    crate::go_backend::response_message(value, fallback)
}

pub(super) use crate::runtime_profile::host_runtime_available;

pub(super) fn ensure_go_success(value: Value) -> anyhow::Result<()> {
    if crate::go_backend::response_success(&value) {
        return Ok(());
    }
    anyhow::bail!(
        "{}",
        crate::go_backend::response_message(&value, GO_BACKEND_UNSUCCESSFUL_RESPONSE)
    )
}

pub(super) use crate::runtime_profile::host_firewall_available;

pub(super) use crate::runtime_profile::deployment_target;
