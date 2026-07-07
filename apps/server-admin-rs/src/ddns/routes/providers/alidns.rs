use super::*;

pub(in crate::ddns::routes) async fn update_alidns(
    translator: &Translator,
    config: &HashMap<String, String>,
    http_options: &DDNSHttpClientOptions,
    ipv4: Option<&str>,
    ipv6: Option<&str>,
) -> anyhow::Result<DDNSProviderUpdateResult> {
    let access_key_id = config_value(config, "access_key_id");
    let access_key_secret = config_value(config, "access_key_secret");
    let root_domain = config_value(config, "root_domain");
    let domain = config_value(config, "domain");
    if access_key_id.is_empty()
        || access_key_secret.is_empty()
        || root_domain.is_empty()
        || domain.is_empty()
    {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.alidns.configIncomplete",
            &[],
        )));
    }
    let ttl = positive_i64(config.get("ttl"), 600).to_string();
    let line = default_string(config_value(config, "line"), "default");
    let parsed = split_domain(translator, &domain, &root_domain)?;
    let client = ddns_http_client(translator, http_options)?;
    let request_failed = ddns_text(translator, "providers.alidns.requestFailed", &[]);
    let update_failed = ddns_text(translator, "providers.alidns.updateFailed", &[]);
    let create_failed = ddns_text(translator, "providers.alidns.createFailed", &[]);
    let provider_label_text = provider_label(Some("alidns"), translator);

    update_dual_stack(
        translator,
        &provider_label_text,
        ipv4,
        ipv6,
        |record_type, ip| {
            let client = client.clone();
            let access_key_id = access_key_id.clone();
            let access_key_secret = access_key_secret.clone();
            let parsed = parsed.clone();
            let ttl = ttl.clone();
            let line = line.clone();
            let request_failed = request_failed.clone();
            let update_failed = update_failed.clone();
            let create_failed = create_failed.clone();
            async move {
                let records = alidns_request(
                    translator,
                    &client,
                    &access_key_id,
                    &access_key_secret,
                    vec![
                        ("Action", "DescribeSubDomainRecords".to_string()),
                        ("DomainName", parsed.root_domain.clone()),
                        ("Line", line.clone()),
                        ("PageSize", "100".to_string()),
                        ("SubDomain", parsed.fqdn.clone()),
                        ("Type", record_type.to_string()),
                    ],
                )
                .await?;
                if let Some(code) = json_text(&records, "Code") {
                    return Err(anyhow::anyhow!(
                        "{code}: {}",
                        json_text(&records, "Message").unwrap_or_else(|| request_failed.clone())
                    ));
                }
                let existing = records
                    .pointer("/DomainRecords/Record")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter(|record| {
                        record
                            .get("RR")
                            .and_then(Value::as_str)
                            .unwrap_or(&parsed.record_name)
                            == parsed.record_name
                            && record
                                .get("Type")
                                .and_then(Value::as_str)
                                .unwrap_or(record_type)
                                == record_type
                            && record
                                .get("Line")
                                .and_then(Value::as_str)
                                .unwrap_or("default")
                                == line
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                if !existing.is_empty() {
                    for record in existing {
                        if record.get("Value").and_then(Value::as_str) == Some(ip.as_str()) {
                            continue;
                        }
                        let record_id = record
                            .get("RecordId")
                            .filter(|value| json_value_js_truthy(Some(value)))
                            .map(value_to_compact_string)
                            .ok_or_else(|| {
                                anyhow::anyhow!(ddns_text(
                                    translator,
                                    "providers.alidns.recordIdMissing",
                                    &[],
                                ))
                            })?;
                        let result = alidns_request(
                            translator,
                            &client,
                            &access_key_id,
                            &access_key_secret,
                            vec![
                                ("Action", "UpdateDomainRecord".to_string()),
                                ("Line", line.clone()),
                                ("RR", parsed.record_name.clone()),
                                ("RecordId", record_id.to_string()),
                                ("TTL", ttl.clone()),
                                ("Type", record_type.to_string()),
                                ("Value", ip.clone()),
                            ],
                        )
                        .await?;
                        if alidns_change_response_failed(&result) {
                            return Err(anyhow::anyhow!(
                                "{}: {}",
                                json_text(&result, "Code").unwrap_or_else(|| update_failed.clone()),
                                json_text(&result, "Message")
                                    .unwrap_or_else(|| update_failed.clone())
                            ));
                        }
                    }
                    return Ok(());
                }
                let result = alidns_request(
                    translator,
                    &client,
                    &access_key_id,
                    &access_key_secret,
                    vec![
                        ("Action", "AddDomainRecord".to_string()),
                        ("DomainName", parsed.root_domain.clone()),
                        ("Line", line.clone()),
                        ("RR", parsed.record_name.clone()),
                        ("TTL", ttl.clone()),
                        ("Type", record_type.to_string()),
                        ("Value", ip),
                    ],
                )
                .await?;
                if alidns_change_response_failed(&result) {
                    Err(anyhow::anyhow!(
                        "{}: {}",
                        json_text(&result, "Code").unwrap_or_else(|| create_failed.clone()),
                        json_text(&result, "Message").unwrap_or_else(|| create_failed.clone())
                    ))
                } else {
                    Ok(())
                }
            }
        },
    )
    .await
}

pub(in crate::ddns::routes) fn alidns_change_response_failed(result: &Value) -> bool {
    json_value_js_truthy(result.get("Code")) || !json_value_js_truthy(result.get("RecordId"))
}
