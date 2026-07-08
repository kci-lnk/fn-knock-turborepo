use super::*;

pub(in crate::ddns::routes) fn huaweicloud_catalog_entry() -> Value {
    provider(
        "huaweicloud",
        "华为云 DNS",
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

pub(in crate::ddns::routes) async fn update_huaweicloud(
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
            "providers.huawei.configIncomplete",
            &[],
        )));
    }
    let ttl = positive_i64(config.get("ttl"), 300);
    let parsed = split_domain(translator, &domain, &root_domain)?;
    let normalized_root = parsed.root_domain.trim_end_matches('.').to_string();
    let fqdn_with_dot = format!("{}.", parsed.fqdn.trim_end_matches('.'));
    let expected_zone_name = format!("{normalized_root}.");
    let client = ddns_http_client(translator, http_options)?;
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

pub(in crate::ddns::routes) async fn huawei_request(
    translator: &Translator,
    client: &DDNSHttpClient,
    access_key_id: &str,
    secret_access_key: &str,
    path: &str,
    method: &str,
    body: Option<Value>,
) -> anyhow::Result<Value> {
    let url = format!("https://dns.myhuaweicloud.com{path}");
    let body_string = body
        .as_ref()
        .map(serde_json::to_string)
        .transpose()?
        .unwrap_or_default();
    let (x_sdk_date, authorization) = huawei_sdk_authorization(
        method,
        &url,
        "application/json",
        access_key_id,
        secret_access_key,
        &body_string,
    )?;
    let method = reqwest::Method::from_bytes(method.as_bytes())?;
    let mut request = client
        .request(method, &url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header(reqwest::header::HOST, "dns.myhuaweicloud.com")
        .header("X-Sdk-Date", x_sdk_date)
        .header(reqwest::header::AUTHORIZATION, authorization);
    if !body_string.is_empty() {
        request = request.body(body_string);
    }
    let response = request.send().await?;
    let status = response.status();
    let status_text = response.status_text().to_string();
    let text = response.text().await?.trim().to_string();
    if !status.is_success() {
        return Err(anyhow::anyhow!(huawei_request_failed_message(
            translator,
            status.as_u16(),
            &status_text,
            &text,
        )));
    }
    if text.is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str::<Value>(&text).map_err(|_| {
        anyhow::anyhow!(ddns_text(
            translator,
            "invalidJsonResponse",
            &[("text", text.clone())],
        ))
    })
}

pub(in crate::ddns::routes) fn huawei_request_failed_message(
    translator: &Translator,
    status: u16,
    status_text: &str,
    text: &str,
) -> String {
    ddns_text(
        translator,
        "providers.huawei.requestFailed",
        &[
            ("status", status.to_string()),
            ("statusText", status_text.to_string()),
            ("detail", huawei_error_detail(text)),
        ],
    )
}

pub(in crate::ddns::routes) fn huawei_error_detail(text: &str) -> String {
    serde_json::from_str::<Value>(text)
        .ok()
        .and_then(|value| serde_json::to_string(&value).ok())
        .unwrap_or_else(|| text.to_string())
}

pub(in crate::ddns::routes) fn canonical_huawei_uri(path: &str) -> String {
    let mut uri = path
        .split('/')
        .map(|segment| rfc3986_encode(&safe_decode_uri_component(segment)))
        .collect::<Vec<_>>()
        .join("/");
    if !uri.starts_with('/') {
        uri.insert(0, '/');
    }
    if !uri.ends_with('/') {
        uri.push('/');
    }
    uri
}

pub(in crate::ddns::routes) fn huawei_sdk_authorization(
    method: &str,
    url: &str,
    content_type: &str,
    access_key_id: &str,
    secret_access_key: &str,
    payload: &str,
) -> anyhow::Result<(String, String)> {
    let url = url::Url::parse(url)?;
    let x_sdk_date = compact_utc_timestamp();
    let canonical_uri = canonical_huawei_uri(url.path());
    let canonical_query = canonical_query_from_url(&url);
    let payload_hash = sha256_hex(payload);
    let canonical_headers = format!(
        "content-type:{}\nhost:{}\nx-sdk-date:{}\n",
        content_type.trim(),
        url.host_str().unwrap_or_default(),
        x_sdk_date
    );
    let signed_headers = "content-type;host;x-sdk-date";
    let canonical_request = [
        method,
        &canonical_uri,
        &canonical_query,
        &canonical_headers,
        signed_headers,
        &payload_hash,
    ]
    .join("\n");
    let string_to_sign = format!(
        "SDK-HMAC-SHA256\n{}\n{}",
        x_sdk_date,
        sha256_hex(&canonical_request)
    );
    let signature = hmac_sha256_hex(secret_access_key.as_bytes(), string_to_sign.as_bytes());
    Ok((
        x_sdk_date,
        format!(
            "SDK-HMAC-SHA256 Access={access_key_id}, SignedHeaders={signed_headers}, Signature={signature}"
        ),
    ))
}
