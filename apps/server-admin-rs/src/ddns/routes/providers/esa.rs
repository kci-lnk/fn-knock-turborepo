use super::*;

pub(in crate::ddns::routes) fn esa_catalog_entry() -> Value {
    provider(
        "esa",
        "阿里云 ESA",
        vec![
            field("access_key_id", "AccessKey ID", "text", "LTAI...", true),
            field(
                "access_key_secret",
                "AccessKey Secret",
                "password",
                "AccessKey Secret",
                true,
            ),
            field("site_name", "Site Name", "text", "example.com", true),
            field("site_id", "Site ID", "text", "123456", false),
            field("domain", "Domain", "text", "home.example.com", true),
            select_field(
                "proxied",
                "Proxied",
                false,
                vec![("DNS only", "false"), ("Enabled", "true")],
            ),
            select_field(
                "biz_name",
                "Business",
                false,
                vec![
                    ("Web", "web"),
                    ("API", "api"),
                    ("Image/Video", "image_video"),
                ],
            ),
            field("ttl", "TTL", "text", "30", false),
        ],
    )
}

pub(in crate::ddns::routes) async fn update_esa(
    translator: &Translator,
    config: &HashMap<String, String>,
    http_options: &DDNSHttpClientOptions,
    ipv4: Option<&str>,
    ipv6: Option<&str>,
) -> anyhow::Result<DDNSProviderUpdateResult> {
    let access_key_id = config_value(config, "access_key_id");
    let access_key_secret = config_value(config, "access_key_secret");
    let site_name = normalize_domain(&config_value(config, "site_name"));
    let site_id = config_value(config, "site_id");
    let domain = normalize_domain(&config_value(config, "domain"));
    if access_key_id.is_empty()
        || access_key_secret.is_empty()
        || domain.is_empty()
        || (site_name.is_empty() && site_id.is_empty())
    {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.esa.configIncomplete",
            &[],
        )));
    }
    let ttl = positive_i64(config.get("ttl"), 30);
    let proxied = config_value(config, "proxied") == "true";
    let biz_name = if proxied {
        default_string(config_value(config, "biz_name"), "web")
    } else {
        String::new()
    };
    let record_value = [ipv4, ipv6]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(",");
    if record_value.is_empty() {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.esa.noIpAvailable",
            &[],
        )));
    }
    let client = ddns_http_client(translator, http_options)?;
    let site_id = if !site_id.is_empty() {
        site_id
    } else {
        let sites = aliyun_acs3_request(
            translator,
            &client,
            &access_key_id,
            &access_key_secret,
            "ListSites",
            "2024-09-10",
            "GET",
            vec![
                ("PageNumber".to_string(), "1".to_string()),
                ("PageSize".to_string(), "100".to_string()),
                ("SiteName".to_string(), site_name.clone()),
                ("SiteSearchType".to_string(), "exact".to_string()),
            ],
            Vec::new(),
        )
        .await?;
        sites
            .get("Sites")
            .and_then(Value::as_array)
            .and_then(|sites| {
                sites.iter().find_map(|site| {
                    (normalize_domain(
                        site.get("SiteName")
                            .and_then(Value::as_str)
                            .unwrap_or_default(),
                    ) == site_name)
                        .then(|| {
                            site.get("SiteId")
                                .filter(|value| json_value_js_truthy(Some(value)))
                                .map(value_to_compact_string)
                        })
                        .flatten()
                })
            })
            .ok_or_else(|| {
                anyhow::anyhow!(ddns_text(
                    translator,
                    "providers.esa.siteNotFound",
                    &[("site", site_name.clone())],
                ))
            })?
    };
    let records = aliyun_acs3_request(
        translator,
        &client,
        &access_key_id,
        &access_key_secret,
        "ListRecords",
        "2024-09-10",
        "GET",
        vec![
            ("PageNumber".to_string(), "1".to_string()),
            ("PageSize".to_string(), "100".to_string()),
            ("RecordMatchType".to_string(), "exact".to_string()),
            ("RecordName".to_string(), domain.clone()),
            ("SiteId".to_string(), site_id.clone()),
            ("Type".to_string(), "A/AAAA".to_string()),
        ],
        Vec::new(),
    )
    .await?;
    let existing = records
        .get("Records")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|record| {
            normalize_domain(
                record
                    .get("RecordName")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            ) == domain
                && record
                    .get("RecordType")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .eq_ignore_ascii_case("A/AAAA")
        })
        .cloned()
        .collect::<Vec<_>>();
    let mut payload = esa_record_payload(&record_value, ttl, proxied, &biz_name);
    if existing.is_empty() {
        payload.push(("RecordName".to_string(), domain));
        payload.push(("SiteId".to_string(), site_id));
        let result = aliyun_acs3_request(
            translator,
            &client,
            &access_key_id,
            &access_key_secret,
            "CreateRecord",
            "2024-09-10",
            "POST",
            payload,
            Vec::new(),
        )
        .await?;
        if json_value_js_truthy(result.get("RecordId")) {
            return Ok(DDNSProviderUpdateResult {
                success: true,
                message: ddns_text(translator, "providers.esa.success", &[]),
            });
        }
        return Err(anyhow::anyhow!(ddns_text(
            translator,
            "providers.esa.createRecordFailed",
            &[],
        )));
    }
    for record in existing {
        let current_value = record
            .pointer("/Data/Value")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let current_ttl = record.get("Ttl").and_then(Value::as_i64).unwrap_or(ttl);
        let current_proxied = record
            .get("Proxied")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let current_biz_name = record
            .get("BizName")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if same_csv_values(current_value, &record_value)
            && current_ttl == ttl
            && current_proxied == proxied
            && current_biz_name == biz_name
        {
            continue;
        }
        let record_id = record
            .get("RecordId")
            .filter(|value| json_value_js_truthy(Some(value)))
            .map(value_to_compact_string)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                anyhow::anyhow!(ddns_text(translator, "providers.esa.recordIdMissing", &[],))
            })?;
        let mut update_payload = esa_record_payload(&record_value, ttl, proxied, &biz_name);
        update_payload.push(("RecordId".to_string(), record_id));
        aliyun_acs3_request(
            translator,
            &client,
            &access_key_id,
            &access_key_secret,
            "UpdateRecord",
            "2024-09-10",
            "POST",
            update_payload,
            Vec::new(),
        )
        .await?;
    }
    Ok(DDNSProviderUpdateResult {
        success: true,
        message: ddns_text(translator, "providers.esa.success", &[]),
    })
}

