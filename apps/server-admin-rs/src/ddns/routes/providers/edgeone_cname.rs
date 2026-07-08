use super::*;

pub(in crate::ddns::routes) fn edgeone_cname_catalog_entry() -> Value {
    let mut value = provider(
        "edgeone_cname",
        "EdgeOne CNAME",
        vec![
            field("secret_id", "SecretId", "text", "AKID...", true),
            field("secret_key", "SecretKey", "password", "SecretKey", true),
            field("zone_id", "Zone ID", "text", "zone-xxxxxxxx", true),
            field("domain", "Domain", "text", "home.example.com", true),
            select_field(
                DDNS_EDGEONE_OVERSEAS_ACCESS_FIELD,
                "Overseas access control",
                false,
                vec![("Off", "off"), ("Block overseas IPs", "block_overseas")],
            ),
            field(
                "endpoint",
                "API Endpoint",
                "text",
                "https://teo.tencentcloudapi.com",
                false,
            ),
            field("region", "Region", "text", "", false),
        ],
    );
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "capabilities".to_string(),
            json!({ "addressMode": "single_address" }),
        );
    }
    value
}

pub(in crate::ddns::routes) async fn update_edgeone_cname(
    translator: &Translator,
    config: &HashMap<String, String>,
    http_options: &DDNSHttpClientOptions,
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
    let client = ddns_http_client(translator, http_options)?;
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
        });
    }
    let raw_host_header = origin_detail
        .get("HostHeader")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let host_header = raw_host_header
        .as_deref()
        .filter(|value| is_valid_edgeone_host_header(value))
        .map(normalize_domain);
    let ignored_invalid_host_header = raw_host_header.is_some() && host_header.is_none();
    let modify_result = edgeone_request(
        translator,
        &client,
        config,
        &secret_id,
        &secret_key,
        "ModifyAccelerationDomain",
        json!({
            "ZoneId": zone_id,
            "DomainName": domain,
            "OriginInfo": edgeone_cname_origin_info(desired.1, host_header.as_deref())
        }),
    )
    .await;
    if let Err(error) = modify_result {
        if host_header.is_none() || !is_edgeone_host_header_format_error(&error) {
            return Err(error);
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
                "OriginInfo": edgeone_cname_origin_info(desired.1, None)
            }),
        )
        .await?;
    }
    Ok(DDNSProviderUpdateResult {
        success: true,
        message: ddns_text(
            translator,
            if ignored_invalid_host_header {
                "providers.edgeone_cname.successWithInvalidHostHeaderIgnored"
            } else {
                "providers.edgeone_cname.success"
            },
            &[],
        ),
    })
}

pub(in crate::ddns::routes) fn edgeone_cname_origin_info(
    origin: &str,
    host_header: Option<&str>,
) -> Value {
    let mut value = json!({
        "OriginType": "IP_DOMAIN",
        "Origin": origin
    });
    if let Some(host_header) = host_header.filter(|value| !value.is_empty()) {
        insert_json_field(&mut value, "HostHeader", json!(host_header));
    }
    value
}

pub(in crate::ddns::routes) fn is_edgeone_host_header_format_error(error: &anyhow::Error) -> bool {
    let message = error.to_string();
    message.contains("InvalidHostHeaderFormat") || message.contains("HostHeaderInvalid")
}
