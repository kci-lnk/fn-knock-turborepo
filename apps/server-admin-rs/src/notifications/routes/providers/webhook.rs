use super::*;

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
        ],
        target_schema: vec![
            string_schema("endpoint_path", "Endpoint path", false, false, None)
                .placeholder("/alerts"),
            json_schema("extra_headers_json", "Extra headers", false)
                .placeholder(r#"{"X-Env":"prod"}"#),
            json_schema("extra_body_json", "Extra body", false)
                .placeholder(r#"{"service":"gateway"}"#),
        ],
        sensitive_fields: vec!["url", "shared_secret"],
        supports_markdown: true,
        supports_actions: true,
        supports_mentions: true,
        supports_provider_dedupe_key: true,
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
        .ok_or_else(|| notification_provider_error_default("webhook", "missingUrl", &[]))?;
    let method = config
        .get("method")
        .and_then(Value::as_str)
        .map(|value| value.trim().to_uppercase())
        .filter(|value| value == "POST" || value == "PUT")
        .unwrap_or_else(|| "POST".to_string());
    let message = build_provider_test_message(translator);
    let body = json!({
        "source": "fn_knock",
        "provider_type": "webhook",
        "message": message,
        "context": { "mode": "provider_test" },
        "payload": { "extra_body": {} }
    });

    let mut request = match method.as_str() {
        "PUT" => state.fallback_client.put(url),
        _ => state.fallback_client.post(url),
    }
    .header("content-type", "application/json")
    .header("x-fn-knock-provider", "webhook")
    .json(&body);
    let mut header_names = vec![
        "content-type".to_string(),
        "x-fn-knock-provider".to_string(),
    ];
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
            "title": message.get("title").cloned().unwrap_or(Value::Null),
            "severity": "info",
            "event_id": Value::Null
        }
    });

    match request.send().await {
        Ok(response) => {
            let (status, ok, text, _) = read_provider_response(response).await;
            let response_summary = json!({
                "status": status,
                "ok": ok,
                "body_preview": truncate_text(&text, 500)
            });
            if ok {
                Ok(ProviderTestResult {
                    success: true,
                    retryable: false,
                    message: notification_service_text(translator, "testSendSuccess", &[]),
                    request_summary: Some(request_summary),
                    response_summary: Some(response_summary),
                })
            } else {
                Ok(ProviderTestResult {
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
                })
            }
        }
        Err(error) => Ok(ProviderTestResult {
            success: false,
            retryable: true,
            message: error.to_string(),
            request_summary: Some(request_summary),
            response_summary: None,
        }),
    }
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
    let config = provider
        .get("connection_config")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let target_config = target
        .get("target_config")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let Some(base_url) = config
        .get("url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return ProviderTestResult {
            success: false,
            retryable: false,
            message: notification_provider_error_default("webhook", "missingUrl", &[]),
            request_summary: None,
            response_summary: None,
        };
    };
    let endpoint_path = target_config
        .get("endpoint_path")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    let url = resolve_webhook_url(base_url, endpoint_path);
    let method = config
        .get("method")
        .and_then(Value::as_str)
        .map(|value| value.trim().to_uppercase())
        .filter(|value| value == "POST" || value == "PUT")
        .unwrap_or_else(|| "POST".to_string());
    let message = delivery
        .get("message_snapshot")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let extra_headers = target_config
        .get("extra_headers_json")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
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
    for (key, value) in extra_headers {
        if let Some(value) = value_to_header_string(&value) {
            header_names.push(key.clone());
            request = request.header(key, value);
        }
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

    match time::timeout(
        Duration::from_secs(timeout_seconds.max(1) as u64),
        request.json(&body).send(),
    )
    .await
    {
        Ok(Ok(response)) => {
            let (status, ok, text, _) = read_provider_response(response).await;
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
            message: notification_provider_error_default("webhook", "requestFailed", &[]),
            request_summary: Some(request_summary),
            response_summary: None,
        },
    }
}
