use super::*;

pub(super) async fn test_public_check_sources_inner(
    sources: &Value,
    _transport: &str,
    translator: &Translator,
) -> anyhow::Result<Vec<Value>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(IP_DETECTION_TIMEOUT_MS))
        .redirect(reqwest::redirect::Policy::limited(20))
        .build()?;
    let mut results = Vec::new();
    for (family, version) in [("ipv4", 4_u8), ("ipv6", 6_u8)] {
        let urls = sources
            .get(family)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for url in urls
            .into_iter()
            .filter_map(|value| value.as_str().map(str::to_string))
        {
            results.push(
                test_single_public_check_source(&client, &url, family, version, translator).await,
            );
        }
    }
    Ok(results)
}

pub(super) async fn test_single_public_check_source(
    client: &reqwest::Client,
    url: &str,
    family: &str,
    version: u8,
    translator: &Translator,
) -> Value {
    match client
        .get(url)
        .header("Accept", "application/json, text/plain")
        .send()
        .await
    {
        Ok(response) => {
            let status = response.status().as_u16();
            let ok = response.status().is_success();
            let text = response.text().await.unwrap_or_default();
            let preview = response_preview(&text);
            if !ok {
                return json!({
                    "family": family,
                    "url": url,
                    "success": false,
                    "status": status,
                    "ip": null,
                    "responsePreview": preview,
                    "error": public_check_request_failed_message(translator, url, status)
                });
            }
            let ip = parse_detected_ip_text(&text, version);
            if let Some(ip) = ip {
                json!({
                    "family": family,
                    "url": url,
                    "success": true,
                    "status": status,
                    "ip": ip,
                    "responsePreview": preview
                })
            } else {
                json!({
                    "family": family,
                    "url": url,
                    "success": false,
                    "status": status,
                    "ip": null,
                    "responsePreview": preview,
                    "error": public_check_invalid_payload_message(translator, url, version)
                })
            }
        }
        Err(error) => json!({
            "family": family,
            "url": url,
            "success": false,
            "status": null,
            "ip": null,
            "error": error.to_string()
        }),
    }
}

pub(super) fn public_check_request_failed_message(
    translator: &Translator,
    url: &str,
    status: u16,
) -> String {
    ddns_text(
        translator,
        "publicCheckSourceRequestFailed",
        &[("url", url.to_string()), ("status", status.to_string())],
    )
}

pub(super) fn public_check_invalid_payload_message(
    translator: &Translator,
    url: &str,
    version: u8,
) -> String {
    ddns_text(
        translator,
        "publicCheckSourceInvalidPayload",
        &[
            ("url", url.to_string()),
            (
                "family",
                if version == 4 { "IPv4" } else { "IPv6" }.to_string(),
            ),
        ],
    )
}

pub(super) fn parse_detected_ip_text(text: &str, version: u8) -> Option<String> {
    parse_detected_ip(text.trim(), version).or_else(|| {
        let value = serde_json::from_str::<Value>(text).ok()?;
        if let Some(ip) = value.get("ip").and_then(Value::as_str) {
            return parse_detected_ip(ip, version);
        }
        if let Some(ip) = value.get("address").and_then(Value::as_str) {
            return parse_detected_ip(ip, version);
        }
        value
            .as_str()
            .and_then(|value| parse_detected_ip(value, version))
    })
}

pub(super) fn parse_detected_ip(value: &str, version: u8) -> Option<String> {
    let ip = value.trim().parse::<IpAddr>().ok()?;
    match (version, ip) {
        (4, IpAddr::V4(_)) | (6, IpAddr::V6(_)) => Some(value.trim().to_string()),
        _ => None,
    }
}

pub(super) fn response_preview(text: &str) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.len() > RESPONSE_PREVIEW_MAX_LENGTH {
        format!("{}...", &normalized[..RESPONSE_PREVIEW_MAX_LENGTH])
    } else {
        normalized
    }
}

pub(super) fn list_ddns_network_interfaces() -> Vec<Value> {
    let mut interfaces = list_docker_host_ipv6_interfaces();
    let mut runtime = HashMap::<String, Vec<Value>>::new();
    if let Ok(addrs) = get_if_addrs() {
        for iface in addrs {
            if iface.is_loopback() {
                continue;
            }
            let address = match iface.addr {
                IfAddr::V4(addr) if is_usable_ipv4(addr.ip) => json!({
                    "family": "ipv4",
                    "address": addr.ip.to_string(),
                    "cidr": format!("{}/{}", addr.ip, ipv4_prefix_len(addr.netmask)),
                    "internal": false,
                    "source": "runtime"
                }),
                IfAddr::V6(addr) if is_usable_ipv6(addr.ip) => json!({
                    "family": "ipv6",
                    "address": addr.ip.to_string(),
                    "cidr": format!("{}/{}", addr.ip, ipv6_prefix_len(addr.netmask)),
                    "internal": false,
                    "source": "runtime"
                }),
                _ => continue,
            };
            runtime.entry(iface.name).or_default().push(address);
        }
    }

    let mut runtime_items = runtime
        .into_iter()
        .filter_map(|(name, addresses)| interface_option(&name, "runtime", addresses))
        .collect::<Vec<_>>();
    runtime_items.sort_by(|left, right| {
        left.get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .cmp(right.get("name").and_then(Value::as_str).unwrap_or(""))
    });
    interfaces.extend(runtime_items);
    interfaces.sort_by(|left, right| {
        let left_source = left
            .get("source")
            .and_then(Value::as_str)
            .unwrap_or("runtime");
        let right_source = right
            .get("source")
            .and_then(Value::as_str)
            .unwrap_or("runtime");
        if left_source != right_source {
            return if left_source == "docker_host" {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            };
        }
        left.get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .cmp(right.get("name").and_then(Value::as_str).unwrap_or(""))
    });
    interfaces
}

