use super::*;

pub(super) fn normalize_subdomain_mode_config(value: &Value) -> Value {
    let object = value.as_object().cloned().unwrap_or_default();
    let mut edge_client_ip_enabled = object
        .get("edge_client_ip_enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut aliyun_esa_enabled = object
        .get("aliyun_esa_enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut tencent_edgeone_enabled = object
        .get("tencent_edgeone_enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    if !object.contains_key("edge_client_ip_enabled")
        && (aliyun_esa_enabled || tencent_edgeone_enabled)
    {
        edge_client_ip_enabled = true;
    }
    if !edge_client_ip_enabled {
        aliyun_esa_enabled = false;
        tencent_edgeone_enabled = false;
    }
    if tencent_edgeone_enabled && aliyun_esa_enabled {
        aliyun_esa_enabled = false;
    }

    json!({
        "root_domain": object.get("root_domain").and_then(Value::as_str).map(|value| value.trim().to_ascii_lowercase()).unwrap_or_default(),
        "auth_host": normalize_host_value(object.get("auth_host").and_then(Value::as_str).unwrap_or("")),
        "auth_target": object.get("auth_target").and_then(Value::as_str).map(str::trim).filter(|value| !value.is_empty()).map(ToString::to_string).unwrap_or_else(default_subdomain_auth_target),
        "cookie_domain": object.get("cookie_domain").and_then(Value::as_str).map(str::trim).unwrap_or("").to_string(),
        "edge_client_ip_enabled": edge_client_ip_enabled,
        "aliyun_esa_enabled": aliyun_esa_enabled,
        "tencent_edgeone_enabled": tencent_edgeone_enabled,
        "public_auth_base_url": object.get("public_auth_base_url").and_then(Value::as_str).map(|value| value.trim().trim_end_matches('/').to_string()).unwrap_or_default(),
        "public_http_port": normalize_public_port(object.get("public_http_port")),
        "public_https_port": normalize_public_port(object.get("public_https_port")),
        "auth_cache_ttl_seconds": normalize_cache_ttl(object.get("auth_cache_ttl_seconds"), 1),
        "auth_cache_unauthorized_ttl_seconds": normalize_cache_ttl(object.get("auth_cache_unauthorized_ttl_seconds"), 1),
        "default_access_mode": normalize_access_mode(object.get("default_access_mode")),
        "auto_add_whitelist_on_login": object.get("auto_add_whitelist_on_login").and_then(Value::as_bool).unwrap_or(true),
        "passkey_rp_mode": if object.get("passkey_rp_mode").and_then(Value::as_str) == Some("parent_domain") { "parent_domain" } else { "auth_host" },
        "passkey_rp_id": object.get("passkey_rp_id").and_then(Value::as_str).map(|value| value.trim().to_ascii_lowercase()).unwrap_or_default(),
    })
}

pub(super) fn validate_subdomain_root_domain(value: &Value) -> Result<(), &'static str> {
    let root_domain = value
        .get("root_domain")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if root_domain.contains('*') {
        return Err("Subdomain root domain cannot contain wildcard");
    }
    Ok(())
}

pub(super) fn previous_host_mappings_by_host(config: &Value) -> HashMap<String, Value> {
    config
        .get("host_mappings")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.get("host")
                        .and_then(Value::as_str)
                        .map(|host| (normalize_host_value(host), item.clone()))
                })
                .collect::<HashMap<_, _>>()
        })
        .unwrap_or_default()
}

pub(super) fn previous_host_mappings_by_target(config: &Value) -> HashMap<String, Vec<Value>> {
    let mut candidates = HashMap::<String, Vec<Value>>::new();
    for mapping in config
        .get("host_mappings")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let target = mapping
            .get("target")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("");
        if target.is_empty() {
            continue;
        }
        candidates
            .entry(target.to_string())
            .or_default()
            .push(mapping.clone());
    }
    candidates
}

