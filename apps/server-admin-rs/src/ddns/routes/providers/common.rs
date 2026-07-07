use super::*;
use tokio::io::AsyncWriteExt;

pub(in crate::ddns::routes) const DEFAULT_DDNS_PROVIDER_TIMEOUT_MS: u64 = 10_000;

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
    }
}

pub(in crate::ddns::routes) fn provider_request_error_message(
    translator: &Translator,
    provider: &str,
    error: impl std::fmt::Display,
) -> String {
    ddns_text(
        translator,
        &format!("providers.{provider}.requestError"),
        &[("detail", error.to_string())],
    )
}

pub(in crate::ddns::routes) fn provider_request_error(
    translator: &Translator,
    provider: &str,
    error: impl std::fmt::Display,
) -> anyhow::Error {
    anyhow::anyhow!(provider_request_error_message(translator, provider, error))
}

#[derive(Clone, Debug, Default)]
pub(in crate::ddns::routes) struct DDNSHttpClientOptions {
    pub(in crate::ddns::routes) transport: &'static str,
    pub(in crate::ddns::routes) network_interface: String,
}

impl DDNSHttpClientOptions {
    pub(in crate::ddns::routes) fn from_settings_and_config(
        settings: &Value,
        config: &HashMap<String, String>,
    ) -> Self {
        Self {
            transport: normalize_http_transport(settings.get("httpTransport")),
            network_interface: normalize_network_interface(
                config.get(DDNS_NETWORK_INTERFACE_FIELD).map(String::as_str),
            ),
        }
    }

    pub(in crate::ddns::routes) fn bindable_interface(&self) -> Option<&str> {
        let value = self.network_interface.trim();
        (!value.is_empty() && !value.starts_with(DOCKER_HOST_INTERFACE_PREFIX)).then_some(value)
    }
}

#[derive(Clone)]
pub(in crate::ddns::routes) struct DDNSHttpClient {
    transport: &'static str,
    network_interface: String,
    timeout_ms: u64,
    reqwest_attempts: Vec<reqwest::Client>,
    translator: Translator,
}

impl DDNSHttpClient {
    pub(in crate::ddns::routes) fn get(&self, url: impl ToString) -> DDNSHttpRequestBuilder {
        self.request(reqwest::Method::GET, url)
    }

    pub(in crate::ddns::routes) fn post(&self, url: impl ToString) -> DDNSHttpRequestBuilder {
        self.request(reqwest::Method::POST, url)
    }

    pub(in crate::ddns::routes) fn put(&self, url: impl ToString) -> DDNSHttpRequestBuilder {
        self.request(reqwest::Method::PUT, url)
    }

    pub(in crate::ddns::routes) fn patch(&self, url: impl ToString) -> DDNSHttpRequestBuilder {
        self.request(reqwest::Method::PATCH, url)
    }

    pub(in crate::ddns::routes) fn request(
        &self,
        method: reqwest::Method,
        url: impl ToString,
    ) -> DDNSHttpRequestBuilder {
        DDNSHttpRequestBuilder {
            client: self.clone(),
            method,
            url: url.to_string(),
            headers: Vec::new(),
            body: None,
        }
    }
}

pub(in crate::ddns::routes) struct DDNSHttpRequestBuilder {
    client: DDNSHttpClient,
    method: reqwest::Method,
    url: String,
    headers: Vec<(String, String)>,
    body: Option<Vec<u8>>,
}

impl DDNSHttpRequestBuilder {
    pub(in crate::ddns::routes) fn header(
        mut self,
        name: impl DDNSHeaderName,
        value: impl ToString,
    ) -> Self {
        self.headers
            .push((name.ddns_header_name(), value.to_string()));
        self
    }

    pub(in crate::ddns::routes) fn bearer_auth(self, token: impl ToString) -> Self {
        self.header("Authorization", format!("Bearer {}", token.to_string()))
    }

    pub(in crate::ddns::routes) fn json<T: serde::Serialize + ?Sized>(mut self, value: &T) -> Self {
        self.body = serde_json::to_vec(value).ok();
        if !self.has_header("content-type") {
            self.headers
                .push(("content-type".to_string(), "application/json".to_string()));
        }
        self
    }

