use super::*;

pub(in crate::ddns::routes) async fn update_dnspod(
    translator: &Translator,
    config: &HashMap<String, String>,
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
    let client = ddns_http_client()?;
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
                    ],
                )
                .await?;
                if list.pointer("/status/code").and_then(Value::as_str) != Some("1") {
                    return Err(anyhow::anyhow!(
                        "{}",
                        list.pointer("/status/message")
                            .and_then(Value::as_str)
                            .unwrap_or(query_failed.as_str())
                    ));
                }
                let record = list
                    .get("records")
                    .and_then(Value::as_array)
                    .and_then(|records| records.first())
                    .cloned();
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
