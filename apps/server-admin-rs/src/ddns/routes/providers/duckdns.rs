use super::*;

pub(in crate::ddns::routes) fn duckdns_catalog_entry() -> Value {
    provider(
        "duckdns",
        "DuckDNS",
        vec![
            field("domains", "Domains", "text", "home,lab", true),
            field("token", "Token", "password", "DuckDNS Token", true),
        ],
    )
}

pub(in crate::ddns::routes) async fn update_duckdns(
    translator: &Translator,
    config: &HashMap<String, String>,
    http_options: &DDNSHttpClientOptions,
    ipv4: Option<&str>,
    ipv6: Option<&str>,
) -> anyhow::Result<DDNSProviderUpdateResult> {
    let domains = config_value(config, "domains");
    let token = config_value(config, "token");
    if domains.is_empty() || token.is_empty() {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.duckdns.configIncomplete",
            &[],
        )));
    }
    if ipv4.is_none() && ipv6.is_none() {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.duckdns.noIpAvailable",
            &[],
        )));
    }
    let client = ddns_http_client(translator, http_options)
        .map_err(|error| provider_request_error(translator, "duckdns", error))?;
    let response = client
        .post("https://ddns.duckdns.fnknock.cn/")
        .header(reqwest::header::ACCEPT, "text/plain")
        .json(&json!({
            "domains": domains,
            "token": token,
            "ip": ipv4,
            "ipv6": ipv6,
            "verbose": true,
        }))
        .send()
        .await
        .map_err(|error| provider_request_error(translator, "duckdns", error))?;
    let status = response.status();
    let text = response_text(response)
        .await
        .map_err(|error| provider_request_error(translator, "duckdns", error))?;
    if !status.is_success() {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.duckdns.updateFailedWithStatus",
            &[
                ("status", status.as_u16().to_string()),
                (
                    "detail",
                    if text.is_empty() {
                        ddns_text(translator, "providers.duckdns.requestFailed", &[])
                    } else {
                        text
                    },
                ),
            ],
        )));
    }
    let lines = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    if lines.first().copied() != Some("OK") {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.duckdns.updateFailed",
            &[(
                "detail",
                if text.is_empty() {
                    ddns_text(translator, "providers.duckdns.nonOkResponse", &[])
                } else {
                    text
                },
            )],
        )));
    }
    let detail = lines
        .last()
        .copied()
        .filter(|value| *value != "OK")
        .unwrap_or("");
    Ok(DDNSProviderUpdateResult {
        success: true,
        message: if detail.is_empty() {
            ddns_text(
                translator,
                "providers.duckdns.success",
                &[("detail", String::new())],
            )
        } else {
            ddns_text(
                translator,
                "providers.duckdns.success",
                &[("detail", format!(" ({detail})"))],
            )
        },
    })
}
