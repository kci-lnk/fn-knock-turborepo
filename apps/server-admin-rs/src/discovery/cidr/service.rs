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
const COMPILED_QUERY_CACHE_VERSION: u64 = 1;
const COMPILED_QUERY_CACHE_VERSION_FIELD: &str = "fnknock_ipset_cache_version";
const COMPILED_QUERY_POLICY_FIELD: &str = "compiled_policy";
const PROVINCE_WIDE_VALUE: &str = "__province_all__";
const PROVINCES_REQUIRING_CITY_AGGREGATION: &[&str] = &["广东", "浙江"];

pub(crate) async fn migrate_cidr_query_caches_on_boot(state: &AppState) -> anyhow::Result<usize> {
    let keys = state
        .store
        .scan_keys(&format!("{CACHE_PREFIX}:"), 200)
        .await?;
    let mut migrated = 0usize;
    for key in keys.into_iter().filter(|key| key.contains(":cidrs:")) {
        let (data, ttl) = state.store.get_json_value_with_ttl(&key).await?;
        let Some(data) = data else {
            state.store.delete_keys(std::slice::from_ref(&key)).await?;
            tracing::warn!(%key, "removed malformed CIDR query cache entry");
            continue;
        };
        match compact_query_data(&data) {
            Ok(compact) if compact != data => {
                state
                    .store
                    .set_json_value_preserve_ttl(&key, &compact, ttl)
                    .await?;
                migrated += 1;
            }
            Ok(_) => {}
            Err(error) => {
                state.store.delete_keys(std::slice::from_ref(&key)).await?;
                tracing::warn!(%key, %error, "removed invalid CIDR query cache entry");
            }
        }
    }
    Ok(migrated)
}

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
    lookup_from_data(query, &data)
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
    if let Some(data) = load_cached_query(state, query, &key).await? {
        return Ok(data);
    }
    if requires_city_aggregation(query) {
        let data = aggregate_province_cidrs_at(state, query, base_url).await?;
        state
            .store
            .set_json_value_ex(&key, &data, SUCCESS_CACHE_TTL_SECONDS)
            .await?;
        return Ok(data);
    }
    get_cached_leaf_query_at(state, query, base_url).await
}

async fn get_cached_leaf_query_at(
    state: &AppState,
    query: &CidrRegionQuery,
    base_url: &str,
) -> Result<Value, CidrError> {
    let key = namespaced_cache_key(
        base_url,
        &format!("cidrs:{}", url_encode_component(&query.key())),
    );
    if let Some(data) = load_cached_query(state, query, &key).await? {
        return Ok(data);
    }
    let pairs = query.query_pairs();
    let data = fetch_data(state, base_url, "cidrs", &pairs)
        .await
        .map_err(CidrError::Service)?;
    validate_operator_echo(&data, query.operator).map_err(CidrError::Service)?;
    let data = compact_query_data(&data)?;
    state
        .store
        .set_json_value_ex(&key, &data, SUCCESS_CACHE_TTL_SECONDS)
        .await?;
    Ok(data)
}

async fn load_cached_query(
    state: &AppState,
    query: &CidrRegionQuery,
    key: &str,
) -> Result<Option<Value>, CidrError> {
    let (data, ttl) = state.store.get_json_value_with_ttl(key).await?;
    let Some(data) = data else {
        return Ok(None);
    };
    validate_operator_echo(&data, query.operator).map_err(CidrError::Service)?;
    let compact = compact_query_data(&data)?;
    if compact != data {
        state
            .store
            .set_json_value_preserve_ttl(key, &compact, ttl)
            .await?;
    }
    Ok(Some(compact))
}

fn compact_query_data(data: &Value) -> Result<Value, CidrError> {
    let policy = query_policy_from_data(data)?;
    let mut compact = data
        .as_object()
        .cloned()
        .ok_or_else(|| CidrError::Service("CIDR cache entry must be an object".to_string()))?;
    compact.remove("cidr_groups");
    compact.insert(
        COMPILED_QUERY_CACHE_VERSION_FIELD.to_string(),
        json!(COMPILED_QUERY_CACHE_VERSION),
    );
    compact.insert(
        COMPILED_QUERY_POLICY_FIELD.to_string(),
        policy.to_transport_value(),
    );
    let counts = compact
        .entry("counts".to_string())
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| CidrError::Service("CIDR cache counts must be an object".to_string()))?;
    let fallback_counts = (counts.get("4").is_none() || counts.get("6").is_none())
        .then(|| policy_cidr_counts(&policy));
    counts
        .entry("4".to_string())
        .or_insert_with(|| json!(fallback_counts.expect("missing CIDR count fallback").0));
    counts
        .entry("6".to_string())
        .or_insert_with(|| json!(fallback_counts.expect("missing CIDR count fallback").1));
    Ok(Value::Object(compact))
}