#[allow(clippy::too_many_arguments)]
pub(in crate::ddns::routes) async fn aliyun_acs3_request(
    translator: &Translator,
    client: &DDNSHttpClient,
    access_key_id: &str,
    access_key_secret: &str,
    action: &str,
    version: &str,
    method: &str,
    query: Vec<(String, String)>,
    form_data: Vec<(String, String)>,
) -> anyhow::Result<Value> {
    let endpoint = "https://esa.cn-hangzhou.aliyuncs.com/";
    let url = url::Url::parse(endpoint)?;
    let query_string = aliyun_canonical_param_string(&query);
    let body_string = aliyun_canonical_param_string(&form_data);
    let payload_hash = sha256_hex(&body_string);
    let acs_date = iso8601_utc_without_millis();
    let nonce = uuid::Uuid::new_v4().to_string();
    let mut headers = vec![
        (
            "host".to_string(),
            url.host_str().unwrap_or_default().to_string(),
        ),
        ("x-acs-action".to_string(), action.to_string()),
        ("x-acs-content-sha256".to_string(), payload_hash.clone()),
        ("x-acs-date".to_string(), acs_date.clone()),
        ("x-acs-signature-nonce".to_string(), nonce),
        ("x-acs-version".to_string(), version.to_string()),
    ];
    if !body_string.is_empty() {
        headers.push((
            "content-type".to_string(),
            "application/x-www-form-urlencoded".to_string(),
        ));
    }
    headers.sort_by(|left, right| left.0.cmp(&right.0));
    let canonical_headers = format!(
        "{}\n",
        headers
            .iter()
            .map(|(key, value)| format!("{key}:{}", value.trim()))
            .collect::<Vec<_>>()
            .join("\n")
    );
    let signed_headers = headers
        .iter()
        .map(|(key, _)| key.as_str())
        .collect::<Vec<_>>()
        .join(";");
    let canonical_request = [
        method,
        url.path(),
        &query_string,
        &canonical_headers,
        &signed_headers,
        &payload_hash,
    ]
    .join("\n");
    let string_to_sign = format!("ACS3-HMAC-SHA256\n{}", sha256_hex(&canonical_request));
    let signature = hmac_sha256_hex(access_key_secret.as_bytes(), string_to_sign.as_bytes());
    let authorization = format!(
        "ACS3-HMAC-SHA256 Credential={access_key_id},SignedHeaders={signed_headers},Signature={signature}"
    );
    let request_url = if query_string.is_empty() {
        endpoint.to_string()
    } else {
        format!("{endpoint}?{query_string}")
    };
    let method = reqwest::Method::from_bytes(method.as_bytes())?;
    let mut request = client
        .request(method, request_url)
        .header(reqwest::header::HOST, url.host_str().unwrap_or_default())
        .header("x-acs-action", action)
        .header("x-acs-content-sha256", payload_hash)
        .header("x-acs-date", acs_date)
        .header(
            "x-acs-signature-nonce",
            headers
                .iter()
                .find(|(key, _)| key == "x-acs-signature-nonce")
                .map(|(_, value)| value.as_str())
                .unwrap_or_default(),
        )
        .header("x-acs-version", version)
        .header(reqwest::header::AUTHORIZATION, authorization);
    if !body_string.is_empty() {
        request = request
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body(body_string);
    }
    let (status, data, text) = response_json(translator, request.send().await?).await?;
    if !status.is_success() || data.get("Code").is_some() {
        return Err(anyhow::anyhow!(
            "{}: {}",
            data.get("Code")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| format!("HTTP {}", status.as_u16())),
            data.get("Message")
                .and_then(Value::as_str)
                .unwrap_or(if text.is_empty() {
                    "Aliyun ACS3 request failed"
                } else {
                    &text
                })
        ));
    }
    Ok(data)
}

