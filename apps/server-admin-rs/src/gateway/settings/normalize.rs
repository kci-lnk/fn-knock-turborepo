use super::*;

pub(super) fn normalize_gateway_visibility(value: &Value) -> Value {
    json!({
        "enabled": value.get("enabled").and_then(Value::as_bool).unwrap_or(false),
        "selections": value.get("selections").and_then(Value::as_array).cloned().unwrap_or_default(),
        "custom_cidrs": string_list(value.get("custom_cidrs")),
    })
}

pub(super) fn normalize_disabled_hosts_config(value: &Value) -> Value {
    let mut seen = std::collections::HashSet::new();
    let disabled_hosts = value
        .get("disabled_hosts")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(normalize_host)
                .filter(|host| !host.is_empty() && seen.insert(host.clone()))
                .map(Value::String)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    json!({ "disabled_hosts": disabled_hosts })
}

pub(super) fn normalize_reverse_proxy_throttle(value: &Value) -> Value {
    json!({
        "enabled": value.get("enabled").and_then(Value::as_bool).unwrap_or(true),
        "requests_per_second": positive_int(value.get("requests_per_second"), 100),
        "burst": positive_int(value.get("burst"), 200),
        "block_seconds": positive_int(value.get("block_seconds"), 30),
    })
}

pub(super) fn normalize_gateway_crawler_blocker(value: &Value) -> Value {
    json!({
        "enabled": value.get("enabled").and_then(Value::as_bool).unwrap_or(false),
        "updated_at": value.get("updated_at").and_then(Value::as_str).map(|value| Value::String(value.to_string())).unwrap_or(Value::Null),
    })
}

pub(super) fn normalize_gateway_portal(value: &Value) -> Value {
    json!({
        "enabled": value.get("enabled").and_then(Value::as_bool).unwrap_or(true),
        "display_style": if value.get("display_style").and_then(Value::as_str) == Some("domain") { "domain" } else { "title" },
        "show_app_icon": value.get("show_app_icon").and_then(Value::as_bool).unwrap_or(true),
        "icon_drag_mode": if value.get("icon_drag_mode").and_then(Value::as_str) == Some("free") { "free" } else { "corners" },
    })
}

pub(super) fn normalize_gateway_unmatched_route(value: &Value) -> Value {
    json!({
        "behavior": if value.get("behavior").and_then(Value::as_str) == Some("reset_connection") {
            "reset_connection"
        } else {
            "error_page"
        },
    })
}

pub(super) fn is_gateway_portal_title_mode(config: &Value) -> bool {
    normalize_gateway_portal(
        config
            .get("gateway_portal")
            .unwrap_or(&default_gateway_portal()),
    )
    .get("display_style")
    .and_then(Value::as_str)
        != Some("domain")
}

pub(super) fn is_gateway_portal_app_icon_mode(config: &Value) -> bool {
    normalize_gateway_portal(
        config
            .get("gateway_portal")
            .unwrap_or(&default_gateway_portal()),
    )
    .get("show_app_icon")
    .and_then(Value::as_bool)
        != Some(false)
}

pub(super) fn merge_objects(previous: &Value, patch: &Value) -> Value {
    let mut merged = previous.as_object().cloned().unwrap_or_default();
    if let Some(patch) = patch.as_object() {
        for (key, value) in patch {
            merged.insert(key.clone(), value.clone());
        }
    }
    Value::Object(merged)
}

pub(super) use crate::json_utils::ensure_object;

pub(super) fn string_list(value: Option<&Value>) -> Vec<Value> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(|item| Value::String(item.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn normalize_host(value: &str) -> String {
    let without_scheme = value
        .trim()
        .to_ascii_lowercase()
        .split_once("://")
        .map(|(_, rest)| rest.to_string())
        .unwrap_or_else(|| value.trim().to_ascii_lowercase());
    without_scheme
        .split('/')
        .next()
        .unwrap_or("")
        .trim_end_matches('.')
        .to_string()
}

pub(super) fn positive_int(value: Option<&Value>, fallback: i64) -> i64 {
    number_floor_value_or_parse(value)
        .filter(|value| *value > 0)
        .unwrap_or(fallback)
}

pub(super) fn normalize_cache_ttl(value: Option<&Value>, fallback: i64) -> i64 {
    number_floor_value_or_parse(value)
        .filter(|value| *value >= 0)
        .unwrap_or(fallback)
}

pub(super) fn number_floor_value_or_parse(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::String(raw) => crate::node_compat::parse_i64_prefix(raw.trim_start()),
        other => number_floor(other),
    }
}

pub(super) fn number_floor(value: &Value) -> Option<i64> {
    if let Some(value) = value.as_i64() {
        return Some(value);
    }
    let value = value.as_f64()?;
    if value.is_finite() {
        Some(value.floor() as i64)
    } else {
        None
    }
}

pub(super) fn default_gateway_visibility_runtime() -> Value {
    json!({ "enabled": false, "cidrs": [], "updated_at": null })
}

pub(super) fn default_gateway_proxy_headers_runtime() -> Value {
    json!({ "enabled": false, "omit_targets": [], "updated_at": null })
}

pub(super) fn default_gateway_host_response_runtime() -> Value {
    json!({ "enabled": false, "omit_targets": [], "updated_at": null })
}

pub(super) fn default_gateway_visibility() -> Value {
    json!({ "enabled": false, "selections": [], "custom_cidrs": [] })
}

pub(super) fn default_disabled_hosts_config() -> Value {
    json!({ "disabled_hosts": [] })
}

pub(super) fn default_reverse_proxy_throttle() -> Value {
    json!({
        "enabled": true,
        "requests_per_second": 100,
        "burst": 200,
        "block_seconds": 30,
    })
}

pub(super) fn default_gateway_crawler_blocker() -> Value {
    json!({ "enabled": false, "updated_at": null })
}

pub(super) fn default_gateway_portal() -> Value {
    json!({
        "enabled": true,
        "display_style": "title",
        "show_app_icon": true,
        "icon_drag_mode": "corners",
    })
}

pub(super) fn default_gateway_unmatched_route() -> Value {
    json!({ "behavior": "error_page" })
}

pub(super) fn default_subdomain_mode() -> Value {
    json!({
        "auth_cache_ttl_seconds": 1,
        "auth_cache_unauthorized_ttl_seconds": 1,
    })
}
