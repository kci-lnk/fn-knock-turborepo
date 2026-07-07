use super::*;

pub(in crate::ddns::routes) async fn update_esa(
    translator: &Translator,
    config: &HashMap<String, String>,
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
    let client = ddns_http_client()?;
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
                        .then(|| site.get("SiteId").map(value_to_compact_string))
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
        if result.get("RecordId").is_some() {
            return Ok(DDNSProviderUpdateResult {
                success: true,
                message: ddns_text(translator, "providers.esa.success", &[]),
                ipv4_updated: ipv4.is_some(),
                ipv6_updated: ipv6.is_some(),
            });
        }
        return Ok(provider_failure(ddns_text(
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
        ipv4_updated: ipv4.is_some(),
        ipv6_updated: ipv6.is_some(),
    })
}

pub(in crate::ddns::routes) async fn update_dynv6(
    translator: &Translator,
    config: &HashMap<String, String>,
    ipv4: Option<&str>,
    ipv6: Option<&str>,
) -> anyhow::Result<DDNSProviderUpdateResult> {
    let token = config_value(config, "token");
    let zone = config_value(config, "zone");
    if token.is_empty() || zone.is_empty() {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.dynv6.configIncomplete",
            &[],
        )));
    }
    if ipv4.is_none() && ipv6.is_none() && config_value(config, "ipv6prefix").is_empty() {
        return Ok(provider_failure(ddns_text(
            translator,
            "dualStackUnavailable",
            &[],
        )));
    }
    let mut query = vec![("hostname", zone), ("token", token)];
    if let Some(ipv4) = ipv4 {
        query.push(("ipv4", ipv4.to_string()));
    }
    if let Some(ipv6) = ipv6 {
        query.push(("ipv6", ipv6.to_string()));
    }
    let ipv6prefix = config_value(config, "ipv6prefix");
    if !ipv6prefix.is_empty() {
        query.push(("ipv6prefix", ipv6prefix));
    }
    let client = ddns_http_client()?;
    let response = client
        .get(build_query_url("https://dynv6.com/api/update", &query))
        .send()
        .await?;
    let status = response.status();
    let text = response_text(response).await?;
    if status.is_success() && (text.contains("updated") || text.contains("unchanged")) {
        Ok(DDNSProviderUpdateResult {
            success: true,
            message: ddns_text(
                translator,
                "providers.dynv6.success",
                &[
                    ("detail", text),
                    ("params", dynv6_sent_params(translator, ipv4, ipv6, config)),
                ],
            ),
            ipv4_updated: ipv4.is_some(),
            ipv6_updated: ipv6.is_some(),
        })
    } else {
        Ok(provider_failure(ddns_text(
            translator,
            "providers.dynv6.updateFailed",
            &[("status", status.as_u16().to_string()), ("detail", text)],
        )))
    }
}

pub(in crate::ddns::routes) fn dynv6_sent_params(
    translator: &Translator,
    ipv4: Option<&str>,
    ipv6: Option<&str>,
    config: &HashMap<String, String>,
) -> String {
    let empty = ddns_text(translator, "providers.dynv6.empty", &[]);
    let mut parts = vec![
        format!("ipv4={}", ipv4.unwrap_or(empty.as_str())),
        format!("ipv6={}", ipv6.unwrap_or(empty.as_str())),
    ];
    let ipv6prefix = config_value(config, "ipv6prefix");
    if !ipv6prefix.is_empty() {
        parts.push(format!("ipv6prefix={ipv6prefix}"));
    }
    parts.join(", ")
}
