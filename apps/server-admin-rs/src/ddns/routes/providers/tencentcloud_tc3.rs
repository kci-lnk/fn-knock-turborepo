use super::*;

pub(in crate::ddns::routes) async fn tencentcloud_tc3_request(
    translator: &Translator,
    client: &DDNSHttpClient,
    secret_id: &str,
    secret_key: &str,
    action: &str,
    payload: Value,
    host: &str,
    service: &str,
    version: &str,
    region: Option<&str>,
) -> anyhow::Result<Value> {
    let timestamp = time_utils::now_ms().div_euclid(1000);
    let date = utc_date(timestamp)?;
    let payload_string = serde_json::to_string(&payload)?;
    let hashed_payload = sha256_hex(&payload_string);
    let content_type = "application/json; charset=utf-8";
    let canonical_headers = tencentcloud_tc3_canonical_headers(content_type, host, action);
    let signed_headers = "content-type;host;x-tc-action";
    let canonical_request = [
        "POST",
        "/",
        "",
        &canonical_headers,
        signed_headers,
        &hashed_payload,
    ]
    .join("\n");
    let credential_scope = format!("{date}/{service}/tc3_request");
    let string_to_sign = [
        "TC3-HMAC-SHA256",
        &timestamp.to_string(),
        &credential_scope,
        &sha256_hex(&canonical_request),
    ]
    .join("\n");
    let secret_date = hmac_sha256_bytes(format!("TC3{secret_key}").as_bytes(), date.as_bytes());
    let secret_service = hmac_sha256_bytes(&secret_date, service.as_bytes());
    let secret_signing = hmac_sha256_bytes(&secret_service, b"tc3_request");
    let signature = hmac_sha256_hex(&secret_signing, string_to_sign.as_bytes());
    let authorization = format!(
        "TC3-HMAC-SHA256 Credential={secret_id}/{credential_scope}, SignedHeaders={signed_headers}, Signature={signature}"
    );
    let mut request = client
        .post(format!("https://{host}/"))
        .header(reqwest::header::CONTENT_TYPE, content_type)
        .header(reqwest::header::HOST, host)
        .header("X-TC-Action", action)
        .header("X-TC-Timestamp", timestamp.to_string())
        .header("X-TC-Version", version)
        .header(reqwest::header::AUTHORIZATION, authorization)
        .body(payload_string);
    if let Some(region) = region {
        request = request.header("X-TC-Region", region);
    }
    let (status, data, _text) = response_json(translator, request.send().await?).await?;
    let response = data.get("Response").cloned().ok_or_else(|| {
        anyhow::anyhow!(ddns_text(
            translator,
            "tencentMissingResponse",
            &[("status", status.as_u16().to_string())],
        ))
    })?;
    if let Some(error) = response.get("Error") {
        let code = error
            .get("Code")
            .and_then(Value::as_str)
            .unwrap_or("TencentCloudError");
        let request_failed = ddns_text(translator, "requestFailed", &[]);
        let message = error
            .get("Message")
            .and_then(Value::as_str)
            .unwrap_or(request_failed.as_str());
        let request_id = response
            .get("RequestId")
            .and_then(Value::as_str)
            .unwrap_or_default();
        return Err(anyhow::anyhow!(
            "{code}: {message}{}",
            if request_id.is_empty() {
                String::new()
            } else {
                format!(" (RequestId: {request_id})")
            }
        ));
    }
    if status.is_success() {
        Ok(response)
    } else {
        Err(anyhow::anyhow!(
            "HTTP {}: {}",
            status.as_u16(),
            ddns_text(translator, "requestFailed", &[])
        ))
    }
}

pub(in crate::ddns::routes) fn tencentcloud_tc3_canonical_headers(
    content_type: &str,
    host: &str,
    action: &str,
) -> String {
    format!(
        "content-type:{}\nhost:{}\nx-tc-action:{}\n",
        content_type.trim().to_ascii_lowercase(),
        host.trim().to_ascii_lowercase(),
        action.trim().to_ascii_lowercase()
    )
}
