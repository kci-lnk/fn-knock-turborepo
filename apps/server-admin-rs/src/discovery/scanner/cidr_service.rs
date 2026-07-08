use super::*;

pub(super) async fn resolve_cidr_exemption_regions(
    state: &AppState,
    regions: &[ScannerCidrExemptionRegionInput],
) -> Result<Vec<ResolvedCidrLookup>, ScannerError> {
    let mut resolved = Vec::new();
    for region in regions {
        resolved.push(lookup_region_cidrs(state, region).await?);
    }
    Ok(resolved)
}

pub(super) async fn lookup_region_cidrs(
    state: &AppState,
    input: &ScannerCidrExemptionRegionInput,
) -> Result<ResolvedCidrLookup, ScannerError> {
    let province = normalize_string(&input.province);
    if province.is_empty() {
        return Err(ScannerError::BadRequest("province is required".to_string()));
    }
    let city = input
        .query_city
        .as_deref()
        .map(normalize_string)
        .filter(|value| !value.is_empty() && value != CIDR_PROVINCE_WIDE_VALUE);
    let cache_key = cidr_cache_key(&province, city.as_deref());

    let data = match state.store.get_json_value(&cache_key).await? {
        Some(data) => data,
        None => {
            let data = fetch_cidr_data(state, &province, city.as_deref()).await?;
            state
                .store
                .set_json_value_ex(&cache_key, &data, CIDR_SUCCESS_CACHE_TTL_SECONDS)
                .await?;
            data
        }
    };

    Ok(cidr_lookup_from_data(&province, city.as_deref(), &data))
}

pub(super) async fn get_cidr_provinces_payload(state: &AppState) -> Result<Value, ScannerError> {
    let cache_key = format!("{CIDR_CACHE_PREFIX}:provinces");
    let data = get_cached_or_fetch_cidr_data(state, &cache_key, "provinces", &[]).await?;
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

    Ok(json!({
        "items": items,
        "options": options,
        "total": total,
    }))
}

