use super::*;

pub(in crate::notifications::routes) fn pushdeer_definition() -> ProviderDefinition {
    ProviderDefinition {
        provider_type: "pushdeer",
        label: "PushDeer",
        description: "Send notifications through PushDeer.",
        connection_schema: vec![
            string_schema(
                "server_url",
                "Server URL",
                true,
                false,
                Some("https://api2.pushdeer.com"),
            )
            .placeholder("https://api2.pushdeer.com"),
            string_schema("pushkey", "PushKey", true, true, None).placeholder("PDUxxxx,PDUyyyy"),
            number_schema("timeout_seconds", "Timeout seconds", true, Some(5)).bounds(1, 30),
        ],
        target_schema: Vec::new(),
        sensitive_fields: vec!["pushkey"],
        supports_markdown: true,
        supports_actions: true,
        supports_mentions: false,
        supports_provider_dedupe_key: false,
    }
}

pub(in crate::notifications::routes) async fn send_pushdeer(
    state: &AppState,
    provider: &Value,
    message: &Value,
    timeout_seconds: i64,
) -> ProviderTestResult {
    let config = provider_config(provider);
    let pushkey = config_text(&config, "pushkey");
    if pushkey.is_empty() {
        return missing_config_result(&notification_provider_error_default(
            "pushdeer",
            "missingPushKey",
            &[],
        ));
    }
    let base_url = default_string(
        config_text(&config, "server_url"),
        "https://api2.pushdeer.com",
    );
    let url = format!("{}/message/push", base_url.trim_end_matches('/'));
    let form = vec![
        ("pushkey".to_string(), pushkey.clone()),
        ("text".to_string(), message_title(message)),
        ("desp".to_string(), build_markdown_body(message, "")),
        ("type".to_string(), "markdown".to_string()),
    ];
    let request_summary = json!({
        "method": "POST",
        "url": url,
        "pushkey_count": split_values(Some(&Value::String(pushkey))).len(),
        "type": "markdown",
        "title_preview": message_title(message)
    });
    let (status, ok, text, parsed) = post_form(state, &url, &form, timeout_seconds).await;
    provider_result_from_api(
        "PushDeer",
        request_summary,
        status,
        ok,
        text,
        parsed,
        |value| json_i64(value, "code").unwrap_or(0) == 0,
        |value| json_text_any(value, &["error", "message", "msg"]),
    )
}
