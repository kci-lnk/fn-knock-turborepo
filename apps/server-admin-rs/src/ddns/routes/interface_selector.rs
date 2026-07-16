use super::*;
use ipnet::IpNet;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

const INTERFACE_SELECTOR_VERSION: u8 = 1;

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum InterfaceSelectorMode {
    #[default]
    Auto,
    Rules,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct InterfaceAddressSelector {
    pub version: u8,
    #[serde(default)]
    pub mode: InterfaceSelectorMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_address: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub include_cidrs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude_cidrs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ipv6_interface_id: Option<String>,
    #[serde(default)]
    pub allow_temporary: bool,
}

impl Default for InterfaceAddressSelector {
    fn default() -> Self {
        Self {
            version: INTERFACE_SELECTOR_VERSION,
            mode: InterfaceSelectorMode::Auto,
            preferred_address: None,
            include_cidrs: Vec::new(),
            exclude_cidrs: Vec::new(),
            ipv6_interface_id: None,
            allow_temporary: false,
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct InterfaceSelection {
    pub selected: Option<String>,
    pub eligible: Vec<Value>,
    pub rejected: Vec<Value>,
    pub reason: &'static str,
}

pub(super) fn selector_field(family: &str) -> &'static str {
    if family == "ipv4" {
        DDNS_INTERFACE_IPV4_SELECTOR_FIELD
    } else {
        DDNS_INTERFACE_IPV6_SELECTOR_FIELD
    }
}

pub(super) fn parse_interface_selector(
    value: Option<&str>,
    family: &str,
) -> anyhow::Result<Option<InterfaceAddressSelector>> {
    let Some(raw) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let selector = serde_json::from_str::<InterfaceAddressSelector>(raw)
        .map_err(|error| anyhow::anyhow!("Invalid DDNS interface selector: {error}"))?;
    normalize_interface_selector(selector, family).map(Some)
}

pub(super) fn parse_interface_selector_value(
    value: &Value,
    family: &str,
) -> anyhow::Result<InterfaceAddressSelector> {
    let selector = serde_json::from_value::<InterfaceAddressSelector>(value.clone())
        .map_err(|error| anyhow::anyhow!("Invalid DDNS interface selector: {error}"))?;
    normalize_interface_selector(selector, family)
}

pub(super) fn normalize_interface_selector_string(value: Option<&str>, family: &str) -> String {
    parse_interface_selector(value, family)
        .ok()
        .flatten()
        .and_then(|selector| serde_json::to_string(&selector).ok())
        .unwrap_or_default()
}

pub(super) fn normalize_interface_selector(
    mut selector: InterfaceAddressSelector,
    family: &str,
) -> anyhow::Result<InterfaceAddressSelector> {
    if selector.version != INTERFACE_SELECTOR_VERSION {
        anyhow::bail!(
            "Invalid DDNS interface selector: unsupported version {}",
            selector.version
        );
    }
    if !matches!(family, "ipv4" | "ipv6") {
        anyhow::bail!("Invalid DDNS interface selector: invalid address family");
    }

    selector.preferred_address = selector
        .preferred_address
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| normalize_family_ip(value, family))
        .transpose()?;
    selector.include_cidrs = normalize_cidrs(&selector.include_cidrs, family)?;
    selector.exclude_cidrs = normalize_cidrs(&selector.exclude_cidrs, family)?;
    selector.ipv6_interface_id = match selector.ipv6_interface_id.as_deref() {
        Some(value) if family == "ipv4" && !value.trim().is_empty() => {
            anyhow::bail!(
                "Invalid DDNS interface selector: IPv6 interface ID is not valid for IPv4"
            );
        }
        Some(value) if !value.trim().is_empty() => Some(normalize_ipv6_interface_id(value)?),
        _ => None,
    };
    Ok(selector)
}

fn normalize_family_ip(value: &str, family: &str) -> anyhow::Result<String> {
    let ip = value.parse::<IpAddr>().map_err(|_| {
        anyhow::anyhow!("Invalid DDNS interface selector: invalid preferred address {value}")
    })?;
    if (family == "ipv4" && !ip.is_ipv4()) || (family == "ipv6" && !ip.is_ipv6()) {
        anyhow::bail!("Invalid DDNS interface selector: preferred address family mismatch");
    }
    Ok(ip.to_string())
}

fn normalize_cidrs(values: &[String], family: &str) -> anyhow::Result<Vec<String>> {
    let mut output = Vec::new();
    for value in values {
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        let network = value.parse::<IpNet>().map_err(|_| {
            anyhow::anyhow!("Invalid DDNS interface selector: invalid CIDR {value}")
        })?;
        if (family == "ipv4" && !network.addr().is_ipv4())
            || (family == "ipv6" && !network.addr().is_ipv6())
        {
            anyhow::bail!("Invalid DDNS interface selector: CIDR family mismatch for {value}");
        }
        let canonical = network.trunc().to_string();
        if !output.contains(&canonical) {
            output.push(canonical);
        }
    }
    Ok(output)
}

pub(super) fn normalize_ipv6_interface_id(value: &str) -> anyhow::Result<String> {
    let value = value.trim();
    let parsed = if let Ok(ip) = value.parse::<Ipv6Addr>() {
        ip
    } else {
        format!("::{value}").parse::<Ipv6Addr>().map_err(|_| {
            anyhow::anyhow!("Invalid DDNS interface selector: invalid IPv6 interface ID {value}")
        })?
    };
    Ok(ipv6_interface_id(parsed))
}

pub(super) fn ipv6_interface_id(ip: Ipv6Addr) -> String {
    let segments = ip.segments();
    format!(
        "{:04x}:{:04x}:{:04x}:{:04x}",
        segments[4], segments[5], segments[6], segments[7]
    )
}

pub(super) fn resolve_interface_selector(
    network: &Value,
    family: &str,
    selector: &InterfaceAddressSelector,
    current_address: Option<&str>,
) -> InterfaceSelection {
    let includes = parse_normalized_cidrs(&selector.include_cidrs);
    let excludes = parse_normalized_cidrs(&selector.exclude_cidrs);
    let mut eligible = Vec::new();
    let mut rejected = Vec::new();

    for item in network
        .get("selectableAddresses")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
    {
        if item.get("family").and_then(Value::as_str) != Some(family) {
            continue;
        }
        let mut reasons = Vec::new();
        let parsed = item
            .get("address")
            .and_then(Value::as_str)
            .and_then(|value| value.parse::<IpAddr>().ok());
        if parsed.is_none() {
            reasons.push("invalid_address");
        }
        if item.get("tentative").and_then(Value::as_bool) == Some(true) {
            reasons.push("tentative");
        }
        if item.get("dadFailed").and_then(Value::as_bool) == Some(true) {
            reasons.push("dad_failed");
        }
        if item.get("deprecated").and_then(Value::as_bool) == Some(true) {
            reasons.push("deprecated");
        }
        if !selector.allow_temporary && item.get("temporary").and_then(Value::as_bool) == Some(true)
        {
            reasons.push("temporary");
        }

        if selector.mode == InterfaceSelectorMode::Rules
            && let Some(ip) = parsed
        {
            if !includes.is_empty() && !includes.iter().any(|network| network.contains(&ip)) {
                reasons.push("outside_include_cidrs");
            }
            if excludes.iter().any(|network| network.contains(&ip)) {
                reasons.push("excluded_cidr");
            }
            if family == "ipv6"
                && let Some(expected) = selector.ipv6_interface_id.as_deref()
                && ip
                    .to_string()
                    .parse::<Ipv6Addr>()
                    .is_ok_and(|address| ipv6_interface_id(address) != expected)
            {
                reasons.push("interface_id_mismatch");
            }
        }

        if reasons.is_empty() {
            eligible.push(item);
        } else {
            rejected.push(json!({
                "address": item.get("address").cloned().unwrap_or(Value::Null),
                "reasons": reasons
            }));
        }
    }

    eligible.sort_by(compare_interface_candidates);
    let current = canonical_candidate_address(current_address, family);
    let preferred = selector.preferred_address.as_deref();
    let (selected, reason) = if let Some(value) = find_candidate(&eligible, current.as_deref()) {
        (Some(value), "current")
    } else if let Some(value) = find_candidate(&eligible, preferred) {
        (Some(value), "preferred")
    } else if let Some(value) = eligible
        .first()
        .and_then(candidate_address)
        .map(str::to_string)
    {
        (Some(value), "ranked")
    } else {
        (None, "no_match")
    };

    InterfaceSelection {
        selected,
        eligible,
        rejected,
        reason,
    }
}

fn parse_normalized_cidrs(values: &[String]) -> Vec<IpNet> {
    values
        .iter()
        .filter_map(|value| value.parse().ok())
        .collect()
}

fn canonical_candidate_address(value: Option<&str>, family: &str) -> Option<String> {
    let ip = value?.trim().parse::<IpAddr>().ok()?;
    ((family == "ipv4" && ip.is_ipv4()) || (family == "ipv6" && ip.is_ipv6()))
        .then(|| ip.to_string())
}

fn find_candidate(candidates: &[Value], expected: Option<&str>) -> Option<String> {
    let expected = expected?;
    candidates
        .iter()
        .filter_map(candidate_address)
        .find(|value| *value == expected)
        .map(str::to_string)
}

fn candidate_address(value: &Value) -> Option<&str> {
    value.get("address").and_then(Value::as_str)
}

fn compare_interface_candidates(left: &Value, right: &Value) -> Ordering {
    candidate_stability_rank(left)
        .cmp(&candidate_stability_rank(right))
        .then_with(|| {
            let left = candidate_address(left).and_then(|value| value.parse::<IpAddr>().ok());
            let right = candidate_address(right).and_then(|value| value.parse::<IpAddr>().ok());
            left.cmp(&right)
        })
}

fn candidate_stability_rank(value: &Value) -> u8 {
    match value.get("temporary").and_then(Value::as_bool) {
        Some(false) => 0,
        None => 1,
        Some(true) => 2,
    }
}

pub(super) fn legacy_select_interface_address(
    candidates: &[Value],
    family: &str,
    index: Option<&str>,
    current_address: Option<&str>,
) -> Option<(String, &'static str)> {
    let family_candidates = candidates
        .iter()
        .filter(|item| item.get("family").and_then(Value::as_str) == Some(family))
        .collect::<Vec<_>>();
    let usable = family_candidates
        .iter()
        .copied()
        .filter(|item| legacy_candidate_is_usable(item))
        .cloned()
        .collect::<Vec<_>>();
    let current = canonical_candidate_address(current_address, family);
    if let Some(value) = find_candidate(&usable, current.as_deref()) {
        return Some((value, "legacy_current"));
    }
    if family == "ipv6"
        && let Some(current) = current
        && let Ok(current) = current.parse::<Ipv6Addr>()
    {
        let expected = ipv6_interface_id(current);
        let mut matches = usable
            .iter()
            .filter_map(candidate_address)
            .filter_map(|value| value.parse::<Ipv6Addr>().ok())
            .filter(|value| ipv6_interface_id(*value) == expected)
            .collect::<Vec<_>>();
        matches.sort();
        if let Some(value) = matches.first() {
            return Some((value.to_string(), "legacy_interface_id"));
        }
    }
    let index = index?.trim().parse::<usize>().ok()?;
    family_candidates
        .get(index)
        .copied()
        .filter(|item| legacy_candidate_is_usable(item))
        .and_then(candidate_address)
        .map(|value| (value.to_string(), "legacy_index"))
}

fn legacy_candidate_is_usable(item: &Value) -> bool {
    item.get("tentative").and_then(Value::as_bool) != Some(true)
        && item.get("dadFailed").and_then(Value::as_bool) != Some(true)
        && item.get("deprecated").and_then(Value::as_bool) != Some(true)
}
