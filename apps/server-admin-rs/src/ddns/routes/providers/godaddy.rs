use super::*;

pub(in crate::ddns::routes) async fn update_godaddy(
    translator: &Translator,
    config: &HashMap<String, String>,
    http_options: &DDNSHttpClientOptions,
    ipv4: Option<&str>,
    ipv6: Option<&str>,
) -> anyhow::Result<DDNSProviderUpdateResult> {
    let api_key = config_value(config, "api_key");
    let api_secret = config_value(config, "api_secret");
    let root_domain = config_value(config, "root_domain");
    let domain = config_value(config, "domain");
    if api_key.is_empty() || api_secret.is_empty() || root_domain.is_empty() || domain.is_empty() {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.godaddy.configIncomplete",
            &[],
        )));
    }
    let ttl = positive_i64(config.get("ttl"), 600);
    let parsed = split_domain(translator, &domain, &root_domain)?;
    let client = ddns_http_client(translator, http_options)?;
    let provider_label_text = provider_label(Some("godaddy"), translator);

    update_dual_stack(
        translator,
        &provider_label_text,
        ipv4,
        ipv6,
        |record_type, ip| {
            let client = client.clone();
            let api_key = api_key.clone();
            let api_secret = api_secret.clone();
            let root_domain = parsed.root_domain.clone();
            let record_name = parsed.record_name.clone();
            async move {
                let response = client
                    .put(format!(
                        "https://api.godaddy.com/v1/domains/{}/records/{}/{}",
                        url_encode_component(&root_domain),
                        record_type,
                        url_encode_component(&record_name)
                    ))
                    .header(
                        reqwest::header::AUTHORIZATION,
                        format!("sso-key {api_key}:{api_secret}"),
                    )
                    .json(&json!([{
                        "data": ip,
                        "name": record_name,
                        "ttl": ttl,
                        "type": record_type
                    }]))
                    .send()
                    .await?;
                let status = response.status();
                let text = response_text(response).await?;
                if status.is_success() {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!(ddns_text(
                        translator,
                        "providers.godaddy.updateFailedWithStatus",
                        &[
                            ("status", status.as_u16().to_string()),
                            (
                                "detail",
                                if text.is_empty() {
                                    ddns_text(translator, "providers.godaddy.updateFailed", &[])
                                } else {
                                    text
                                },
                            ),
                        ],
                    )))
                }
            }
        },
    )
    .await
}
