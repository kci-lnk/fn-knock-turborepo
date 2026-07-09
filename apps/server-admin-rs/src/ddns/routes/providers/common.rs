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

pub(in crate::ddns::routes) use crate::text_utils::EmptyStringExt;

pub(in crate::ddns::routes) fn insert_json_field(object: &mut Value, key: &str, value: Value) {
    if let Some(map) = object.as_object_mut() {
        map.insert(key.to_string(), value);
    }
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

pub(in crate::ddns::routes) fn form_body(params: &[(&str, String)]) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in params {
        serializer.append_pair(key, value);
    }
    serializer.finish()
}

pub(in crate::ddns::routes) use crate::text_utils::default_string;

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
    iso8601_utc_without_millis().replace(['-', ':'], "")
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

pub(in crate::ddns::routes) use crate::crypto_utils::sha256_hex_str as sha256_hex;

pub(in crate::ddns::routes) use crate::crypto_utils::{
    hmac_sha1_base64, hmac_sha256_bytes, hmac_sha256_hex,
};

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

pub(in crate::ddns::routes) use crate::http_utils::url_encode_component;

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
    let Some(status) = status.filter(|_| http_version.starts_with("HTTP/")) else {
        anyhow::bail!(
            "{}",
            ddns_text(
                translator,
                "curlStatusLineParseFailed",
                &[("line", status_line.to_string())],
            )
        );
    };
    Ok((status, parts.collect::<Vec<_>>().join(" ")))
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
