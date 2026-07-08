use super::*;

pub(super) use crate::{
    http_utils::url_encode_component as percent_encode_uri_component,
    node_compat::{env_i64, floor_to_i64, parse_i64_or as parse_i64},
};

pub(super) fn parse_blacklist_delete_ips(body: &[u8]) -> Result<Vec<String>, &'static str> {
    let parsed = parse_json_body(body)?;
    if let Some(array) = parsed.as_array() {
        return Ok(array
            .iter()
            .filter_map(Value::as_str)
            .map(ToString::to_string)
            .collect());
    }
    if let Some(array) = parsed.get("ips").and_then(Value::as_array) {
        return Ok(array
            .iter()
            .filter_map(Value::as_str)
            .map(ToString::to_string)
            .collect());
    }
    Ok(Vec::new())
}

pub(super) fn parse_json_body(body: &[u8]) -> Result<Value, &'static str> {
    if body.is_empty() {
        return Ok(Value::Null);
    }
    let parsed = serde_json::from_slice::<Value>(body).map_err(|_| "Invalid request body")?;
    if let Some(inner) = parsed.as_str() {
        return serde_json::from_str(inner).map_err(|_| "Invalid request body");
    }
    Ok(parsed)
}

pub(super) fn sanitize_scanner_ips(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for value in values {
        let normalized = value.trim();
        if normalized.is_empty() || !seen.insert(normalized.to_string()) {
            continue;
        }
        result.push(normalized.to_string());
    }
    result
}

pub(super) fn validate_scanner_cidr_exemptions(
    values: Vec<String>,
) -> Result<Vec<String>, ScannerError> {
    let normalized = normalize_scanner_cidr_exemptions_from_strings(values);
    let invalid = normalized
        .iter()
        .filter(|cidr| !is_valid_cidr(cidr))
        .cloned()
        .collect::<Vec<_>>();
    if !invalid.is_empty() {
        return Err(ScannerError::BadRequest(format!(
            "Invalid CIDR exemptions: {}",
            invalid.join(", ")
        )));
    }
    Ok(normalized)
}

pub(super) fn normalize_scanner_cidr_exemptions(value: Option<&Value>) -> Vec<String> {
    let values = value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    item.as_str()
                        .map(ToString::to_string)
                        .unwrap_or_else(|| item.to_string())
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    normalize_scanner_cidr_exemptions_from_strings(values)
        .into_iter()
        .filter(|cidr| is_valid_cidr(cidr))
        .collect()
}

pub(super) fn normalize_scanner_cidr_exemptions_from_strings(values: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for value in values {
        let normalized = value.trim();
        if normalized.is_empty() {
            continue;
        }
        let key = normalized.to_ascii_lowercase();
        if seen.insert(key) {
            result.push(normalized.to_string());
        }
    }
    result
}

pub(super) fn is_valid_cidr(value: &str) -> bool {
    let normalized = value.trim();
    let Some((address, prefix_raw)) = normalized.split_once('/') else {
        return false;
    };
    if address.trim().is_empty()
        || prefix_raw.trim().is_empty()
        || prefix_raw.trim().chars().any(|ch| !ch.is_ascii_digit())
    {
        return false;
    }
    let Ok(prefix) = prefix_raw.trim().parse::<u16>() else {
        return false;
    };
    match address.trim().parse::<IpAddr>() {
        Ok(IpAddr::V4(_)) => prefix <= 32,
        Ok(IpAddr::V6(_)) => prefix <= 128,
        Err(_) => false,
    }
}

pub(super) fn normalize_scanner_cidr_exemption_regions(
    value: Option<&Value>,
) -> Vec<ScannerCidrExemptionSelection> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(normalize_scanner_cidr_exemption_selection)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

