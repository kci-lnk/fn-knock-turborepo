use super::*;

pub(in crate::notifications::routes) const WEBHOOK_MAX_CUSTOM_HEADERS: usize = 32;
pub(in crate::notifications::routes) const WEBHOOK_MAX_HEADER_NAME_BYTES: usize = 128;
pub(in crate::notifications::routes) const WEBHOOK_MAX_HEADER_VALUE_BYTES: usize = 8 * 1024;
pub(in crate::notifications::routes) const WEBHOOK_MAX_HEADERS_TOTAL_BYTES: usize = 16 * 1024;

pub(in crate::notifications::routes) const WEBHOOK_RESERVED_HEADER_NAMES: &[&str] = &[
    "host",
    "content-type",
    "content-length",
    "connection",
    "proxy-connection",
    "proxy-authenticate",
    "proxy-authorization",
    "http2-settings",
    "keep-alive",
    "transfer-encoding",
    "te",
    "trailer",
    "upgrade",
    "x-fn-knock-provider",
    "x-fn-knock-signature",
    "x-fn-knock-trace-id",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::notifications::routes) struct WebhookHeader {
    pub(in crate::notifications::routes) name: String,
    pub(in crate::notifications::routes) value: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::notifications::routes) struct WebhookHeaderValidationError {
    key: &'static str,
    params: Vec<(&'static str, String)>,
}

impl WebhookHeaderValidationError {
    pub(in crate::notifications::routes) fn text(&self, translator: &Translator) -> String {
        notification_provider_error_text(translator, "webhook", self.key, &self.params)
    }

    pub(in crate::notifications::routes) fn default_text(&self) -> String {
        notification_provider_error_default("webhook", self.key, &self.params)
    }
}

fn webhook_headers_schema() -> SchemaField {
    SchemaField {
        key: "custom_headers",
        label: "Custom headers",
        field_type: "headers",
        required: false,
        sensitive: true,
        placeholder: None,
        default_value: None,
        min: None,
        max: None,
        options: Vec::new(),
        constraints: Some(json!({
            "max_items": WEBHOOK_MAX_CUSTOM_HEADERS,
            "max_name_bytes": WEBHOOK_MAX_HEADER_NAME_BYTES,
            "max_value_bytes": WEBHOOK_MAX_HEADER_VALUE_BYTES,
            "max_total_bytes": WEBHOOK_MAX_HEADERS_TOTAL_BYTES,
            "reserved_names": WEBHOOK_RESERVED_HEADER_NAMES
        })),
    }
}

fn webhook_header_error(
    key: &'static str,
    params: &[(&'static str, String)],
) -> WebhookHeaderValidationError {
    WebhookHeaderValidationError {
        key,
        params: params.to_vec(),
    }
}

pub(in crate::notifications::routes) fn parse_webhook_custom_headers(
    value: &Value,
) -> Result<Vec<WebhookHeader>, WebhookHeaderValidationError> {
    let entries = value
        .as_array()
        .ok_or_else(|| webhook_header_error("invalidHeadersFormat", &[]))?;
    if entries.len() > WEBHOOK_MAX_CUSTOM_HEADERS {
        return Err(webhook_header_error(
            "tooManyHeaders",
            &[("max", WEBHOOK_MAX_CUSTOM_HEADERS.to_string())],
        ));
    }

    let mut normalized = Vec::with_capacity(entries.len());
    let mut seen_names = HashSet::new();
    let mut total_bytes = 0_usize;
    for entry in entries {
        let object = entry
            .as_object()
            .ok_or_else(|| webhook_header_error("invalidHeadersFormat", &[]))?;
        let raw_name = object
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let name = raw_name.trim();
        if name.is_empty() {
            return Err(webhook_header_error("headerNameRequired", &[]));
        }
        if name.len() > WEBHOOK_MAX_HEADER_NAME_BYTES {
            return Err(webhook_header_error(
                "headerNameTooLong",
                &[
                    ("name", name.to_string()),
                    ("max", WEBHOOK_MAX_HEADER_NAME_BYTES.to_string()),
                ],
            ));
        }
        if raw_name.chars().any(char::is_control)
            || reqwest::header::HeaderName::from_bytes(name.as_bytes()).is_err()
        {
            return Err(webhook_header_error(
                "invalidHeaderName",
                &[("name", name.to_string())],
            ));
        }
        let normalized_name = name.to_ascii_lowercase();
        if WEBHOOK_RESERVED_HEADER_NAMES.contains(&normalized_name.as_str()) {
            return Err(webhook_header_error(
                "reservedHeaderName",
                &[("name", name.to_string())],
            ));
        }
        if !seen_names.insert(normalized_name) {
            return Err(webhook_header_error(
                "duplicateHeaderName",
                &[("name", name.to_string())],
            ));
        }

        let value = match object.get("value") {
            None => String::new(),
            Some(Value::String(value)) => {
                if value.chars().any(char::is_control) {
                    return Err(webhook_header_error(
                        "invalidHeaderValue",
                        &[("name", name.to_string())],
                    ));
                }
                value.trim().to_string()
            }
            Some(_) => {
                return Err(webhook_header_error(
                    "invalidHeaderValue",
                    &[("name", name.to_string())],
                ));
            }
        };
        if value.len() > WEBHOOK_MAX_HEADER_VALUE_BYTES {
            return Err(webhook_header_error(
                "headerValueTooLong",
                &[
                    ("name", name.to_string()),
                    ("max", WEBHOOK_MAX_HEADER_VALUE_BYTES.to_string()),
                ],
            ));
        }
        if reqwest::header::HeaderValue::try_from(value.as_str()).is_err() {
            return Err(webhook_header_error(
                "invalidHeaderValue",
                &[("name", name.to_string())],
            ));
        }
        total_bytes = total_bytes
            .saturating_add(name.len())
            .saturating_add(value.len());
        if total_bytes > WEBHOOK_MAX_HEADERS_TOTAL_BYTES {
            return Err(webhook_header_error(
                "headersTooLarge",
                &[("max", WEBHOOK_MAX_HEADERS_TOTAL_BYTES.to_string())],
            ));
        }
        normalized.push(WebhookHeader {
            name: name.to_string(),
            value,
        });
    }
    Ok(normalized)
}

pub(in crate::notifications::routes) fn normalize_webhook_custom_headers(
    value: &Value,
) -> NotifyResult<Value> {
    parse_webhook_custom_headers(value)
        .map(|headers| {
            Value::Array(
                headers
                    .into_iter()
                    .map(|header| json!({ "name": header.name, "value": header.value }))
                    .collect(),
            )
        })
        .map_err(|error| NotifyError::BadRequest(error.default_text()))
}

pub(in crate::notifications::routes) fn webhook_definition() -> ProviderDefinition {
    ProviderDefinition {
        provider_type: "webhook",
        label: "Webhook",
        description: "Send a JSON payload to a custom webhook endpoint.",
        connection_schema: vec![
            string_schema("url", "Webhook URL", true, true, None)
                .placeholder("https://example.com/hooks/fn-knock"),
            select_schema("method", "Method", true, Some("POST"), &["POST", "PUT"]),
            number_schema("timeout_seconds", "Timeout seconds", true, Some(5)).bounds(1, 30),
            string_schema("shared_secret", "Shared secret", false, true, None)
                .placeholder("secret"),
            webhook_headers_schema(),
        ],
        target_schema: vec![
            string_schema("endpoint_path", "Endpoint path", false, false, None)
                .placeholder("/alerts"),
            json_schema("extra_body_json", "Extra body", false)
                .placeholder(r#"{"service":"gateway"}"#),
        ],
        sensitive_fields: vec!["url", "shared_secret", "custom_headers"],
        supports_markdown: true,
        supports_actions: true,
        supports_mentions: true,
        supports_provider_dedupe_key: true,
    }
}

fn webhook_method(config: &Map<String, Value>) -> String {
    config
        .get("method")
        .and_then(Value::as_str)
        .map(|value| value.trim().to_uppercase())
        .filter(|value| value == "POST" || value == "PUT")
        .unwrap_or_else(|| "POST".to_string())
}

fn legacy_webhook_headers(
    target_config: &Map<String, Value>,
) -> Result<Vec<WebhookHeader>, WebhookHeaderValidationError> {
    let Some(raw_headers) = target_config.get("extra_headers_json") else {
        return Ok(Vec::new());
    };
    let headers = raw_headers
        .as_object()
        .ok_or_else(|| webhook_header_error("invalidHeadersFormat", &[]))?;
    let mut entries = Vec::with_capacity(headers.len());
    for (name, value) in headers {
        if name.eq_ignore_ascii_case("x-fn-knock-trace-id") {
            continue;
        }
        if value
            .as_str()
            .is_some_and(|value| value.chars().any(char::is_control))
        {
            return Err(webhook_header_error(
                "invalidHeaderValue",
                &[("name", name.to_string())],
            ));
        }
        if let Some(value) = value_to_header_string(value) {
            entries.push(json!({ "name": name, "value": value }));
        }
    }
    parse_webhook_custom_headers(&Value::Array(entries))
}

pub(in crate::notifications::routes) fn resolve_webhook_headers(
    config: &Map<String, Value>,
    target_config: Option<&Map<String, Value>>,
) -> Result<Vec<WebhookHeader>, WebhookHeaderValidationError> {
    if let Some(headers) = config.get("custom_headers") {
        return parse_webhook_custom_headers(headers);
    }
    target_config
        .map(legacy_webhook_headers)
        .unwrap_or_else(|| Ok(Vec::new()))
}

#[allow(clippy::too_many_arguments)]
async fn execute_webhook_request(
    state: &AppState,
    config: &Map<String, Value>,
    target_config: Option<&Map<String, Value>>,
    method: String,
    url: String,
    body: Value,
    timeout_seconds: i64,
    translator: &Translator,
) -> ProviderTestResult {
    let headers = match resolve_webhook_headers(config, target_config) {
        Ok(headers) => headers,
        Err(error) => {
            return ProviderTestResult {
                success: false,
                retryable: false,
                message: error.text(translator),
                request_summary: None,
                response_summary: None,
            };
        }
    };
    let mut request = match method.as_str() {
        "PUT" => state.fallback_client.put(&url),
        _ => state.fallback_client.post(&url),
    }
    .header("content-type", "application/json")
    .header("x-fn-knock-provider", "webhook");
    let mut header_names = vec![
        "content-type".to_string(),
        "x-fn-knock-provider".to_string(),
    ];
    for header in headers {
        header_names.push(header.name.clone());
        request = request.header(header.name.as_str(), header.value.as_str());
    }
    if let Some(secret) = config
        .get("shared_secret")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        request = request.header("x-fn-knock-signature", secret);
        header_names.push("x-fn-knock-signature".to_string());
    }
    let request_summary = json!({
        "method": method,
        "url": url,
        "header_names": header_names,
        "body_preview": {
            "title": body.pointer("/message/title").cloned().unwrap_or(Value::Null),
            "severity": body.pointer("/message/severity").cloned().unwrap_or(Value::Null),
            "event_id": body.pointer("/context/event_id").cloned().unwrap_or(Value::Null)
        }
    });

    match time::timeout(Duration::from_secs(timeout_seconds.max(1) as u64), async {
        let response = request.json(&body).send().await?;
        Ok::<_, reqwest::Error>(read_provider_response(response).await)
    })
    .await
    {
        Ok(Ok((status, ok, text, _))) => {
            let response_summary = json!({
                "status": status,
                "ok": ok,
                "body_preview": truncate_text(&text, 500)
            });
            if ok {
                ProviderTestResult {
                    success: true,
                    retryable: false,
                    message: notification_service_text(translator, "testSendSuccess", &[]),
                    request_summary: Some(request_summary),
                    response_summary: Some(response_summary),
                }
            } else {
                ProviderTestResult {
                    success: false,
                    retryable: status >= 500 || status == 429,
                    message: notification_provider_error_text(
                        translator,
                        "webhook",
                        "requestReturned",
                        &[("status", status.to_string())],
                    ),
                    request_summary: Some(request_summary),
                    response_summary: Some(response_summary),
                }
            }
        }
        Ok(Err(error)) => ProviderTestResult {
            success: false,
            retryable: true,
            message: error.to_string(),
            request_summary: Some(request_summary),
            response_summary: None,
        },
        Err(_) => ProviderTestResult {
            success: false,
            retryable: true,
            message: notification_provider_error_text(translator, "webhook", "requestFailed", &[]),
            request_summary: Some(request_summary),
            response_summary: None,
        },
    }
}

pub(in crate::notifications::routes) async fn send_webhook_test(
    state: &AppState,
    provider: &Value,
    translator: &Translator,
) -> Result<ProviderTestResult, String> {
    let config = provider
        .get("connection_config")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            notification_provider_error_text(translator, "webhook", "missingUrl", &[])
        })?;
    let url = config
        .get("url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            notification_provider_error_text(translator, "webhook", "missingUrl", &[])
        })?;
    let message = build_provider_test_message(translator);
    let body = json!({
        "source": "fn_knock",
        "provider_type": "webhook",
        "message": message,
        "context": { "mode": "provider_test" },
        "payload": { "extra_body": {} }
    });
    Ok(execute_webhook_request(
        state,
        config,
        None,
        webhook_method(config),
        url.to_string(),
        body,
        provider_timeout_seconds(provider, 5),
        translator,
    )
    .await)
}