fn query_policy_from_data(data: &Value) -> Result<crate::cidr::CompiledIpSet, CidrError> {
    if let Some(value) = data.get(COMPILED_QUERY_POLICY_FIELD) {
        return crate::cidr::CompiledIpSet::from_transport_value(value)
            .map(crate::cidr::CompiledIpSet::into_current_format)
            .map_err(|error| {
                CidrError::Service(format!("CIDR cache compiled policy is invalid: {error}"))
            });
    }
    let groups = data
        .get("cidr_groups")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            CidrError::Service("CIDR upstream response missing cidr_groups".to_string())
        })?;
    let mut cidrs = Vec::new();
    for family in ["4", "6"] {
        let Some(items) = groups.get(family) else {
            continue;
        };
        let items = items.as_array().ok_or_else(|| {
            CidrError::Service(format!(
                "CIDR upstream cidr_groups.{family} must be an array"
            ))
        })?;
        for item in items {
            let value = item.as_str().ok_or_else(|| {
                CidrError::Service(format!(
                    "CIDR upstream cidr_groups.{family} contains a non-string value"
                ))
            })?;
            cidrs.push(value);
        }
    }
    crate::cidr::compile_ip_set(cidrs).map_err(CidrError::Service)
}

fn policy_cidr_groups(policy: &crate::cidr::CompiledIpSet) -> (Vec<Value>, Vec<Value>) {
    let mut ipv4 = Vec::new();
    let mut ipv6 = Vec::new();
    for cidr in policy.to_cidrs() {
        let item = Value::String(cidr);
        if item.as_str().is_some_and(|value| value.contains(':')) {
            ipv6.push(item);
        } else {
            ipv4.push(item);
        }
    }
    (ipv4, ipv6)
}

fn policy_cidr_counts(policy: &crate::cidr::CompiledIpSet) -> (usize, usize) {
    let (ipv4, ipv6) = policy_cidr_groups(policy);
    (ipv4.len(), ipv6.len())
}