pub(super) async fn get_cidr_cities_payload(
    state: &AppState,
    province_input: &str,
    translator: Option<&Translator>,
) -> Result<Value, ScannerError> {
    let province = normalize_required_province(province_input)?;
    let cache_key = format!(
        "{CIDR_CACHE_PREFIX}:cities:{}",
        percent_encode_uri_component(&province)
    );
    let path = format!(
        "provinces/{}/cities",
        percent_encode_uri_component(&province)
    );
    let data = get_cached_or_fetch_cidr_data(state, &cache_key, &path, &[]).await?;
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
    let supports_province_wide =
        !is_municipality && !CIDR_CITY_ONLY_PROVINCES.contains(&resolved_province.as_str());

    let mut options = Vec::new();
    if supports_province_wide {
        options.push(json!({
            "label": province_wide_label(translator, &resolved_province),
            "value": CIDR_PROVINCE_WIDE_VALUE,
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
        CIDR_PROVINCE_WIDE_VALUE.to_string()
    } else {
        items
            .first()
            .and_then(|item| item.get("name"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    let total = cidr_cities_total(&data, items.len());

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

pub(super) async fn get_cidr_lookup_payload(
    state: &AppState,
    province_input: &str,
    city_input: Option<&str>,
    translator: Option<&Translator>,
) -> Result<Value, ScannerError> {
    let province = normalize_required_province(province_input)?;
    let city = city_input
        .map(normalize_string)
        .filter(|value| !value.is_empty() && value != CIDR_PROVINCE_WIDE_VALUE);
    let cache_key = cidr_cache_key(&province, city.as_deref());
    let mut query = vec![("province", province.as_str())];
    if let Some(city) = city.as_deref() {
        query.push(("city", city));
    }
    let data = get_cached_or_fetch_cidr_data(state, &cache_key, "cidrs", &query).await?;
    Ok(cidr_lookup_payload_from_data(
        &province,
        city.as_deref(),
        &data,
        translator,
    ))
}

pub(super) async fn get_cached_or_fetch_cidr_data(
    state: &AppState,
    cache_key: &str,
    path: &str,
    query: &[(&str, &str)],
) -> Result<Value, ScannerError> {
    match state.store.get_json_value(cache_key).await? {
        Some(data) => Ok(data),
        None => {
            let data = fetch_cidr_api_data(state, path, query).await?;
            state
                .store
                .set_json_value_ex(cache_key, &data, CIDR_SUCCESS_CACHE_TTL_SECONDS)
                .await?;
            Ok(data)
        }
    }
}

pub(super) async fn fetch_cidr_data(
    state: &AppState,
    province: &str,
    city: Option<&str>,
) -> Result<Value, ScannerError> {
    let mut query = vec![("province", province)];
    if let Some(city) = city {
        query.push(("city", city));
    }
    fetch_cidr_api_data(state, "cidrs", &query).await
}

pub(super) async fn fetch_cidr_api_data(
    state: &AppState,
    path: &str,
    query: &[(&str, &str)],
) -> Result<Value, ScannerError> {
    let base_url = resolve_cidr_api_base_url(state).await?;
    let clean_path = path.trim_start_matches('/');
    let mut url = Url::parse(&format!("{base_url}/{clean_path}"))
        .map_err(|error| ScannerError::Cidr(format!("Invalid CIDR API URL: {error}")))?;
    for (key, value) in query {
        if !value.trim().is_empty() {
            url.query_pairs_mut().append_pair(key, value.trim());
        }
    }

    let response = state
        .fallback_client
        .get(url.clone())
        .header(reqwest::header::USER_AGENT, CIDR_USER_AGENT)
        .send()
        .await
        .map_err(|error| ScannerError::Cidr(format!("CIDR upstream request failed: {error}")))?;
    let status = response.status();
    let raw_body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(ScannerError::Cidr(format!(
            "CIDR upstream request failed: HTTP {}",
            status.as_u16()
        )));
    }
    let payload: Value =
        serde_json::from_str(raw_body.trim_start_matches('\u{feff}')).map_err(|error| {
            ScannerError::Cidr(format!("CIDR upstream returned invalid JSON: {error}"))
        })?;
    if payload.get("code").and_then(Value::as_i64) != Some(0) {
        let message = payload
            .get("message")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("CIDR upstream returned unexpected payload");
        return Err(ScannerError::Cidr(message.to_string()));
    }
    payload
        .get("data")
        .cloned()
        .ok_or_else(|| ScannerError::Cidr("CIDR upstream response missing data".to_string()))
}

pub(super) async fn resolve_cidr_api_base_url(state: &AppState) -> Result<String, ScannerError> {
    let settings = state
        .store
        .get_json_value(IP_LOCATION_API_SETTINGS_KEY)
        .await?
        .unwrap_or_else(|| {
            json!({
                "cidr_mode": "online",
                "cidr_url": DEFAULT_CIDR_API_URL,
            })
        });
    let mode = settings
        .get("cidr_mode")
        .and_then(Value::as_str)
        .unwrap_or("online");
    let configured_url = settings
        .get("cidr_url")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let raw_url = if mode == "custom" {
        configured_url
    } else {
        DEFAULT_CIDR_API_URL
    };
    resolve_ip_location_api_base_url(raw_url)
}

pub(super) fn resolve_ip_location_api_base_url(value: &str) -> Result<String, ScannerError> {
    http_utils::normalize_api_base_url(value, "/api/v1")
        .map_err(|error| ScannerError::Cidr(format!("Invalid CIDR API URL: {error}")))
}

pub(super) fn cidr_lookup_from_data(
    province: &str,
    city: Option<&str>,
    data: &Value,
) -> ResolvedCidrLookup {
    let resolved_province = data
        .get("province")
        .and_then(Value::as_str)
        .map(normalize_string)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| province.to_string());
    let resolved_city = data
        .get("city")
        .and_then(Value::as_str)
        .map(normalize_string)
        .filter(|value| !value.is_empty())
        .or_else(|| city.map(ToString::to_string));
    let ipv4 = json_string_array(data.pointer("/cidr_groups/4"));
    let ipv6 = json_string_array(data.pointer("/cidr_groups/6"));
    let is_municipality = resolved_city
        .as_deref()
        .is_some_and(|city| city == resolved_province);
    let is_province_wide = resolved_city.is_none();

    ResolvedCidrLookup {
        selection: ScannerCidrExemptionSelection {
            province: resolved_province.clone(),
            city: resolved_city.clone(),
            label: resolved_city
                .clone()
                .unwrap_or_else(|| format!("{resolved_province}全省")),
            value: resolved_city
                .clone()
                .unwrap_or_else(|| CIDR_PROVINCE_WIDE_VALUE.to_string()),
            query_city: resolved_city,
            is_province_wide,
            is_municipality,
        },
        cidrs: normalize_scanner_cidr_exemptions_from_strings(
            ipv4.into_iter().chain(ipv6).collect::<Vec<_>>(),
        ),
    }
}

pub(super) fn cidr_lookup_payload_from_data(
    province: &str,
    city: Option<&str>,
    data: &Value,
    translator: Option<&Translator>,
) -> Value {
    let resolved_province = data
        .get("province")
        .and_then(Value::as_str)
        .map(normalize_string)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| province.to_string());
    let resolved_city = data
        .get("city")
        .and_then(Value::as_str)
        .map(normalize_string)
        .filter(|value| !value.is_empty())
        .or_else(|| city.map(ToString::to_string));
    let ipv4 = json_array_values(data.pointer("/cidr_groups/4"));
    let ipv6 = json_array_values(data.pointer("/cidr_groups/6"));
    let ipv4_count = to_safe_i64(data.pointer("/counts/4"), ipv4.len() as i64);
    let ipv6_count = to_safe_i64(data.pointer("/counts/6"), ipv6.len() as i64);
    let is_municipality = resolved_city
        .as_deref()
        .is_some_and(|city| city == resolved_province);
    let is_province_wide = resolved_city.is_none();
    let label = resolved_city
        .clone()
        .unwrap_or_else(|| province_wide_label(translator, &resolved_province));
    let value = resolved_city
        .clone()
        .unwrap_or_else(|| CIDR_PROVINCE_WIDE_VALUE.to_string());

    json!({
        "province": resolved_province,
        "city": resolved_city,
        "selection": {
            "province": resolved_province,
            "city": resolved_city,
            "label": label,
            "value": value,
            "queryCity": resolved_city,
            "isProvinceWide": is_province_wide,
            "isMunicipality": is_municipality,
        },
        "cidrGroups": {
            "ipv4": ipv4,
            "ipv6": ipv6,
        },
        "counts": {
            "ipv4": ipv4_count,
            "ipv6": ipv6_count,
        },
        "totalCount": ipv4_count + ipv6_count,
    })
}

pub(super) fn province_wide_label(translator: Option<&Translator>, province: &str) -> String {
    translator.map_or_else(
        || format!("{province}全省"),
        |translator| {
            cidr_text_params(
                translator,
                "provinceWideLabel",
                &[("province", province.to_string())],
            )
        },
    )
}

pub(super) fn cidr_cities_total(data: &Value, item_count: usize) -> i64 {
    to_safe_i64(data.get("total"), item_count as i64)
}
