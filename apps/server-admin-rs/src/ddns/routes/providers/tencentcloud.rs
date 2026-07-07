use super::*;

pub(in crate::ddns::routes) async fn update_tencentcloud(
    translator: &Translator,
    config: &HashMap<String, String>,
    ipv4: Option<&str>,
    ipv6: Option<&str>,
) -> anyhow::Result<DDNSProviderUpdateResult> {
    let secret_id = config_value(config, "secret_id");
    let secret_key = config_value(config, "secret_key");
    let root_domain = config_value(config, "root_domain");
    let domain = config_value(config, "domain");
    if secret_id.is_empty() || secret_key.is_empty() || root_domain.is_empty() || domain.is_empty()
    {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.tencentcloud.configIncomplete",
            &[],
        )));
    }
    let ttl = positive_i64(config.get("ttl"), 600);
    let record_line = default_string(config_value(config, "record_line"), "默认");
    let record_line_id = config_value(config, "record_line_id");
    let parsed = split_domain(translator, &domain, &root_domain)?;
    let client = ddns_http_client()?;
    let missing_updated_record_id = ddns_text(
        translator,
        "providers.tencentcloud.missingUpdatedRecordId",
        &[],
    );
    let missing_created_record_id = ddns_text(
        translator,
        "providers.tencentcloud.missingCreatedRecordId",
        &[],
    );

    let provider_label_text = provider_label(Some("tencentcloud"), translator);
    update_dual_stack(
        translator,
        &provider_label_text,
        ipv4,
        ipv6,
        |record_type, ip| {
            let client = client.clone();
            let secret_id = secret_id.clone();
            let secret_key = secret_key.clone();
            let parsed = parsed.clone();
            let record_line = record_line.clone();
            let record_line_id = record_line_id.clone();
            let missing_updated_record_id = missing_updated_record_id.clone();
            let missing_created_record_id = missing_created_record_id.clone();
            async move {
                let mut base_payload = serde_json::Map::new();
                base_payload.insert("Domain".to_string(), json!(parsed.root_domain));
                base_payload.insert("RecordType".to_string(), json!(record_type));
                if record_line_id.is_empty() {
                    base_payload.insert("RecordLine".to_string(), json!(record_line));
                } else {
                    base_payload.insert("RecordLineId".to_string(), json!(record_line_id));
                }

                let mut list_payload = base_payload.clone();
                list_payload.insert("Limit".to_string(), json!(100));
                list_payload.insert("Offset".to_string(), json!(0));
                list_payload.insert("Subdomain".to_string(), json!(parsed.record_name));
                let list = match tencentcloud_request(
                    translator,
                    &client,
                    &secret_id,
                    &secret_key,
                    "DescribeRecordList",
                    Value::Object(list_payload),
                )
                .await
                {
                    Ok(value) => value,
                    Err(error)
                        if error
                            .to_string()
                            .starts_with("ResourceNotFound.NoDataOfRecord:") =>
                    {
                        json!({ "RecordList": [] })
                    }
                    Err(error) => return Err(error),
                };
                let existing = list
                    .get("RecordList")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .find(|record| {
                        record
                            .get("Name")
                            .and_then(Value::as_str)
                            .unwrap_or(&parsed.record_name)
                            == parsed.record_name
                            && record
                                .get("Type")
                                .and_then(Value::as_str)
                                .unwrap_or(record_type)
                                == record_type
                            && if record_line_id.is_empty() {
                                record.get("Line").and_then(Value::as_str).unwrap_or("默认")
                                    == record_line
                            } else {
                                record
                                    .get("LineId")
                                    .and_then(Value::as_str)
                                    .unwrap_or_default()
                                    == record_line_id
                            }
                    })
                    .cloned();
                if let Some(record) = existing {
                    if record.get("Value").and_then(Value::as_str) == Some(ip.as_str()) {
                        return Ok(());
                    }
                    let record_id = record
                        .get("RecordId")
                        .and_then(Value::as_i64)
                        .ok_or_else(|| anyhow::anyhow!(missing_updated_record_id.clone()))?;
                    let mut payload = base_payload;
                    payload.insert("RecordId".to_string(), json!(record_id));
                    payload.insert("SubDomain".to_string(), json!(parsed.record_name));
                    payload.insert("TTL".to_string(), json!(ttl));
                    payload.insert("Value".to_string(), json!(ip));
                    let result = tencentcloud_request(
                        translator,
                        &client,
                        &secret_id,
                        &secret_key,
                        "ModifyRecord",
                        Value::Object(payload),
                    )
                    .await?;
                    if result.get("RecordId").and_then(Value::as_i64).is_some() {
                        return Ok(());
                    }
                    return Err(anyhow::anyhow!(missing_updated_record_id));
                }

                let mut payload = base_payload;
                payload.insert("SubDomain".to_string(), json!(parsed.record_name));
                payload.insert("TTL".to_string(), json!(ttl));
                payload.insert("Value".to_string(), json!(ip));
                let result = tencentcloud_request(
                    translator,
                    &client,
                    &secret_id,
                    &secret_key,
                    "CreateRecord",
                    Value::Object(payload),
                )
                .await?;
                if result.get("RecordId").and_then(Value::as_i64).is_some() {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!(missing_created_record_id))
                }
            }
        },
    )
    .await
}
