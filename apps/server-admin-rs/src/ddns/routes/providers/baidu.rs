use super::*;

pub(in crate::ddns::routes) async fn update_baiducloud(
    translator: &Translator,
    config: &HashMap<String, String>,
    ipv4: Option<&str>,
    ipv6: Option<&str>,
) -> anyhow::Result<DDNSProviderUpdateResult> {
    let access_key_id = config_value(config, "access_key_id");
    let secret_access_key = config_value(config, "secret_access_key");
    let root_domain = config_value(config, "root_domain");
    let domain = config_value(config, "domain");
    if access_key_id.is_empty()
        || secret_access_key.is_empty()
        || root_domain.is_empty()
        || domain.is_empty()
    {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.baidu.configIncomplete",
            &[],
        )));
    }
    let ttl = positive_i64(config.get("ttl"), 300);
    let parsed = split_domain(translator, &domain, &root_domain)?;
    let client = ddns_http_client()?;
    let query_failed = ddns_text(translator, "providers.baidu.queryFailed", &[]);
    let update_failed = ddns_text(translator, "providers.baidu.updateFailed", &[]);
    let create_failed = ddns_text(translator, "providers.baidu.createFailed", &[]);
    let provider_label_text = provider_label(Some("baiducloud"), translator);

    update_dual_stack(translator, &provider_label_text, ipv4, ipv6, |record_type, ip| {
        let client = client.clone();
        let access_key_id = access_key_id.clone();
        let secret_access_key = secret_access_key.clone();
        let parsed = parsed.clone();
        let query_failed = query_failed.clone();
        let update_failed = update_failed.clone();
        let create_failed = create_failed.clone();
        async move {
            let list = baidu_request(
                translator,
                &client,
                &access_key_id,
                &secret_access_key,
                "/v1/domain/resolve/list",
                json!({
                    "domain": parsed.root_domain,
                    "pageNum": 1,
                    "pageSize": 1000
                }),
            )
            .await?;
            if let Some(code) = json_text(&list, "code") {
                return Err(anyhow::anyhow!(
                    "{code}: {}",
                    json_text(&list, "message").unwrap_or_else(|| query_failed.clone())
                ));
            }
            let existing = list
                .get("result")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .find(|record| {
                    record.get("domain").and_then(Value::as_str) == Some(&parsed.record_name)
                })
                .cloned();
            if let Some(record) = existing {
                if record.get("rdata").and_then(Value::as_str) == Some(ip.as_str()) {
                    return Ok(());
                }
                let record_id = record
                    .get("recordId")
                    .and_then(Value::as_i64)
                    .ok_or_else(|| anyhow::anyhow!(update_failed.clone()))?;
                let result = baidu_request(
                    translator,
                    &client,
                    &access_key_id,
                    &secret_access_key,
                    "/v1/domain/resolve/edit",
                    json!({
                        "recordId": record_id,
                        "domain": record.get("domain").and_then(Value::as_str).unwrap_or(&parsed.record_name),
                        "view": record.get("view").and_then(Value::as_str).unwrap_or("default"),
                        "rdType": record_type,
                        "ttl": record.get("ttl").and_then(Value::as_i64).unwrap_or(ttl),
                        "rdata": ip,
                        "zoneName": record.get("zoneName").and_then(Value::as_str).unwrap_or(&parsed.root_domain)
                    }),
                )
                .await?;
                if let Some(code) = json_text(&result, "code") {
                    return Err(anyhow::anyhow!(
                        "{code}: {}",
                        json_text(&result, "message").unwrap_or_else(|| update_failed.clone())
                    ));
                }
                return Ok(());
            }
            let result = baidu_request(
                translator,
                &client,
                &access_key_id,
                &secret_access_key,
                "/v1/domain/resolve/add",
                json!({
                    "domain": parsed.record_name,
                    "rdType": record_type,
                    "ttl": ttl,
                    "rdata": ip,
                    "zoneName": parsed.root_domain
                }),
            )
            .await?;
            if let Some(code) = json_text(&result, "code") {
                Err(anyhow::anyhow!(
                    "{code}: {}",
                    json_text(&result, "message").unwrap_or_else(|| create_failed.clone())
                ))
            } else {
                Ok(())
            }
        }
    })
    .await
}