async fn aggregate_province_cidrs_at(
    state: &AppState,
    query: &CidrRegionQuery,
    base_url: &str,
) -> Result<Value, CidrError> {
    let province = required_province(&query.province)?;
    let encoded = url_encode_component(&province);
    let cities = get_cached_data_at(
        state,
        base_url,
        &format!("cities:{encoded}"),
        &format!("provinces/{encoded}/cities"),
        &[],
    )
    .await?;
    let city_names = cities
        .get("items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("name").and_then(Value::as_str))
        .map(normalize_string)
        .filter(|city| !city.is_empty())
        .collect::<Vec<_>>();
    if city_names.is_empty() {
        return Err(CidrError::Service(
            "CIDR upstream response missing city data".to_string(),
        ));
    }

    let mut policies = Vec::new();
    for city in city_names {
        let city_query = CidrRegionQuery::new(province.clone(), Some(city), query.operator);
        let data = get_cached_leaf_query_at(state, &city_query, base_url).await?;
        policies.push(query_policy_from_data(&data)?);
    }
    let policy = crate::cidr::union_ip_sets(policies.iter());
    let (ipv4_count, ipv6_count) = policy_cidr_counts(&policy);
    let mut data = json!({
        "province": province,
        "city": Value::Null,
        "counts": {
            "4": ipv4_count,
            "6": ipv6_count,
        },
        "fnknock_ipset_cache_version": COMPILED_QUERY_CACHE_VERSION,
        "compiled_policy": policy.to_transport_value(),
    });
    if let Some(operator) = query.operator {
        data["operator"] = Value::String(operator.as_str().to_string());
    }
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
    get_cached_data_at(state, &base_url, logical_key, path, query).await
}

async fn get_cached_data_at(
    state: &AppState,
    base_url: &str,
    logical_key: &str,
    path: &str,
    query: &[(&str, &str)],
) -> Result<Value, CidrError> {
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
    Ok(())
}

fn supports_province_wide(_province: &str, is_municipality: bool) -> bool {
    !is_municipality
}

fn requires_city_aggregation(query: &CidrRegionQuery) -> bool {
    if query.query_city.is_some() {
        return false;
    }
    let province = query.province.trim().trim_end_matches('省');
    PROVINCES_REQUIRING_CITY_AGGREGATION.contains(&province)
}

fn required_province(value: &str) -> Result<String, CidrError> {
    let value = normalize_string(value);
    if value.is_empty() {
        Err(CidrError::BadRequest("province is required".to_string()))
    } else {
        Ok(value)
    }
}

fn lookup_from_data(query: &CidrRegionQuery, data: &Value) -> Result<CidrLookup, CidrError> {
    let selection = selection_from_data(query, data, None);
    let policy = query_policy_from_data(data)?;
    Ok(CidrLookup { selection, policy })
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
    let (ipv4, ipv6) = query_policy_from_data(data).map_or_else(
        |_| {
            (
                json_array_values(data.pointer("/cidr_groups/4")),
                json_array_values(data.pointer("/cidr_groups/6")),
            )
        },
        |policy| policy_cidr_groups(&policy),
    );
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

fn to_safe_i64(value: Option<&Value>, fallback: i64) -> i64 {
    value
        .and_then(js_number_like_i64_floor)
        .unwrap_or(fallback)
        .max(0)
}

fn json_array_values(value: Option<&Value>) -> Vec<Value> {
    value.and_then(Value::as_array).cloned().unwrap_or_default()
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
    fn all_non_municipalities_offer_province_wide_selection() {
        assert!(supports_province_wide("浙江", false));
        assert!(supports_province_wide("浙江省", false));
        assert!(supports_province_wide("广东", false));
        assert!(supports_province_wide("江苏", false));
        assert!(!supports_province_wide("北京", true));

        let zhejiang = CidrRegionQuery::new("浙江", None::<String>, None);
        let guangdong = CidrRegionQuery::new("广东省", None::<String>, None);
        assert!(validate_query(&zhejiang).is_ok());
        assert!(validate_query(&guangdong).is_ok());
        assert!(requires_city_aggregation(&zhejiang));
        assert!(requires_city_aggregation(&guangdong));
        assert!(validate_query(&CidrRegionQuery::new("浙江", Some("杭州"), None)).is_ok());
        assert!(validate_query(&CidrRegionQuery::new("江苏", None::<String>, None)).is_ok());
        assert!(!requires_city_aggregation(&CidrRegionQuery::new(
            "浙江",
            Some("杭州"),
            None,
        )));
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

    #[test]
    fn query_cache_compacts_legacy_arrays_and_materializes_an_exact_cover() {
        let raw = json!({
            "province": "广东",
            "city": "深圳",
            "cidr_groups": {
                "4": ["192.0.2.0/25", "192.0.2.128/25"],
                "6": ["2001:db8::/33", "2001:db8:8000::/33"],
            },
            "counts": { "4": 2, "6": 2 },
        });
        let compact = compact_query_data(&raw).unwrap();
        assert!(compact.get("cidr_groups").is_none());
        assert_eq!(
            compact[COMPILED_QUERY_CACHE_VERSION_FIELD],
            json!(COMPILED_QUERY_CACHE_VERSION)
        );
        assert_eq!(
            compact
                .pointer("/compiled_policy/format_version")
                .and_then(Value::as_u64),
            Some(crate::cidr::ipset::COMPILED_IP_SET_FORMAT_VERSION as u64)
        );
        let policy = query_policy_from_data(&compact).unwrap();
        assert_eq!(policy.to_cidrs(), vec!["192.0.2.0/24", "2001:db8::/32"]);

        let query = CidrRegionQuery::new("广东", Some("深圳"), None);
        let payload = lookup_payload_from_data(&query, &compact, None);
        assert_eq!(payload["cidrGroups"]["ipv4"], json!(["192.0.2.0/24"]));
        assert_eq!(payload["cidrGroups"]["ipv6"], json!(["2001:db8::/32"]));
        assert_eq!(payload["counts"], json!({ "ipv4": 2, "ipv6": 2 }));
    }
}