pub(super) fn get_auth_host_mapping(config: &Value) -> Option<&Value> {
    config
        .get("host_mappings")
        .and_then(Value::as_array)?
        .iter()
        .find(|mapping| {
            mapping
                .get("target")
                .and_then(Value::as_str)
                .is_some_and(is_auth_service_target)
        })
}

pub(super) fn normalize_metadata_string(
    input: Option<&Value>,
    previous: Option<&Value>,
    previous_key: &str,
    can_reuse_previous: bool,
) -> String {
    if let Some(value) = input.and_then(Value::as_str) {
        return value.trim().to_string();
    }
    if can_reuse_previous {
        return previous
            .and_then(|value| value.get(previous_key))
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("")
            .to_string();
    }
    String::new()
}

pub(super) fn normalize_host_basic_auth(value: Option<&Value>) -> Value {
    if host_basic_auth_invalid(value) || !host_basic_auth_enabled(value) {
        return disabled_host_basic_auth();
    }
    let Some(object) = value.and_then(Value::as_object) else {
        return disabled_host_basic_auth();
    };
    json!({
        "enabled": true,
        "username": object.get("username").and_then(Value::as_str).unwrap_or("").trim(),
        "password": object.get("password").and_then(Value::as_str).unwrap_or(""),
    })
}

pub(super) fn disabled_host_basic_auth() -> Value {
    json!({
        "enabled": false,
        "username": "",
        "password": "",
    })
}