pub(super) fn normalize_scanner_cidr_exemption_selection(
    value: &Value,
) -> Option<ScannerCidrExemptionSelection> {
    let province = normalize_string(value.get("province")?.as_str()?);
    let label = normalize_string(value.get("label")?.as_str()?);
    let value_label = normalize_string(value.get("value")?.as_str()?);
    if province.is_empty() || label.is_empty() || value_label.is_empty() {
        return None;
    }
    Some(ScannerCidrExemptionSelection {
        province,
        city: value
            .get("city")
            .and_then(Value::as_str)
            .map(normalize_string)
            .filter(|value| !value.is_empty()),
        label,
        value: value_label,
        query_city: value
            .get("query_city")
            .and_then(Value::as_str)
            .map(normalize_string)
            .filter(|value| !value.is_empty()),
        is_province_wide: value
            .get("is_province_wide")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        is_municipality: value
            .get("is_municipality")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

pub(super) fn dedupe_scanner_cidr_exemption_region_inputs(
    values: Vec<ScannerCidrExemptionRegionBody>,
) -> Vec<ScannerCidrExemptionRegionInput> {
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for value in values {
        let province = normalize_string(&value.province);
        if province.is_empty() {
            continue;
        }
        let query_city = value
            .query_city
            .as_deref()
            .map(normalize_string)
            .filter(|value| !value.is_empty());
        let key = scanner_cidr_region_key(&province, query_city.as_deref());
        if seen.insert(key) {
            result.push(ScannerCidrExemptionRegionInput {
                province,
                query_city,
            });
        }
    }
    result
}

pub(super) fn scanner_cidr_region_keys_equal(
    left: &[ScannerCidrExemptionRegionInput],
    right: &[ScannerCidrExemptionRegionInput],
) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            scanner_cidr_region_key(&left.province, left.query_city.as_deref())
                == scanner_cidr_region_key(&right.province, right.query_city.as_deref())
        })
}

pub(super) fn scanner_cidr_region_key(province: &str, query_city: Option<&str>) -> String {
    format!("{}::{}", province.trim(), query_city.unwrap_or("").trim())
}

pub(super) fn cidr_cache_key(province: &str, city: Option<&str>) -> String {
    let province = percent_encode_uri_component(province);
    match city {
        Some(city) => format!(
            "{CIDR_CACHE_PREFIX}:cidrs:{province}:{}",
            percent_encode_uri_component(city)
        ),
        None => format!("{CIDR_CACHE_PREFIX}:cidrs:{province}"),
    }
}

pub(super) fn json_string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn json_array_values(value: Option<&Value>) -> Vec<Value> {
    value.and_then(Value::as_array).cloned().unwrap_or_default()
}

pub(super) fn positive_i64(value: Option<&Value>) -> Option<i64> {
    let value = value?;
    let parsed = value
        .as_i64()
        .or_else(|| value.as_f64().map(floor_to_i64))
        .or_else(|| value.as_str().and_then(|text| text.trim().parse().ok()))?;
    (parsed > 0).then_some(parsed)
}

pub(super) fn normalize_required_province(value: &str) -> Result<String, ScannerError> {
    let normalized = normalize_string(value);
    if normalized.is_empty() {
        return Err(ScannerError::BadRequest("province is required".to_string()));
    }
    Ok(normalized)
}

pub(super) fn to_safe_i64(value: Option<&Value>, fallback: i64) -> i64 {
    let parsed = value.and_then(js_number_like_i64_floor).unwrap_or(fallback);
    parsed.max(0)
}

pub(super) fn js_number_like_i64_floor(value: &Value) -> Option<i64> {
    let parsed = match value {
        Value::Null => 0.0,
        Value::Bool(value) => {
            if *value {
                1.0
            } else {
                0.0
            }
        }
        Value::Number(value) => value.as_f64()?,
        Value::String(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                0.0
            } else {
                trimmed.parse::<f64>().ok()?
            }
        }
        Value::Array(items) => match items.as_slice() {
            [] => 0.0,
            [item] => {
                let text = match item {
                    Value::Null => String::new(),
                    Value::Bool(value) => value.to_string(),
                    Value::Number(value) => value.to_string(),
                    Value::String(value) => value.clone(),
                    Value::Array(_) | Value::Object(_) => return None,
                };
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    0.0
                } else {
                    trimmed.parse::<f64>().ok()?
                }
            }
            _ => return None,
        },
        Value::Object(_) => return None,
    };
    parsed.is_finite().then(|| floor_to_i64(parsed))
}

pub(super) fn normalize_string(value: &str) -> String {
    value.trim().to_string()
}
