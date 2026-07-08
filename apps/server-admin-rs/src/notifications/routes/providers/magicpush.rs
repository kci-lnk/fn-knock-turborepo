use super::*;

pub(in crate::notifications::routes) fn magicpush_definition() -> ProviderDefinition {
    ProviderDefinition {
        provider_type: "magicpush",
        label: "MagicPush",
        description: "Send notifications through MagicPush.",
        connection_schema: vec![
            string_schema("server_url", "Server URL", true, false, None)
                .placeholder("http://192.168.31.98:3000"),
            select_schema(
                "delivery_mode",
                "Delivery mode",
                true,
                Some("push"),
                &["push", "inbound"],
            ),
            string_schema("token", "Token", true, false, None).placeholder("your_token"),
            number_schema("timeout_seconds", "Timeout seconds", true, Some(5)).bounds(1, 30),
        ],
        target_schema: Vec::new(),
        sensitive_fields: Vec::new(),
        supports_markdown: false,
        supports_actions: false,
        supports_mentions: false,
        supports_provider_dedupe_key: false,
    }
}

pub(in crate::notifications::routes) async fn send_magicpush(
    state: &AppState,
    provider: &Value,
    message: &Value,
    timeout_seconds: i64,
) -> ProviderTestResult {
    let config = provider_config(provider);
    let token = config_text(&config, "token");
    if token.is_empty() {
        return missing_config_result(&notification_provider_error_default(
            "magicpush",
            "missingToken",
            &[],
        ));
    }
    let base_url = config_text(&config, "server_url");
    if base_url.is_empty() {
        return missing_config_result(&notification_provider_error_default(
            "magicpush",
            "missingBaseUrl",
            &[],
        ));
    }
    let delivery_mode = if config_text(&config, "delivery_mode") == "inbound" {
        "inbound"
    } else {
        "push"
    };
    let url = resolve_magicpush_url(&base_url, &token, delivery_mode);
    let title = message_title(message);
    let content = default_string(build_magicpush_content(message), &title);
    let magicpush_facts = magicpush_facts_object(message);
    let payload = if delivery_mode == "inbound" {
        json!({
            "source": "fn-knock",
            "title": title,
            "summary": message.get("summary").cloned().unwrap_or(Value::Null),
            "content": content,
            "body": content,
            "body_text": message.get("body_text").cloned().unwrap_or(Value::Null),
            "body_markdown": message.get("body_markdown").cloned().unwrap_or(Value::Null),
            "type": if message_text(message, "body_markdown").is_empty() { "text" } else { "markdown" },
            "severity": message.get("severity").cloned().unwrap_or(Value::Null),
            "facts": magicpush_facts,
            "facts_list": message.get("facts").cloned().unwrap_or_else(|| json!([])),
            "actions": message.get("actions").cloned().unwrap_or_else(|| json!([])),
            "mentions": message.get("mentions").cloned().unwrap_or_else(|| json!([])),
            "dedupe_key": message.get("dedupe_key").cloned().unwrap_or(Value::Null),
            "occurred_at": message.get("occurred_at").cloned().unwrap_or(Value::Null),
            "event_id": message.get("event_id").cloned().unwrap_or(Value::Null),
            "metadata": message.get("metadata").cloned().unwrap_or_else(|| json!({}))
        })
    } else {
        json!({ "title": title, "content": content, "type": "text" })
    };
    let request_summary = json!({
        "method": "POST",
        "url": url,
        "delivery_mode": delivery_mode,
        "type": payload.get("type").cloned().unwrap_or(Value::Null),
        "title_preview": payload.get("title").cloned().unwrap_or(Value::Null),
        "content_preview": truncate_text(payload.get("content").and_then(Value::as_str).unwrap_or(""), 500)
    });
    let mut request = state
        .fallback_client
        .post(&url)
        .header("content-type", "application/json; charset=utf-8");
    if delivery_mode == "inbound" {
        request = request.header("x-fn-knock-provider", "magicpush");
    } else {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    let (status, ok, text, parsed) = send_prepared_json(request, &payload, timeout_seconds).await;
    provider_result_from_api(
        "MagicPush",
        request_summary,
        status,
        ok,
        text,
        parsed,
        |value| {
            value.get("success").and_then(Value::as_bool) != Some(false)
                && json_i64(value, "code").is_none_or(|code| code == 200)
        },
        |value| json_text_any(value, &["message", "msg", "error"]),
    )
}
