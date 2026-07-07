use super::*;

pub(in crate::notifications::routes) fn apply_keyword_prefix(value: &str, keyword: &str) -> String {
    let keyword = keyword.trim();
    let value = value.trim();
    if keyword.is_empty() || value.contains(keyword) {
        value.to_string()
    } else if value.is_empty() {
        keyword.to_string()
    } else {
        format!("[{keyword}] {value}")
    }
}

pub(in crate::notifications::routes) fn hmac_sha256_base64(key: &[u8], payload: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts keys of any size");
    mac.update(payload);
    BASE64_STANDARD.encode(mac.finalize().into_bytes())
}

pub(in crate::notifications::routes) fn append_query_params(
    url: &str,
    params: &[(&str, String)],
) -> String {
    if let Ok(mut parsed) = url::Url::parse(url) {
        {
            let mut query = parsed.query_pairs_mut();
            for (key, value) in params {
                query.append_pair(key, value);
            }
        }
        return parsed.to_string();
    }
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in params {
        serializer.append_pair(key, value);
    }
    let query = serializer.finish();
    format!(
        "{}{}{}",
        url,
        if url.contains('?') { "&" } else { "?" },
        query
    )
}

pub(in crate::notifications::routes) fn redact_query_value(value: &str, key: &str) -> String {
    if let Ok(mut url) = url::Url::parse(value) {
        if url.query_pairs().any(|(name, _)| name == key) {
            let pairs = url
                .query_pairs()
                .map(|(name, value)| {
                    if name == key {
                        (name.to_string(), "<redacted>".to_string())
                    } else {
                        (name.to_string(), value.to_string())
                    }
                })
                .collect::<Vec<_>>();
            url.set_query(None);
            for (name, value) in pairs {
                url.query_pairs_mut().append_pair(&name, &value);
            }
        }
        return url.to_string();
    }
    value.replace(&format!("{key}="), &format!("{key}=<redacted>"))
}

pub(in crate::notifications::routes) fn redact_path_tail(value: &str) -> String {
    if let Ok(mut url) = url::Url::parse(value) {
        if let Some(mut segments) = url.path_segments_mut().ok() {
            segments.pop().push("<redacted>");
        }
        return url.to_string();
    }
    value.to_string()
}

pub(in crate::notifications::routes) fn resolve_pushplus_url(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    let lower = trimmed.to_ascii_lowercase();
    if lower.ends_with("/send") || lower.ends_with("/batchsend") {
        trimmed.to_string()
    } else {
        format!("{trimmed}/send")
    }
}

pub(in crate::notifications::routes) fn resolve_magicpush_url(
    base_url: &str,
    token: &str,
    delivery_mode: &str,
) -> String {
    let base = base_url.trim_end_matches('/');
    let lower = base.to_ascii_lowercase();
    if delivery_mode == "inbound" {
        if path_matches_magicpush_endpoint_with_tail(&lower, "/api/inbound") {
            base.to_string()
        } else if lower.ends_with("/api/inbound") {
            format!("{base}/{}", url_encode_component(token))
        } else {
            format!("{base}/api/inbound/{}", url_encode_component(token))
        }
    } else if lower.ends_with("/api/push")
        || path_matches_magicpush_endpoint_with_tail(&lower, "/api/push")
    {
        base.to_string()
    } else {
        format!("{base}/api/push")
    }
}

pub(in crate::notifications::routes) fn path_matches_magicpush_endpoint_with_tail(
    value: &str,
    endpoint: &str,
) -> bool {
    let Some((prefix, tail)) = value.rsplit_once('/') else {
        return false;
    };
    prefix.ends_with(endpoint) && !tail.is_empty() && !tail.contains('/')
}

