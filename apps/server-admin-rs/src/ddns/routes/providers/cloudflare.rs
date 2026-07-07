use super::*;

pub(in crate::ddns::routes) async fn update_cloudflare(
    translator: &Translator,
    config: &HashMap<String, String>,
    ipv4: Option<&str>,
    ipv6: Option<&str>,
) -> anyhow::Result<DDNSProviderUpdateResult> {
    let api_token = config_value(config, "api_token");
    let zone_id = config_value(config, "zone_id");
    let domain = config_value(config, "domain");
    if api_token.is_empty() || zone_id.is_empty() || domain.is_empty() {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.cloudflare.configIncomplete",
            &[],
        )));
    }
    let proxied = config_value(config, "proxied") == "true";
    let client = ddns_http_client()?;
    let base_url = format!("https://api.cloudflare.com/client/v4/zones/{zone_id}/dns_records");
    let provider_label_text = provider_label(Some("cloudflare"), translator);

    update_dual_stack(
        translator,
        &provider_label_text,
        ipv4,
        ipv6,
        |record_type, ip| {
            let client = client.clone();
            let api_token = api_token.clone();
            let domain = domain.clone();
            let base_url = base_url.clone();
            async move {
                let search_url = build_query_url(
                    &base_url,
                    &[("type", record_type.to_string()), ("name", domain.clone())],
                );
                let (search_status, search_data, _) = response_json(
                    translator,
                    client
                        .get(search_url)
                        .bearer_auth(&api_token)
                        .header(reqwest::header::ACCEPT, "application/json")
                        .send()
                        .await?,
                )
                .await?;
                if !search_status.is_success()
                    || search_data.get("success").and_then(Value::as_bool) != Some(true)
                {
                    return Err(anyhow::anyhow!(
                        "failed to search {record_type} record: {}",
                        compact_json(search_data.get("errors").unwrap_or(&search_data))
                    ));
                }

                let existing_id = search_data
                    .get("result")
                    .and_then(Value::as_array)
                    .and_then(|items| items.first())
                    .and_then(|item| item.get("id"))
                    .and_then(Value::as_str)
                    .map(str::to_string);
                let body = json!({
                    "type": record_type,
                    "name": domain,
                    "content": ip,
                    "proxied": proxied,
                    "ttl": 1
                });
                let request = if let Some(id) = existing_id {
                    client
                        .patch(format!("{base_url}/{id}"))
                        .bearer_auth(&api_token)
                        .json(&body)
                } else {
                    client.post(&base_url).bearer_auth(&api_token).json(&body)
                };
                let (status, data, _) = response_json(
                    translator,
                    request
                        .header(reqwest::header::ACCEPT, "application/json")
                        .send()
                        .await?,
                )
                .await?;
                if status.is_success() && data.get("success").and_then(Value::as_bool) == Some(true)
                {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!(
                        "failed to upsert {record_type} record: {}",
                        compact_json(data.get("errors").unwrap_or(&data))
                    ))
                }
            }
        },
    )
    .await
}
