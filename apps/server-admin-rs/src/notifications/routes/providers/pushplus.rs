use super::*;

pub(in crate::notifications::routes) fn pushplus_definition() -> ProviderDefinition {
    ProviderDefinition {
        provider_type: "pushplus",
        label: "PushPlus",
        description: "Send notifications through PushPlus.",
        connection_schema: vec![
            string_schema(
                "server_url",
                "Server URL",
                true,
                false,
                Some("https://www.pushplus.plus"),
            )
            .placeholder("https://www.pushplus.plus"),
            string_schema("token", "Token", true, true, None)
                .placeholder("xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"),
            number_schema("timeout_seconds", "Timeout seconds", true, Some(5)).bounds(1, 30),
        ],
        target_schema: vec![
            string_schema("topic", "Topic", false, false, None).placeholder("alarm-topic"),
            select_schema(
                "template",
                "Template",
                false,
                Some("markdown"),
                &["markdown", "html", "txt", "json"],
            ),
            select_schema(
                "channel",
                "Channel",
                false,
                Some("wechat"),
                &[
                    "wechat",
                    "webhook",
                    "cp",
                    "mail",
                    "sms",
                    "voice",
                    "extension",
                    "app",
                    "clawbot",
                ],
            ),
            string_schema("option", "Option", false, false, None).placeholder("my-channel-code"),
            string_schema("to", "Recipient", false, false, None)
                .placeholder("friend_token or user1,user2"),
            string_schema("callback_url", "Callback URL", false, false, None)
                .placeholder("https://example.com/hooks/pushplus"),
            string_schema("pre", "Pre", false, false, None).placeholder("appendMsg"),
        ],
        sensitive_fields: vec!["token"],
        supports_markdown: true,
        supports_actions: true,
        supports_mentions: false,
        supports_provider_dedupe_key: false,
    }
}

pub(in crate::notifications::routes) async fn send_pushplus(
    state: &AppState,
    provider: &Value,
    target: &Value,
    message: &Value,
    timeout_seconds: i64,
) -> ProviderTestResult {
    let config = provider_config(provider);
    let target_config = target_config(target);
    let token = config_text(&config, "token");
    if token.is_empty() {
        return missing_config_result(&notification_provider_error_default(
            "pushplus",
            "missingToken",
            &[],
        ));
    }
    let url = resolve_pushplus_url(&default_string(
        config_text(&config, "server_url"),
        "https://www.pushplus.plus",
    ));
    let template = match config_text(&target_config, "template").as_str() {
        "html" => "html",
        "txt" => "txt",
        "json" => "json",
        _ => "markdown",
    };
    let channel = default_string(config_text(&target_config, "channel"), "wechat");
    let topic = config_text(&target_config, "topic");
    let option = config_text(&target_config, "option");
    let to = config_text(&target_config, "to");
    let callback_url = config_text(&target_config, "callback_url");
    let pre = config_text(&target_config, "pre");
    let title = truncate_text(&message_title(message), 128);
    let content = match template {
        "html" => build_pushplus_html_content(message),
        "txt" => build_pushplus_text_content(message),
        "json" => build_pushplus_json_content(message),
        _ => build_pushplus_markdown_content(message),
    };
    let mut body = json!({
        "token": token,
        "title": title,
        "content": default_string(content, "fn-knock"),
        "template": template,
        "channel": channel
    });
    insert_non_empty(&mut body, "topic", topic.clone());
    insert_non_empty(&mut body, "option", option.clone());
    insert_non_empty(&mut body, "to", to.clone());
    insert_non_empty(&mut body, "callbackUrl", callback_url.clone());
    insert_non_empty(&mut body, "pre", pre.clone());
    let request_summary = json!({
        "method": "POST",
        "endpoint": url,
        "channel": channel,
        "template": template,
        "has_topic": !topic.is_empty(),
        "has_option": !option.is_empty(),
        "has_to": !to.is_empty(),
        "has_callback_url": !callback_url.is_empty(),
        "has_pre": !pre.is_empty(),
        "title_preview": title
    });
    let (status, ok, text, parsed) = post_json(state, &url, &body, timeout_seconds).await;
    provider_result_from_api(
        "PushPlus",
        request_summary,
        status,
        ok,
        text,
        parsed,
        |value| json_i64(value, "code") == Some(200),
        |value| json_text_any(value, &["msg", "message", "error"]),
    )
}
