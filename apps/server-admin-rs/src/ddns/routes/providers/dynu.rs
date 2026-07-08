use super::*;

pub(in crate::ddns::routes) fn dynu_catalog_entry() -> Value {
    provider(
        "dynu",
        "Dynu",
        vec![
            field("api_key", "API Key", "password", "Dynu API Key", true),
            field("domain", "Domain", "text", "home.example.com", true),
            field("ttl", "TTL", "text", "120", false),
            field("group", "Group", "text", "default", false),
        ],
    )
}

pub(in crate::ddns::routes) async fn update_dynu(
    translator: &Translator,
    config: &HashMap<String, String>,
    http_options: &DDNSHttpClientOptions,
    ipv4: Option<&str>,
    ipv6: Option<&str>,
) -> anyhow::Result<DDNSProviderUpdateResult> {
    let api_key = config_value(config, "api_key");
    let raw_domain = config_value(config, "domain");
    let wildcard = raw_domain.trim().starts_with("*.");
    let domain = if wildcard {
        normalize_domain(raw_domain.trim().trim_start_matches("*."))
    } else {
        normalize_domain(&raw_domain)
    };
    if api_key.is_empty() || domain.is_empty() {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.dynu.configIncomplete",
            &[],
        )));
    }
    if ipv4.is_none() && ipv6.is_none() {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.dynu.noIpAvailable",
            &[],
        )));
    }
    let client = ddns_http_client(translator, http_options)?;
    let result = async {
        if wildcard {
            return update_dynu_wildcard(
                translator, &client, &api_key, config, &domain, ipv4, ipv6,
            )
            .await;
        }
        let root = resolve_dynu_root(translator, &client, &api_key, &domain).await?;
        let provider_label_text = provider_label(Some("dynu"), translator);
        update_dual_stack(
            translator,
            &provider_label_text,
            ipv4,
            ipv6,
            |record_type, ip| {
                let client = client.clone();
                let api_key = api_key.clone();
                let domain = domain.clone();
                let root = root.clone();
                let ttl_config = config_value(config, "ttl");
                let group_config = config_value(config, "group");
                async move {
                    let list = dynu_request(
                        translator,
                        &client,
                        &api_key,
                        &format!(
                            "/dns/record/{}?recordType={}",
                            url_encode_component(&domain),
                            record_type
                        ),
                        None,
                    )
                    .await?;
                    let existing = list
                        .get("dnsRecords")
                        .and_then(Value::as_array)
                        .and_then(|records| {
                            find_dynu_record(records, record_type, &domain, &root.node_name)
                        });
                    if let Some(existing) = existing.clone()
                        && dynu_record_address(&existing, record_type) == ip
                    {
                        return Ok(());
                    }
                    let ttl = positive_i64(
                        Some(&ttl_config),
                        existing
                            .as_ref()
                            .and_then(|record| record.get("ttl"))
                            .and_then(Value::as_i64)
                            .filter(|value| *value > 0)
                            .unwrap_or(300),
                    );
                    let group = default_string(
                        group_config.clone(),
                        existing
                            .as_ref()
                            .and_then(|record| record.get("group"))
                            .and_then(Value::as_str)
                            .unwrap_or(""),
                    );
                    let mut body = json!({
                        "nodeName": root.node_name,
                        "recordType": record_type,
                        "ttl": ttl,
                        "state": existing.as_ref().and_then(|record| record.get("state")).and_then(Value::as_bool).unwrap_or(true),
                        "group": group
                    });
                    if record_type == "A" {
                        insert_json_field(&mut body, "ipv4Address", json!(ip));
                    } else {
                        insert_json_field(&mut body, "ipv6Address", json!(ip));
                    }
                    let path = if let Some(existing) = existing {
                        let record_id = read_positive_id(existing.get("id")).ok_or_else(|| {
                            anyhow::anyhow!(ddns_text(
                                translator,
                                "providers.dynu.recordIdMissing",
                                &[],
                            ))
                        })?;
                        format!("/dns/{}/record/{record_id}", root.domain_id)
                    } else {
                        format!("/dns/{}/record", root.domain_id)
                    };
                    dynu_request(translator, &client, &api_key, &path, Some(body)).await?;
                    Ok(())
                }
            },
        )
        .await
    }
    .await;
    match result {
        Ok(result) => Ok(result),
        Err(error) => Ok(dynu_request_error_result(translator, &error.to_string())),
    }
}

pub(in crate::ddns::routes) fn dynu_request_error_result(
    translator: &Translator,
    detail: &str,
) -> DDNSProviderUpdateResult {
    provider_failure(ddns_text(
        translator,
        "providers.dynu.requestError",
        &[("detail", detail.to_string())],
    ))
}