pub(in crate::notifications::routes) fn url_encode_component(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

pub(in crate::notifications::routes) fn json_i64(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(value_as_i64)
}

pub(in crate::notifications::routes) fn json_i64_any(value: &Value, keys: &[&str]) -> Option<i64> {
    keys.iter().find_map(|key| json_i64(value, key))
}

pub(in crate::notifications::routes) fn value_as_i64(value: &Value) -> Option<i64> {
    value.as_i64().or_else(|| {
        value
            .as_str()
            .and_then(|value| parse_int_prefix_like_node(value, 10))
    })
}

pub(in crate::notifications::routes) fn json_text(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .map(value_to_trimmed_string)
        .filter(|value| !value.is_empty())
}

pub(in crate::notifications::routes) fn json_text_any(
    value: &Value,
    keys: &[&str],
) -> Option<String> {
    keys.iter().find_map(|key| json_text(value, key))
}

pub(in crate::notifications::routes) fn default_smtp_port(security: &str) -> i64 {
    match security {
        "starttls" => 587,
        "none" => 25,
        _ => 465,
    }
}

pub(in crate::notifications::routes) fn build_smtp_transport(
    host: &str,
    port: u16,
    security: &str,
    auth_mode: &str,
    username: &str,
    password: &str,
) -> Result<AsyncSmtpTransport<Tokio1Executor>, String> {
    let mut builder = match security {
        "none" => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(host).tls(Tls::None),
        "starttls" => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host)
            .map_err(|error| error.to_string())?,
        _ => {
            AsyncSmtpTransport::<Tokio1Executor>::relay(host).map_err(|error| error.to_string())?
        }
    }
    .port(port);
    if auth_mode != "none" && !username.trim().is_empty() {
        builder = builder.credentials(Credentials::new(username.to_string(), password.to_string()));
    }
    Ok(builder.build())
}

pub(in crate::notifications::routes) fn parse_mailboxes(
    value: &str,
    field_key: &str,
    translator: &Translator,
) -> Result<Vec<Mailbox>, String> {
    let field_label =
        notification_provider_field_text(translator, "email", field_key, "addressLabel", field_key);
    value
        .split([',', '\n'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            value.parse::<Mailbox>().map_err(|_| {
                notification_provider_error_text(
                    translator,
                    "email",
                    "invalidEmailAddress",
                    &[("field", field_label.clone()), ("value", value.to_string())],
                )
            })
        })
        .collect()
}

pub(in crate::notifications::routes) fn build_from_mailbox(
    address: &str,
    name: &str,
    translator: &Translator,
) -> Result<Mailbox, String> {
    if name.trim().is_empty() {
        return address.parse::<Mailbox>().map_err(|_| {
            notification_provider_error_text(translator, "email", "invalidFromAddress", &[])
        });
    }
    let address = address.parse::<Address>().map_err(|_| {
        notification_provider_error_text(translator, "email", "invalidFromAddress", &[])
    })?;
    Ok(Mailbox::new(Some(name.trim().to_string()), address))
}

pub(in crate::notifications::routes) fn build_email_plain_text_body(
    message: &Value,
    translator: &Translator,
) -> String {
    let mut body = build_text_body(message);
    let mut footer = Vec::new();
    let severity = message_text(message, "severity");
    if !severity.is_empty() {
        footer.push(notification_email_message_text(
            translator,
            "severity",
            &[("value", severity)],
        ));
    }
    let event_id = message_text(message, "event_id");
    if !event_id.is_empty() {
        footer.push(notification_email_message_text(
            translator,
            "eventId",
            &[("value", event_id)],
        ));
    }
    let occurred_at = message_text(message, "occurred_at");
    if !occurred_at.is_empty() {
        footer.push(notification_email_message_text(
            translator,
            "occurredAt",
            &[("value", occurred_at)],
        ));
    }
    if !footer.is_empty() {
        body.push_str("\n\n");
        body.push_str(&footer.join("\n"));
    }
    body
}

pub(in crate::notifications::routes) fn resolve_webhook_url(
    base_url: &str,
    endpoint_path: &str,
) -> String {
    if endpoint_path.trim().is_empty() {
        return base_url.to_string();
    }
    if let Ok(base) = url::Url::parse(base_url)
        && let Ok(joined) = base.join(endpoint_path)
    {
        return joined.to_string();
    }
    format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        endpoint_path.trim_start_matches('/')
    )
}

pub(in crate::notifications::routes) fn value_to_header_string(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Value::Bool(value) => Some(value.to_string()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

pub(in crate::notifications::routes) fn apply_provider_test_result(
    provider: &mut Value,
    result: &ProviderTestResult,
) {
    let Some(object) = provider.as_object_mut() else {
        return;
    };
    object.insert(
        "last_test_at".to_string(),
        Value::String(time_utils::now_iso()),
    );
    object.insert(
        "last_test_status".to_string(),
        Value::String(if result.success { "success" } else { "failed" }.to_string()),
    );
    object.insert(
        "last_error".to_string(),
        if result.success {
            Value::Null
        } else {
            Value::String(result.message.clone())
        },
    );
    object.insert(
        "updated_at".to_string(),
        Value::String(time_utils::now_iso()),
    );
}