#[allow(clippy::too_many_arguments)]
pub(in crate::notifications::routes) async fn send_webhook_delivery(
    state: &AppState,
    provider: &Value,
    target: &Value,
    delivery: &Value,
    trigger: &Value,
    rule: &Value,
    timeout_seconds: i64,
    translator: &Translator,
) -> ProviderTestResult {
    let config = provider_config(provider);
    let target_config = target_config(target);
    let Some(base_url) = config
        .get("url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return missing_config_result(&notification_provider_error_text(
            translator,
            "webhook",
            "missingUrl",
            &[],
        ));
    };
    let endpoint_path = target_config
        .get("endpoint_path")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    let url = resolve_webhook_url(base_url, endpoint_path);
    let message = sanitize_notification_message(
        &delivery
            .get("message_snapshot")
            .cloned()
            .unwrap_or_else(|| json!({})),
    );
    let extra_body = target_config
        .get("extra_body_json")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let body = json!({
        "source": "fn_knock",
        "provider_type": "webhook",
        "message": message,
        "context": {
            "trigger_id": trigger.get("id").cloned().unwrap_or(Value::Null),
            "delivery_id": delivery.get("id").cloned().unwrap_or(Value::Null),
            "rule_id": rule.get("id").cloned().unwrap_or(Value::Null),
            "target_id": target.get("id").cloned().unwrap_or(Value::Null),
            "event_id": delivery.get("event_id").cloned().unwrap_or(Value::Null)
        },
        "payload": { "extra_body": extra_body }
    });
    execute_webhook_request(
        state,
        &config,
        Some(&target_config),
        webhook_method(&config),
        url,
        body,
        timeout_seconds,
        translator,
    )
    .await
}
