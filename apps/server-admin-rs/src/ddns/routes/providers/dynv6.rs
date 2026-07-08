use super::*;

pub(in crate::ddns::routes) fn dynv6_catalog_entry() -> Value {
    provider(
        "dynv6",
        "dynv6",
        vec![
            field("token", "HTTP Token", "password", "dynv6 HTTP Token", true),
            field("zone", "Zone", "text", "myhost.dynv6.net", true),
            field(
                "ipv6prefix",
                "IPv6 Prefix",
                "text",
                "2001:db8:1234::/64",
                false,
            ),
        ],
    )
}

pub(in crate::ddns::routes) async fn update_dynv6(
    translator: &Translator,
    config: &HashMap<String, String>,
    http_options: &DDNSHttpClientOptions,
    ipv4: Option<&str>,
    ipv6: Option<&str>,
) -> anyhow::Result<DDNSProviderUpdateResult> {
    let token = config_value(config, "token");
    let zone = config_value(config, "zone");
    if token.is_empty() || zone.is_empty() {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.dynv6.configIncomplete",
            &[],
        )));
    }
    if ipv4.is_none() && ipv6.is_none() && config_value(config, "ipv6prefix").is_empty() {
        return Ok(provider_failure(ddns_text(
            translator,
            "dualStackUnavailable",
            &[],
        )));
    }
    let mut query = vec![("hostname", zone), ("token", token)];
    if let Some(ipv4) = ipv4 {
        query.push(("ipv4", ipv4.to_string()));
    }
    if let Some(ipv6) = ipv6 {
        query.push(("ipv6", ipv6.to_string()));
    }
    let ipv6prefix = config_value(config, "ipv6prefix");
    if !ipv6prefix.is_empty() {
        query.push(("ipv6prefix", ipv6prefix));
    }
    let client = ddns_http_client(translator, http_options)
        .map_err(|error| provider_request_error(translator, "dynv6", error))?;
    let response = client
        .get(build_query_url("https://dynv6.com/api/update", &query))
        .send()
        .await
        .map_err(|error| provider_request_error(translator, "dynv6", error))?;
    let status = response.status();
    let text = response_text(response)
        .await
        .map_err(|error| provider_request_error(translator, "dynv6", error))?;
    if status.is_success() && (text.contains("updated") || text.contains("unchanged")) {
        Ok(DDNSProviderUpdateResult {
            success: true,
            message: ddns_text(
                translator,
                "providers.dynv6.success",
                &[
                    ("detail", text),
                    ("params", dynv6_sent_params(translator, ipv4, ipv6, config)),
                ],
            ),
        })
    } else {
        Ok(provider_failure(ddns_text(
            translator,
            "providers.dynv6.updateFailed",
            &[("status", status.as_u16().to_string()), ("detail", text)],
        )))
    }
}

pub(in crate::ddns::routes) fn dynv6_sent_params(
    translator: &Translator,
    ipv4: Option<&str>,
    ipv6: Option<&str>,
    config: &HashMap<String, String>,
) -> String {
    let empty = ddns_text(translator, "providers.dynv6.empty", &[]);
    let mut parts = vec![
        format!("ipv4={}", ipv4.unwrap_or(empty.as_str())),
        format!("ipv6={}", ipv6.unwrap_or(empty.as_str())),
    ];
    let ipv6prefix = config_value(config, "ipv6prefix");
    if !ipv6prefix.is_empty() {
        parts.push(format!("ipv6prefix={ipv6prefix}"));
    }
    parts.join(", ")
}
