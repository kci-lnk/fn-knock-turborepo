use super::*;

pub(in crate::notifications::routes) fn wxpusher_definition() -> ProviderDefinition {
    ProviderDefinition {
        provider_type: "wxpusher",
        label: "WxPusher",
        description: "Send notifications through WxPusher.",
        connection_schema: vec![
            string_schema(
                "server_url",
                "Server URL",
                true,
                false,
                Some("https://wxpusher.zjiecode.com"),
            )
            .placeholder("https://wxpusher.zjiecode.com"),
            string_schema("app_token", "AppToken", true, true, None).placeholder("AT_xxx"),
            number_schema("timeout_seconds", "Timeout seconds", true, Some(5)).bounds(1, 30),
            string_schema("uids", "UIDs", false, false, None).placeholder("UID_xxx,UID_yyy"),
            string_schema("topic_ids", "Topic IDs", false, false, None).placeholder("123,456"),
            string_schema("url", "URL", false, false, None)
                .placeholder("https://example.com/events/123"),
            select_schema(
                "verify_pay_type",
                "Verify pay type",
                false,
                Some("0"),
                &["0", "1", "2"],
            ),
        ],
        target_schema: vec![
            string_schema("uids", "UIDs", false, false, None).placeholder("UID_xxx,UID_yyy"),
            string_schema("topic_ids", "Topic IDs", false, false, None).placeholder("123,456"),
            string_schema("url", "URL", false, false, None)
                .placeholder("https://example.com/events/123"),
            select_schema(
                "verify_pay_type",
                "Verify pay type",
                false,
                Some("__inherit__"),
                &["__inherit__", "0", "1", "2"],
            ),
        ],
        sensitive_fields: vec!["app_token"],
        supports_markdown: true,
        supports_actions: true,
        supports_mentions: false,
        supports_provider_dedupe_key: false,
    }
}

pub(in crate::notifications::routes) async fn send_wxpusher(
    state: &AppState,
    provider: &Value,
    target: &Value,
    message: &Value,
    timeout_seconds: i64,
) -> ProviderTestResult {
    let config = provider_config(provider);
    let target_config = target_config(target);
    let app_token = first_config_text(&config, &["app_token", "appToken"]);
    if app_token.is_empty() {
        return missing_config_result(&notification_provider_error_default(
            "wxpusher",
            "missingAppToken",
            &[],
        ));
    }
    let uids = split_values(effective_config_value(&config, &target_config, &["uids"]).as_ref());
    let topic_ids_value = effective_config_value(
        &config,
        &target_config,
        &[
            "topic_ids",
            "topicIds",
            "topic_id",
            "topicId",
            "topic",
            "Topic",
        ],
    );
    let (topic_ids, invalid_topic_ids) = parse_topic_ids(topic_ids_value.as_ref());
    if !invalid_topic_ids.is_empty() {
        return ProviderTestResult {
            success: false,
            retryable: false,
            message: notification_provider_error_default(
                "wxpusher",
                "invalidTopicIds",
                &[("values", invalid_topic_ids.join(", "))],
            ),
            request_summary: None,
            response_summary: None,
        };
    }
    if uids.is_empty() && topic_ids.is_empty() {
        return missing_config_result(&notification_provider_error_default(
            "wxpusher",
            "recipientRequired",
            &[],
        ));
    }
    let link_url = effective_config_value(&config, &target_config, &["url"])
        .as_ref()
        .map(value_to_trimmed_string)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| primary_action_url(message));
    let verify_pay_type = effective_config_value(
        &config,
        &target_config,
        &["verify_pay_type", "verifyPayType"],
    )
    .as_ref()
    .and_then(|value| {
        let parsed = value_to_i64(value, -1);
        (0..=2).contains(&parsed).then_some(parsed)
    });
    let mut body = json!({
        "appToken": app_token,
        "content": default_string(
            build_wxpusher_html_content(message),
            &format!("<p>{}</p>", escape_html(DEFAULT_NOTIFICATION_MESSAGE_TITLE)),
        ),
        "summary": truncate_utf8_bytes(
            &default_string(message_summary(message), &message_title(message)),
            100,
        ),
        "contentType": 2
    });
    if !topic_ids.is_empty() {
        insert_value(&mut body, "topicIds", json!(topic_ids));
    }
    if !uids.is_empty() {
        insert_value(&mut body, "uids", json!(uids));
    }
    insert_non_empty(&mut body, "url", link_url.clone());
    if let Some(verify_pay_type) = verify_pay_type {
        insert_i64(&mut body, "verifyPayType", verify_pay_type);
    }
    let url = format!(
        "{}/api/send/message",
        default_string(
            first_config_text(&config, &["server_url", "serverUrl"]),
            "https://wxpusher.zjiecode.com"
        )
        .trim_end_matches('/')
    );
    let request_summary = json!({
        "method": "POST",
        "url": url,
        "uid_count": body.get("uids").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
        "topic_id_count": body.get("topicIds").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
        "content_type": 2,
        "has_url": !link_url.is_empty()
    });
    let (status, ok, text, parsed) = post_json(state, &url, &body, timeout_seconds).await;
    let failed_items = parsed
        .as_ref()
        .and_then(|value| value.get("data"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter(|item| json_i64(item, "code") != Some(1000))
                .count()
        })
        .unwrap_or(0);
    provider_result_from_api(
        "WxPusher",
        request_summary,
        status,
        ok,
        text,
        parsed,
        move |value| {
            value
                .get("success")
                .and_then(Value::as_bool)
                .unwrap_or(true)
                && json_i64(value, "code").unwrap_or(1000) == 1000
                && failed_items == 0
        },
        |value| json_text_any(value, &["msg", "message", "error"]),
    )
}
