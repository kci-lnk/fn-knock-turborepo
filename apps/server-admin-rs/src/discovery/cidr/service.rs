use std::collections::BTreeSet;

use serde_json::{Value, json};

use super::{
    CidrError, CidrLookup, CidrRegionQuery, CidrSelection, configured_cidr_source, fetch_data,
    source_fingerprint, validate_operator_echo,
};
use crate::{
    http_utils::url_encode_component, i18n::Translator, node_compat::floor_to_i64, state::AppState,
};

const CACHE_PREFIX: &str = "fn_knock:cidr";
const SUCCESS_CACHE_TTL_SECONDS: usize = 30 * 24 * 60 * 60;
const PROVINCE_WIDE_VALUE: &str = "__province_all__";
const CITY_ONLY_PROVINCES: &[&str] = &["广东", "浙江"];

pub(crate) async fn lookup_regions(
    state: &AppState,
    queries: &[CidrRegionQuery],
) -> Result<Vec<CidrLookup>, CidrError> {
    let mut result = Vec::with_capacity(queries.len());
    for query in queries {
        result.push(lookup_region(state, query).await?);
    }
    Ok(result)
}

pub(crate) async fn lookup_region(
    state: &AppState,
    query: &CidrRegionQuery,
) -> Result<CidrLookup, CidrError> {
    validate_query(query)?;
    let (_, base_url) = configured_cidr_source(state)
        .await
        .map_err(CidrError::Service)?;
    lookup_region_at(state, query, &base_url).await
}

pub(crate) async fn provinces_payload(state: &AppState) -> Result<Value, CidrError> {
    let data = get_cached_data(state, "provinces", "provinces", &[]).await?;
    let items = data
        .get("items")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let name = item
                        .get("name")
                        .and_then(Value::as_str)
                        .map(normalize_string)
                        .filter(|value| !value.is_empty())?;
                    let city_count = to_safe_i64(item.get("city_count"), 0);
                    let is_municipality = city_count <= 1;
                    Some(json!({
                        "name": name,
                        "cityCount": city_count,
                        "isMunicipality": is_municipality,
                        "hasChildren": !is_municipality,
                    }))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let options = items
        .iter()
        .map(|item| {
            let name = item.get("name").and_then(Value::as_str).unwrap_or("");
            json!({
                "label": name,
                "value": name,
                "cityCount": item.get("cityCount").and_then(Value::as_i64).unwrap_or(0),
                "isMunicipality": item.get("isMunicipality").and_then(Value::as_bool).unwrap_or(false),
            })
        })
        .collect::<Vec<_>>();
    let total = to_safe_i64(data.get("total"), items.len() as i64);

    Ok(json!({ "items": items, "options": options, "total": total }))
}

