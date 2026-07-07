use super::*;

pub(super) fn command_available(command: &str) -> bool {
    Command::new(command)
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

pub(super) fn resolve_ssh_ports() -> Vec<i64> {
    let path =
        std::env::var("SSHD_CONFIG_PATH").unwrap_or_else(|_| "/etc/ssh/sshd_config".to_string());
    let Ok(content) = fs::read_to_string(path) else {
        return vec![22];
    };
    let mut ports = content
        .lines()
        .filter_map(|line| {
            let line = line.split('#').next()?.trim();
            let mut parts = line.split_whitespace();
            if parts.next()?.eq_ignore_ascii_case("port") {
                parts.next()?.parse::<i64>().ok()
            } else {
                None
            }
        })
        .filter(|port| *port > 0 && *port <= 65535)
        .collect::<Vec<_>>();
    ports.sort_unstable();
    ports.dedup();
    if ports.is_empty() { vec![22] } else { ports }
}

pub(super) fn normalize_allowed_regions(value: Option<&Value>) -> Value {
    let Some(items) = value.and_then(Value::as_array) else {
        return json!([]);
    };
    let mut seen = HashSet::new();
    let regions = items
        .iter()
        .filter_map(|item| {
            let province = item.get("province")?.as_str()?.trim();
            if province.is_empty() {
                return None;
            }
            let query_city = item
                .get("query_city")
                .or_else(|| item.get("queryCity"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty());
            let key = format!("{province}::{}", query_city.unwrap_or(""));
            if !seen.insert(key) {
                return None;
            }
            Some(json!({
                "province": province,
                "city": item.get("city").and_then(Value::as_str).unwrap_or(""),
                "label": item.get("label").and_then(Value::as_str).unwrap_or(province),
                "value": item.get("value").and_then(Value::as_str).unwrap_or(province),
                "query_city": query_city,
                "is_province_wide": item.get("is_province_wide").and_then(Value::as_bool).unwrap_or(query_city.is_none()),
                "is_municipality": item.get("is_municipality").and_then(Value::as_bool).unwrap_or(false)
            }))
        })
        .collect::<Vec<_>>();
    Value::Array(regions)
}

pub(super) fn normalize_cidrs(value: Option<&Value>) -> Value {
    let Some(items) = value.and_then(Value::as_array) else {
        return json!([]);
    };
    Value::Array(
        normalize_cidr_strings(items.iter().filter_map(Value::as_str).map(str::to_string))
            .into_iter()
            .map(Value::String)
            .collect(),
    )
}

pub(super) fn normalize_cidr_strings(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = HashSet::new();
    values
        .into_iter()
        .filter_map(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return None;
            }
            trimmed.parse::<IpNet>().ok()
        })
        .map(|cidr| cidr.to_string())
        .filter(|cidr| seen.insert(cidr.clone()))
        .collect()
}

pub(super) fn validate_cidrs(
    value: Option<&Value>,
    translator: &Translator,
) -> Result<(), SshError> {
    let Some(value) = value else {
        return Ok(());
    };
    let Some(items) = value.as_array() else {
        return Err(SshError::BadRequest(ssh_security_text(
            translator,
            "customCidrsMustBeArray",
        )));
    };
    let invalid = items
        .iter()
        .filter_map(|item| match item.as_str() {
            Some(value) if value.trim().is_empty() => None,
            Some(value) if value.trim().parse::<IpNet>().is_err() => Some(value.trim()),
            Some(_) => None,
            None => Some("<non-string>"),
        })
        .collect::<Vec<_>>();
    if invalid.is_empty() {
        Ok(())
    } else {
        Err(SshError::BadRequest(ssh_security_text_params(
            translator,
            "customCidrInvalid",
            &[("cidrs", invalid.join(", "))],
        )))
    }
}

pub(super) fn normalize_duration_unit(value: Option<&str>) -> String {
    match value {
        Some("minute" | "hour" | "day") => value.unwrap().to_string(),
        _ => "day".to_string(),
    }
}

pub(super) fn int_field(
    raw: &Map<String, Value>,
    key: &str,
    fallback: i64,
    min: i64,
    max: i64,
) -> i64 {
    raw.get(key)
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_str().and_then(|value| value.parse::<i64>().ok()))
        })
        .unwrap_or(fallback)
        .clamp(min, max)
}

pub(super) fn int_field_or_previous(
    raw: &Map<String, Value>,
    previous: &Value,
    key: &str,
    fallback: i64,
    min: i64,
    max: i64,
) -> i64 {
    raw.get(key)
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_str().and_then(|value| value.parse::<i64>().ok()))
        })
        .or_else(|| previous.get(key).and_then(Value::as_i64))
        .unwrap_or(fallback)
        .clamp(min, max)
}

pub(super) fn normalize_timestamp(value: Option<&Value>) -> Option<Value> {
    let value = value.and_then(Value::as_str)?;
    time_utils::parse_iso_ms(value).map(|_| Value::String(value.to_string()))
}

pub(super) fn iso_score(value: Option<&str>) -> i64 {
    value.and_then(time_utils::parse_iso_ms).unwrap_or_default()
}

pub(super) fn parse_positive(value: Option<&str>, fallback: i64, max: i64) -> i64 {
    value
        .and_then(|value| crate::node_compat::parse_i64_prefix(value.trim_start()))
        .filter(|value| *value > 0)
        .unwrap_or(fallback)
        .min(max)
}

pub(super) fn parse_json_body(body: &Bytes) -> Value {
    if body.is_empty() {
        return json!({});
    }
    serde_json::from_slice(body).unwrap_or_else(|_| json!({}))
}

pub(super) fn delete_ip_value_to_string(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value.clone(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::Array(items) => items
            .iter()
            .map(delete_ip_value_to_string)
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".to_string(),
    }
}

pub(super) fn positive_i64_from_value(value: Option<&Value>) -> Option<i64> {
    parse_i64_from_json_like_node(value?).filter(|value| *value > 0)
}

pub(super) fn parse_i64_from_json_like_node(value: &Value) -> Option<i64> {
    crate::node_compat::parse_i64_from_json_like_node(value)
}

pub(super) fn extract_between<'a>(value: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let (_, rest) = value.split_once(start)?;
    let (part, _) = rest.split_once(end)?;
    Some(part)
}

pub(super) fn extract_after<'a>(value: &'a str, marker: &str) -> Option<&'a str> {
    value.split_once(marker).map(|(_, rest)| rest)
}

pub(super) fn fingerprint(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    hex::encode(hasher.finalize())[..24].to_string()
}

pub(super) fn millis_to_iso(ms: i64) -> String {
    time::OffsetDateTime::from_unix_timestamp(ms.div_euclid(1000))
        .ok()
        .and_then(|time| {
            time.format(&time::format_description::well_known::Rfc3339)
                .ok()
        })
        .unwrap_or_else(time_utils::now_iso)
}