pub(super) fn host_basic_auth_enabled(value: Option<&Value>) -> bool {
    value
        .and_then(Value::as_object)
        .and_then(|object| object.get("enabled"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

pub(super) fn host_basic_auth_invalid(value: Option<&Value>) -> bool {
    if !host_basic_auth_enabled(value) {
        return false;
    }
    let Some(object) = value.and_then(Value::as_object) else {
        return true;
    };
    let username = object
        .get("username")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let password = object.get("password").and_then(Value::as_str).unwrap_or("");
    username.is_empty() || password.is_empty() || username.contains(':')
}

pub(super) fn is_supported_proxy_target_url(value: &str) -> bool {
    let target = value.trim();
    if target.is_empty() || has_explicit_empty_port(target) {
        return false;
    }
    let Ok(parsed) = Url::parse(target) else {
        return false;
    };
    matches!(parsed.scheme(), "http" | "https" | "ws" | "wss")
        && parsed
            .host_str()
            .is_some_and(|host| !host.trim().is_empty())
}

pub(super) fn has_explicit_empty_port(value: &str) -> bool {
    let Some((_, endpoint)) = value.trim().split_once("://") else {
        return false;
    };
    let boundary = endpoint.find(['/', '?', '#']).unwrap_or(endpoint.len());
    let authority_with_credentials = &endpoint[..boundary];
    let authority = authority_with_credentials
        .rsplit_once('@')
        .map(|(_, authority)| authority)
        .unwrap_or(authority_with_credentials);
    authority.ends_with(':')
}

pub(super) fn is_valid_host_port(value: &str) -> bool {
    let target = value.trim();
    if target.is_empty()
        || target.contains("://")
        || target.contains('/')
        || target.chars().any(char::is_whitespace)
    {
        return false;
    }
    if let Some(rest) = target.strip_prefix('[') {
        let Some((host, port_part)) = rest.split_once("]:") else {
            return false;
        };
        return !host.trim().is_empty() && valid_port_string(port_part);
    }
    let Some((host, port_part)) = target.rsplit_once(':') else {
        return false;
    };
    !host.trim().is_empty() && !host.contains(':') && valid_port_string(port_part)
}

pub(super) fn valid_port_string(value: &str) -> bool {
    let Ok(port) = value.parse::<u16>() else {
        return false;
    };
    port > 0
}

pub(super) fn is_auth_service_target(target: &str) -> bool {
    is_supported_proxy_target_url(target)
        && parse_target_port(target).is_some_and(|port| port == resolve_auth_service_port())
}

pub(super) use crate::proxy_utils::parse_target_port_i64 as parse_target_port;

pub(super) fn resolve_auth_service_port() -> i64 {
    i64::from(crate::proxy_utils::auth_service_port())
}

pub(super) fn default_subdomain_auth_target() -> String {
    crate::proxy_utils::default_auth_service_target()
}

pub(super) fn normalize_host_value(value: &str) -> String {
    let without_scheme = value
        .trim()
        .to_ascii_lowercase()
        .split_once("://")
        .map(|(_, rest)| rest.to_string())
        .unwrap_or_else(|| value.trim().to_ascii_lowercase());
    let authority = without_scheme
        .split('/')
        .next()
        .unwrap_or("")
        .trim()
        .trim_start_matches('.');
    let without_port = if authority.starts_with('[') {
        authority
            .find(']')
            .map(|end| &authority[..=end])
            .unwrap_or(authority)
    } else if let Some((host, _port)) = authority.rsplit_once(':') {
        if host.contains(':') { authority } else { host }
    } else {
        authority
    };
    without_port.trim_end_matches('.').to_string()
}

pub(super) fn normalize_access_mode(value: Option<&Value>) -> String {
    if value.and_then(Value::as_str) == Some("strict_whitelist") {
        "strict_whitelist".to_string()
    } else {
        "login_first".to_string()
    }
}

pub(super) fn normalize_protocol_mode(value: Option<&Value>) -> String {
    match value.and_then(Value::as_str).map(str::trim) {
        Some(value) if value.eq_ignore_ascii_case("auto") => "auto".to_string(),
        Some(value) if value.eq_ignore_ascii_case("http1") => "http1".to_string(),
        Some(value) if value.eq_ignore_ascii_case("http2") => "http2".to_string(),
        _ => "auto".to_string(),
    }
}

pub(super) fn normalize_target_path_mode(value: Option<&Value>) -> String {
    match value.and_then(Value::as_str).map(str::trim) {
        Some(value) if value.eq_ignore_ascii_case("prefix") => "prefix".to_string(),
        _ => "entry".to_string(),
    }
}

pub(super) fn parse_explicit_target_path_mode(value: Option<&Value>) -> Option<String> {
    let value = value?.as_str()?.trim();
    if value.eq_ignore_ascii_case("entry") {
        Some("entry".to_string())
    } else if value.eq_ignore_ascii_case("prefix") {
        Some("prefix".to_string())
    } else {
        None
    }
}

pub(super) fn parse_explicit_protocol_mode(value: Option<&Value>) -> Option<String> {
    let value = value?.as_str()?.trim();
    if value.eq_ignore_ascii_case("auto") {
        Some("auto".to_string())
    } else if value.eq_ignore_ascii_case("http1") {
        Some("http1".to_string())
    } else if value.eq_ignore_ascii_case("http2") {
        Some("http2".to_string())
    } else {
        None
    }
}

pub(super) fn clean_host_location_path(value: &str) -> String {
    let raw = value.trim();
    if !raw.starts_with('/') {
        return raw.to_string();
    }
    let mut segments = Vec::new();
    for segment in raw.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment == ".." {
            segments.pop();
            continue;
        }
        segments.push(segment);
    }
    format!("/{}", segments.join("/"))
}

pub(super) fn is_valid_http_header_name(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| {
            matches!(
                byte,
                b'!' | b'#'
                    | b'$'
                    | b'%'
                    | b'&'
                    | b'\''
                    | b'*'
                    | b'+'
                    | b'-'
                    | b'.'
                    | b'^'
                    | b'_'
                    | b'`'
                    | b'|'
                    | b'~'
                    | b'0'..=b'9'
                    | b'A'..=b'Z'
                    | b'a'..=b'z'
            )
        })
}

pub(super) fn forbidden_response_header(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "connection"
            | "keep-alive"
            | "proxy-connection"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
            | "content-length"
            | "content-type"
    )
}

