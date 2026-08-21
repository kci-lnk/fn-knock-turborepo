use super::*;

pub(in crate::storage::redis_store) fn deserialize_whitelist_region_group(
    raw: &str,
) -> Option<WhitelistRegionGroupRecord> {
    let parsed = serde_json::from_str::<Value>(raw).ok()?;
    let object = parsed.as_object()?;
    let id = object
        .get("id")
        .map(js_string)
        .unwrap_or_default()
        .trim()
        .to_string();
    if id.is_empty() {
        return None;
    }

    let regions = object
        .get("regions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|region| {
            let object = region.as_object()?;
            let province = object
                .get("province")
                .map(js_string)
                .unwrap_or_default()
                .trim()
                .to_string();
            if province.is_empty() {
                return None;
            }
            let query_city = object
                .get("query_city")
                .map(js_string)
                .unwrap_or_default()
                .trim()
                .to_string();
            let operator = crate::cidr::CidrOperator::parse_value(object.get("operator"))
                .ok()
                .flatten();
            Some(WhitelistRegionInput {
                province,
                query_city: (!query_city.is_empty()).then_some(query_city),
                operator,
            })
        })
        .collect::<Vec<_>>();
    let cidrs = object
        .get("cidrs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(js_string)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    let policy_id = object
        .get("policyId")
        .or_else(|| object.get("policy_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default()
        .to_string();
    let policy = object
        .get("policy")
        .filter(|value| !value.is_null())
        .cloned();
    let source_cidr_count = object
        .get("sourceCidrCount")
        .or_else(|| object.get("source_cidr_count"))
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(cidrs.len());
    let range_count = object
        .get("rangeCount")
        .or_else(|| object.get("range_count"))
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or_default();
    let created_at = js_finite_number(object.get("createdAt"))
        .map(|value| value.trunc() as i64)
        .unwrap_or(0);
    let updated_at = js_finite_number(object.get("updatedAt"))
        .map(|value| value.trunc() as i64)
        .unwrap_or(0);
    let expire_at = match object.get("expireAt") {
        None | Some(Value::Null) => None,
        value => js_finite_number(value).map(|value| value.trunc() as i64),
    };
    let status = match object.get("status").and_then(Value::as_str) {
        Some("deleted") => "deleted",
        Some("expired") => "expired",
        _ => "active",
    };
    let comment = object.contains_key("comment").then(|| {
        object
            .get("comment")
            .map(js_string)
            .unwrap_or_default()
            .trim()
            .to_string()
    });

    Some(WhitelistRegionGroupRecord {
        id,
        regions,
        cidrs,
        policy_id,
        policy,
        source_cidr_count,
        range_count,
        expire_at,
        source: "manual".to_string(),
        created_at,
        updated_at,
        status: status.to_string(),
        comment,
    })
}

pub(in crate::storage::redis_store) fn deserialize_whitelist_record(
    raw: &str,
) -> Option<WhitelistRecord> {
    let parsed = serde_json::from_str::<Value>(raw).ok()?;
    let object = parsed.as_object()?;
    let id = object
        .get("id")
        .map(js_string)
        .unwrap_or_default()
        .trim()
        .to_string();
    if id.is_empty() {
        return None;
    }

    let raw_target = object
        .get("ip")
        .map(js_string)
        .unwrap_or_default()
        .trim()
        .to_string();
    let target_type = match object.get("targetType").and_then(Value::as_str) {
        Some("cidr") => "cidr",
        Some("cname") => "cname",
        _ => infer_whitelist_target_type(&raw_target)?,
    };
    let normalized_target = normalize_whitelist_target(&raw_target, target_type)?;

    let source = if object.get("source").and_then(Value::as_str) == Some("auto") {
        "auto"
    } else {
        "manual"
    };
    let status = match object.get("status").and_then(Value::as_str) {
        Some("pending") => "pending",
        Some("expired") => "expired",
        Some("deleted") => "deleted",
        _ => "active",
    };
    let created_at = object
        .get("createdAt")
        .map(js_string)
        .as_deref()
        .and_then(parse_int_like_js)
        .unwrap_or(0);
    let expire_at = optional_whitelist_timestamp(object.get("expireAt"));
    let comment = object
        .get("comment")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let ip_location = (target_type == "ip")
        .then(|| {
            object
                .get("ipLocation")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
        })
        .flatten();
    let resolved_targets = (target_type == "cname")
        .then(|| normalize_whitelist_resolved_targets(object.get("resolvedTargets")));
    let check_interval_minutes = (target_type == "cname")
        .then(|| normalize_whitelist_cname_check_interval(object.get("checkIntervalMinutes")));
    let last_checked_at = optional_whitelist_timestamp(object.get("lastCheckedAt"));
    let last_resolved_at = optional_whitelist_timestamp(object.get("lastResolvedAt"));
    let resolve_status = match object.get("resolveStatus").and_then(Value::as_str) {
        Some("resolved") => Some("resolved".to_string()),
        Some("empty") => Some("empty".to_string()),
        Some("error") => Some("error".to_string()),
        Some("pending") => Some("pending".to_string()),
        _ if target_type == "cname" => Some("pending".to_string()),
        _ => None,
    };
    let resolve_message = object
        .get("resolveMessage")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);

    Some(WhitelistRecord {
        id,
        ip: normalized_target,
        target_type: target_type.to_string(),
        expire_at,
        source: source.to_string(),
        created_at,
        status: status.to_string(),
        comment,
        ip_location,
        resolved_targets,
        check_interval_minutes,
        last_checked_at,
        last_resolved_at,
        resolve_status,
        resolve_message,
    })
}

pub(in crate::storage::redis_store) fn infer_whitelist_target_type(
    value: &str,
) -> Option<&'static str> {
    if normalize_whitelist_cidr(value).is_some() {
        return Some("cidr");
    }
    if !normalize_ip(value).is_empty() {
        return Some("ip");
    }
    if normalize_whitelist_domain(value).is_some() {
        return Some("cname");
    }
    None
}