pub(super) fn interface_option(name: &str, source: &str, addresses: Vec<Value>) -> Option<Value> {
    if addresses.is_empty() {
        return None;
    }
    let selectable = addresses
        .iter()
        .filter(|item| is_selectable_interface_address(item))
        .cloned()
        .collect::<Vec<_>>();
    let summary = addresses
        .iter()
        .filter_map(|item| {
            let family = item.get("family").and_then(Value::as_str)?;
            let address = item.get("address").and_then(Value::as_str)?;
            Some(format!(
                "{}: {}",
                if family == "ipv4" { "IPv4" } else { "IPv6" },
                address
            ))
        })
        .collect::<Vec<_>>()
        .join(" / ");
    if selectable.is_empty() {
        return None;
    }
    Some(json!({
        "name": name,
        "label": format!("{name} ({summary})"),
        "summary": summary,
        "source": source,
        "hasIpv4": addresses.iter().any(|item| item.get("family").and_then(Value::as_str) == Some("ipv4")),
        "hasIpv6": addresses.iter().any(|item| item.get("family").and_then(Value::as_str) == Some("ipv6")),
        "addresses": addresses,
        "selectableAddresses": selectable
    }))
}

pub(super) fn list_docker_host_ipv6_interfaces() -> Vec<Value> {
    let path = env::var("DDNS_HOST_IF_INET6_PATH")
        .unwrap_or_else(|_| DEFAULT_DOCKER_HOST_IF_INET6_PATH.to_string());
    fs::read_to_string(path)
        .ok()
        .map(|content| parse_host_if_inet6(&content))
        .unwrap_or_default()
}

pub(super) fn parse_host_if_inet6(content: &str) -> Vec<Value> {
    let mut by_interface = HashMap::<String, Vec<Value>>::new();
    for line in content.lines() {
        let parts = line.split_whitespace().collect::<Vec<_>>();
        if parts.len() < 6 {
            continue;
        }
        let Some(address) = format_ipv6_from_proc_hex(parts[0]) else {
            continue;
        };
        let prefix_len = u8::from_str_radix(parts[2], 16).unwrap_or(0);
        let scope = u8::from_str_radix(parts[3], 16).unwrap_or(255);
        if scope != 0 {
            continue;
        }
        let Ok(ip) = address.parse::<Ipv6Addr>() else {
            continue;
        };
        if !is_usable_ipv6(ip) {
            continue;
        }
        let name = parts[5].to_string();
        by_interface.entry(name).or_default().push(json!({
            "family": "ipv6",
            "address": address,
            "cidr": format!("{address}/{prefix_len}"),
            "internal": false,
            "source": "docker_host"
        }));
    }
    let mut items = by_interface
        .into_iter()
        .filter_map(|(name, mut addresses)| {
            addresses.sort_by(|left, right| {
                left.get("address")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .cmp(right.get("address").and_then(Value::as_str).unwrap_or(""))
            });
            interface_option(
                &format!("{DOCKER_HOST_INTERFACE_PREFIX}{name}"),
                "docker_host",
                addresses,
            )
        })
        .collect::<Vec<_>>();
    items.sort_by(|left, right| {
        left.get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .cmp(right.get("name").and_then(Value::as_str).unwrap_or(""))
    });
    items
}

pub(super) fn format_ipv6_from_proc_hex(value: &str) -> Option<String> {
    if value.len() != 32 || !value.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return None;
    }
    let mut segments = Vec::new();
    for chunk in value.as_bytes().chunks(4) {
        let raw = std::str::from_utf8(chunk).ok()?;
        segments.push(u16::from_str_radix(raw, 16).ok()?);
    }
    Some(
        Ipv6Addr::new(
            segments[0],
            segments[1],
            segments[2],
            segments[3],
            segments[4],
            segments[5],
            segments[6],
            segments[7],
        )
        .to_string(),
    )
}

pub(super) fn is_selectable_interface_address(value: &Value) -> bool {
    let Some(address) = value.get("address").and_then(Value::as_str) else {
        return false;
    };
    match value.get("family").and_then(Value::as_str) {
        Some("ipv4") => address
            .parse::<Ipv4Addr>()
            .is_ok_and(|ip| !is_private_ipv4(ip)),
        Some("ipv6") => address
            .parse::<Ipv6Addr>()
            .is_ok_and(|ip| !is_unique_local_ipv6(ip)),
        _ => false,
    }
}

pub(super) fn is_usable_ipv4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    octets[0] != 127 && !(octets[0] == 169 && octets[1] == 254)
}

pub(super) fn is_usable_ipv6(ip: Ipv6Addr) -> bool {
    !(ip.is_loopback() || ip.is_unicast_link_local() || ip.is_unspecified())
}

pub(super) fn is_private_ipv4(ip: Ipv4Addr) -> bool {
    ip.is_private()
}

pub(super) fn is_unique_local_ipv6(ip: Ipv6Addr) -> bool {
    let first = ip.octets()[0];
    first == 0xfc || first == 0xfd
}

pub(super) fn ipv4_prefix_len(mask: Ipv4Addr) -> u32 {
    mask.octets().iter().map(|byte| byte.count_ones()).sum()
}

pub(super) fn ipv6_prefix_len(mask: Ipv6Addr) -> u32 {
    mask.octets().iter().map(|byte| byte.count_ones()).sum()
}