pub(crate) async fn cities_payload(
    state: &AppState,
    province_input: &str,
    translator: Option<&Translator>,
) -> Result<Value, CidrError> {
    let province = required_province(province_input)?;
    let encoded = url_encode_component(&province);
    let data = get_cached_data(
        state,
        &format!("cities:{encoded}"),
        &format!("provinces/{encoded}/cities"),
        &[],
    )
    .await?;
    let items = data
        .get("items")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let name = item
                        .get("name")
                        .and_then(Value::as_str)
                        .map(normalize_string)
                        .filter(|value| !value.is_empty())?;
                    Some(json!({
                        "name": name,
                        "ipv4Count": to_safe_i64(item.get("ipv4_count"), 0),
                        "ipv6Count": to_safe_i64(item.get("ipv6_count"), 0),
                    }))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let resolved_province = data
        .get("province")
        .and_then(Value::as_str)
        .map(normalize_string)
        .filter(|value| !value.is_empty())
        .unwrap_or(province);
    let is_municipality = items.len() == 1
        && items
            .first()
            .and_then(|item| item.get("name"))
            .and_then(Value::as_str)
            == Some(resolved_province.as_str());
    let supports_province_wide = supports_province_wide(&resolved_province, is_municipality);

    let mut options = Vec::new();
    if supports_province_wide {
        options.push(json!({
            "label": province_wide_label(translator, &resolved_province),
            "value": PROVINCE_WIDE_VALUE,
            "queryCity": Value::Null,
            "isProvinceWide": true,
            "isMunicipality": false,
            "ipv4Count": 0,
            "ipv6Count": 0,
        }));
    }
    for item in &items {
        let name = item.get("name").and_then(Value::as_str).unwrap_or("");
        options.push(json!({
            "label": name,
            "value": name,
            "queryCity": if is_municipality {
                Value::String(resolved_province.clone())
            } else {
                Value::String(name.to_string())
            },
            "isProvinceWide": false,
            "isMunicipality": is_municipality,
            "ipv4Count": item.get("ipv4Count").and_then(Value::as_i64).unwrap_or(0),
            "ipv6Count": item.get("ipv6Count").and_then(Value::as_i64).unwrap_or(0),
        }));
    }
    let default_value = if supports_province_wide {
        PROVINCE_WIDE_VALUE.to_string()
    } else {
        items
            .first()
            .and_then(|item| item.get("name"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    let total = cities_total(&data, items.len());

    Ok(json!({
        "province": resolved_province,
        "items": items,
        "options": options,
        "total": total,
        "isMunicipality": is_municipality,
        "supportsProvinceWide": supports_province_wide,
        "defaultValue": default_value,
    }))
}

pub(crate) async fn lookup_payload(
    state: &AppState,
    query: &CidrRegionQuery,
    translator: Option<&Translator>,
) -> Result<Value, CidrError> {
    validate_query(query)?;
    let (_, base_url) = configured_cidr_source(state)
        .await
        .map_err(CidrError::Service)?;
    let data = get_cached_query_at(state, query, &base_url).await?;
    Ok(lookup_payload_from_data(query, &data, translator))
}

async fn lookup_region_at(
    state: &AppState,
    query: &CidrRegionQuery,
    base_url: &str,
) -> Result<CidrLookup, CidrError> {
    let data = get_cached_query_at(state, query, base_url).await?;
    Ok(lookup_from_data(query, &data))
}

async fn get_cached_query_at(
    state: &AppState,
    query: &CidrRegionQuery,
    base_url: &str,
) -> Result<Value, CidrError> {
    let key = namespaced_cache_key(
        base_url,
        &format!("cidrs:{}", url_encode_component(&query.key())),
    );
    if let Some(data) = state.store.get_json_value(&key).await? {
        validate_operator_echo(&data, query.operator).map_err(CidrError::Service)?;
        return Ok(data);
    }
    let pairs = query.query_pairs();
    let data = fetch_data(state, base_url, "cidrs", &pairs)
        .await
        .map_err(CidrError::Service)?;
    validate_operator_echo(&data, query.operator).map_err(CidrError::Service)?;
    state
        .store
        .set_json_value_ex(&key, &data, SUCCESS_CACHE_TTL_SECONDS)
        .await?;
    Ok(data)
}

async fn get_cached_data(
    state: &AppState,
    logical_key: &str,
    path: &str,
    query: &[(&str, &str)],
) -> Result<Value, CidrError> {
    let (_, base_url) = configured_cidr_source(state)
        .await
        .map_err(CidrError::Service)?;
    let key = namespaced_cache_key(&base_url, logical_key);
    if let Some(data) = state.store.get_json_value(&key).await? {
        return Ok(data);
    }
    let data = fetch_data(state, &base_url, path, query)
        .await
        .map_err(CidrError::Service)?;
    state
        .store
        .set_json_value_ex(&key, &data, SUCCESS_CACHE_TTL_SECONDS)
        .await?;
    Ok(data)
}

fn namespaced_cache_key(base_url: &str, logical_key: &str) -> String {
    format!(
        "{CACHE_PREFIX}:{}:{}",
        source_fingerprint(base_url),
        logical_key.trim_start_matches(&format!("{CACHE_PREFIX}:"))
    )
}

fn validate_query(query: &CidrRegionQuery) -> Result<(), CidrError> {
    if query.province.trim().is_empty() {
        return Err(CidrError::BadRequest("province is required".to_string()));
    }
    if query.query_city.is_none() && is_city_only_province(&query.province) {
        return Err(CidrError::BadRequest(
            "province-wide CIDR selection is unavailable".to_string(),
        ));
    }
    Ok(())
}

fn supports_province_wide(province: &str, is_municipality: bool) -> bool {
    !is_municipality && !is_city_only_province(province)
}

fn is_city_only_province(province: &str) -> bool {
    let province = province.trim().trim_end_matches('省');
    CITY_ONLY_PROVINCES.contains(&province)
}

fn required_province(value: &str) -> Result<String, CidrError> {
    let value = normalize_string(value);
    if value.is_empty() {
        Err(CidrError::BadRequest("province is required".to_string()))
    } else {
        Ok(value)
    }
}

fn lookup_from_data(query: &CidrRegionQuery, data: &Value) -> CidrLookup {
    let selection = selection_from_data(query, data, None);
    let cidrs = normalize_cidr_strings(
        json_string_array(data.pointer("/cidr_groups/4"))
            .into_iter()
            .chain(json_string_array(data.pointer("/cidr_groups/6"))),
    );
    CidrLookup { selection, cidrs }
}

fn selection_from_data(
    query: &CidrRegionQuery,
    data: &Value,
    translator: Option<&Translator>,
) -> CidrSelection {
    let province = data
        .get("province")
        .and_then(Value::as_str)
        .map(normalize_string)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| query.province.clone());
    let city = data
        .get("city")
        .and_then(Value::as_str)
        .map(normalize_string)
        .filter(|value| !value.is_empty())
        .or_else(|| query.query_city.clone());
    let is_municipality = city.as_deref().is_some_and(|city| city == province);
    let is_province_wide = city.is_none();
    let region_label = city
        .clone()
        .unwrap_or_else(|| province_wide_label(translator, &province));
    let label = query.operator.map_or(region_label.clone(), |operator| {
        format!("{region_label} · {}", operator.as_str())
    });
    CidrSelection {
        province,
        city: city.clone(),
        label,
        value: city
            .clone()
            .unwrap_or_else(|| PROVINCE_WIDE_VALUE.to_string()),
        query_city: city,
        operator: query.operator,
        is_province_wide,
        is_municipality,
    }
}

pub(crate) fn lookup_payload_from_data(
    query: &CidrRegionQuery,
    data: &Value,
    translator: Option<&Translator>,
) -> Value {
    let selection = selection_from_data(query, data, translator);
    let ipv4 = json_array_values(data.pointer("/cidr_groups/4"));
    let ipv6 = json_array_values(data.pointer("/cidr_groups/6"));
    let ipv4_count = to_safe_i64(data.pointer("/counts/4"), ipv4.len() as i64);
    let ipv6_count = to_safe_i64(data.pointer("/counts/6"), ipv6.len() as i64);
    json!({
        "province": selection.province,
        "city": selection.city,
        "selection": {
            "province": selection.province,
            "city": selection.city,
            "label": selection.label,
            "value": selection.value,
            "queryCity": selection.query_city,
            "operator": selection.operator,
            "isProvinceWide": selection.is_province_wide,
            "isMunicipality": selection.is_municipality,
        },
        "cidrGroups": { "ipv4": ipv4, "ipv6": ipv6 },
        "counts": { "ipv4": ipv4_count, "ipv6": ipv6_count },
        "totalCount": ipv4_count + ipv6_count,
    })
}

pub(crate) fn province_wide_label(translator: Option<&Translator>, province: &str) -> String {
    translator.map_or_else(
        || format!("{province}全省"),
        |translator| {
            translator.t_params(
                "server.cidr.provinceWideLabel",
                &[("province", province.to_string())],
            )
        },
    )
}

pub(crate) fn cities_total(data: &Value, item_count: usize) -> i64 {
    to_safe_i64(data.get("total"), item_count as i64)
}

fn normalize_cidr_strings(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .filter_map(|value| {
            let value = value.trim();
            if value.is_empty() || !seen.insert(value.to_ascii_lowercase()) {
                None
            } else {
                Some(value.to_string())
            }
        })
        .collect()
}

fn json_string_array(value: Option<&Value>) -> Vec<String> {
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

fn json_array_values(value: Option<&Value>) -> Vec<Value> {
    value.and_then(Value::as_array).cloned().unwrap_or_default()
}

fn to_safe_i64(value: Option<&Value>, fallback: i64) -> i64 {
    value
        .and_then(js_number_like_i64_floor)
        .unwrap_or(fallback)
        .max(0)
}

fn js_number_like_i64_floor(value: &Value) -> Option<i64> {
    let parsed = match value {
        Value::Null => 0.0,
        Value::Bool(value) => i32::from(*value) as f64,
        Value::Number(value) => value.as_f64()?,
        Value::String(value) => parse_number_text(value)?,
        Value::Array(items) => match items.as_slice() {
            [] => 0.0,
            [item] => match item {
                Value::Null => 0.0,
                Value::Bool(value) => parse_number_text(&value.to_string())?,
                Value::Number(value) => parse_number_text(&value.to_string())?,
                Value::String(value) => parse_number_text(value)?,
                Value::Array(_) | Value::Object(_) => return None,
            },
            _ => return None,
        },
        Value::Object(_) => return None,
    };
    parsed.is_finite().then(|| floor_to_i64(parsed))
}

fn parse_number_text(value: &str) -> Option<f64> {
    let value = value.trim();
    if value.is_empty() {
        Some(0.0)
    } else {
        value.parse().ok()
    }
}

fn normalize_string(value: &str) -> String {
    value.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cidr::CidrOperator;

    #[test]
    fn city_only_provinces_remain_explicit_business_rules() {
        assert!(!supports_province_wide("浙江", false));
        assert!(!supports_province_wide("浙江省", false));
        assert!(!supports_province_wide("广东", false));
        assert!(supports_province_wide("江苏", false));
        assert!(!supports_province_wide("北京", true));

        assert!(validate_query(&CidrRegionQuery::new("浙江", None::<String>, None)).is_err());
        assert!(validate_query(&CidrRegionQuery::new("广东省", None::<String>, None)).is_err());
        assert!(validate_query(&CidrRegionQuery::new("浙江", Some("杭州"), None)).is_ok());
        assert!(validate_query(&CidrRegionQuery::new("江苏", None::<String>, None)).is_ok());
    }

    #[test]
    fn cache_keys_include_source_and_operator() {
        let mobile = CidrRegionQuery::new("浙江", Some("杭州"), Some(CidrOperator::Mobile));
        let telecom = CidrRegionQuery::new("浙江", Some("杭州"), Some(CidrOperator::Telecom));
        assert_ne!(
            namespaced_cache_key("https://a.example/api/v1", &mobile.key()),
            namespaced_cache_key("https://b.example/api/v1", &mobile.key())
        );
        assert_ne!(
            namespaced_cache_key("https://a.example/api/v1", &mobile.key()),
            namespaced_cache_key("https://a.example/api/v1", &telecom.key())
        );
    }

    #[test]
    fn safe_integer_conversion_matches_existing_payload_contract() {
        assert_eq!(to_safe_i64(None, 9), 9);
        assert_eq!(to_safe_i64(Some(&json!("7.9")), 9), 7);
        assert_eq!(to_safe_i64(Some(&json!("")), 9), 0);
        assert_eq!(to_safe_i64(Some(&json!(null)), 9), 0);
        assert_eq!(to_safe_i64(Some(&json!(true)), 9), 1);
        assert_eq!(to_safe_i64(Some(&json!(-3)), 9), 0);
        assert_eq!(to_safe_i64(Some(&json!(["4.2"])), 9), 4);
        assert_eq!(to_safe_i64(Some(&json!(["1", "2"])), 9), 9);
    }
}
