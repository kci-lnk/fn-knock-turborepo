use super::*;

pub(in crate::notifications::routes) fn email_definition() -> ProviderDefinition {
    ProviderDefinition {
        provider_type: "email",
        label: "Email",
        description: "Send notifications through SMTP.",
        connection_schema: vec![
            string_schema("smtp_host", "SMTP host", true, false, None)
                .placeholder("smtp.example.com"),
            number_schema("smtp_port", "SMTP port", true, Some(465)).bounds(1, 65535),
            select_schema(
                "smtp_security",
                "SMTP security",
                true,
                Some("ssl_tls"),
                &["ssl_tls", "starttls", "none"],
            ),
            select_schema(
                "smtp_auth_mode",
                "SMTP auth mode",
                true,
                Some("auto"),
                &["auto", "plain", "login", "none"],
            ),
            string_schema("smtp_username", "SMTP username", false, false, None)
                .placeholder("no-reply@example.com"),
            string_schema("smtp_password", "SMTP password", false, true, None)
                .placeholder("password"),
            string_schema("from_address", "From address", true, false, None)
                .placeholder("no-reply@example.com"),
            string_schema("from_name", "From name", false, false, None).placeholder("fn-knock"),
            string_schema("to_addresses", "To addresses", true, false, None)
                .placeholder("ops@example.com, admin@example.com"),
            string_schema("cc_addresses", "CC addresses", false, false, None)
                .placeholder("audit@example.com"),
            string_schema("bcc_addresses", "BCC addresses", false, false, None)
                .placeholder("archive@example.com"),
            string_schema("reply_to", "Reply-To", false, false, None)
                .placeholder("support@example.com"),
            bool_schema("allow_invalid_tls", "Allow invalid TLS", false, Some(false)),
            number_schema("timeout_seconds", "Timeout seconds", true, Some(10)).bounds(1, 30),
            string_schema("imap_host", "IMAP host", false, false, None)
                .placeholder("imap.example.com"),
            number_schema("imap_port", "IMAP port", false, Some(993)).bounds(1, 65535),
            select_schema(
                "imap_security",
                "IMAP security",
                false,
                Some("ssl_tls"),
                &["ssl_tls", "starttls", "none"],
            ),
            string_schema("imap_username", "IMAP username", false, false, None)
                .placeholder("no-reply@example.com"),
            string_schema("imap_password", "IMAP password", false, true, None)
                .placeholder("password"),
            string_schema("imap_mailbox", "IMAP mailbox", false, false, Some("INBOX"))
                .placeholder("INBOX"),
        ],
        target_schema: vec![
            string_schema("to_addresses", "To addresses", false, false, None)
                .placeholder("team@example.com"),
            string_schema("cc_addresses", "CC addresses", false, false, None)
                .placeholder("audit@example.com"),
            string_schema("bcc_addresses", "BCC addresses", false, false, None)
                .placeholder("archive@example.com"),
            string_schema("reply_to", "Reply-To", false, false, None)
                .placeholder("support@example.com"),
            string_schema("subject_prefix", "Subject prefix", false, false, None),
        ],
        sensitive_fields: vec!["smtp_password", "imap_password"],
        supports_markdown: false,
        supports_actions: true,
        supports_mentions: false,
        supports_provider_dedupe_key: false,
    }
}

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
                retryable: false,
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
            retryable: false,
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
            retryable: true,
            message: error.to_string(),
            request_summary: Some(request_summary),
            response_summary: Some(json!({ "ok": false, "error": error.to_string() })),
        },
        Err(_) => ProviderTestResult {
            success: false,
            retryable: true,
            message: notification_provider_error_default("email", "smtpConnectionTimeout", &[]),
            request_summary: Some(request_summary),
            response_summary: Some(json!({ "ok": false, "timeout": true })),
        },
    }
}
