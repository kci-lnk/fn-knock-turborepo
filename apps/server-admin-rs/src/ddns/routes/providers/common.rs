use super::*;

pub(in crate::ddns::routes) fn config_value(config: &HashMap<String, String>, key: &str) -> String {
    config
        .get(key)
        .map(|value| value.trim().to_string())
        .unwrap_or_default()
}

pub(in crate::ddns::routes) fn provider_failure(
    message: impl Into<String>,
) -> DDNSProviderUpdateResult {
    DDNSProviderUpdateResult {
        success: false,
        message: message.into(),
        ipv4_updated: false,
        ipv6_updated: false,
    }
}

pub(in crate::ddns::routes) async fn update_dual_stack<F, Fut>(
    translator: &Translator,
    provider_label: &str,
    ipv4: Option<&str>,
    ipv6: Option<&str>,
    update_record: F,
) -> anyhow::Result<DDNSProviderUpdateResult>
where
    F: Fn(&'static str, String) -> Fut,
    Fut: Future<Output = anyhow::Result<()>>,
{
    let mut ipv4_updated = false;
    let mut ipv6_updated = false;
    let mut errors = Vec::new();

    if let Some(ip) = ipv4 {
        match update_record("A", ip.to_string()).await {
            Ok(()) => ipv4_updated = true,
            Err(error) => errors.push(format!(
                "{}: {error}",
                ddns_text(translator, "aRecordFailed", &[])
            )),
        }
    }
    if let Some(ip) = ipv6 {
        match update_record("AAAA", ip.to_string()).await {
            Ok(()) => ipv6_updated = true,
            Err(error) => errors.push(format!(
                "{}: {error}",
                ddns_text(translator, "aaaaRecordFailed", &[])
            )),
        }
    }
    if !errors.is_empty() {
        return Ok(DDNSProviderUpdateResult {
            success: false,
            message: errors.join("; "),
            ipv4_updated,
            ipv6_updated,
        });
    }
    Ok(DDNSProviderUpdateResult {
        success: true,
        message: ddns_text(
            translator,
            "providerDnsUpdateSuccess",
            &[("provider", provider_label.to_string())],
        ),
        ipv4_updated,
        ipv6_updated,
    })
}

#[derive(Clone)]
pub(in crate::ddns::routes) struct SplitDomain {
    pub(in crate::ddns::routes) fqdn: String,
    pub(in crate::ddns::routes) root_domain: String,
    pub(in crate::ddns::routes) record_name: String,
}

pub(in crate::ddns::routes) fn split_domain(
    translator: &Translator,
    full_domain: &str,
    root_domain: &str,
) -> anyhow::Result<SplitDomain> {
    let fqdn = normalize_domain(full_domain);
    let zone = normalize_domain(root_domain);
    if fqdn.is_empty() || zone.is_empty() {
        return Err(anyhow::anyhow!(ddns_text(
            translator,
            "domainConfigIncomplete",
            &[],
        )));
    }
    if fqdn == zone {
        return Ok(SplitDomain {
            fqdn,
            root_domain: zone,
            record_name: "@".to_string(),
        });
    }
    let suffix = format!(".{zone}");
    if !fqdn.ends_with(&suffix) {
        return Err(anyhow::anyhow!(ddns_text(
            translator,
            "domainNotInZone",
            &[("fqdn", fqdn), ("zone", zone)],
        )));
    }
    Ok(SplitDomain {
        fqdn: fqdn.clone(),
        root_domain: zone,
        record_name: fqdn[..fqdn.len() - suffix.len()].to_string(),
    })
}

pub(in crate::ddns::routes) fn positive_i64(value: Option<&String>, fallback: i64) -> i64 {
    value
        .and_then(|value| value.trim().parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value > 0.0)
        .map(|value| value.floor() as i64)
        .unwrap_or(fallback)
}

pub(in crate::ddns::routes) async fn response_json(
    translator: &Translator,
    response: reqwest::Response,
) -> anyhow::Result<(StatusCode, Value, String)> {
    let status = response.status();
    let text = response.text().await?.trim().to_string();
    let value = if text.is_empty() {
        json!({})
    } else {
        serde_json::from_str::<Value>(&text).map_err(|_| {
            anyhow::anyhow!(ddns_text(
                translator,
                "invalidJsonResponse",
                &[("text", text.clone())],
            ))
        })?
    };
    Ok((status, value, text))
}

pub(in crate::ddns::routes) async fn porkbun_request(
    translator: &Translator,
    client: &reqwest::Client,
    path: &str,
    api_key: &str,
    secret_api_key: &str,
    body: Value,
) -> anyhow::Result<Value> {
    let mut payload = body.as_object().cloned().unwrap_or_default();
    payload.insert("apikey".to_string(), json!(api_key));
    payload.insert("secretapikey".to_string(), json!(secret_api_key));
    let (_status, value, _text) = response_json(
        translator,
        client
            .post(format!("https://porkbun.com/api/json/v3/dns{path}"))
            .json(&Value::Object(payload))
            .send()
            .await?,
    )
    .await?;
    Ok(value)
}

pub(in crate::ddns::routes) async fn dnspod_request(
    translator: &Translator,
    client: &reqwest::Client,
    api: &str,
    token_id: &str,
    token_key: &str,
    params: Vec<(&str, String)>,
) -> anyhow::Result<Value> {
    let mut form = vec![
        ("login_token", format!("{token_id},{token_key}")),
        ("format", "json".to_string()),
    ];
    form.extend(params);
    let (status, value, text) = response_json(
        translator,
        client
            .post(api)
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body(form_body(&form))
            .send()
            .await?,
    )
    .await?;
    if status.is_success() {
        Ok(value)
    } else {
        Err(anyhow::anyhow!(
            "DNSPod returned HTTP {}: {}",
            status.as_u16(),
            text
        ))
    }
}

pub(in crate::ddns::routes) async fn alidns_request(
    translator: &Translator,
    client: &reqwest::Client,
    access_key_id: &str,
    access_key_secret: &str,
    params: Vec<(&str, String)>,
) -> anyhow::Result<Value> {
    let body = build_aliyun_signed_params(access_key_id, access_key_secret, params, "POST");
    let (status, value, text) = response_json(
        translator,
        client
            .post("https://alidns.aliyuncs.com/")
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body(body)
            .send()
            .await?,
    )
    .await?;
    if status.is_success() {
        Ok(value)
    } else {
        Err(anyhow::anyhow!(
            "AliDNS returned HTTP {}: {}",
            status.as_u16(),
            text
        ))
    }
}

pub(in crate::ddns::routes) async fn tencentcloud_request(
    translator: &Translator,
    client: &reqwest::Client,
    secret_id: &str,
    secret_key: &str,
    action: &str,
    payload: Value,
) -> anyhow::Result<Value> {
    const HOST: &str = "dnspod.tencentcloudapi.com";
    const SERVICE: &str = "dnspod";
    const VERSION: &str = "2021-03-23";

    let timestamp = time_utils::now_ms().div_euclid(1000);
    let date = utc_date(timestamp)?;
    let payload_string = serde_json::to_string(&payload)?;
    let hashed_payload = sha256_hex(&payload_string);
    let content_type = "application/json; charset=utf-8";
    let canonical_headers = format!(
        "content-type:{content_type}\nhost:{HOST}\nx-tc-action:{}\n",
        action.to_ascii_lowercase()
    );
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
    let credential_scope = format!("{date}/{SERVICE}/tc3_request");
    let string_to_sign = [
        "TC3-HMAC-SHA256",
        &timestamp.to_string(),
        &credential_scope,
        &sha256_hex(&canonical_request),
    ]
    .join("\n");
    let secret_date = hmac_sha256_bytes(format!("TC3{secret_key}").as_bytes(), date.as_bytes());
    let secret_service = hmac_sha256_bytes(&secret_date, SERVICE.as_bytes());
    let secret_signing = hmac_sha256_bytes(&secret_service, b"tc3_request");
    let signature = hmac_sha256_hex(&secret_signing, string_to_sign.as_bytes());
    let authorization = format!(
        "TC3-HMAC-SHA256 Credential={secret_id}/{credential_scope}, SignedHeaders={signed_headers}, Signature={signature}"
    );

    let (status, data, _text) = response_json(
        translator,
        client
            .post(format!("https://{HOST}/"))
            .header(reqwest::header::CONTENT_TYPE, content_type)
            .header(reqwest::header::HOST, HOST)
            .header("X-TC-Action", action)
            .header("X-TC-Timestamp", timestamp.to_string())
            .header("X-TC-Version", VERSION)
            .header(reqwest::header::AUTHORIZATION, authorization)
            .body(payload_string)
            .send()
            .await?,
    )
    .await?;
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

pub(in crate::ddns::routes) async fn edgeone_request(
    translator: &Translator,
    client: &reqwest::Client,
    config: &HashMap<String, String>,
    secret_id: &str,
    secret_key: &str,
    action: &str,
    payload: Value,
) -> anyhow::Result<Value> {
    let host = edgeone_api_host(config.get("endpoint").map(String::as_str));
    let region = config_value(config, "region");
    tencentcloud_tc3_request(
        translator,
        client,
        secret_id,
        secret_key,
        action,
        payload,
        &host,
        "teo",
        "2022-09-01",
        if region.is_empty() {
            None
        } else {
            Some(region.as_str())
        },
    )
    .await
}

pub(in crate::ddns::routes) async fn tencentcloud_tc3_request(
    translator: &Translator,
    client: &reqwest::Client,
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
    let canonical_headers = format!(
        "content-type:{content_type}\nhost:{host}\nx-tc-action:{}\n",
        action.to_ascii_lowercase()
    );
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

pub(in crate::ddns::routes) async fn baidu_request(
    translator: &Translator,
    client: &reqwest::Client,
    access_key_id: &str,
    secret_access_key: &str,
    path: &str,
    body: Value,
) -> anyhow::Result<Value> {
    let url = format!("https://bcd.baidubce.com{path}");
    let body_string = serde_json::to_string(&body)?;
    let (timestamp, authorization) =
        baidu_bce_authorization("POST", &url, access_key_id, secret_access_key)?;
    let (status, data, text) = response_json(
        translator,
        client
            .post(&url)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header(reqwest::header::HOST, "bcd.baidubce.com")
            .header("x-bce-date", timestamp)
            .header(reqwest::header::AUTHORIZATION, authorization)
            .body(body_string)
            .send()
            .await?,
    )
    .await?;
    if status.is_success() {
        Ok(data)
    } else {
        Err(anyhow::anyhow!(
            "Baidu Cloud returned HTTP {}: {}",
            status.as_u16(),
            text
        ))
    }
}

pub(in crate::ddns::routes) async fn huawei_request(
    translator: &Translator,
    client: &reqwest::Client,
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
    let (status, data, text) = response_json(translator, request.send().await?).await?;
    if status.is_success() {
        Ok(data)
    } else {
        Err(anyhow::anyhow!(
            "Huawei Cloud DNS returned HTTP {}: {}",
            status.as_u16(),
            text
        ))
    }
}

#[derive(Clone)]
pub(in crate::ddns::routes) struct DynuRoot {
    pub(in crate::ddns::routes) domain_id: i64,
    pub(in crate::ddns::routes) domain_name: String,
    pub(in crate::ddns::routes) node_name: String,
}

pub(in crate::ddns::routes) async fn dynu_request(
    translator: &Translator,
    client: &reqwest::Client,
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
    client: &reqwest::Client,
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

pub(in crate::ddns::routes) trait EmptyDynuString {
    fn if_empty(self, fallback: String) -> String;
}

impl EmptyDynuString for String {
    fn if_empty(self, fallback: String) -> String {
        if self.is_empty() { fallback } else { self }
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

pub(in crate::ddns::routes) fn normalize_edgeone_location(value: Option<&str>) -> String {
    let trimmed = value.unwrap_or_default().trim();
    if trimmed.is_empty() {
        "default".to_string()
    } else {
        trimmed.to_ascii_lowercase()
    }
}

pub(in crate::ddns::routes) fn edgeone_api_host(endpoint: Option<&str>) -> String {
    let value = endpoint.unwrap_or_default().trim();
    if value.is_empty() {
        return "teo.tencentcloudapi.com".to_string();
    }
    if value.starts_with("http://") || value.starts_with("https://") {
        return url::Url::parse(value)
            .ok()
            .and_then(|url| url.host_str().map(str::to_string))
            .filter(|host| !host.is_empty())
            .unwrap_or_else(|| "teo.tencentcloudapi.com".to_string());
    }
    value.trim_end_matches('/').to_string()
}

pub(in crate::ddns::routes) fn is_valid_edgeone_host_header(value: &str) -> bool {
    let host = normalize_domain(value);
    if host.is_empty()
        || host.contains('/')
        || host.contains(':')
        || host.contains('[')
        || host.contains(']')
        || host.contains('*')
        || host.len() > 253
        || value.split_whitespace().count() > 1
        || value.starts_with("http://")
        || value.starts_with("https://")
    {
        return false;
    }
    host.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
    })
}

pub(in crate::ddns::routes) fn insert_json_field(object: &mut Value, key: &str, value: Value) {
    if let Some(map) = object.as_object_mut() {
        map.insert(key.to_string(), value);
    }
}

pub(in crate::ddns::routes) fn baidu_bce_authorization(
    method: &str,
    url: &str,
    access_key_id: &str,
    secret_access_key: &str,
) -> anyhow::Result<(String, String)> {
    let url = url::Url::parse(url)?;
    let timestamp = iso8601_utc_without_millis();
    let signed_header_names = ["content-type", "host", "x-bce-date"];
    let header_values = [
        ("content-type", "application/json"),
        ("host", url.host_str().unwrap_or_default()),
        ("x-bce-date", timestamp.as_str()),
    ];
    let canonical_headers = signed_header_names
        .iter()
        .filter_map(|name| {
            header_values
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| format!("{name}:{}", rfc3986_encode(value.trim())))
        })
        .collect::<Vec<_>>()
        .join("\n");
    let auth_string_prefix = format!("bce-auth-v1/{access_key_id}/{timestamp}/1800");
    let signing_key = hmac_sha256_hex(secret_access_key.as_bytes(), auth_string_prefix.as_bytes());
    let canonical_request = [
        method,
        url.path(),
        &canonical_query_from_url(&url),
        &canonical_headers,
    ]
    .join("\n");
    let signature = hmac_sha256_hex(signing_key.as_bytes(), canonical_request.as_bytes());
    Ok((
        timestamp,
        format!(
            "{auth_string_prefix}/{}/{}",
            signed_header_names.join(";"),
            signature
        ),
    ))
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

pub(in crate::ddns::routes) fn build_aliyun_signed_params(
    access_key_id: &str,
    access_key_secret: &str,
    extra_params: Vec<(&str, String)>,
    method: &str,
) -> String {
    let mut params = vec![
        ("AccessKeyId".to_string(), access_key_id.to_string()),
        ("Format".to_string(), "JSON".to_string()),
        ("SignatureMethod".to_string(), "HMAC-SHA1".to_string()),
        (
            "SignatureNonce".to_string(),
            uuid::Uuid::new_v4().to_string(),
        ),
        ("SignatureVersion".to_string(), "1.0".to_string()),
        ("Timestamp".to_string(), iso8601_utc_without_millis()),
        ("Version".to_string(), "2015-01-09".to_string()),
    ];
    params.extend(
        extra_params
            .into_iter()
            .map(|(key, value)| (key.to_string(), value)),
    );
    params.sort_by(|left, right| left.0.cmp(&right.0));
    let canonicalized = params
        .iter()
        .map(|(key, value)| format!("{}={}", rfc3986_encode(key), rfc3986_encode(value)))
        .collect::<Vec<_>>()
        .join("&");
    let string_to_sign = format!(
        "{}&{}&{}",
        method,
        rfc3986_encode("/"),
        rfc3986_encode(&canonicalized)
    );
    let signature = hmac_sha1_base64(
        format!("{access_key_secret}&").as_bytes(),
        string_to_sign.as_bytes(),
    );
    params.push(("Signature".to_string(), signature));
    params
        .iter()
        .map(|(key, value)| format!("{}={}", rfc3986_encode(key), rfc3986_encode(value)))
        .collect::<Vec<_>>()
        .join("&")
}

pub(in crate::ddns::routes) async fn aliyun_acs3_request(
    translator: &Translator,
    client: &reqwest::Client,
    access_key_id: &str,
    access_key_secret: &str,
    action: &str,
    version: &str,
    method: &str,
    query: Vec<(String, String)>,
    form_data: Vec<(String, String)>,
) -> anyhow::Result<Value> {
    let endpoint = "https://esa.cn-hangzhou.aliyuncs.com/";
    let url = url::Url::parse(endpoint)?;
    let query_string = aliyun_canonical_param_string(&query);
    let body_string = aliyun_canonical_param_string(&form_data);
    let payload_hash = sha256_hex(&body_string);
    let acs_date = iso8601_utc_without_millis();
    let nonce = uuid::Uuid::new_v4().to_string();
    let mut headers = vec![
        (
            "host".to_string(),
            url.host_str().unwrap_or_default().to_string(),
        ),
        ("x-acs-action".to_string(), action.to_string()),
        ("x-acs-content-sha256".to_string(), payload_hash.clone()),
        ("x-acs-date".to_string(), acs_date.clone()),
        ("x-acs-signature-nonce".to_string(), nonce),
        ("x-acs-version".to_string(), version.to_string()),
    ];
    if !body_string.is_empty() {
        headers.push((
            "content-type".to_string(),
            "application/x-www-form-urlencoded".to_string(),
        ));
    }
    headers.sort_by(|left, right| left.0.cmp(&right.0));
    let canonical_headers = format!(
        "{}\n",
        headers
            .iter()
            .map(|(key, value)| format!("{key}:{}", value.trim()))
            .collect::<Vec<_>>()
            .join("\n")
    );
    let signed_headers = headers
        .iter()
        .map(|(key, _)| key.as_str())
        .collect::<Vec<_>>()
        .join(";");
    let canonical_request = [
        method,
        url.path(),
        &query_string,
        &canonical_headers,
        &signed_headers,
        &payload_hash,
    ]
    .join("\n");
    let string_to_sign = format!("ACS3-HMAC-SHA256\n{}", sha256_hex(&canonical_request));
    let signature = hmac_sha256_hex(access_key_secret.as_bytes(), string_to_sign.as_bytes());
    let authorization = format!(
        "ACS3-HMAC-SHA256 Credential={access_key_id},SignedHeaders={signed_headers},Signature={signature}"
    );
    let request_url = if query_string.is_empty() {
        endpoint.to_string()
    } else {
        format!("{endpoint}?{query_string}")
    };
    let method = reqwest::Method::from_bytes(method.as_bytes())?;
    let mut request = client
        .request(method, request_url)
        .header(reqwest::header::HOST, url.host_str().unwrap_or_default())
        .header("x-acs-action", action)
        .header("x-acs-content-sha256", payload_hash)
        .header("x-acs-date", acs_date)
        .header(
            "x-acs-signature-nonce",
            headers
                .iter()
                .find(|(key, _)| key == "x-acs-signature-nonce")
                .map(|(_, value)| value.as_str())
                .unwrap_or_default(),
        )
        .header("x-acs-version", version)
        .header(reqwest::header::AUTHORIZATION, authorization);
    if !body_string.is_empty() {
        request = request
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body(body_string);
    }
    let (status, data, text) = response_json(translator, request.send().await?).await?;
    if !status.is_success() || data.get("Code").is_some() {
        return Err(anyhow::anyhow!(
            "{}: {}",
            data.get("Code")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| format!("HTTP {}", status.as_u16())),
            data.get("Message")
                .and_then(Value::as_str)
                .unwrap_or(if text.is_empty() {
                    "Aliyun ACS3 request failed"
                } else {
                    &text
                })
        ));
    }
    Ok(data)
}

pub(in crate::ddns::routes) fn aliyun_canonical_param_string(
    params: &[(String, String)],
) -> String {
    let mut values = params.to_vec();
    values.sort_by(|left, right| {
        let key_order = left.0.cmp(&right.0);
        if key_order == std::cmp::Ordering::Equal {
            left.1.cmp(&right.1)
        } else {
            key_order
        }
    });
    values
        .into_iter()
        .map(|(key, value)| format!("{}={}", rfc3986_encode(&key), rfc3986_encode(&value)))
        .collect::<Vec<_>>()
        .join("&")
}

pub(in crate::ddns::routes) fn esa_record_payload(
    value: &str,
    ttl: i64,
    proxied: bool,
    biz_name: &str,
) -> Vec<(String, String)> {
    let mut payload = vec![
        ("Data".to_string(), json!({ "Value": value }).to_string()),
        ("Proxied".to_string(), proxied.to_string()),
        ("Ttl".to_string(), ttl.to_string()),
        ("Type".to_string(), "A/AAAA".to_string()),
    ];
    if proxied {
        payload.push((
            "BizName".to_string(),
            default_string(biz_name.to_string(), "web"),
        ));
    }
    payload
}

pub(in crate::ddns::routes) fn value_to_compact_string(value: &Value) -> String {
    match value {
        Value::String(value) => value.trim().to_string(),
        Value::Number(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        _ => String::new(),
    }
}

pub(in crate::ddns::routes) fn same_csv_values(left: &str, right: &str) -> bool {
    let mut left_values = left
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    let mut right_values = right
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    left_values.sort_unstable();
    right_values.sort_unstable();
    left_values == right_values
}

pub(in crate::ddns::routes) fn form_body(params: &[(&str, String)]) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in params {
        serializer.append_pair(key, value);
    }
    serializer.finish()
}

pub(in crate::ddns::routes) fn default_string(value: String, fallback: &str) -> String {
    if value.trim().is_empty() {
        fallback.to_string()
    } else {
        value
    }
}

pub(in crate::ddns::routes) fn iso8601_utc_without_millis() -> String {
    let value = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| time_utils::now_iso());
    strip_fractional_seconds(&value)
}

pub(in crate::ddns::routes) fn utc_date(timestamp: i64) -> anyhow::Result<String> {
    let value = OffsetDateTime::from_unix_timestamp(timestamp)?
        .format(&Rfc3339)
        .unwrap_or_else(|_| time_utils::now_iso());
    Ok(strip_fractional_seconds(&value).chars().take(10).collect())
}

pub(in crate::ddns::routes) fn strip_fractional_seconds(value: &str) -> String {
    if let Some(dot) = value.find('.')
        && let Some(z_index) = value[dot..].find('Z')
    {
        return format!("{}Z", &value[..dot + z_index]);
    }
    value.to_string()
}

pub(in crate::ddns::routes) fn compact_utc_timestamp() -> String {
    iso8601_utc_without_millis()
        .replace(['-', ':'], "")
        .replace('Z', "Z")
}

pub(in crate::ddns::routes) fn canonical_query_from_url(url: &url::Url) -> String {
    let mut pairs = url
        .query_pairs()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect::<Vec<_>>();
    pairs.sort_by(|left, right| {
        let key_order = rfc3986_encode(&left.0).cmp(&rfc3986_encode(&right.0));
        if key_order == std::cmp::Ordering::Equal {
            rfc3986_encode(&left.1).cmp(&rfc3986_encode(&right.1))
        } else {
            key_order
        }
    });
    pairs
        .into_iter()
        .map(|(key, value)| format!("{}={}", rfc3986_encode(&key), rfc3986_encode(&value)))
        .collect::<Vec<_>>()
        .join("&")
}

pub(in crate::ddns::routes) fn canonical_huawei_uri(path: &str) -> String {
    let mut uri = path
        .split('/')
        .map(rfc3986_encode)
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

pub(in crate::ddns::routes) fn rfc3986_encode(value: &str) -> String {
    let mut output = String::new();
    for byte in value.bytes() {
        let ch = byte as char;
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '~') {
            output.push(ch);
        } else {
            output.push_str(&format!("%{byte:02X}"));
        }
    }
    output
}

pub(in crate::ddns::routes) fn sha256_hex(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    hex::encode(digest)
}

pub(in crate::ddns::routes) fn hmac_sha1_base64(key: &[u8], payload: &[u8]) -> String {
    let mut mac = HmacSha1::new_from_slice(key).expect("HMAC accepts keys of any size");
    mac.update(payload);
    BASE64_STANDARD.encode(mac.finalize().into_bytes())
}

pub(in crate::ddns::routes) fn hmac_sha256_bytes(key: &[u8], payload: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts keys of any size");
    mac.update(payload);
    mac.finalize().into_bytes().to_vec()
}

pub(in crate::ddns::routes) fn hmac_sha256_hex(key: &[u8], payload: &[u8]) -> String {
    hex::encode(hmac_sha256_bytes(key, payload))
}

pub(in crate::ddns::routes) fn compact_json(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| value.to_string())
}

pub(in crate::ddns::routes) fn json_text(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(in crate::ddns::routes) fn url_encode_component(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

pub(in crate::ddns::routes) fn build_query_url(base: &str, pairs: &[(&str, String)]) -> String {
    let query = pairs
        .iter()
        .fold(
            url::form_urlencoded::Serializer::new(String::new()),
            |mut serializer, (key, value)| {
                serializer.append_pair(key, value);
                serializer
            },
        )
        .finish();
    if query.is_empty() {
        base.to_string()
    } else {
        format!("{base}?{query}")
    }
}

pub(in crate::ddns::routes) fn ddns_http_client() -> anyhow::Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .timeout(Duration::from_millis(env_u64("DDNS_TIMEOUT_MS", 15_000)))
        .build()?)
}

pub(in crate::ddns::routes) fn env_u64(name: &str, fallback: u64) -> u64 {
    env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(fallback)
}

pub(in crate::ddns::routes) async fn response_text(
    response: reqwest::Response,
) -> anyhow::Result<String> {
    Ok(response.text().await?.trim().to_string())
}