pub(super) use crate::json_utils::ensure_object;

pub(super) fn json_integer(value: &Value) -> Option<i64> {
    if let Some(value) = value.as_i64() {
        return Some(value);
    }
    let number = value.as_f64()?;
    if number.is_finite() && number.fract() == 0.0 {
        Some(number as i64)
    } else {
        None
    }
}

pub(super) fn json_number_floor(value: &Value) -> Option<i64> {
    if let Some(value) = value.as_i64() {
        return Some(value);
    }
    let number = value.as_f64()?;
    if number.is_finite() {
        Some(number.floor() as i64)
    } else {
        None
    }
}

pub(super) fn normalize_public_port(value: Option<&Value>) -> i64 {
    json_number_floor_value_or_parse(value)
        .filter(|port| *port > 0 && *port <= 65535)
        .unwrap_or(0)
}

pub(super) fn normalize_cache_ttl(value: Option<&Value>, fallback: i64) -> i64 {
    json_number_floor_value_or_parse(value)
        .filter(|ttl| *ttl >= 0)
        .unwrap_or(fallback)
}

pub(super) fn json_number_floor_value_or_parse(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::String(raw) => raw.trim().parse::<i64>().ok(),
        other => json_number_floor(other),
    }
}

pub(super) use crate::proxy_utils::{
    is_any_subdomain_routing_mode, is_edge_client_ip_active, is_reverse_proxy_subdomain_mode,
};

pub(super) fn is_cloudflared_reverse_proxy_subdomain_mode(config: &Value) -> bool {
    is_reverse_proxy_subdomain_mode(config)
        && config
            .get("default_tunnel")
            .and_then(Value::as_str)
            .unwrap_or("frp")
            == "cloudflared"
}

pub(super) fn should_omit_public_access_entry_port(config: &Value) -> bool {
    is_cloudflared_reverse_proxy_subdomain_mode(config) || is_edge_client_ip_active(config)
}

pub(super) use crate::system_info::resolve_public_gateway_port;

#[cfg(test)]
pub(super) use crate::proxy_utils::parse_env_port_i64_with_fallback_value as parse_env_port_with_fallback_value;

pub(super) use crate::node_compat::parse_i64_prefix as parse_js_parse_int_radix_10;

pub(super) fn parse_explicit_url_port(raw_url: &str, scheme: &str) -> Option<i64> {
    let parsed = Url::parse(raw_url.trim()).ok()?;
    if parsed.scheme() != scheme {
        return None;
    }
    parsed.port().map(i64::from)
}

pub(super) fn resolve_configured_public_port(
    config: &Value,
    scheme: &str,
    allow_reverse_proxy_configured_port: bool,
) -> Option<i64> {
    if is_reverse_proxy_subdomain_mode(config) && !allow_reverse_proxy_configured_port {
        return None;
    }
    let pointer = if scheme == "https" {
        "/subdomain_mode/public_https_port"
    } else {
        "/subdomain_mode/public_http_port"
    };
    config
        .pointer(pointer)
        .and_then(json_number_floor)
        .filter(|port| *port > 0)
}

pub(super) fn resolve_public_port_for_scheme(
    config: &Value,
    scheme: &str,
    raw_public_base_url: &str,
    gateway_fallback: bool,
    allow_reverse_proxy_configured_port: bool,
) -> Option<i64> {
    if should_omit_public_access_entry_port(config) {
        return None;
    }
    if let Some(port) = parse_explicit_url_port(raw_public_base_url, scheme) {
        return Some(port);
    }
    if let Some(port) =
        resolve_configured_public_port(config, scheme, allow_reverse_proxy_configured_port)
    {
        return Some(port);
    }
    if !gateway_fallback {
        return None;
    }
    resolve_public_gateway_port(config)
}