    pub(in crate::ddns::routes) fn body(mut self, body: impl Into<String>) -> Self {
        self.body = Some(body.into().into_bytes());
        self
    }

    pub(in crate::ddns::routes) async fn send(self) -> anyhow::Result<DDNSHttpResponse> {
        if self.client.transport == "curl" {
            self.send_via_curl().await
        } else {
            self.send_via_reqwest().await
        }
    }

    fn has_header(&self, name: &str) -> bool {
        self.headers
            .iter()
            .any(|(key, _)| key.eq_ignore_ascii_case(name))
    }

    async fn send_via_reqwest(self) -> anyhow::Result<DDNSHttpResponse> {
        let mut last_error = None;
        for client in &self.client.reqwest_attempts {
            let mut request = client.request(self.method.clone(), self.url.clone());
            for (name, value) in &self.headers {
                request = request.header(name, value);
            }
            if let Some(body) = self.body.clone() {
                request = request.body(body);
            }
            match request.send().await {
                Ok(response) => {
                    let status = response.status();
                    let status_text = status.canonical_reason().unwrap_or_default().to_string();
                    let body = response.bytes().await?.to_vec();
                    return Ok(DDNSHttpResponse {
                        status,
                        status_text,
                        body,
                    });
                }
                Err(error) => {
                    last_error = Some(error);
                }
            }
        }
        Err(last_error
            .map(anyhow::Error::from)
            .unwrap_or_else(|| anyhow::anyhow!("null")))
    }

    async fn send_via_curl(self) -> anyhow::Result<DDNSHttpResponse> {
        const PROXY_ENV_KEYS: [&str; 8] = [
            "http_proxy",
            "https_proxy",
            "all_proxy",
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "ALL_PROXY",
            "no_proxy",
            "NO_PROXY",
        ];

        let temp_dir = env::temp_dir().join(format!(
            "ddns-curl-{}-{}",
            std::process::id(),
            Uuid::new_v4()
        ));
        tokio::fs::create_dir_all(&temp_dir).await?;
        let header_path = temp_dir.join("headers.txt");
        let body_path = temp_dir.join("body.bin");
        let result = async {
            let mut command = tokio::process::Command::new("curl");
            command
                .arg("-q")
                .arg("--silent")
                .arg("--show-error")
                .arg("--location")
                .arg("--max-time")
                .arg(format!("{:.3}", self.client.timeout_ms as f64 / 1000.0))
                .arg("--dump-header")
                .arg(&header_path)
                .arg("--output")
                .arg(&body_path)
                .arg("--request")
                .arg(self.method.as_str())
                .stdin(if self.body.is_some() {
                    std::process::Stdio::piped()
                } else {
                    std::process::Stdio::null()
                })
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::piped());
            for key in PROXY_ENV_KEYS {
                command.env_remove(key);
            }
            if !self.client.network_interface.is_empty()
                && !self
                    .client
                    .network_interface
                    .starts_with(DOCKER_HOST_INTERFACE_PREFIX)
            {
                command
                    .arg("--interface")
                    .arg(self.client.network_interface.as_str());
            }
            for (name, value) in &self.headers {
                command.arg("--header").arg(format!("{name}: {value}"));
            }
            if self.body.is_some() {
                command.arg("--data-binary").arg("@-");
            }
            command.arg(&self.url);

            let mut child = command.spawn()?;
            if let Some(body) = self.body
                && let Some(mut stdin) = child.stdin.take()
            {
                stdin.write_all(&body).await?;
            }
            let output = child.wait_with_output().await?;
            if !output.status.success() {
                let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
                anyhow::bail!(
                    "{}",
                    ddns_text(
                        &self.client.translator,
                        "curlRequestFailed",
                        &[(
                            "detail",
                            if detail.is_empty() {
                                output
                                    .status
                                    .code()
                                    .map(|code| format!("exit {code}"))
                                    .unwrap_or_else(|| "terminated".to_string())
                            } else {
                                detail
                            },
                        )],
                    )
                );
            }
            let raw_headers = tokio::fs::read_to_string(&header_path).await?;
            let (status, status_text) =
                parse_curl_headers_for_response(&self.client.translator, &raw_headers)?;
            let body = tokio::fs::read(&body_path).await.unwrap_or_default();
            Ok(DDNSHttpResponse {
                status,
                status_text,
                body,
            })
        }
        .await;
        let cleanup = tokio::fs::remove_dir_all(&temp_dir).await;
        if let Err(error) = cleanup {
            tracing::debug!(%error, "failed to cleanup DDNS curl temp dir");
        }
        result
    }
}

