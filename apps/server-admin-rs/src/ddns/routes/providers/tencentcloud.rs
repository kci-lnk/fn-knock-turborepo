use super::*;

pub(in crate::ddns::routes) fn tencentcloud_catalog_entry() -> Value {
    provider(
        "tencentcloud",
        "腾讯云 DNSPod",
        vec![
            field("secret_id", "SecretId", "text", "AKID...", true),
            field("secret_key", "SecretKey", "password", "SecretKey", true),
            field("root_domain", "Root Domain", "text", "example.com", true),
            field("domain", "Domain", "text", "home.example.com", true),
            field("record_line", "Record Line", "text", "默认", false),
            field("record_line_id", "Record Line ID", "text", "0", false),
            field("ttl", "TTL", "text", "600", false),
        ],
    )
}

pub(in crate::ddns::routes) async fn update_tencentcloud(
    translator: &Translator,
    config: &HashMap<String, String>,
    http_options: &DDNSHttpClientOptions,
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
    let client = ddns_http_client(translator, http_options)?;
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
                        .filter(|value| json_value_js_truthy(Some(value)))
                        .cloned()
                        .ok_or_else(|| anyhow::anyhow!(missing_updated_record_id.clone()))?;
                    let mut payload = base_payload;
                    payload.insert("RecordId".to_string(), record_id);
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
                    if json_value_js_truthy(result.get("RecordId")) {
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

pub(in crate::ddns::routes) async fn tencentcloud_request(
    translator: &Translator,
    client: &DDNSHttpClient,
    secret_id: &str,
    secret_key: &str,
    action: &str,
    payload: Value,
) -> anyhow::Result<Value> {
    const HOST: &str = "dnspod.tencentcloudapi.com";
    const SERVICE: &str = "dnspod";
    const VERSION: &str = "2021-03-23";

    let timestamp = time_utils::now_ms().div_euclid(1000);
    let date = utc_date(timestamp)?;
    let payload_string = serde_json::to_string(&payload)?;
    let hashed_payload = sha256_hex(&payload_string);
    let content_type = "application/json; charset=utf-8";
    let canonical_headers = tencentcloud_tc3_canonical_headers(content_type, HOST, action);
    let signed_headers = "content-type;host;x-tc-action";
    let canonical_request = [
        "POST",
        "/",
        "",
        &canonical_headers,
        signed_headers,
        &hashed_payload,
    ]
    .join("\n");
    let credential_scope = format!("{date}/{SERVICE}/tc3_request");
    let string_to_sign = [
        "TC3-HMAC-SHA256",
        &timestamp.to_string(),
        &credential_scope,
        &sha256_hex(&canonical_request),
    ]
    .join("\n");
    let secret_date = hmac_sha256_bytes(format!("TC3{secret_key}").as_bytes(), date.as_bytes());
    let secret_service = hmac_sha256_bytes(&secret_date, SERVICE.as_bytes());
    let secret_signing = hmac_sha256_bytes(&secret_service, b"tc3_request");
    let signature = hmac_sha256_hex(&secret_signing, string_to_sign.as_bytes());
    let authorization = format!(
        "TC3-HMAC-SHA256 Credential={secret_id}/{credential_scope}, SignedHeaders={signed_headers}, Signature={signature}"
    );

    let (status, data, _text) = response_json(
        translator,
        client
            .post(format!("https://{HOST}/"))
            .header(reqwest::header::CONTENT_TYPE, content_type)
            .header(reqwest::header::HOST, HOST)
            .header("X-TC-Action", action)
            .header("X-TC-Timestamp", timestamp.to_string())
            .header("X-TC-Version", VERSION)
            .header(reqwest::header::AUTHORIZATION, authorization)
            .body(payload_string)
            .send()
            .await?,
    )
    .await?;
    let response = data.get("Response").cloned().ok_or_else(|| {
        anyhow::anyhow!(ddns_text(
            translator,
            "tencentMissingResponse",
            &[("status", status.as_u16().to_string())],
        ))
    })?;
    if let Some(error) = response.get("Error") {
        let code = error
            .get("Code")
            .and_then(Value::as_str)
            .unwrap_or("TencentCloudError");
        let request_failed = ddns_text(translator, "requestFailed", &[]);
        let message = error
            .get("Message")
            .and_then(Value::as_str)
            .unwrap_or(request_failed.as_str());
        let request_id = response
            .get("RequestId")
            .and_then(Value::as_str)
            .unwrap_or_default();
        return Err(anyhow::anyhow!(
            "{code}: {message}{}",
            if request_id.is_empty() {
                String::new()
            } else {
                format!(" (RequestId: {request_id})")
            }
        ));
    }
    if status.is_success() {
        Ok(response)
    } else {
        Err(anyhow::anyhow!(
            "HTTP {}: {}",
            status.as_u16(),
            ddns_text(translator, "requestFailed", &[])
        ))
    }
}