pub(in crate::ddns::routes) fn aliyun_canonical_param_string(
    params: &[(String, String)],
) -> String {
    let mut values = params.to_vec();
    values.sort_by(|left, right| {
        let key_order = left.0.cmp(&right.0);
        if key_order == std::cmp::Ordering::Equal {
            left.1.cmp(&right.1)
        } else {
            key_order
        }
    });
    values
        .into_iter()
        .map(|(key, value)| format!("{}={}", rfc3986_encode(&key), rfc3986_encode(&value)))
        .collect::<Vec<_>>()
        .join("&")
}

pub(in crate::ddns::routes) fn esa_record_payload(
    value: &str,
    ttl: i64,
    proxied: bool,
    biz_name: &str,
) -> Vec<(String, String)> {
    let mut payload = vec![
        ("Data".to_string(), json!({ "Value": value }).to_string()),
        ("Proxied".to_string(), proxied.to_string()),
        ("Ttl".to_string(), ttl.to_string()),
        ("Type".to_string(), "A/AAAA".to_string()),
    ];
    if proxied {
        payload.push((
            "BizName".to_string(),
            default_string(biz_name.to_string(), "web"),
        ));
    }
    payload
}

pub(in crate::ddns::routes) fn same_csv_values(left: &str, right: &str) -> bool {
    let mut left_values = left
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    let mut right_values = right
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    left_values.sort_unstable();
    right_values.sort_unstable();
    left_values == right_values
}
