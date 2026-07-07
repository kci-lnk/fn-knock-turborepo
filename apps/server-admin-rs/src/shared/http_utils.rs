use std::net::IpAddr;

use axum::http::{HeaderMap, Uri};
use url::Url;

pub fn get_client_ip(headers: &HeaderMap) -> String {
    for name in [
        "x-forwarded-for",
        "x-real-ip",
        "eo-connecting-ip",
        "ali-real-client-ip",
    ] {
        if let Some(value) = headers.get(name).and_then(|value| value.to_str().ok()) {
            let first = value.split(',').next().unwrap_or("").trim();
            let normalized = normalize_ip(first);
            if !normalized.is_empty() {
                return normalized;
            }
        }
    }
    String::new()
}

pub fn first_header_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

pub fn forwarded_header_value(headers: &HeaderMap, key: &str) -> Option<String> {
    let value = headers.get("forwarded")?.to_str().ok()?;
    let first = value.split(',').next()?.trim();
    for segment in first.split(';') {
        let Some((raw_key, raw_value)) = segment.split_once('=') else {
            continue;
        };
        if raw_key.trim().eq_ignore_ascii_case(key) {
            let value = raw_value.trim().trim_matches('"');
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

pub fn normalize_ip(value: &str) -> String {
    let mut candidate = value.trim().to_string();
    if candidate.is_empty() {
        return String::new();
    }
    if let Some(bracketed) = strip_bracketed_host(&candidate) {
        candidate = bracketed.to_string();
    } else if let Some(ipv4) = strip_ipv4_port(&candidate) {
        candidate = ipv4.to_string();
    }
    if let Some((ip, _zone)) = candidate.split_once('%') {
        if !ip.is_empty() {
            candidate = ip.to_string();
        }
    }
    if let Some(mapped) = candidate.strip_prefix("::ffff:")
        && is_valid_ipv4(mapped)
    {
        candidate = mapped.to_string();
    }
    if candidate == "::1" {
        candidate = "127.0.0.1".to_string();
    }
    if is_valid_ip(&candidate) {
        candidate
    } else {
        String::new()
    }
}

pub fn normalize_api_base_url(value: &str, default_path: &str) -> Result<String, String> {
    let normalized = value.trim().trim_end_matches('/');
    let mut url = Url::parse(normalized).map_err(|error| error.to_string())?;
    let path = url.path().trim_end_matches('/').to_string();
    if path.is_empty() {
        url.set_path(default_path);
    } else {
        url.set_path(&path);
    }
    url.set_query(None);
    url.set_fragment(None);
    Ok(url.as_str().trim_end_matches('/').to_string())
}

fn strip_bracketed_host(candidate: &str) -> Option<&str> {
    if !candidate.starts_with('[') {
        return None;
    }
    let end = candidate.rfind(']')?;
    if end == 0 {
        return None;
    }
    let suffix = &candidate[end + 1..];
    let suffix_valid = suffix.is_empty()
        || suffix
            .strip_prefix(':')
            .is_some_and(|port| !port.is_empty() && port.chars().all(|ch| ch.is_ascii_digit()));
    suffix_valid.then_some(&candidate[1..end])
}

fn strip_ipv4_port(candidate: &str) -> Option<&str> {
    let (ip, port) = candidate.rsplit_once(':')?;
    if port.is_empty() || !port.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    let parts = ip.split('.').collect::<Vec<_>>();
    if parts.len() == 4
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit()))
    {
        Some(ip)
    } else {
        None
    }
}

fn is_valid_ip(value: &str) -> bool {
    value.parse::<IpAddr>().is_ok()
}

fn is_valid_ipv4(value: &str) -> bool {
    matches!(value.parse::<IpAddr>(), Ok(IpAddr::V4(_)))
}

pub fn is_private_or_local_ip(value: &str) -> bool {
    match normalize_ip(value).parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => {
            let octets = ip.octets();
            matches!(octets[0], 0 | 10 | 127)
                || (octets[0] == 169 && octets[1] == 254)
                || (octets[0] == 172 && (16..=31).contains(&octets[1]))
                || (octets[0] == 192 && octets[1] == 168)
                || (octets[0] == 100 && (64..=127).contains(&octets[1]))
        }
        Ok(IpAddr::V6(ip)) => {
            ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_unique_local()
                || ip.is_unicast_link_local()
        }
        Err(_) => false,
    }
}

pub fn is_secure_request(headers: &HeaderMap, uri: &Uri) -> bool {
    if let Some(proto) = headers
        .get("x-forwarded-proto")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(|value| value.trim().to_ascii_lowercase())
    {
        if proto == "https" {
            return true;
        }
        if proto == "http" {
            return false;
        }
    }
    uri.scheme_str() == Some("https")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_ipv4_mapped_ipv6_and_ports() {
        assert_eq!(normalize_ip("::ffff:192.168.1.2"), "192.168.1.2");
        assert_eq!(normalize_ip("10.0.0.1:443"), "10.0.0.1");
        assert_eq!(normalize_ip("[::1]:443"), "127.0.0.1");
    }

    #[test]
    fn normalizes_ip_like_node_without_canonicalizing_valid_literals() {
        assert_eq!(normalize_ip("2001:0DB8::1"), "2001:0DB8::1");
        assert_eq!(normalize_ip("[2001:0DB8::1]"), "2001:0DB8::1");
        assert_eq!(normalize_ip("[::1]:abc"), "");
        assert_eq!(normalize_ip("10.0.0.1:abc"), "");
        assert_eq!(normalize_ip("%eth0"), "");
        assert_eq!(normalize_ip("fe80::1%eth0"), "fe80::1");
    }

    #[test]
    fn detects_private_network_ranges() {
        assert!(is_private_or_local_ip("127.0.0.1"));
        assert!(is_private_or_local_ip("192.168.31.2"));
        assert!(is_private_or_local_ip("172.16.0.1"));
        assert!(!is_private_or_local_ip("8.8.8.8"));
    }
}
