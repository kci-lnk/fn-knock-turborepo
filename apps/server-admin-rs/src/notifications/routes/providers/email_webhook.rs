use super::*;

pub(in crate::notifications::routes) async fn send_email_notification(
    provider: &Value,
    target: &Value,
    message: &Value,
    timeout_seconds: i64,
    translator: &Translator,
) -> ProviderTestResult {
    let config = provider_config(provider);
    let target_config = target_config(target);
    let smtp_host = config_text(&config, "smtp_host");
    let from_address = config_text(&config, "from_address");
    if smtp_host.is_empty() {
        return missing_config_result(&notification_provider_error_default(
            "email",
            "missingSmtpHost",
            &[],
        ));
    }
    if from_address.is_empty() {
        return missing_config_result(&notification_provider_error_default(
            "email",
            "invalidFromAddress",
            &[],
        ));
    }
    let smtp_security = default_string(config_text(&config, "smtp_security"), "ssl_tls");
    let smtp_port = config
        .get("smtp_port")
        .map(|value| value_to_i64(value, default_smtp_port(&smtp_security)))
        .unwrap_or_else(|| default_smtp_port(&smtp_security))
        .clamp(1, 65535) as u16;
    let auth_mode = default_string(config_text(&config, "smtp_auth_mode"), "auto");
    let smtp_username = config_text(&config, "smtp_username");
    let smtp_password = config_text(&config, "smtp_password");
    let from_name = config_text(&config, "from_name");
    let subject_prefix = config_text(&target_config, "subject_prefix");
    let subject = [subject_prefix, message_title(message)]
        .into_iter()
        .filter(|value| !value.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let to_addresses =
        config_text(&target_config, "to_addresses").if_empty(config_text(&config, "to_addresses"));
    let cc_addresses =
        config_text(&target_config, "cc_addresses").if_empty(config_text(&config, "cc_addresses"));
    let bcc_addresses = config_text(&target_config, "bcc_addresses")
        .if_empty(config_text(&config, "bcc_addresses"));
    let reply_to =
        config_text(&target_config, "reply_to").if_empty(config_text(&config, "reply_to"));
    let to = match parse_mailboxes(&to_addresses, "to_addresses", translator) {
        Ok(value) if !value.is_empty() => value,
        Ok(_) => {
            return missing_config_result(&notification_provider_error_default(
                "email",
                "recipientRequired",
                &[],
            ));
        }
        Err(message) => return missing_config_result(&message),
    };
    let cc = match parse_mailboxes(&cc_addresses, "cc_addresses", translator) {
        Ok(value) => value,
        Err(message) => return missing_config_result(&message),
    };
    let bcc = match parse_mailboxes(&bcc_addresses, "bcc_addresses", translator) {
        Ok(value) => value,
        Err(message) => return missing_config_result(&message),
    };
    let reply_to = match parse_mailboxes(&reply_to, "reply_to", translator) {
        Ok(value) => value,
        Err(message) => return missing_config_result(&message),
    };
    let from = match build_from_mailbox(&from_address, &from_name, translator) {
        Ok(value) => value,
        Err(message) => return missing_config_result(&message),
    };

    let fallback_subject = notification_email_message_text(translator, "fallbackTitle", &[]);
    let mut builder = Message::builder()
        .from(from)
        .subject(default_string(subject.clone(), &fallback_subject));
    for item in &to {
        builder = builder.to(item.clone());
    }
    for item in &cc {
        builder = builder.cc(item.clone());
    }
    for item in &bcc {
        builder = builder.bcc(item.clone());
    }
    for item in &reply_to {
        builder = builder.reply_to(item.clone());
    }
    let email = match builder.body(build_email_plain_text_body(message, translator)) {
        Ok(value) => value,
        Err(error) => {
            return ProviderTestResult {
                success: false,
                message: error.to_string(),
                request_summary: None,
                response_summary: None,
            };
        }
    };

    let transport_result = build_smtp_transport(
        &smtp_host,
        smtp_port,
        &smtp_security,
        &auth_mode,
        &smtp_username,
        &smtp_password,
    );
    let mailer = match transport_result {
        Ok(value) => value,
        Err(message) => return missing_config_result(&message),
    };
    let request_summary = json!({
        "method": "SMTP",
        "host": smtp_host,
        "port": smtp_port,
        "security": smtp_security,
        "auth_mode": auth_mode,
        "to_count": to.len(),
        "cc_count": cc.len(),
        "bcc_count": bcc.len(),
        "subject_preview": truncate_text(&subject, 160)
    });
    match time::timeout(
        Duration::from_secs(timeout_seconds.max(1) as u64),
        mailer.send(email),
    )
    .await
    {
        Ok(Ok(response)) => ProviderTestResult {
            success: true,
            message: notification_service_default_text("testSendSuccess", &[]),
            request_summary: Some(request_summary),
            response_summary: Some(json!({
                "ok": true,
                "code": response.code().to_string(),
                "message": response.message().collect::<Vec<_>>().join("\n")
            })),
        },
        Ok(Err(error)) => ProviderTestResult {
            success: false,
            message: error.to_string(),
            request_summary: Some(request_summary),
            response_summary: Some(json!({ "ok": false, "error": error.to_string() })),
        },
        Err(_) => ProviderTestResult {
            success: false,
            message: notification_provider_error_default("email", "smtpConnectionTimeout", &[]),
            request_summary: Some(request_summary),
            response_summary: Some(json!({ "ok": false, "timeout": true })),
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
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            let response_summary = json!({
                "status": status.as_u16(),
                "ok": status.is_success(),
                "body_preview": truncate_text(&text, 500)
            });
            if status.is_success() {
                Ok(ProviderTestResult {
                    success: true,
                    message: notification_service_text(translator, "testSendSuccess", &[]),
                    request_summary: Some(request_summary),
                    response_summary: Some(response_summary),
                })
            } else {
                Ok(ProviderTestResult {
                    success: false,
                    message: notification_provider_error_text(
                        translator,
                        "webhook",
                        "requestReturned",
                        &[("status", status.as_u16().to_string())],
                    ),
                    request_summary: Some(request_summary),
                    response_summary: Some(response_summary),
                })
            }
        }
        Err(error) => Ok(ProviderTestResult {
            success: false,
            message: error.to_string(),
            request_summary: Some(request_summary),
            response_summary: None,
        }),
    }
}

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
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            let response_summary = json!({
                "status": status.as_u16(),
                "ok": status.is_success(),
                "body_preview": truncate_text(&text, 500)
            });
            if status.is_success() {
                ProviderTestResult {
                    success: true,
                    message: notification_service_text(translator, "testSendSuccess", &[]),
                    request_summary: Some(request_summary),
                    response_summary: Some(response_summary),
                }
            } else {
                ProviderTestResult {
                    success: false,
                    message: notification_provider_error_text(
                        translator,
                        "webhook",
                        "requestReturned",
                        &[("status", status.as_u16().to_string())],
                    ),
                    request_summary: Some(request_summary),
                    response_summary: Some(response_summary),
                }
            }
        }
        Ok(Err(error)) => ProviderTestResult {
            success: false,
            message: error.to_string(),
            request_summary: Some(request_summary),
            response_summary: None,
        },
        Err(_) => ProviderTestResult {
            success: false,
            message: notification_provider_error_default("webhook", "requestFailed", &[]),
            request_summary: Some(request_summary),
            response_summary: None,
        },
    }
}
