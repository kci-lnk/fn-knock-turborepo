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
            "kind": "headers",
            "max_items": WEBHOOK_MAX_CUSTOM_HEADERS,
            "max_name_bytes": WEBHOOK_MAX_HEADER_NAME_BYTES,
            "max_value_bytes": WEBHOOK_MAX_HEADER_VALUE_BYTES,
            "max_total_bytes": WEBHOOK_MAX_HEADERS_TOTAL_BYTES,
            "reserved_names": WEBHOOK_RESERVED_HEADER_NAMES
        })),
    }
}

fn webhook_body_schema(
    key: &'static str,
    label: &'static str,
    scope: WebhookBodyScope,
) -> SchemaField {
    let (scope_name, default_mode) = match scope {
        WebhookBodyScope::Provider => ("provider", "standard"),
        WebhookBodyScope::Target => ("target", "inherit"),
    };
    SchemaField {
        key,
        label,
        field_type: "webhook_body",
        required: false,
        sensitive: scope == WebhookBodyScope::Provider,
        placeholder: None,
        default_value: Some(json!({ "mode": default_mode })),
        min: None,
        max: None,
        options: Vec::new(),
        constraints: Some(json!({
            "kind": "webhook_body",
            "scope": scope_name,
            "formats": ["json", "text"],
            "variable_roots": WEBHOOK_BODY_VARIABLE_ROOTS,
            "max_template_bytes": WEBHOOK_MAX_BODY_TEMPLATE_BYTES,
            "max_sample_bytes": WEBHOOK_MAX_BODY_SAMPLE_BYTES,
            "max_placeholders": WEBHOOK_MAX_BODY_PLACEHOLDERS,
            "max_rendered_bytes": WEBHOOK_MAX_RENDERED_BODY_BYTES,
            "max_content_type_bytes": WEBHOOK_MAX_CONTENT_TYPE_BYTES
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
        description: "Send a standard or custom JSON or text payload to an HTTP endpoint.",
        connection_schema: vec![
            string_schema("url", "Webhook URL", true, true, None)
                .placeholder("https://example.com/hooks/fn-knock"),
            select_schema("method", "Method", true, Some("POST"), &["POST", "PUT"]),
            number_schema("timeout_seconds", "Timeout seconds", true, Some(5)).bounds(1, 30),
            string_schema("shared_secret", "Shared secret", false, true, None)
                .placeholder("secret"),
            webhook_headers_schema(),
            webhook_body_schema("body_config", "Request body", WebhookBodyScope::Provider),
        ],
        target_schema: vec![
            string_schema("endpoint_path", "Endpoint path", false, false, None)
                .placeholder("/alerts"),
            webhook_body_schema(
                "body_override",
                "Request body override",
                WebhookBodyScope::Target,
            ),
        ],
        sensitive_fields: vec!["url", "shared_secret", "custom_headers", "body_config"],
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

#[derive(Clone, Debug, Default)]
pub(in crate::notifications::routes) struct WebhookTestOptions {
    pub(in crate::notifications::routes) target_config: Option<Map<String, Value>>,
    pub(in crate::notifications::routes) sample_context: Option<Value>,
}

pub(in crate::notifications::routes) fn webhook_test_options_from_body(
    body: &Value,
    translator: &Translator,
) -> NotifyResult<WebhookTestOptions> {
    let target_config = if let Some(raw_target) = body.get("target_config") {
        let raw_target = raw_target.as_object().ok_or_else(|| {
            NotifyError::BadRequest(notification_provider_error_text(
                translator,
                "webhook",
                "invalidBodyConfig",
                &[],
            ))
        })?;
        if let Some(body_override) = raw_target.get("body_override") {
            parse_webhook_body_config(body_override, WebhookBodyScope::Target)
                .map_err(|error| NotifyError::BadRequest(error.text(translator)))?;
        }
        let definition = webhook_definition();
        let mut normalized = normalize_schema_config(raw_target, &definition.target_schema)?;
        if let Some(extra_headers) = raw_target.get("extra_headers_json") {
            let extra_headers = normalize_json_field(extra_headers, "Extra headers")?;
            if !extra_headers.is_null() {
                normalized.insert("extra_headers_json".to_string(), extra_headers);
            }
        }
        if let Some(extra_body) = raw_target.get("extra_body_json") {
            let extra_body = normalize_json_field(extra_body, "Extra body")?;
            if !extra_body.is_null() {
                normalized.insert("extra_body_json".to_string(), extra_body);
            }
        }
        Some(normalized)
    } else {
        None
    };
    Ok(WebhookTestOptions {
        target_config,
        sample_context: body.get("sample_context").cloned(),
    })
}

fn webhook_body_error_result(
    error: &WebhookBodyValidationError,
    translator: &Translator,
) -> ProviderTestResult {
    ProviderTestResult {
        success: false,
        retryable: false,
        message: error.text(translator),
        request_summary: None,
        response_summary: None,
    }
}

fn prepare_webhook_body(
    config: &Map<String, Value>,
    target_config: Option<&Map<String, Value>>,
    standard_body: &Value,
    template_context: &Value,
) -> Result<RenderedWebhookBody, WebhookBodyValidationError> {
    match resolve_webhook_body_config(config, target_config)? {
        Some(body_config) => render_webhook_body(&body_config, template_context),
        None => render_standard_webhook_body(standard_body),
    }
}

#[allow(clippy::too_many_arguments)]
async fn execute_webhook_request(
    state: &AppState,
    config: &Map<String, Value>,
    target_config: Option<&Map<String, Value>>,
    method: String,
    url: String,
    body: RenderedWebhookBody,
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
    .header("content-type", body.content_type.as_str())
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
        "body_format": body.format.as_str(),
        "content_type": body.content_type,
        "body_bytes": body.bytes.len(),
        "missing_variable_count": body.missing_variables.len()
    });
    let request_body = body.bytes;

    match time::timeout(Duration::from_secs(timeout_seconds.max(1) as u64), async {
        let response = request.body(request_body).send().await?;
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

#[cfg(test)]
pub(in crate::notifications::routes) async fn send_webhook_test(
    state: &AppState,
    provider: &Value,
    translator: &Translator,
) -> Result<ProviderTestResult, String> {
    send_webhook_test_with_options(state, provider, translator, WebhookTestOptions::default()).await
}

pub(in crate::notifications::routes) async fn send_webhook_test_with_options(
    state: &AppState,
    provider: &Value,
    translator: &Translator,
    options: WebhookTestOptions,
) -> Result<ProviderTestResult, String> {
    let config = provider
        .get("connection_config")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            notification_provider_error_text(translator, "webhook", "missingUrl", &[])
        })?;
    let base_url = config
        .get("url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            notification_provider_error_text(translator, "webhook", "missingUrl", &[])
        })?;
    let target_config = options.target_config.as_ref();
    let endpoint_path = target_config
        .and_then(|target| target.get("endpoint_path"))
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    let url = resolve_webhook_url(base_url, endpoint_path);
    let message = build_provider_test_message(translator);
    let extra_body = target_config
        .and_then(|target| target.get("extra_body_json"))
        .cloned()
        .unwrap_or_else(|| json!({}));
    let mode = if target_config.is_some() {
        "target_test"
    } else {
        "provider_test"
    };
    let standard_body = json!({
        "source": "fn_knock",
        "provider_type": "webhook",
        "message": message.clone(),
        "context": { "mode": mode },
        "payload": { "extra_body": extra_body.clone() }
    });
    let target = json!({
        "id": "ntftarget_test",
        "provider_id": provider.get("id").cloned().unwrap_or(Value::Null)
    });
    let rule = json!({
        "id": "ntfrule_test",
        "name": "Webhook test",
        "event_type": "FN_EVENT_AUTH_LOGIN_SUCCESS",
        "group_by": "GLOBAL",
        "window_seconds": 60,
        "threshold_count": 1,
        "cooldown_seconds": 60
    });
    let event = json!({
        "id": "evt_webhook_test",
        "type": "FN_EVENT_AUTH_LOGIN_SUCCESS",
        "source": "SERVER_ADMIN",
        "level": "INFO",
        "happened_at": message.get("occurred_at").cloned().unwrap_or(Value::Null),
        "dedupe_key": Value::Null,
        "subject": { "kind": "APPLICATION", "id": "fn-knock" },
        "tags": ["test"],
        "payload": { "test": true }
    });
    let template_context = build_webhook_template_context(
        &message,
        &event,
        json!({
            "mode": mode,
            "trigger_id": Value::Null,
            "delivery_id": Value::Null,
            "event_id": "evt_webhook_test",
            "rule_id": "ntfrule_test",
            "target_id": "ntftarget_test",
            "provider_id": provider.get("id").cloned().unwrap_or(Value::Null)
        }),
        &rule,
        &target,
        provider,
        extra_body,
    );
    let template_context = match apply_webhook_sample_context(
        template_context,
        options.sample_context.as_ref(),
        mode,
        provider,
    ) {
        Ok(context) => context,
        Err(error) => return Ok(webhook_body_error_result(&error, translator)),
    };
    let body = match prepare_webhook_body(config, target_config, &standard_body, &template_context)
    {
        Ok(body) => body,
        Err(error) => return Ok(webhook_body_error_result(&error, translator)),
    };
    Ok(execute_webhook_request(
        state,
        config,
        target_config,
        webhook_method(config),
        url,
        body,
        provider_timeout_seconds(provider, 5),
        translator,
    )
    .await)
}

pub(in crate::notifications::routes) fn preview_webhook_body(
    provider: &Value,
    translator: &Translator,
    options: WebhookTestOptions,
) -> Result<Value, String> {
    let config = provider
        .get("connection_config")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            notification_provider_error_text(translator, "webhook", "missingUrl", &[])
        })?;
    let target_config = options.target_config.as_ref();
    let message = build_provider_test_message(translator);
    let extra_body = target_config
        .and_then(|target| target.get("extra_body_json"))
        .cloned()
        .unwrap_or_else(|| json!({}));
    let mode = if target_config.is_some() {
        "target_test"
    } else {
        "provider_test"
    };
    let standard_body = json!({
        "source": "fn_knock",
        "provider_type": "webhook",
        "message": message.clone(),
        "context": { "mode": mode },
        "payload": { "extra_body": extra_body.clone() }
    });
    let target = json!({
        "id": "ntftarget_test",
        "provider_id": provider.get("id").cloned().unwrap_or(Value::Null)
    });
    let rule = json!({
        "id": "ntfrule_test",
        "name": "Webhook test",
        "event_type": "FN_EVENT_AUTH_LOGIN_SUCCESS",
        "group_by": "GLOBAL",
        "window_seconds": 60,
        "threshold_count": 1,
        "cooldown_seconds": 60
    });
    let event = json!({
        "id": "evt_webhook_test",
        "type": "FN_EVENT_AUTH_LOGIN_SUCCESS",
        "source": "SERVER_ADMIN",
        "level": "INFO",
        "happened_at": message.get("occurred_at").cloned().unwrap_or(Value::Null),
        "dedupe_key": Value::Null,
        "subject": { "kind": "APPLICATION", "id": "fn-knock" },
        "tags": ["test"],
        "payload": { "test": true }
    });
    let context = build_webhook_template_context(
        &message,
        &event,
        json!({
            "mode": mode,
            "trigger_id": Value::Null,
            "delivery_id": Value::Null,
            "event_id": "evt_webhook_test",
            "rule_id": "ntfrule_test",
            "target_id": "ntftarget_test",
            "provider_id": provider.get("id").cloned().unwrap_or(Value::Null)
        }),
        &rule,
        &target,
        provider,
        extra_body,
    );
    let context =
        apply_webhook_sample_context(context, options.sample_context.as_ref(), mode, provider)
            .map_err(|error| error.text(translator))?;
    let body = prepare_webhook_body(config, target_config, &standard_body, &context)
        .map_err(|error| error.text(translator))?;
    Ok(json!({
        "format": body.format.as_str(),
        "content_type": body.content_type,
        "body": String::from_utf8_lossy(&body.bytes),
        "byte_length": body.bytes.len(),
        "missing_variables": body.missing_variables
    }))
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
    let standard_body = json!({
        "source": "fn_knock",
        "provider_type": "webhook",
        "message": message.clone(),
        "context": {
            "trigger_id": trigger.get("id").cloned().unwrap_or(Value::Null),
            "delivery_id": delivery.get("id").cloned().unwrap_or(Value::Null),
            "rule_id": rule.get("id").cloned().unwrap_or(Value::Null),
            "target_id": target.get("id").cloned().unwrap_or(Value::Null),
            "event_id": delivery.get("event_id").cloned().unwrap_or(Value::Null)
        },
        "payload": { "extra_body": extra_body }
    });
    let event = delivery
        .get("webhook_event_snapshot")
        .cloned()
        .unwrap_or_else(|| {
            json!({
                "id": delivery.get("event_id").cloned().unwrap_or(Value::Null),
                "type": message.pointer("/metadata/event_type").cloned().unwrap_or(Value::Null),
                "source": message.pointer("/metadata/event_source").cloned().unwrap_or(Value::Null),
                "level": message.pointer("/metadata/event_level").cloned().unwrap_or(Value::Null),
                "happened_at": message.get("occurred_at").cloned().unwrap_or(Value::Null),
                "payload": {}
            })
        });
    let rule_snapshot = trigger.get("rule_snapshot").unwrap_or(rule);
    let target_snapshot = delivery.get("target_snapshot").unwrap_or(target);
    let provider_snapshot = delivery.get("provider_snapshot").unwrap_or(provider);
    let template_context = build_webhook_template_context(
        &message,
        &event,
        json!({
            "mode": "delivery",
            "trigger_id": trigger.get("id").cloned().unwrap_or(Value::Null),
            "delivery_id": delivery.get("id").cloned().unwrap_or(Value::Null),
            "rule_id": rule.get("id").cloned().unwrap_or(Value::Null),
            "target_id": target.get("id").cloned().unwrap_or(Value::Null),
            "provider_id": provider.get("id").cloned().unwrap_or(Value::Null),
            "event_id": delivery.get("event_id").cloned().unwrap_or(Value::Null)
        }),
        rule_snapshot,
        target_snapshot,
        provider_snapshot,
        target_config
            .get("extra_body_json")
            .cloned()
            .unwrap_or_else(|| json!({})),
    );
    let body = match prepare_webhook_body(
        &config,
        Some(&target_config),
        &standard_body,
        &template_context,
    ) {
        Ok(body) => body,
        Err(error) => return webhook_body_error_result(&error, translator),
    };
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
