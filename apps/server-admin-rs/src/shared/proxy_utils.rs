use serde_json::Value;
use url::Url;

const DEFAULT_AUTH_SERVICE_PORT: u16 = 7997;

pub(crate) fn is_reverse_proxy_subdomain_mode(config: &Value) -> bool {
    config.get("run_type").and_then(Value::as_i64) == Some(1)
        && config
            .get("reverse_proxy_submode")
            .and_then(Value::as_str)
            .unwrap_or("path")
            == "subdomain"
}

pub(crate) fn is_any_subdomain_routing_mode(config: &Value) -> bool {
    config.get("run_type").and_then(Value::as_i64) == Some(3)
        || is_reverse_proxy_subdomain_mode(config)
}

pub(crate) fn parse_target_port_i64(target: &str) -> Option<i64> {
    let normalized = target.trim();
    if normalized.is_empty() {
        return None;
    }
    if let Ok(parsed) = Url::parse(normalized) {
        return parsed
            .port()
            .map(i64::from)
            .or_else(|| default_port_for_scheme(parsed.scheme()).map(i64::from));
    }
    let (_, tail) = normalized.rsplit_once(':')?;
    let digits = tail
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return None;
    }
    digits
        .parse::<i64>()
        .ok()
        .filter(|port| *port > 0 && *port <= u16::MAX as i64)
}

pub(crate) fn parse_target_port_u16(target: &str) -> Option<u16> {
    parse_target_port_i64(target).and_then(|port| u16::try_from(port).ok())
}

pub(crate) fn parse_url_target_port_u16(target: &str) -> Option<u16> {
    let parsed = Url::parse(target.trim()).ok()?;
    parsed
        .port()
        .or_else(|| default_port_for_scheme(parsed.scheme()))
}

pub(crate) fn parse_env_port_i64_with_fallback_value(value: Option<String>, fallback: i64) -> i64 {
    let raw = value
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| fallback.to_string());
    crate::node_compat::parse_i64_prefix_trim_start(&raw)
        .filter(|port| *port > 0)
        .unwrap_or(fallback)
}

pub(crate) fn parse_env_port_u16_with_fallback(name: &str, fallback: u16) -> u16 {
    parse_env_port_u16_with_fallback_value(std::env::var(name).ok(), fallback)
}

pub(crate) fn parse_env_port_u16_with_fallback_value(value: Option<String>, fallback: u16) -> u16 {
    let parsed = parse_env_port_i64_with_fallback_value(value, i64::from(fallback));
    u16::try_from(parsed)
        .ok()
        .filter(|port| *port > 0)
        .unwrap_or(fallback)
}

pub(crate) fn auth_service_port() -> u16 {
    parse_env_port_u16_with_fallback("AUTH_PORT", DEFAULT_AUTH_SERVICE_PORT)
}

pub(crate) fn default_auth_service_target() -> String {
    format!("http://127.0.0.1:{}", auth_service_port())
}

fn default_port_for_scheme(scheme: &str) -> Option<u16> {
    match scheme {
        "https" | "wss" => Some(443),
        "http" | "ws" => Some(80),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn detects_subdomain_routing_modes() {
        assert!(is_any_subdomain_routing_mode(&json!({ "run_type": 3 })));
        assert!(is_any_subdomain_routing_mode(&json!({
            "run_type": 1,
            "reverse_proxy_submode": "subdomain"
        })));
        assert!(!is_any_subdomain_routing_mode(&json!({
            "run_type": 1,
            "reverse_proxy_submode": "path"
        })));
    }

    #[test]
    fn parses_target_ports_with_existing_lenient_rules() {
        assert_eq!(parse_target_port_i64("http://example.com"), Some(80));
        assert_eq!(parse_target_port_i64("https://example.com"), Some(443));
        assert_eq!(parse_target_port_i64("127.0.0.1:7997/path"), Some(7997));
        assert_eq!(parse_target_port_i64("example.com:0"), None);
        assert_eq!(parse_url_target_port_u16("example.com:7997/path"), None);
    }

    #[test]
    fn env_port_parsers_preserve_node_parse_int_edges() {
        assert_eq!(parse_env_port_i64_with_fallback_value(None, 7997), 7997);
        assert_eq!(
            parse_env_port_i64_with_fallback_value(Some(String::new()), 7997),
            7997
        );
        assert_eq!(
            parse_env_port_i64_with_fallback_value(Some(" 7997x ".to_string()), 7997),
            7997
        );
        assert_eq!(
            parse_env_port_i64_with_fallback_value(Some("8000x".to_string()), 7997),
            8000
        );
        assert_eq!(
            parse_env_port_i64_with_fallback_value(Some("0x10".to_string()), 7997),
            7997
        );
        assert_eq!(
            parse_env_port_i64_with_fallback_value(Some("abc".to_string()), 7997),
            7997
        );
        assert_eq!(
            parse_env_port_u16_with_fallback_value(Some("65536".to_string()), 7997),
            7997
        );
    }

    #[test]
    fn default_auth_target_follows_the_runtime_auth_port() {
        let environment = crate::test_support::EnvGuard::new(&["AUTH_PORT"]);
        environment.set("AUTH_PORT", "8997");

        assert_eq!(auth_service_port(), 8997);
        assert_eq!(default_auth_service_target(), "http://127.0.0.1:8997");
    }
}