pub(super) fn resolve_auth_public_port_for_scheme(
    config: &Value,
    scheme: &str,
    raw_public_base_url: &str,
    gateway_fallback: bool,
) -> Option<i64> {
    resolve_public_port_for_scheme(config, scheme, raw_public_base_url, gateway_fallback, true)
}

pub(super) fn apply_public_port_to_base_url(raw_base_url: &str, config: &Value) -> String {
    let trimmed = raw_base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return String::new();
    }
    let Ok(mut parsed) = Url::parse(trimmed) else {
        return trimmed.to_string();
    };
    let scheme = match parsed.scheme() {
        "http" => "http",
        "https" => "https",
        _ => return trimmed.to_string(),
    };
    if should_omit_public_access_entry_port(config) {
        // Edge and managed Cloudflare ingress use the scheme's standard
        // browser-facing port. Stale origin ports must never leak into URLs.
        let _ = parsed.set_port(None);
    } else if parsed.port().is_none()
        && let Some(port) = resolve_public_port_for_scheme(config, scheme, trimmed, true, false)
        && !is_default_scheme_port(scheme, port)
    {
        let _ = parsed.set_port(Some(port as u16));
    }
    parsed.set_query(None);
    parsed.set_fragment(None);
    let path = parsed.path().trim_end_matches('/').to_string();
    parsed.set_path(if path.is_empty() { "/" } else { &path });
    parsed.to_string().trim_end_matches('/').to_string()
}

pub(super) fn resolve_public_auth_base_url(config: &Value) -> String {
    let explicit = if is_reverse_proxy_subdomain_mode(config) {
        String::new()
    } else {
        apply_public_port_to_base_url(
            config
                .pointer("/subdomain_mode/public_auth_base_url")
                .and_then(Value::as_str)
                .unwrap_or(""),
            config,
        )
    };
    if !explicit.is_empty() {
        return explicit;
    }
    if let Some(host) = get_auth_host_mapping(config)
        .and_then(|mapping| mapping.get("host"))
        .and_then(Value::as_str)
        .filter(|host| !host.trim().is_empty())
    {
        return format_derived_public_auth_base_url(host, config, "https");
    }
    if let Some(host) = config
        .pointer("/subdomain_mode/auth_host")
        .and_then(Value::as_str)
        .filter(|host| !host.trim().is_empty())
    {
        return format_derived_public_auth_base_url(host, config, "https");
    }
    String::new()
}

pub(super) fn format_derived_public_auth_base_url(
    host: &str,
    config: &Value,
    scheme: &str,
) -> String {
    let normalized_host = normalize_host_value(host);
    if normalized_host.is_empty() {
        return String::new();
    }
    let public_base = config
        .pointer("/subdomain_mode/public_auth_base_url")
        .and_then(Value::as_str)
        .unwrap_or("");
    let Some(port) = resolve_auth_public_port_for_scheme(config, scheme, public_base, true) else {
        return format!("{scheme}://{normalized_host}");
    };
    if is_default_scheme_port(scheme, port) {
        format!("{scheme}://{normalized_host}")
    } else {
        format!("{scheme}://{normalized_host}:{port}")
    }
}

pub(super) fn is_default_scheme_port(scheme: &str, port: i64) -> bool {
    (scheme == "https" && port == 443) || (scheme == "http" && port == 80)
}

pub(super) fn default_subdomain_mode() -> Value {
    json!({
        "root_domain": "",
        "auth_host": "",
        "auth_target": default_subdomain_auth_target(),
        "cookie_domain": "",
        "edge_client_ip_enabled": false,
        "aliyun_esa_enabled": false,
        "tencent_edgeone_enabled": false,
        "public_auth_base_url": "",
        "public_http_port": 0,
        "public_https_port": 0,
        "auth_cache_ttl_seconds": 1,
        "auth_cache_unauthorized_ttl_seconds": 1,
        "default_access_mode": "login_first",
        "auto_add_whitelist_on_login": true,
        "passkey_rp_mode": "auth_host",
        "passkey_rp_id": ""
    })
}
