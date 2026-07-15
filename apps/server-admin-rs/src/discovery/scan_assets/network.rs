use super::*;
use crate::net_utils::ipv4_prefix_len;

pub(super) fn normalize_allowed_scan_cidrs(
    values: impl IntoIterator<Item = String>,
) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut output = Vec::new();
    for value in values {
        let Some(parsed) = parse_allowed_scan_cidr(&value) else {
            continue;
        };
        if seen.insert(parsed.cidr.clone()) {
            output.push(parsed.cidr);
        }
    }
    output
}

pub(super) fn validate_scan_cidrs(values: &[String]) -> Result<Vec<String>, String> {
    let mut output = Vec::new();
    let mut seen = BTreeSet::new();
    let mut invalid = Vec::new();
    for value in values {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Some(parsed) = parse_allowed_scan_cidr(trimmed) else {
            invalid.push(trimmed.to_string());
            continue;
        };
        if seen.insert(parsed.cidr.clone()) {
            output.push(parsed.cidr);
        }
    }
    if !invalid.is_empty() {
        return Err(format!(
            "Only local IPv4 CIDR ranges are supported: {}",
            invalid.into_iter().take(3).collect::<Vec<_>>().join(", ")
        ));
    }
    if output.len() > MAX_SCAN_CIDRS {
        return Err(format!(
            "At most {MAX_SCAN_CIDRS} CIDR ranges can be selected"
        ));
    }
    count_scan_hosts(&output)?;
    Ok(output)
}

pub(super) fn count_scan_hosts(cidrs: &[String]) -> Result<usize, String> {
    let mut seen = BTreeSet::new();
    for cidr in cidrs {
        let Some(parsed) = parse_allowed_scan_cidr(cidr) else {
            continue;
        };
        for value in parsed.first_host..=parsed.last_host {
            if seen.insert(value) && seen.len() > MAX_SCAN_HOSTS as usize {
                return Err(format!("At most {MAX_SCAN_HOSTS} hosts can be scanned"));
            }
        }
    }
    Ok(seen.len())
}

pub(super) fn parse_allowed_scan_cidr(value: &str) -> Option<ParsedIpv4Cidr> {
    let parsed = parse_ipv4_cidr(value)?;
    (parsed.host_count > 0 && allowed_scan_range(parsed.first_host, parsed.last_host))
        .then_some(parsed)
}

pub(super) fn parse_ipv4_cidr(value: &str) -> Option<ParsedIpv4Cidr> {
    let (address, prefix) = value.trim().split_once('/')?;
    let ip = address.trim().parse::<Ipv4Addr>().ok()?;
    let prefix = prefix.trim().parse::<u8>().ok()?;
    if prefix > 32 {
        return None;
    }
    let address_number = u32::from(ip);
    let mask = if prefix == 0 {
        0
    } else {
        u32::MAX.checked_shl((32 - prefix) as u32).unwrap_or(0)
    };
    let network = address_number & mask;
    let host_size = 1_u64.checked_shl((32 - prefix) as u32)?;
    let broadcast = network as u64 + host_size - 1;
    if broadcast > u32::MAX as u64 {
        return None;
    }
    let first_host = if prefix >= 31 { network } else { network + 1 };
    let last_host = if prefix >= 31 {
        broadcast as u32
    } else {
        broadcast as u32 - 1
    };
    let host_count = if prefix >= 31 {
        host_size
    } else {
        host_size.saturating_sub(2)
    };
    Some(ParsedIpv4Cidr {
        cidr: format!("{}/{}", Ipv4Addr::from(network), prefix),
        first_host,
        last_host,
        host_count,
    })
}

pub(super) fn build_ipv4_cidr(value: &str, prefix: u8) -> Option<String> {
    (prefix <= 32).then_some(())?;
    let ip = value.trim().parse::<Ipv4Addr>().ok()?;
    parse_ipv4_cidr(&format!("{ip}/{prefix}")).map(|parsed| parsed.cidr)
}

pub(super) fn build_interface_ipv4_cidr(value: &str, prefix: Option<u8>) -> Option<String> {
    let mut candidates = Vec::new();
    if let Some(prefix) = prefix {
        candidates.push(prefix);
    }
    if !candidates.contains(&24) {
        candidates.push(24);
    }
    for candidate in candidates {
        let Some(cidr) = build_ipv4_cidr(value, candidate) else {
            continue;
        };
        if parse_allowed_scan_cidr(&cidr).is_some_and(|parsed| parsed.host_count <= MAX_SCAN_HOSTS)
        {
            return Some(cidr);
        }
    }
    None
}

pub(super) fn is_allowed_scan_ipv4(value: &str) -> bool {
    let Ok(ip) = value.parse::<Ipv4Addr>() else {
        return false;
    };
    let number = u32::from(ip);
    allowed_ranges()
        .iter()
        .any(|(start, end)| number >= *start && number <= *end)
}

pub(super) fn allowed_scan_range(first: u32, last: u32) -> bool {
    allowed_ranges()
        .iter()
        .any(|(start, end)| first >= *start && last <= *end)
}

