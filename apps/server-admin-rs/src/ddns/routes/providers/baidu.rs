use super::*;

pub(in crate::ddns::routes) fn baiducloud_catalog_entry() -> Value {
    provider(
        "baiducloud",
        "百度智能云",
        vec![
            field("access_key_id", "Access Key", "text", "Access Key", true),
            field(
                "secret_access_key",
                "Secret Key",
                "password",
                "Secret Key",
                true,
            ),
            field("root_domain", "Root Domain", "text", "example.com", true),
            field("domain", "Domain", "text", "home.example.com", true),
            field("ttl", "TTL", "text", "300", false),
        ],
    )
}

pub(in crate::ddns::routes) async fn update_baiducloud(
    translator: &Translator,
    config: &HashMap<String, String>,
    http_options: &DDNSHttpClientOptions,
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
    let client = ddns_http_client(translator, http_options)?;
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

pub(in crate::ddns::routes) async fn baidu_request(
    translator: &Translator,
    client: &DDNSHttpClient,
    access_key_id: &str,
    secret_access_key: &str,
    path: &str,
    body: Value,
) -> anyhow::Result<Value> {
    let url = format!("https://bcd.baidubce.com{path}");
    let body_string = serde_json::to_string(&body)?;
    let (timestamp, authorization) =
        baidu_bce_authorization("POST", &url, access_key_id, secret_access_key)?;
    let (_status, data, _text) = response_json(
        translator,
        client
            .post(&url)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header(reqwest::header::HOST, "bcd.baidubce.com")
            .header("x-bce-date", timestamp)
            .header(reqwest::header::AUTHORIZATION, authorization)
            .body(body_string)
            .send()
            .await?,
    )
    .await?;
    Ok(data)
}

pub(in crate::ddns::routes) fn baidu_bce_authorization(
    method: &str,
    url: &str,
    access_key_id: &str,
    secret_access_key: &str,
) -> anyhow::Result<(String, String)> {
    let url = url::Url::parse(url)?;
    let timestamp = iso8601_utc_without_millis();
    let signed_header_names = ["content-type", "host", "x-bce-date"];
    let header_values = [
        ("content-type", "application/json"),
        ("host", url.host_str().unwrap_or_default()),
        ("x-bce-date", timestamp.as_str()),
    ];
    let canonical_headers = signed_header_names
        .iter()
        .filter_map(|name| {
            header_values
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| format!("{name}:{}", rfc3986_encode(value.trim())))
        })
        .collect::<Vec<_>>()
        .join("\n");
    let auth_string_prefix = format!("bce-auth-v1/{access_key_id}/{timestamp}/1800");
    let signing_key = hmac_sha256_hex(secret_access_key.as_bytes(), auth_string_prefix.as_bytes());
    let canonical_request = [
        method,
        url.path(),
        &canonical_query_from_url(&url),
        &canonical_headers,
    ]
    .join("\n");
    let signature = hmac_sha256_hex(signing_key.as_bytes(), canonical_request.as_bytes());
    Ok((
        timestamp,
        format!(
            "{auth_string_prefix}/{}/{}",
            signed_header_names.join(";"),
            signature
        ),
    ))
}
