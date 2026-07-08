use super::*;

pub(in crate::notifications::routes) fn feishu_definition() -> ProviderDefinition {
    webhook_like_definition(
        "feishu",
        "Feishu",
        "Send notifications through Feishu robot webhook.",
        &["webhook_url", "secret"],
        vec![
            string_schema("mention_user_ids", "Mention user IDs", false, false, None)
                .placeholder("ou_xxx,all"),
        ],
    )
}

pub(in crate::notifications::routes) async fn send_feishu(
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
            "feishu",
            "missingWebhookUrl",
            &[],
        ));
    }
    let secret = config_text(&config, "secret");
    let keyword_prefix = config_text(&config, "keyword_prefix");
    let mention_user_ids = split_values(target_config.get("mention_user_ids"));
    let title = apply_keyword_prefix(&message_title(message), &keyword_prefix);
    let mut body = json!({
        "msg_type": "post",
        "content": {
            "post": {
                "zh_cn": {
                    "title": title,
                    "content": build_feishu_post_content(message, &mention_user_ids)
                }
            }
        }
    });
    if !secret.is_empty() {
        let timestamp = (time_utils::now_ms() / 1000).to_string();
        let key = format!("{timestamp}\n{secret}");
        let sign = hmac_sha256_base64(key.as_bytes(), b"");
        if let Some(object) = body.as_object_mut() {
            object.insert("timestamp".to_string(), Value::String(timestamp));
            object.insert("sign".to_string(), Value::String(sign));
        }
    }
    let request_summary = json!({
        "method": "POST",
        "url": redact_path_tail(&webhook_url),
        "msg_type": "post",
        "signed": !secret.is_empty(),
        "mentioned_user_count": mention_user_ids.len(),
        "title_preview": truncate_text(body.pointer("/content/post/zh_cn/title").and_then(Value::as_str).unwrap_or(""), 120)
    });
    let (status, ok, text, parsed) = post_json(state, &webhook_url, &body, timeout_seconds).await;
    provider_result_from_api(
        "Feishu",
        request_summary,
        status,
        ok,
        text,
        parsed,
        |value| json_i64(value, "code").unwrap_or(0) == 0,
        |value| json_text(value, "msg"),
    )
}
