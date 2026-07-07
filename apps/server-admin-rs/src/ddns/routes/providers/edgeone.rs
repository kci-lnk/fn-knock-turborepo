use super::*;

pub(in crate::ddns::routes) async fn update_edgeone(
    translator: &Translator,
    config: &HashMap<String, String>,
    ipv4: Option<&str>,
    ipv6: Option<&str>,
) -> anyhow::Result<DDNSProviderUpdateResult> {
    let secret_id = config_value(config, "secret_id");
    let secret_key = config_value(config, "secret_key");
    let zone_id = config_value(config, "zone_id");
    let domain = normalize_domain(&config_value(config, "domain"));
    if secret_id.is_empty() || secret_key.is_empty() || zone_id.is_empty() || domain.is_empty() {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.edgeone.configIncomplete",
            &[],
        )));
    }
    let ttl = positive_i64(config.get("ttl"), 300);
    let desired_location = normalize_edgeone_location(config.get("location").map(String::as_str));
    let client = ddns_http_client()?;
    let missing_record_id = ddns_text(translator, "providers.edgeone.missingRecordId", &[]);
    let missing_created_record_id =
        ddns_text(translator, "providers.edgeone.missingCreatedRecordId", &[]);
    let provider_label_text = provider_label(Some("edgeone"), translator);
    update_dual_stack(
        translator,
        &provider_label_text,
        ipv4,
        ipv6,
        |record_type, ip| {
            let client = client.clone();
            let config = config.clone();
            let secret_id = secret_id.clone();
            let secret_key = secret_key.clone();
            let zone_id = zone_id.clone();
            let domain = domain.clone();
            let desired_location = desired_location.clone();
            let missing_record_id = missing_record_id.clone();
            let missing_created_record_id = missing_created_record_id.clone();
            async move {
                let list = edgeone_request(
                    translator,
                    &client,
                    &config,
                    &secret_id,
                    &secret_key,
                    "DescribeDnsRecords",
                    json!({
                        "ZoneId": zone_id,
                        "Offset": 0,
                        "Limit": 100,
                        "Match": "all",
                        "Filters": [{
                            "Name": "name",
                            "Values": [domain],
                            "Fuzzy": false
                        }]
                    }),
                )
                .await?;
                let existing = list
                    .get("DnsRecords")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .find(|record| {
                        normalize_domain(
                            record
                                .get("Name")
                                .and_then(Value::as_str)
                                .unwrap_or_default(),
                        ) == domain
                            && record
                                .get("Type")
                                .and_then(Value::as_str)
                                .is_some_and(|value| value.eq_ignore_ascii_case(record_type))
                            && normalize_edgeone_location(
                                record.get("Location").and_then(Value::as_str),
                            ) == desired_location
                    })
                    .cloned();
                if let Some(existing) = existing {
                    if existing.get("Content").and_then(Value::as_str) == Some(ip.as_str()) {
                        return Ok(());
                    }
                    let record_id = existing
                        .get("RecordId")
                        .and_then(Value::as_str)
                        .ok_or_else(|| anyhow::anyhow!(missing_record_id.clone()))?;
                    let mut record = json!({
                        "RecordId": record_id,
                        "Name": domain,
                        "Type": record_type,
                        "Content": ip,
                        "TTL": ttl
                    });
                    if desired_location != "default" {
                        insert_json_field(
                            &mut record,
                            "Location",
                            json!(config_value(&config, "location")),
                        );
                    }
                    edgeone_request(
                        translator,
                        &client,
                        &config,
                        &secret_id,
                        &secret_key,
                        "ModifyDnsRecords",
                        json!({ "ZoneId": zone_id, "DnsRecords": [record] }),
                    )
                    .await?;
                    return Ok(());
                }
                let mut payload = json!({
                    "ZoneId": zone_id,
                    "Name": domain,
                    "Type": record_type,
                    "Content": ip,
                    "TTL": ttl
                });
                if desired_location != "default" {
                    insert_json_field(
                        &mut payload,
                        "Location",
                        json!(config_value(&config, "location")),
                    );
                }
                let result = edgeone_request(
                    translator,
                    &client,
                    &config,
                    &secret_id,
                    &secret_key,
                    "CreateDnsRecord",
                    payload,
                )
                .await?;
                if result.get("RecordId").and_then(Value::as_str).is_some() {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!(missing_created_record_id))
                }
            }
        },
    )
    .await
}

