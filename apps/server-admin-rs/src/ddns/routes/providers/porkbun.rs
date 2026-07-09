use super::*;

pub(in crate::ddns::routes) fn porkbun_catalog_entry() -> Value {
    provider(
        "porkbun",
        "Porkbun",
        vec![
            field("api_key", "API Key", "text", "Porkbun API Key", true),
            field(
                "secret_api_key",
                "Secret API Key",
                "password",
                "Porkbun Secret API Key",
                true,
            ),
            field("root_domain", "Root Domain", "text", "example.com", true),
            field("domain", "Domain", "text", "home.example.com", true),
            field("ttl", "TTL", "text", "600", false),
        ],
    )
}

pub(in crate::ddns::routes) async fn update_porkbun(
    translator: &Translator,
    config: &HashMap<String, String>,
    http_options: &DDNSHttpClientOptions,
    ipv4: Option<&str>,
    ipv6: Option<&str>,
) -> anyhow::Result<DDNSProviderUpdateResult> {
    let api_key = config_value(config, "api_key");
    let secret_api_key = config_value(config, "secret_api_key");
    let root_domain = config_value(config, "root_domain");
    let domain = config_value(config, "domain");
    if api_key.is_empty()
        || secret_api_key.is_empty()
        || root_domain.is_empty()
        || domain.is_empty()
    {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.porkbun.configIncomplete",
            &[],
        )));
    }
    let ttl = positive_i64(config.get("ttl"), 600).to_string();
    let parsed = split_domain(translator, &domain, &root_domain)?;
    let client = ddns_http_client(translator, http_options)?;
    let query_failed = ddns_text(translator, "providers.porkbun.queryRecordFailed", &[]);
    let update_failed = ddns_text(translator, "providers.porkbun.updateRecordFailed", &[]);
    let create_failed = ddns_text(translator, "providers.porkbun.createRecordFailed", &[]);
    let provider_label_text = provider_label(Some("porkbun"), translator);

    update_dual_stack(
        translator,
        &provider_label_text,
        ipv4,
        ipv6,
        |record_type, ip| {
            let client = client.clone();
            let api_key = api_key.clone();
            let secret_api_key = secret_api_key.clone();
            let root_domain = parsed.root_domain.clone();
            let record_name = parsed.record_name.clone();
            let ttl = ttl.clone();
            let query_failed = query_failed.clone();
            let update_failed = update_failed.clone();
            let create_failed = create_failed.clone();
            async move {
                let list = porkbun_request(
                    translator,
                    &client,
                    &format!(
                        "/retrieveByNameType/{}/{}/{}",
                        url_encode_component(&root_domain),
                        record_type,
                        url_encode_component(&record_name)
                    ),
                    &api_key,
                    &secret_api_key,
                    json!({}),
                )
                .await?;
                if list.get("status").and_then(Value::as_str) != Some("SUCCESS") {
                    return Err(anyhow::anyhow!(
                        "{}",
                        json_text(&list, "message").unwrap_or_else(|| query_failed.clone())
                    ));
                }
                let existing_content = list
                    .get("records")
                    .and_then(Value::as_array)
                    .and_then(|items| items.first())
                    .and_then(|record| record.get("content"))
                    .and_then(Value::as_str);
                if existing_content == Some(ip.as_str()) {
                    return Ok(());
                }
                let path = if existing_content.is_some() {
                    format!(
                        "/editByNameType/{}/{}/{}",
                        url_encode_component(&root_domain),
                        record_type,
                        url_encode_component(&record_name)
                    )
                } else {
                    format!("/create/{}", url_encode_component(&root_domain))
                };
                let mut body = json!({
                    "content": ip,
                    "ttl": ttl
                });
                if existing_content.is_none()
                    && let Some(object) = body.as_object_mut()
                {
                    object.insert("name".to_string(), json!(record_name));
                    object.insert("type".to_string(), json!(record_type));
                }
                let result =
                    porkbun_request(translator, &client, &path, &api_key, &secret_api_key, body)
                        .await?;
                if result.get("status").and_then(Value::as_str) == Some("SUCCESS") {
                    Ok(())
                } else {
                    let fallback = if existing_content.is_some() {
                        update_failed.clone()
                    } else {
                        create_failed.clone()
                    };
                    Err(anyhow::anyhow!(
                        "{}",
                        json_text(&result, "message").unwrap_or(fallback)
                    ))
                }
            }
        },
    )
    .await
}

pub(in crate::ddns::routes) async fn porkbun_request(
    translator: &Translator,
    client: &DDNSHttpClient,
    path: &str,
    api_key: &str,
    secret_api_key: &str,
    body: Value,
) -> anyhow::Result<Value> {
    let mut payload = body.as_object().cloned().unwrap_or_default();
    payload.insert("apikey".to_string(), json!(api_key));
    payload.insert("secretapikey".to_string(), json!(secret_api_key));
    let (_status, value, _text) = response_json(
        translator,
        client
            .post(format!("https://porkbun.com/api/json/v3/dns{path}"))
            .json(&Value::Object(payload))
            .send()
            .await?,
    )
    .await?;
    Ok(value)
}