pub(in crate::ddns::routes) async fn update_dynu_wildcard(
    translator: &Translator,
    client: &DDNSHttpClient,
    api_key: &str,
    config: &HashMap<String, String>,
    domain: &str,
    ipv4: Option<&str>,
    ipv6: Option<&str>,
) -> anyhow::Result<DDNSProviderUpdateResult> {
    let root = resolve_dynu_root(translator, client, api_key, domain).await?;
    if root.domain_name != domain || !root.node_name.is_empty() {
        return Ok(provider_failure(ddns_text(
            translator,
            "providers.dynu.wildcardUnsupported",
            &[("domain", domain.to_string())],
        )));
    }
    let details = dynu_request(
        translator,
        client,
        api_key,
        &format!("/dns/{}", root.domain_id),
        None,
    )
    .await?;
    let ipv4_unchanged = ipv4.is_none_or(|ip| {
        details.get("ipv4Address").and_then(Value::as_str) == Some(ip)
            && details
                .get("ipv4WildcardAlias")
                .and_then(Value::as_bool)
                .unwrap_or(false)
    });
    let ipv6_unchanged = ipv6.is_none_or(|ip| {
        details.get("ipv6Address").and_then(Value::as_str) == Some(ip)
            && details
                .get("ipv6WildcardAlias")
                .and_then(Value::as_bool)
                .unwrap_or(false)
    });
    if ipv4_unchanged && ipv6_unchanged {
        return Ok(DDNSProviderUpdateResult {
            success: true,
            message: ddns_text(translator, "providers.dynu.wildcardUnchanged", &[]),
        });
    }
    let ttl = positive_i64(
        config.get("ttl"),
        details
            .get("ttl")
            .and_then(Value::as_i64)
            .filter(|value| *value > 0)
            .unwrap_or(300),
    );
    let group = default_string(
        config_value(config, "group"),
        details.get("group").and_then(Value::as_str).unwrap_or(""),
    );
    let mut body = json!({
        "name": normalize_domain(details.get("name").and_then(Value::as_str).unwrap_or(domain)),
        "group": group,
        "ttl": ttl,
        "ipv4": ipv4.is_some() || details.get("ipv4").and_then(Value::as_bool).unwrap_or(false) || details.get("ipv4Address").and_then(Value::as_str).is_some(),
        "ipv6": ipv6.is_some() || details.get("ipv6").and_then(Value::as_bool).unwrap_or(false) || details.get("ipv6Address").and_then(Value::as_str).is_some(),
        "ipv4WildcardAlias": ipv4.is_some() || details.get("ipv4WildcardAlias").and_then(Value::as_bool).unwrap_or(false),
        "ipv6WildcardAlias": ipv6.is_some() || details.get("ipv6WildcardAlias").and_then(Value::as_bool).unwrap_or(false),
        "allowZoneTransfer": details.get("allowZoneTransfer").and_then(Value::as_bool).unwrap_or(false),
        "dnssec": details.get("dnssec").and_then(Value::as_bool).unwrap_or(false)
    });
    if let Some(ipv4) = ipv4.or_else(|| details.get("ipv4Address").and_then(Value::as_str)) {
        insert_json_field(&mut body, "ipv4Address", json!(ipv4));
    }
    if let Some(ipv6) = ipv6.or_else(|| details.get("ipv6Address").and_then(Value::as_str)) {
        insert_json_field(&mut body, "ipv6Address", json!(ipv6));
    }
    dynu_request(
        translator,
        client,
        api_key,
        &format!("/dns/{}", root.domain_id),
        Some(body),
    )
    .await?;
    Ok(DDNSProviderUpdateResult {
        success: true,
        message: ddns_text(translator, "providers.dynu.wildcardSuccess", &[]),
    })
}

#[derive(Clone)]
pub(in crate::ddns::routes) struct DynuRoot {
    pub(in crate::ddns::routes) domain_id: i64,
    pub(in crate::ddns::routes) domain_name: String,
    pub(in crate::ddns::routes) node_name: String,
}

pub(in crate::ddns::routes) async fn dynu_request(
    translator: &Translator,
    client: &DDNSHttpClient,
    api_key: &str,
    path: &str,
    body: Option<Value>,
) -> anyhow::Result<Value> {
    let url = format!("https://api.dynu.com/v2{path}");
    let mut request = client
        .request(
            if body.is_some() {
                reqwest::Method::POST
            } else {
                reqwest::Method::GET
            },
            &url,
        )
        .header(reqwest::header::ACCEPT, "application/json")
        .header("API-Key", api_key);
    if let Some(body) = body {
        request = request
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .json(&body);
    }
    let (status, data, text) = response_json(translator, request.send().await?).await?;
    assert_dynu_success(status, &data, &text)?;
    Ok(data)
}

