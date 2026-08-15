use super::*;

pub(in crate::ddns::routes) fn dnspod_catalog_entry() -> Value {
    provider(
        "dnspod",
        "DNSPod",
        vec![
            field("token_id", "Token ID", "text", "DNSPod Token ID", true),
            field(
                "token_key",
                "Token Key",
                "password",
                "DNSPod Token Key",
                true,
            ),
            field("root_domain", "Root Domain", "text", "example.com", true),
            field("domain", "Domain", "text", "home.example.com", true),
            field("record_line", "Record Line", "text", "默认", false),
            field("ttl", "TTL", "text", "600", false),
        ],
    )
}

pub(in crate::ddns::routes) async fn update_dnspod(
    translator: &Translator,
    config: &HashMap<String, String>,
    http_options: &DDNSHttpClientOptions,
    ipv4: Option<&str>,
    ipv6: Option<&str>,
) -> anyhow::Result<DDNSProviderUpdateResult> {
    let token_id = config_value(config, "token_id");
    let token_key = config_value(config, "token_key");
    let root_domain = config_value(config, "root_domain");
    let domain = config_value(config, "domain");
    if token_id.is_empty() || token_key.is_empty() || root_domain.is_empty() || domain.is_empty() {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.dnspod.configIncomplete",
            &[],
        )));
    }
    let ttl = positive_i64(config.get("ttl"), 600).to_string();
    let record_line = default_string(config_value(config, "record_line"), "默认");
    let parsed = split_domain(translator, &domain, &root_domain)?;
    let client = ddns_http_client(translator, http_options)?;
    let query_failed = ddns_text(translator, "providers.dnspod.queryRecordFailed", &[]);
    let update_failed = ddns_text(translator, "providers.dnspod.updateRecordFailed", &[]);
    let create_failed = ddns_text(translator, "providers.dnspod.createRecordFailed", &[]);
    let provider_label_text = provider_label(Some("dnspod"), translator);

    update_dual_stack(
        translator,
        &provider_label_text,
        ipv4,
        ipv6,
        |record_type, ip| {
            let client = client.clone();
            let token_id = token_id.clone();
            let token_key = token_key.clone();
            let parsed = parsed.clone();
            let ttl = ttl.clone();
            let record_line = record_line.clone();
            let query_failed = query_failed.clone();
            let update_failed = update_failed.clone();
            let create_failed = create_failed.clone();
            async move {
                let list = dnspod_request(
                    translator,
                    &client,
                    "https://dnsapi.cn/Record.List",
                    &token_id,
                    &token_key,
                    vec![
                        ("domain", parsed.root_domain.clone()),
                        ("sub_domain", parsed.record_name.clone()),
                        ("record_type", record_type.to_string()),
                        ("record_line", record_line.clone()),
                        ("error_on_empty", "no".to_string()),
                    ],
                )
                .await?;
                let record = dnspod_record_from_list(&list, &query_failed)?;
                if let Some(record) = record {
                    if record.get("value").and_then(Value::as_str) == Some(ip.as_str()) {
                        return Ok(());
                    }
                    let record_id = record
                        .get("id")
                        .and_then(Value::as_str)
                        .ok_or_else(|| anyhow::anyhow!(update_failed.clone()))?;
                    let result = dnspod_request(
                        translator,
                        &client,
                        "https://dnsapi.cn/Record.Modify",
                        &token_id,
                        &token_key,
                        vec![
                            ("domain", parsed.root_domain.clone()),
                            ("sub_domain", parsed.record_name.clone()),
                            ("record_type", record_type.to_string()),
                            ("record_line", record_line.clone()),
                            ("record_id", record_id.to_string()),
                            ("value", ip.clone()),
                            ("ttl", ttl.clone()),
                        ],
                    )
                    .await?;
                    if result.pointer("/status/code").and_then(Value::as_str) == Some("1") {
                        return Ok(());
                    }
                    return Err(anyhow::anyhow!(
                        "{}",
                        result
                            .pointer("/status/message")
                            .and_then(Value::as_str)
                            .unwrap_or(update_failed.as_str())
                    ));
                }
                let result = dnspod_request(
                    translator,
                    &client,
                    "https://dnsapi.cn/Record.Create",
                    &token_id,
                    &token_key,
                    vec![
                        ("domain", parsed.root_domain.clone()),
                        ("sub_domain", parsed.record_name.clone()),
                        ("record_type", record_type.to_string()),
                        ("record_line", record_line.clone()),
                        ("value", ip),
                        ("ttl", ttl),
                    ],
                )
                .await?;
                if result.pointer("/status/code").and_then(Value::as_str) == Some("1") {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!(
                        "{}",
                        result
                            .pointer("/status/message")
                            .and_then(Value::as_str)
                            .unwrap_or(create_failed.as_str())
                    ))
                }
            }
        },
    )
    .await
}

pub(in crate::ddns::routes) fn dnspod_record_from_list(
    list: &Value,
    query_failed: &str,
) -> anyhow::Result<Option<Value>> {
    match list.pointer("/status/code").and_then(Value::as_str) {
        Some("1") => Ok(list
            .get("records")
            .and_then(Value::as_array)
            .and_then(|records| records.first())
            .cloned()),
        // Record.List uses code 10 for an empty result unless error_on_empty=no
        // is honored. Both responses mean that Record.Create should run.
        Some("10") => Ok(None),
        _ => Err(anyhow::anyhow!(
            "{}",
            list.pointer("/status/message")
                .and_then(Value::as_str)
                .unwrap_or(query_failed)
        )),
    }
}

pub(in crate::ddns::routes) async fn dnspod_request(
    translator: &Translator,
    client: &DDNSHttpClient,
    api: &str,
    token_id: &str,
    token_key: &str,
    params: Vec<(&str, String)>,
) -> anyhow::Result<Value> {
    let mut form = vec![
        ("login_token", format!("{token_id},{token_key}")),
        ("format", "json".to_string()),
    ];
    form.extend(params);
    let (_status, value, _text) = response_json(
        translator,
        client
            .post(api)
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body(form_body(&form))
            .send()
            .await?,
    )
    .await?;
    Ok(value)
}
