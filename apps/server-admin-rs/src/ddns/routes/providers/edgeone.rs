use super::*;

pub(in crate::ddns::routes) fn edgeone_catalog_entry() -> Value {
    provider(
        "edgeone",
        "Tencent EdgeOne",
        vec![
            field("secret_id", "SecretId", "text", "AKID...", true),
            field("secret_key", "SecretKey", "password", "SecretKey", true),
            field("zone_id", "Zone ID", "text", "zone-xxxxxxxx", true),
            field("domain", "Domain", "text", "home.example.com", true),
            field("location", "Location", "text", "", false),
            field("ttl", "TTL", "text", "300", false),
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
    )
}

pub(in crate::ddns::routes) async fn update_edgeone(
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
            "providers.edgeone.configIncomplete",
            &[],
        )));
    }
    let ttl = positive_i64(config.get("ttl"), 300);
    let desired_location = normalize_edgeone_location(config.get("location").map(String::as_str));
    let client = ddns_http_client(translator, http_options)?;
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
                        .filter(|value| json_value_js_truthy(Some(value)))
                        .cloned()
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
                if json_value_js_truthy(result.get("RecordId")) {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!(missing_created_record_id))
                }
            }
        },
    )
    .await
}