pub(in crate::ddns::routes) trait DDNSHeaderName {
    fn ddns_header_name(self) -> String;
}

impl DDNSHeaderName for &str {
    fn ddns_header_name(self) -> String {
        self.to_string()
    }
}

impl DDNSHeaderName for String {
    fn ddns_header_name(self) -> String {
        self
    }
}

impl DDNSHeaderName for reqwest::header::HeaderName {
    fn ddns_header_name(self) -> String {
        self.as_str().to_string()
    }
}

impl DDNSHeaderName for &reqwest::header::HeaderName {
    fn ddns_header_name(self) -> String {
        self.as_str().to_string()
    }
}

pub(in crate::ddns::routes) struct DDNSHttpResponse {
    status: StatusCode,
    status_text: String,
    body: Vec<u8>,
}

impl DDNSHttpResponse {
    pub(in crate::ddns::routes) fn status(&self) -> StatusCode {
        self.status
    }

    pub(in crate::ddns::routes) fn status_text(&self) -> &str {
        &self.status_text
    }

    pub(in crate::ddns::routes) async fn text(self) -> anyhow::Result<String> {
        Ok(String::from_utf8_lossy(&self.body).to_string())
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
    let mut errors = Vec::new();

    if let Some(ip) = ipv4 {
        match update_record("A", ip.to_string()).await {
            Ok(()) => {}
            Err(error) => errors.push(format!(
                "{}: {error}",
                ddns_text(translator, "aRecordFailed", &[])
            )),
        }
    }
    if let Some(ip) = ipv6 {
        match update_record("AAAA", ip.to_string()).await {
            Ok(()) => {}
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
        });
    }
    Ok(DDNSProviderUpdateResult {
        success: true,
        message: ddns_text(
            translator,
            "providerDnsUpdateSuccess",
            &[("provider", provider_label.to_string())],
        ),
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
    response: DDNSHttpResponse,
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
    client: &DDNSHttpClient,
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
    client: &DDNSHttpClient,
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
    let (_status, value, _text) = response_json(
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
    Ok(value)
}

pub(in crate::ddns::routes) async fn alidns_request(
    translator: &Translator,
    client: &DDNSHttpClient,
    access_key_id: &str,
    access_key_secret: &str,
    params: Vec<(&str, String)>,
) -> anyhow::Result<Value> {
    let body = build_aliyun_signed_params(access_key_id, access_key_secret, params, "POST");
    let (_status, value, _text) = response_json(
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
    Ok(value)
}

pub(in crate::ddns::routes) async fn tencentcloud_request(
    translator: &Translator,
    client: &DDNSHttpClient,
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
    let canonical_headers = tencentcloud_tc3_canonical_headers(content_type, HOST, action);
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
    client: &DDNSHttpClient,
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

pub(in crate::ddns::routes) async fn baidu_request(
    translator: &Translator,
    client: &DDNSHttpClient,
    access_key_id: &str,
    secret_access_key: &str,
    path: &str,
    body: Value,
) -> anyhow::Result<Value> {
    let url = format!("https://bcd.baidubce.com{path}");
    let body_string = serde_json::to_string(&body)?;
    let (timestamp, authorization) =
        baidu_bce_authorization("POST", &url, access_key_id, secret_access_key)?;
    let (_status, data, _text) = response_json(
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
    Ok(data)
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
    client: &DDNSHttpClient,
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

pub(in crate::ddns::routes) fn json_value_js_truthy(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Null) | None => false,
        Some(Value::Bool(value)) => *value,
        Some(Value::Number(value)) => value.as_f64().is_some_and(|value| value != 0.0),
        Some(Value::String(value)) => !value.is_empty(),
        Some(Value::Array(_)) | Some(Value::Object(_)) => true,
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

pub(in crate::ddns::routes) fn safe_decode_uri_component(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let Some(hex) = bytes.get(index + 1..index + 3) else {
                return value.to_string();
            };
            let Ok(hex) = std::str::from_utf8(hex) else {
                return value.to_string();
            };
            let Ok(byte) = u8::from_str_radix(hex, 16) else {
                return value.to_string();
            };
            decoded.push(byte);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).unwrap_or_else(|_| value.to_string())
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

pub(in crate::ddns::routes) fn parse_curl_headers_for_response(
    translator: &Translator,
    raw_headers: &str,
) -> anyhow::Result<(StatusCode, String)> {
    let normalized = raw_headers.replace("\r\n", "\n").trim().to_string();
    if normalized.is_empty() {
        anyhow::bail!("{}", ddns_text(translator, "curlNoHeaders", &[]));
    }
    let final_block = normalized
        .split("\n\n")
        .map(str::trim)
        .filter(|block| !block.is_empty())
        .last()
        .unwrap_or(normalized.as_str());
    let status_line = final_block.lines().next().unwrap_or_default().trim();
    let mut parts = status_line.split_whitespace();
    let http_version = parts.next().unwrap_or_default();
    let status = parts
        .next()
        .and_then(|value| value.parse::<u16>().ok())
        .and_then(|value| StatusCode::from_u16(value).ok());
    if !http_version.starts_with("HTTP/") || status.is_none() {
        anyhow::bail!(
            "{}",
            ddns_text(
                translator,
                "curlStatusLineParseFailed",
                &[("line", status_line.to_string())],
            )
        );
    }
    Ok((status.unwrap(), parts.collect::<Vec<_>>().join(" ")))
}

pub(in crate::ddns::routes) fn ddns_http_client(
    translator: &Translator,
    options: &DDNSHttpClientOptions,
) -> anyhow::Result<DDNSHttpClient> {
    let normalized_interface = options.network_interface.trim();
    if !normalized_interface.is_empty() {
        let exists = list_ddns_network_interfaces()
            .iter()
            .any(|item| item.get("name").and_then(Value::as_str) == Some(normalized_interface));
        if !exists {
            anyhow::bail!(
                "{}",
                ddns_text(
                    translator,
                    "interfaceNotFound",
                    &[("name", normalized_interface.to_string())],
                )
            );
        }
    }
    let timeout_ms = provider_timeout_ms_like_node()?;
    let reqwest_attempts = ddns_reqwest_clients_for_options(translator, options, timeout_ms)?;
    Ok(DDNSHttpClient {
        transport: options.transport,
        network_interface: options
            .bindable_interface()
            .map(str::to_string)
            .unwrap_or_default(),
        timeout_ms,
        reqwest_attempts,
        translator: translator.clone(),
    })
}

fn ddns_reqwest_client_builder(timeout_ms: u64) -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .timeout(Duration::from_millis(timeout_ms))
        .redirect(reqwest::redirect::Policy::limited(20))
        .no_proxy()
}

fn ddns_reqwest_clients_for_options(
    translator: &Translator,
    options: &DDNSHttpClientOptions,
    timeout_ms: u64,
) -> anyhow::Result<Vec<reqwest::Client>> {
    if options.transport != "node" {
        return Ok(vec![ddns_reqwest_client_builder(timeout_ms).build()?]);
    }
    let interface = options.network_interface.trim();
    if interface.is_empty() || interface.starts_with(DOCKER_HOST_INTERFACE_PREFIX) {
        return Ok(vec![ddns_reqwest_client_builder(timeout_ms).build()?]);
    }
    let addresses = node_transport_local_addresses(interface);
    if addresses.is_empty() {
        anyhow::bail!(
            "{}",
            ddns_text(
                translator,
                "nodeTransportInterfaceNoAddress",
                &[("name", interface.to_string())],
            )
        );
    }
    addresses
        .into_iter()
        .map(|address| {
            ddns_reqwest_client_builder(timeout_ms)
                .local_address(address)
                .build()
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(Into::into)
}

pub(in crate::ddns::routes) fn node_transport_local_addresses(interface: &str) -> Vec<IpAddr> {
    let Some(item) = list_ddns_network_interfaces()
        .into_iter()
        .find(|item| item.get("name").and_then(Value::as_str) == Some(interface))
    else {
        return Vec::new();
    };
    if item.get("source").and_then(Value::as_str) == Some("docker_host") {
        return Vec::new();
    }
    node_transport_local_addresses_from_interface(&item)
}

pub(in crate::ddns::routes) fn node_transport_local_addresses_from_interface(
    item: &Value,
) -> Vec<IpAddr> {
    let addresses = item
        .get("addresses")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut output = Vec::new();
    output.extend(addresses.iter().filter_map(|item| {
        (item.get("family").and_then(Value::as_str) == Some("ipv4"))
            .then(|| item.get("address").and_then(Value::as_str))
            .flatten()
            .and_then(|value| value.parse::<IpAddr>().ok())
            .filter(IpAddr::is_ipv4)
    }));
    output.extend(addresses.iter().filter_map(|item| {
        (item.get("family").and_then(Value::as_str) == Some("ipv6"))
            .then(|| item.get("address").and_then(Value::as_str))
            .flatten()
            .and_then(|value| value.parse::<IpAddr>().ok())
            .filter(IpAddr::is_ipv6)
    }));
    output
}

pub(in crate::ddns::routes) fn provider_timeout_ms_like_node() -> anyhow::Result<u64> {
    provider_timeout_ms_from_env_value(env::var("DDNS_TIMEOUT_MS").ok().as_deref())
}

pub(in crate::ddns::routes) fn provider_timeout_ms_from_env_value(
    value: Option<&str>,
) -> anyhow::Result<u64> {
    const NODE_TIMEOUT_MAX_MS: f64 = 4_294_967_295.0;
    let number = match value {
        None | Some("") => DEFAULT_DDNS_PROVIDER_TIMEOUT_MS as f64,
        Some(value) => {
            let parsed = js_number_from_string_like_node(value).unwrap_or(f64::NAN);
            if parsed.is_finite() && parsed > 0.0 {
                parsed
            } else {
                DEFAULT_DDNS_PROVIDER_TIMEOUT_MS as f64
            }
        }
    };
    if number.fract() != 0.0 {
        anyhow::bail!(
            r#"The value of "delay" is out of range. It must be an integer. Received {}"#,
            number
        );
    }
    if !(0.0..=NODE_TIMEOUT_MAX_MS).contains(&number) {
        anyhow::bail!(
            r#"The value of "delay" is out of range. It must be >= 0 && <= 4294967295. Received {}"#,
            number
        );
    }
    Ok(number as u64)
}

fn js_number_from_string_like_node(value: &str) -> Option<f64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Some(0.0);
    }

    let radix_value = if let Some(rest) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        Some(u128::from_str_radix(rest, 16).ok()? as f64)
    } else if let Some(rest) = trimmed
        .strip_prefix("0b")
        .or_else(|| trimmed.strip_prefix("0B"))
    {
        Some(u128::from_str_radix(rest, 2).ok()? as f64)
    } else if let Some(rest) = trimmed
        .strip_prefix("0o")
        .or_else(|| trimmed.strip_prefix("0O"))
    {
        Some(u128::from_str_radix(rest, 8).ok()? as f64)
    } else {
        None
    };
    if let Some(value) = radix_value {
        return Some(value);
    }

    trimmed.parse::<f64>().ok()
}

pub(in crate::ddns::routes) async fn response_text(
    response: DDNSHttpResponse,
) -> anyhow::Result<String> {
    Ok(response.text().await?.trim().to_string())
}
