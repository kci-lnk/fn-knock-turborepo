use super::*;

pub(in crate::notifications::routes) fn wecom_definition() -> ProviderDefinition {
    webhook_like_definition(
        "wecom",
        "WeCom",
        "Send notifications through WeCom robot webhook.",
        &["webhook_url"],
        vec![
            string_schema("mentioned_list", "Mentioned users", false, false, None)
                .placeholder("zhangsan,@all"),
            string_schema(
                "mentioned_mobile_list",
                "Mentioned mobile list",
                false,
                false,
                None,
            )
            .placeholder("13800001111,@all"),
        ],
    )
}

pub(in crate::notifications::routes) async fn send_wecom(
    state: &AppState,
    provider: &Value,
    target: &Value,
    message: &Value,
    timeout_seconds: i64,
) -> ProviderTestResult {
    let config = provider_config(provider);
    let target_config = target_config(target);
    let webhook_url = config_text(&config, "webhook_url");
    if webhook_url.is_empty() {
        return missing_config_result(&notification_provider_error_default(
            "wecom",
            "missingWebhookUrl",
            &[],
        ));
    }

    let mentioned_list = split_values(target_config.get("mentioned_list"));
    let mentioned_mobile_list = split_values(target_config.get("mentioned_mobile_list"));
    let markdown_content = build_wecom_markdown_content(message, &mentioned_list);
    let use_text_payload = !mentioned_mobile_list.is_empty() || markdown_content.len() > 4096;
    let body = if use_text_payload {
        json!({
            "msgtype": "text",
            "text": {
                "content": default_string(
                    truncate_utf8_bytes(&build_wecom_text_content(message), 2048),
                    DEFAULT_NOTIFICATION_MESSAGE_TITLE,
                ),
                "mentioned_list": mentioned_list,
                "mentioned_mobile_list": mentioned_mobile_list
            }
        })
    } else {
        json!({
            "msgtype": "markdown",
            "markdown": {
                "content": default_string(
                    truncate_utf8_bytes(&markdown_content, 4096),
                    DEFAULT_NOTIFICATION_MESSAGE_TITLE,
                )
            }
        })
    };
    let request_summary = json!({
        "method": "POST",
        "url": redact_query_value(&webhook_url, "key"),
        "msgtype": body.get("msgtype").cloned().unwrap_or(Value::Null),
        "mentioned_count": split_values(target_config.get("mentioned_list")).len(),
        "mentioned_mobile_count": split_values(target_config.get("mentioned_mobile_list")).len()
    });

    let (status, ok, text, parsed) = post_json(state, &webhook_url, &body, timeout_seconds).await;
    provider_result_from_api(
        "WeCom",
        request_summary,
        status,
        ok,
        text,
        parsed,
        |value| json_i64(value, "errcode").unwrap_or(0) == 0,
        |value| json_text(value, "errmsg"),
    )
}
