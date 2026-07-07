use super::*;

pub(in crate::ddns::routes) async fn update_huaweicloud(
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
            "providers.huawei.configIncomplete",
            &[],
        )));
    }
    let ttl = positive_i64(config.get("ttl"), 300);
    let parsed = split_domain(translator, &domain, &root_domain)?;
    let normalized_root = parsed.root_domain.trim_end_matches('.').to_string();
    let fqdn_with_dot = format!("{}.", parsed.fqdn.trim_end_matches('.'));
    let expected_zone_name = format!("{normalized_root}.");
    let client = ddns_http_client()?;
    let zone_response = huawei_request(
        translator,
        &client,
        &access_key_id,
        &secret_access_key,
        &format!("/v2/zones?name={}", url_encode_component(&normalized_root)),
        "GET",
        None,
    )
    .await?;
    let zone_id = zone_response
        .get("zones")
        .and_then(Value::as_array)
        .and_then(|zones| {
            zones.iter().find_map(|zone| {
                (zone.get("name").and_then(Value::as_str) == Some(expected_zone_name.as_str()))
                    .then(|| zone.get("id").and_then(Value::as_str).map(str::to_string))
                    .flatten()
            })
        });
    let Some(zone_id) = zone_id else {
        return Ok(provider_failure(format!(
            "{}",
            ddns_text(
                translator,
                "providers.huawei.zoneNotFound",
                &[("zone", expected_zone_name.clone())],
            )
        )));
    };

    let provider_label_text = provider_label(Some("huaweicloud"), translator);
    update_dual_stack(
        translator,
        &provider_label_text,
        ipv4,
        ipv6,
        |record_type, ip| {
            let client = client.clone();
            let access_key_id = access_key_id.clone();
            let secret_access_key = secret_access_key.clone();
            let zone_id = zone_id.clone();
            let fqdn_with_dot = fqdn_with_dot.clone();
            async move {
                let recordset_path = format!(
                    "/v2/zones/{}/recordsets?search_mode=equal&type={}&name={}&limit=500",
                    url_encode_component(&zone_id),
                    record_type,
                    url_encode_component(&fqdn_with_dot)
                );
                let records = huawei_request(
                    translator,
                    &client,
                    &access_key_id,
                    &secret_access_key,
                    &recordset_path,
                    "GET",
                    None,
                )
                .await?;
                let existing = records
                    .get("recordsets")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .find(|record| {
                        record.get("zone_id").and_then(Value::as_str) == Some(zone_id.as_str())
                            && record.get("name").and_then(Value::as_str)
                                == Some(fqdn_with_dot.as_str())
                            && record.get("type").and_then(Value::as_str) == Some(record_type)
                    })
                    .cloned();
                if let Some(existing) = existing {
                    let same_records = existing
                        .get("records")
                        .and_then(Value::as_array)
                        .is_some_and(|records| {
                            records.len() == 1
                                && records.first().and_then(Value::as_str) == Some(ip.as_str())
                        });
                    let same_ttl = existing.get("ttl").and_then(Value::as_i64) == Some(ttl);
                    if same_records && same_ttl {
                        return Ok(());
                    }
                    let record_id =
                        existing.get("id").and_then(Value::as_str).ok_or_else(|| {
                            anyhow::anyhow!(ddns_text(
                                translator,
                                "providers.huawei.recordsetIdMissing",
                                &[],
                            ))
                        })?;
                    huawei_request(
                        translator,
                        &client,
                        &access_key_id,
                        &secret_access_key,
                        &format!(
                            "/v2.1/zones/{}/recordsets/{}",
                            url_encode_component(&zone_id),
                            url_encode_component(record_id)
                        ),
                        "PUT",
                        Some(json!({
                            "name": fqdn_with_dot,
                            "type": record_type,
                            "ttl": ttl,
                            "records": [ip]
                        })),
                    )
                    .await?;
                    return Ok(());
                }
                huawei_request(
                    translator,
                    &client,
                    &access_key_id,
                    &secret_access_key,
                    &format!("/v2/zones/{}/recordsets", url_encode_component(&zone_id)),
                    "POST",
                    Some(json!({
                        "name": fqdn_with_dot,
                        "type": record_type,
                        "ttl": ttl,
                        "records": [ip]
                    })),
                )
                .await?;
                Ok(())
            }
        },
    )
    .await
}