pub(super) fn allowed_ranges() -> Vec<(u32, u32)> {
    [
        ("127.0.0.0", "127.255.255.255"),
        ("10.0.0.0", "10.255.255.255"),
        ("172.16.0.0", "172.31.255.255"),
        ("192.168.0.0", "192.168.255.255"),
        ("100.64.0.0", "100.127.255.255"),
        ("169.254.0.0", "169.254.255.255"),
    ]
    .into_iter()
    .filter_map(|(start, end)| {
        Some((
            u32::from(start.parse::<Ipv4Addr>().ok()?),
            u32::from(end.parse::<Ipv4Addr>().ok()?),
        ))
    })
    .collect()
}

pub(super) struct Ipv4Candidate {
    pub(super) name: String,
    pub(super) address: String,
    pub(super) prefix: Option<u8>,
}

pub(super) fn list_private_ipv4_candidates() -> Vec<Ipv4Candidate> {
    let mut seen = BTreeSet::new();
    let mut output = Vec::new();
    let Ok(addrs) = get_if_addrs() else {
        return output;
    };
    for iface in addrs {
        if is_excluded_interface(&iface.name) || iface.is_loopback() {
            continue;
        }
        let IfAddr::V4(addr) = iface.addr else {
            continue;
        };
        let address = addr.ip.to_string();
        if !is_private_ipv4(addr.ip) || !seen.insert(address.clone()) {
            continue;
        }
        output.push(Ipv4Candidate {
            name: iface.name,
            address,
            prefix: Some(ipv4_prefix_len(addr.netmask) as u8),
        });
    }
    output.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.address.cmp(&right.address))
    });
    output
}

pub(super) fn is_excluded_interface(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower == "lo"
        || lower.starts_with("docker")
        || lower.starts_with("br-")
        || lower.starts_with("veth")
        || lower.starts_with("tailscale")
        || lower.starts_with("zt")
        || lower.starts_with("tun")
        || lower.starts_with("tap")
        || lower.starts_with("wg")
}

pub(super) fn is_private_ipv4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    octets[0] == 10
        || (octets[0] == 172 && (16..=31).contains(&octets[1]))
        || (octets[0] == 192 && octets[1] == 168)
}

pub(super) fn extract_ipv4_from_target(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let host = Url::parse(trimmed)
        .ok()
        .and_then(|url| url.host_str().map(str::to_string))
        .or_else(|| {
            Url::parse(&format!("http://{trimmed}"))
                .ok()
                .and_then(|url| url.host_str().map(str::to_string))
        })?;
    let ip = host.parse::<Ipv4Addr>().ok()?;
    is_allowed_scan_ipv4(&ip.to_string()).then(|| ip.to_string())
}

pub(super) fn resolve_docker_discover_host(headers: &HeaderMap) -> Option<String> {
    headers
        .get(DOCKER_DISCOVER_IP_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| is_usable_private_discover_ipv4(value))
        .map(str::to_string)
        .or_else(|| {
            env::var("DOCKER_DISCOVER_LAN_IP")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| is_usable_private_discover_ipv4(value))
        })
        .or_else(|| {
            for header in ["x-forwarded-host", "host"] {
                let Some(host) = headers.get(header).and_then(|value| value.to_str().ok()) else {
                    continue;
                };
                for candidate in host.split(',').map(normalize_host_like) {
                    if is_usable_private_discover_ipv4(&candidate) {
                        return Some(candidate);
                    }
                    if let Some(resolved) = resolve_private_ipv4_host(&candidate) {
                        return Some(resolved);
                    }
                }
            }
            None
        })
}

pub(super) fn normalize_host_like(value: &str) -> String {
    Url::parse(&format!("http://{}", value.trim()))
        .ok()
        .and_then(|url| {
            url.host_str()
                .map(|host| host.trim_matches(['[', ']']).to_lowercase())
        })
        .unwrap_or_else(|| value.trim().trim_matches(['[', ']']).to_lowercase())
}

pub(super) fn resolve_private_ipv4_host(host: &str) -> Option<String> {
    if host.is_empty() || host == "localhost" || host.parse::<Ipv4Addr>().is_ok() {
        return None;
    }
    (host, 0)
        .to_socket_addrs()
        .ok()?
        .filter_map(|addr| match addr.ip() {
            std::net::IpAddr::V4(ip) if is_usable_private_discover_ipv4(&ip.to_string()) => {
                Some(ip.to_string())
            }
            _ => None,
        })
        .next()
}

pub(super) fn is_usable_private_discover_ipv4(value: &str) -> bool {
    value.parse::<Ipv4Addr>().is_ok() && is_allowed_scan_ipv4(value) && !value.starts_with("127.")
}

pub(super) use crate::runtime_profile::deployment_target;

pub(super) use crate::proxy_utils::parse_env_port_u16_with_fallback as resolve_env_port_with_fallback;
#[cfg(test)]
pub(super) use crate::proxy_utils::parse_env_port_u16_with_fallback_value as resolve_env_port_with_fallback_value;

pub(super) fn excluded_env_port(name: &str, fallback: u16) -> Option<u16> {
    excluded_env_port_value(env::var(name).ok(), fallback)
}

pub(super) fn excluded_env_port_value(value: Option<String>, fallback: u16) -> Option<u16> {
    let raw = value
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| fallback.to_string());
    parse_js_parse_int_radix_10(raw.trim_start())
        .filter(|port| *port > 0 && *port <= u16::MAX as i64)
        .map(|port| port as u16)
}

pub(super) use crate::node_compat::parse_i64_prefix_trim_start as parse_js_parse_int_radix_10;
