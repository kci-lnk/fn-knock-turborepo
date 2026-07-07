use super::*;

pub(in crate::ddns::routes) async fn update_dynu(
    translator: &Translator,
    config: &HashMap<String, String>,
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
    let client = ddns_http_client()?;
    if wildcard {
        return update_dynu_wildcard(translator, &client, &api_key, config, &domain, ipv4, ipv6)
            .await;
    }
    let root = resolve_dynu_root(translator, &client, &api_key, &domain).await?;
    let provider_label_text = provider_label(Some("dynu"), translator);
    update_dual_stack(translator, &provider_label_text, ipv4, ipv6, |record_type, ip| {
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
                .and_then(|records| find_dynu_record(records, record_type, &domain, &root.node_name));
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
    })
    .await
}

pub(in crate::ddns::routes) async fn update_dynu_wildcard(
    translator: &Translator,
    client: &reqwest::Client,
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
            ipv4_updated: ipv4.is_some(),
            ipv6_updated: ipv6.is_some(),
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
        ipv4_updated: ipv4.is_some(),
        ipv6_updated: ipv6.is_some(),
    })
}