pub(in crate::storage::redis_store) fn normalize_whitelist_target(
    value: &str,
    target_type: &str,
) -> Option<String> {
    match target_type {
        "cidr" => normalize_whitelist_cidr(value),
        "cname" => normalize_whitelist_domain(value),
        _ => {
            let normalized = normalize_ip(value);
            (!normalized.is_empty()).then_some(normalized)
        }
    }
}

pub(in crate::storage::redis_store) fn normalize_whitelist_cidr(value: &str) -> Option<String> {
    let parsed = IpNet::from_str(value.trim()).ok()?;
    Some(match parsed {
        IpNet::V4(network) => format!("{}/{}", network.network(), network.prefix_len()),
        IpNet::V6(network) => format!("{}/{}", network.network(), network.prefix_len()),
    })
}

pub(in crate::storage::redis_store) fn normalize_whitelist_domain(value: &str) -> Option<String> {
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

pub(in crate::storage::redis_store) fn normalize_whitelist_resolved_targets(
    value: Option<&Value>,
) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let normalized = normalize_ip(js_string(item).trim());
            (!normalized.is_empty()).then_some(normalized)
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(in crate::storage::redis_store) fn optional_whitelist_timestamp(
    value: Option<&Value>,
) -> Option<i64> {
    let value = value?;
    if value.is_null() {
        return None;
    }
    let string_value = js_string(value);
    if string_value.is_empty() {
        return None;
    }
    parse_int_like_js(&string_value)
}

pub(in crate::storage::redis_store) fn normalize_whitelist_cname_check_interval(
    value: Option<&Value>,
) -> i64 {
    parse_int_like_js(&value.map(js_string).unwrap_or_default())
        .unwrap_or(5)
        .clamp(1, 24 * 60)
}

pub(in crate::storage::redis_store) fn parse_int_like_js(value: &str) -> Option<i64> {
    crate::node_compat::parse_i64_prefix_trim_start(value)
}
