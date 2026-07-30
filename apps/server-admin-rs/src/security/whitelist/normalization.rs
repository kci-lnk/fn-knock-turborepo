use std::{collections::BTreeSet, str::FromStr};

use ipnet::IpNet;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{
    cidr::{CidrOperator, CidrRegionQuery, CompiledIpSet},
    http_utils::normalize_ip,
    state::AppState,
    store::{WhitelistConcreteTarget, WhitelistRegionInput},
};

const DEFAULT_CNAME_CHECK_INTERVAL_MINUTES: i64 = 5;
const MIN_CNAME_CHECK_INTERVAL_MINUTES: i64 = 1;
const MAX_CNAME_CHECK_INTERVAL_MINUTES: i64 = 24 * 60;

pub(crate) fn whitelist_auto_owner_record_key(owner_key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(owner_key.trim());
    format!(
        "fn_knock:whitelist:auto_owner:{}",
        hex::encode(hasher.finalize())
    )
}

pub(super) fn normalize_source(value: Option<&str>) -> String {
    if value == Some("auto") {
        "auto".to_string()
    } else {
        "manual".to_string()
    }
}

#[derive(Debug)]
pub(super) enum WhitelistRegionResolveError {
    Empty,
    Lookup(String),
}

pub(super) fn normalize_whitelist_region_inputs(
    value: &[Value],
) -> Result<Vec<WhitelistRegionInput>, String> {
    let mut result = Vec::new();
    let mut seen = BTreeSet::new();
    for item in value {
        let Some(object) = item.as_object() else {
            continue;
        };
        let province = js_region_string(object.get("province")).trim().to_string();
        if province.is_empty() {
            continue;
        }
        let query_city = js_region_string(object.get("query_city"))
            .trim()
            .to_string();
        let query_city = (!query_city.is_empty()).then_some(query_city);
        let operator = CidrOperator::parse_value(object.get("operator"))?;
        let key = CidrRegionQuery::new(province.clone(), query_city.clone(), operator).key();
        if seen.insert(key) {
            result.push(WhitelistRegionInput {
                province,
                query_city,
                operator,
            });
        }
    }
    Ok(result)
}

fn js_region_string(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(value)) => value.clone(),
        Some(Value::Bool(value)) => value.to_string(),
        Some(Value::Number(value)) => value.to_string(),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| js_region_string(Some(item)))
            .collect::<Vec<_>>()
            .join(","),
        Some(Value::Object(_)) => "[object Object]".to_string(),
    }
}

pub(super) async fn resolve_whitelist_region_policy(
    state: &AppState,
    regions: &[WhitelistRegionInput],
) -> Result<CompiledIpSet, WhitelistRegionResolveError> {
    let mut policies = Vec::new();
    for region in regions {
        let query = CidrRegionQuery::new(
            region.province.clone(),
            region.query_city.clone(),
            region.operator,
        );
        let lookup = crate::cidr::lookup_region(state, &query)
            .await
            .map_err(|error| WhitelistRegionResolveError::Lookup(error.to_string()))?;
        policies.push(lookup.policy);
    }
    let policy = crate::cidr::union_ip_sets(policies.iter());
    if policy.range_count() == 0 {
        return Err(WhitelistRegionResolveError::Empty);
    }
    Ok(policy)
}

pub(super) fn normalize_target(
    value: &str,
    source: &str,
    target_type: Option<&str>,
) -> Result<(String, String), &'static str> {
    let inferred = match target_type {
        Some("ip") => Some("ip"),
        Some("cidr") => Some("cidr"),
        Some("cname") => Some("cname"),
        _ => infer_target_type(value),
    }
    .ok_or("Invalid whitelist target format")?;

    if source == "auto" && inferred != "ip" {
        return Err("Automatic whitelist grants only support IP targets");
    }

    let target = match inferred {
        "cidr" => normalize_cidr(value),
        "cname" => normalize_domain(value),
        _ => {
            let normalized = normalize_ip(value);
            (!normalized.is_empty()).then_some(normalized)
        }
    }
    .ok_or(match inferred {
        "cidr" => "Invalid whitelist CIDR",
        "cname" => "Invalid whitelist domain",
        _ => "Invalid whitelist IP",
    })?;

    Ok((target, inferred.to_string()))
}

fn infer_target_type(value: &str) -> Option<&'static str> {
    if normalize_cidr(value).is_some() {
        return Some("cidr");
    }
    if !normalize_ip(value).is_empty() {
        return Some("ip");
    }
    if normalize_domain(value).is_some() {
        return Some("cname");
    }
    None
}

fn normalize_cidr(value: &str) -> Option<String> {
    let parsed = IpNet::from_str(value.trim()).ok()?;
    Some(match parsed {
        IpNet::V4(network) => format!("{}/{}", network.network(), network.prefix_len()),
        IpNet::V6(network) => format!("{}/{}", network.network(), network.prefix_len()),
    })
}

fn normalize_domain(value: &str) -> Option<String> {
    let trimmed = value.trim().trim_end_matches('.').to_ascii_lowercase();
    if trimmed.is_empty() || trimmed.contains('/') || trimmed.contains("..") {
        return None;
    }
    let ascii = idna::domain_to_ascii(&trimmed).ok()?;
    if ascii.is_empty() || ascii.len() > 253 {
        return None;
    }
    let labels = ascii.split('.').collect::<Vec<_>>();
    if labels.len() < 2 {
        return None;
    }
    for label in labels {
        if label.is_empty()
            || label.len() > 63
            || label.starts_with('-')
            || label.ends_with('-')
            || !label
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '-')
        {
            return None;
        }
    }
    Some(ascii)
}

pub(super) fn normalize_cname_check_interval(value: Option<i64>) -> i64 {
    value.unwrap_or(DEFAULT_CNAME_CHECK_INTERVAL_MINUTES).clamp(
        MIN_CNAME_CHECK_INTERVAL_MINUTES,
        MAX_CNAME_CHECK_INTERVAL_MINUTES,
    )
}

pub(super) fn diff_targets(
    left: &[WhitelistConcreteTarget],
    right: &[WhitelistConcreteTarget],
) -> Vec<WhitelistConcreteTarget> {
    left.iter()
        .filter(|candidate| {
            !right.iter().any(|other| {
                other.target == candidate.target && other.target_type == candidate.target_type
            })
        })
        .cloned()
        .collect()
}
