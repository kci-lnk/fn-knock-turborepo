use super::*;

pub(super) use crate::node_compat::{env_i64, floor_to_i64, parse_i64_or as parse_i64};

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
        operator: CidrOperator::parse_value(value.get("operator"))
            .ok()
            .flatten(),
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
) -> Result<Vec<CidrRegionQuery>, ScannerError> {
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
        let operator =
            CidrOperator::parse_value(value.operator.as_ref()).map_err(ScannerError::BadRequest)?;
        let query = CidrRegionQuery::new(province, query_city, operator);
        let key = query.key();
        if seen.insert(key) {
            result.push(query);
        }
    }
    Ok(result)
}

pub(super) fn scanner_cidr_region_keys_equal(
    left: &[CidrRegionQuery],
    right: &[CidrRegionQuery],
) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| left.key() == right.key())
}

pub(super) fn positive_i64(value: Option<&Value>) -> Option<i64> {
    let value = value?;
    let parsed = value
        .as_i64()
        .or_else(|| value.as_f64().map(floor_to_i64))
        .or_else(|| value.as_str().and_then(|text| text.trim().parse().ok()))?;
    (parsed > 0).then_some(parsed)
}

pub(super) fn normalize_string(value: &str) -> String {
    value.trim().to_string()
}
