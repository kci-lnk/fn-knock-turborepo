use super::*;

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
    let use_text_payload =
        !mentioned_mobile_list.is_empty() || markdown_content.as_bytes().len() > 4096;
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

pub(in crate::notifications::routes) async fn send_dingtalk(
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
            "dingtalk",
            "missingWebhookUrl",
            &[],
        ));
    }
    let secret = config_text(&config, "secret");
    let keyword_prefix = config_text(&config, "keyword_prefix");
    let at_mobiles = split_values(target_config.get("at_mobiles"));
    let at_user_ids = split_values(target_config.get("at_user_ids"));
    let is_at_all = target_config
        .get("is_at_all")
        .map(value_to_bool)
        .unwrap_or(false);
    let mention_text = build_dingtalk_mention_text(&at_mobiles, &at_user_ids, is_at_all);
    let title = apply_keyword_prefix(&message_title(message), &keyword_prefix);
    let markdown_text = non_empty_or(
        build_markdown_body(message, &mention_text),
        message_summary(message),
        &title,
    );
    let request_url = if secret.is_empty() {
        webhook_url.clone()
    } else {
        let timestamp = time_utils::now_ms().to_string();
        let sign = hmac_sha256_base64(
            secret.as_bytes(),
            format!("{timestamp}\n{secret}").as_bytes(),
        );
        append_query_params(&webhook_url, &[("timestamp", timestamp), ("sign", sign)])
    };
    let body = json!({
        "msgtype": "markdown",
        "markdown": { "title": title, "text": markdown_text },
        "at": {
            "atMobiles": at_mobiles,
            "atUserIds": at_user_ids,
            "isAtAll": is_at_all
        }
    });
    let request_summary = json!({
        "method": "POST",
        "url": redact_query_value(&redact_query_value(&request_url, "access_token"), "sign"),
        "msgtype": "markdown",
        "signed": !secret.is_empty(),
        "mentioned_mobile_count": body.pointer("/at/atMobiles").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
        "mentioned_user_count": body.pointer("/at/atUserIds").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
        "is_at_all": is_at_all,
        "title_preview": truncate_text(body.pointer("/markdown/title").and_then(Value::as_str).unwrap_or(""), 120)
    });
    let (status, ok, text, parsed) = post_json(state, &request_url, &body, timeout_seconds).await;
    provider_result_from_api(
        "DingTalk",
        request_summary,
        status,
        ok,
        text,
        parsed,
        |value| json_i64(value, "errcode").unwrap_or(0) == 0,
        |value| json_text(value, "errmsg"),
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
