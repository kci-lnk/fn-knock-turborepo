use super::*;

pub(in crate::notifications::routes) fn telegram_definition() -> ProviderDefinition {
    ProviderDefinition {
        provider_type: "telegram",
        label: "Telegram",
        description: "Send notifications through Telegram bot API.",
        connection_schema: vec![
            string_schema(
                "server_url",
                "Server URL",
                true,
                false,
                Some("https://api.telegram.org"),
            )
            .placeholder("https://api.telegram.org"),
            string_schema("bot_token", "Bot Token", true, true, None)
                .placeholder("123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11"),
            string_schema("chat_id", "Chat ID", true, false, None).placeholder("-1001234567890"),
            number_schema("timeout_seconds", "Timeout seconds", true, Some(5)).bounds(1, 30),
        ],
        target_schema: vec![
            number_schema("message_thread_id", "Topic ID", false, None).min(1),
            bool_schema(
                "disable_notification",
                "Disable notification",
                false,
                Some(false),
            ),
        ],
        sensitive_fields: vec!["bot_token"],
        supports_markdown: false,
        supports_actions: true,
        supports_mentions: false,
        supports_provider_dedupe_key: false,
    }
}

pub(in crate::notifications::routes) async fn send_telegram(
    state: &AppState,
    provider: &Value,
    target: &Value,
    message: &Value,
    timeout_seconds: i64,
) -> ProviderTestResult {
    let config = provider_config(provider);
    let target_config = target_config(target);
    let base_url = default_string(
        config_text(&config, "server_url"),
        "https://api.telegram.org",
    );
    let bot_token = config_text(&config, "bot_token");
    let chat_id = config_text(&config, "chat_id");
    if bot_token.is_empty() {
        return missing_config_result(&notification_provider_error_default(
            "telegram",
            "missingBotToken",
            &[],
        ));
    }
    if chat_id.is_empty() {
        return missing_config_result(&notification_provider_error_default(
            "telegram",
            "missingChatId",
            &[],
        ));
    }
    let url = format!(
        "{}/bot{}/sendMessage",
        base_url.trim_end_matches('/'),
        bot_token
    );
    let reply_markup = build_telegram_reply_markup(message);
    let message_thread_id = optional_positive_i64(target_config.get("message_thread_id"));
    let disable_notification = target_config
        .get("disable_notification")
        .map(value_to_bool)
        .unwrap_or(false);
    let mut body = json!({
        "chat_id": chat_id,
        "text": default_string(build_telegram_text(message), DEFAULT_NOTIFICATION_MESSAGE_TITLE),
        "parse_mode": "HTML",
    });
    if disable_notification {
        insert_value(&mut body, "disable_notification", Value::Bool(true));
    }
    if let Some(message_thread_id) = message_thread_id {
        insert_i64(&mut body, "message_thread_id", message_thread_id);
    }
    if let Some(reply_markup) = reply_markup {
        insert_value(&mut body, "reply_markup", reply_markup);
    }
    let request_summary = json!({
        "method": "POST",
        "url": format!("{}/bot<redacted>/sendMessage", base_url.trim_end_matches('/')),
        "chat_id": chat_id,
        "message_thread_id": message_thread_id,
        "disable_notification": disable_notification,
        "has_inline_keyboard": body.get("reply_markup").is_some(),
        "text_preview": truncate_text(&message_title(message), 120)
    });
    let (status, ok, text, parsed) = post_json(state, &url, &body, timeout_seconds).await;
    provider_result_from_api(
        "Telegram",
        request_summary,
        status,
        ok,
        text,
        parsed,
        |value| value.get("ok").and_then(Value::as_bool).unwrap_or(true),
        |value| json_text(value, "description"),
    )
}