pub(in crate::ddns::routes) async fn update_edgeone_cname(
    translator: &Translator,
    config: &HashMap<String, String>,
    ipv4: Option<&str>,
    ipv6: Option<&str>,
) -> anyhow::Result<DDNSProviderUpdateResult> {
    let secret_id = config_value(config, "secret_id");
    let secret_key = config_value(config, "secret_key");
    let zone_id = config_value(config, "zone_id");
    let domain = normalize_domain(&config_value(config, "domain"));
    if secret_id.is_empty() || secret_key.is_empty() || zone_id.is_empty() || domain.is_empty() {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.edgeone_cname.configIncomplete",
            &[],
        )));
    }
    let desired = match (ipv4, ipv6) {
        (Some(_), Some(_)) => {
            return Ok(provider_failure(ddns_text(
                translator,
                "providers.edgeone_cname.singleAddressOnly",
                &[],
            )));
        }
        (Some(value), None) => ("ipv4", value),
        (None, Some(value)) => ("ipv6", value),
        (None, None) => {
            return Ok(provider_failure(ddns_text(
                translator,
                "providers.edgeone_cname.noIpAvailable",
                &[],
            )));
        }
    };
    let client = ddns_http_client()?;
    let list = edgeone_request(
        translator,
        &client,
        config,
        &secret_id,
        &secret_key,
        "DescribeAccelerationDomains",
        json!({
            "ZoneId": zone_id,
            "Offset": 0,
            "Limit": 20,
            "Match": "all",
            "Filters": [{
                "Name": "domain-name",
                "Values": [domain],
                "Fuzzy": false
            }]
        }),
    )
    .await?;
    let existing = list
        .get("AccelerationDomains")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|item| {
            normalize_domain(
                item.get("DomainName")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            ) == domain
        })
        .cloned();
    let Some(existing) = existing else {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.edgeone_cname.domainNotFound",
            &[("domain", domain.clone())],
        )));
    };
    let origin_detail = existing.get("OriginDetail").unwrap_or(&Value::Null);
    let origin_type = origin_detail
        .get("OriginType")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_ascii_uppercase();
    if !origin_type.is_empty() && origin_type != "IP_DOMAIN" {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.edgeone_cname.unsupportedOriginType",
            &[("originType", origin_type)],
        )));
    }
    let current_origin = origin_detail
        .get("Origin")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if current_origin == desired.1 {
        return Ok(DDNSProviderUpdateResult {
            success: true,
            message: ddns_text(translator, "providers.edgeone_cname.originUnchanged", &[]),
            ipv4_updated: desired.0 == "ipv4",
            ipv6_updated: desired.0 == "ipv6",
        });
    }
    let raw_host_header = origin_detail
        .get("HostHeader")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let mut origin_info = json!({
        "OriginType": "IP_DOMAIN",
        "Origin": desired.1
    });
    if let Some(host_header) = raw_host_header
        && is_valid_edgeone_host_header(host_header)
    {
        insert_json_field(
            &mut origin_info,
            "HostHeader",
            json!(normalize_domain(host_header)),
        );
    }
    edgeone_request(
        translator,
        &client,
        config,
        &secret_id,
        &secret_key,
        "ModifyAccelerationDomain",
        json!({
            "ZoneId": zone_id,
            "DomainName": domain,
            "OriginInfo": origin_info
        }),
    )
    .await?;
    Ok(DDNSProviderUpdateResult {
        success: true,
        message: ddns_text(translator, "providers.edgeone_cname.success", &[]),
        ipv4_updated: desired.0 == "ipv4",
        ipv6_updated: desired.0 == "ipv6",
    })
}