pub(in crate::ddns::routes) fn assert_dynu_success(
    status: StatusCode,
    data: &Value,
    text: &str,
) -> anyhow::Result<()> {
    if !status.is_success() {
        return Err(anyhow::anyhow!(
            "[{}] {}",
            status.as_u16(),
            format_dynu_error(data, text)
        ));
    }
    if data.get("exception").is_some() {
        return Err(anyhow::anyhow!("{}", format_dynu_error(data, text)));
    }
    if let Some(status_code) = data.get("statusCode").and_then(Value::as_i64)
        && status_code != 200
    {
        return Err(anyhow::anyhow!(
            "[{status_code}] {}",
            format_dynu_error(data, text)
        ));
    }
    Ok(())
}

pub(in crate::ddns::routes) fn format_dynu_error(data: &Value, fallback: &str) -> String {
    if let Some(exception) = data.get("exception") {
        let status = exception
            .get("statusCode")
            .and_then(Value::as_i64)
            .map(|value| format!("[{value}] "))
            .unwrap_or_default();
        let error_type = exception
            .get("type")
            .and_then(Value::as_str)
            .map(|value| format!("{value}: "))
            .unwrap_or_default();
        let message = exception
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or(fallback);
        return format!("{status}{error_type}{message}");
    }
    json_text(data, "message").unwrap_or_else(|| fallback.to_string())
}

pub(in crate::ddns::routes) async fn resolve_dynu_root(
    translator: &Translator,
    client: &DDNSHttpClient,
    api_key: &str,
    domain: &str,
) -> anyhow::Result<DynuRoot> {
    let root = dynu_request(
        translator,
        client,
        api_key,
        &format!("/dns/getroot/{}", url_encode_component(domain)),
        None,
    )
    .await?;
    let domain_id = read_positive_id(root.get("id")).ok_or_else(|| {
        anyhow::anyhow!(ddns_text(translator, "providers.dynu.invalidRootInfo", &[],))
    })?;
    let domain_name = normalize_domain(
        root.get("domainName")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    );
    if domain_name.is_empty() {
        return Err(anyhow::anyhow!(ddns_text(
            translator,
            "providers.dynu.invalidRootInfo",
            &[],
        )));
    }
    let node_name = normalize_dynu_node_name(root.get("node").and_then(Value::as_str))
        .if_empty(build_dynu_fallback_node_name(domain, &domain_name));
    Ok(DynuRoot {
        domain_id,
        domain_name,
        node_name,
    })
}

pub(in crate::ddns::routes) fn read_positive_id(value: Option<&Value>) -> Option<i64> {
    value
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
        })
        .filter(|value| *value > 0)
}

pub(in crate::ddns::routes) fn normalize_dynu_node_name(value: Option<&str>) -> String {
    let trimmed = value.unwrap_or_default().trim();
    if trimmed == "@" {
        String::new()
    } else {
        trimmed.to_string()
    }
}

pub(in crate::ddns::routes) fn build_dynu_fallback_node_name(
    domain: &str,
    root_domain: &str,
) -> String {
    let fqdn = normalize_domain(domain);
    let root = normalize_domain(root_domain);
    if fqdn.is_empty() || root.is_empty() || fqdn == root {
        return String::new();
    }
    let suffix = format!(".{root}");
    if fqdn.ends_with(&suffix) {
        fqdn[..fqdn.len() - suffix.len()].to_string()
    } else {
        String::new()
    }
}

pub(in crate::ddns::routes) fn find_dynu_record(
    records: &[Value],
    record_type: &str,
    domain: &str,
    node_name: &str,
) -> Option<Value> {
    let normalized_domain = normalize_domain(domain);
    let normalized_node = normalize_dynu_node_name(Some(node_name));
    let matching = records
        .iter()
        .filter(|record| {
            record
                .get("recordType")
                .and_then(Value::as_str)
                .is_some_and(|value| value.eq_ignore_ascii_case(record_type))
        })
        .collect::<Vec<_>>();
    if let Some(record) = matching
        .iter()
        .find(|record| build_dynu_record_hostname(record) == normalized_domain)
    {
        return Some((*record).clone());
    }
    if normalized_node.is_empty() {
        return None;
    }
    matching
        .into_iter()
        .find(|record| {
            normalize_dynu_node_name(record.get("nodeName").and_then(Value::as_str))
                == normalized_node
        })
        .cloned()
}

pub(in crate::ddns::routes) fn build_dynu_record_hostname(record: &Value) -> String {
    if let Some(hostname) = record.get("hostname").and_then(Value::as_str) {
        return normalize_domain(hostname);
    }
    let domain_name = normalize_domain(
        record
            .get("domainName")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    );
    if domain_name.is_empty() {
        return String::new();
    }
    let node_name = normalize_dynu_node_name(record.get("nodeName").and_then(Value::as_str));
    if node_name.is_empty() {
        domain_name
    } else {
        format!("{node_name}.{domain_name}")
    }
}

pub(in crate::ddns::routes) fn dynu_record_address(record: &Value, record_type: &str) -> String {
    let key = if record_type == "A" {
        "ipv4Address"
    } else {
        "ipv6Address"
    };
    record
        .get(key)
        .or_else(|| record.get("content"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string()
}
