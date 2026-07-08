use super::*;

pub(in crate::ddns::routes) fn cloudflare_catalog_entry() -> Value {
    provider(
        "cloudflare",
        "Cloudflare",
        vec![
            field(
                "api_token",
                "API Token",
                "password",
                "Cloudflare API Token",
                true,
            ),
            field("zone_id", "Zone ID", "text", "Zone ID", true),
            field("domain", "Domain", "text", "home.example.com", true),
            select_field(
                "proxied",
                "Proxied",
                false,
                vec![("DNS only", "false"), ("Orange cloud", "true")],
            ),
        ],
    )
}

pub(in crate::ddns::routes) async fn update_cloudflare(
    translator: &Translator,
    config: &HashMap<String, String>,
    http_options: &DDNSHttpClientOptions,
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
    let client = ddns_http_client(translator, http_options)
        .map_err(|error| cloudflare_record_operation_error(translator, "A", error))?;
    let base_url = format!("https://api.cloudflare.com/client/v4/zones/{zone_id}/dns_records");
    let mut errors = Vec::new();

    if let Some(ipv4) = ipv4 {
        if let Some(message) = update_cloudflare_record(
            translator, &client, &base_url, &api_token, &domain, proxied, "A", ipv4,
        )
        .await?
        {
            errors.push(message);
        }
    }

    if let Some(ipv6) = ipv6 {
        if let Some(message) = update_cloudflare_record(
            translator, &client, &base_url, &api_token, &domain, proxied, "AAAA", ipv6,
        )
        .await?
        {
            errors.push(message);
        }
    }

    if errors.is_empty() {
        Ok(DDNSProviderUpdateResult {
            success: true,
            message: ddns_text(translator, "providers.cloudflare.success", &[]),
        })
    } else {
        Ok(provider_failure(errors.join("; ")))
    }
}

async fn update_cloudflare_record(
    translator: &Translator,
    client: &DDNSHttpClient,
    base_url: &str,
    api_token: &str,
    domain: &str,
    proxied: bool,
    record_type: &'static str,
    ip: &str,
) -> anyhow::Result<Option<String>> {
    let search_url = build_query_url(
        base_url,
        &[
            ("type", record_type.to_string()),
            ("name", domain.to_string()),
        ],
    );
    let (search_status, search_data, _) = response_json(
        translator,
        client
            .get(search_url)
            .bearer_auth(api_token)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await
            .map_err(|error| cloudflare_record_operation_error(translator, record_type, error))?,
    )
    .await
    .map_err(|error| cloudflare_record_operation_error(translator, record_type, error))?;
    if !search_status.is_success()
        || search_data.get("success").and_then(Value::as_bool) != Some(true)
    {
        return Ok(Some(ddns_text(
            translator,
            "providers.cloudflare.searchRecordFailed",
            &[
                ("type", record_type.to_string()),
                (
                    "detail",
                    compact_json(search_data.get("errors").unwrap_or(&search_data)),
                ),
            ],
        )));
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
    let updating_existing = existing_id.is_some();
    let request = if let Some(id) = existing_id {
        client
            .patch(format!("{base_url}/{id}"))
            .bearer_auth(api_token)
            .json(&body)
    } else {
        client.post(base_url).bearer_auth(api_token).json(&body)
    };
    let (status, data, _) = response_json(
        translator,
        request
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await
            .map_err(|error| cloudflare_record_operation_error(translator, record_type, error))?,
    )
    .await
    .map_err(|error| cloudflare_record_operation_error(translator, record_type, error))?;
    if status.is_success() && data.get("success").and_then(Value::as_bool) == Some(true) {
        Ok(None)
    } else {
        Ok(Some(ddns_text(
            translator,
            if updating_existing {
                "providers.cloudflare.updateRecordFailed"
            } else {
                "providers.cloudflare.createRecordFailed"
            },
            &[
                ("type", record_type.to_string()),
                ("detail", compact_json(data.get("errors").unwrap_or(&data))),
            ],
        )))
    }
}

fn cloudflare_record_operation_error(
    translator: &Translator,
    record_type: &str,
    error: impl std::fmt::Display,
) -> anyhow::Error {
    anyhow::anyhow!(ddns_text(
        translator,
        "providers.cloudflare.recordOperationError",
        &[
            ("type", record_type.to_string()),
            ("detail", error.to_string()),
        ],
    ))
}
