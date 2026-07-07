use super::*;

pub(in crate::notifications::routes) async fn send_bark(
    state: &AppState,
    provider: &Value,
    target: &Value,
    message: &Value,
    timeout_seconds: i64,
) -> ProviderTestResult {
    let config = provider_config(provider);
    let device_keys = split_values(config.get("device_key"));
    if device_keys.is_empty() {
        return missing_config_result(&notification_provider_error_default(
            "bark",
            "missingDeviceKey",
            &[],
        ));
    }
    let base_url = default_string(config_text(&config, "server_url"), "https://api.day.app");
    let url = format!("{}/push", base_url.trim_end_matches('/'));
    let payload_preview = build_bark_payload(message, target);
    let mut results = Vec::new();
    for device_key in &device_keys {
        let mut payload = payload_preview.clone();
        insert_string(&mut payload, "device_key", device_key.clone());
        let (status, ok, text, parsed) = post_json(state, &url, &payload, timeout_seconds).await;
        let bark_code = parsed.as_ref().and_then(|value| json_i64(value, "code"));
        let success = ok && bark_code.is_none_or(|code| code == 200);
        let retryable = !success && (status >= 500 || status == 429);
        let reason = parsed
            .as_ref()
            .and_then(|value| json_text(value, "message"))
            .unwrap_or_else(|| {
                if status == 599 && !text.is_empty() {
                    return text.clone();
                }
                if ok {
                    String::new()
                } else {
                    format!("Bark returned {status}")
                }
            });
        let response_summary = if status == 599 && parsed.is_none() {
            Value::Null
        } else {
            json!({
            "status": status,
            "ok": ok,
            "code": bark_code,
            "message": parsed.as_ref().and_then(|value| json_text(value, "message")),
            "body_preview": truncate_text(&text, 500)
            })
        };
        let mut result = Map::new();
        result.insert("success".to_string(), Value::Bool(success));
        result.insert("retryable".to_string(), Value::Bool(retryable));
        if !success && !reason.is_empty() {
            result.insert("reason".to_string(), Value::String(reason));
        }
        result.insert("response_summary".to_string(), response_summary);
        results.push(Value::Object(result));
    }
    let failed_count = results
        .iter()
        .filter(|result| result.get("success").and_then(Value::as_bool) != Some(true))
        .count();
    let response_results = if failed_count == 0 {
        results
            .iter()
            .map(|result| {
                result
                    .get("response_summary")
                    .cloned()
                    .unwrap_or(Value::Null)
            })
            .collect::<Vec<_>>()
    } else {
        results.clone()
    };
    ProviderTestResult {
        success: failed_count == 0,
        retryable: results.iter().any(|result| {
            result
                .get("retryable")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        }),
        message: if failed_count == 0 {
            notification_service_default_text("testSendSuccess", &[])
        } else if failed_count == 1 {
            results
                .iter()
                .find(|result| result.get("success").and_then(Value::as_bool) != Some(true))
                .and_then(|result| result.get("reason"))
                .and_then(Value::as_str)
                .filter(|reason| !reason.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| notification_provider_error_default("bark", "pushFailed", &[]))
        } else {
            format!("Bark failed for {failed_count}/{} target(s)", results.len())
        },
        request_summary: Some(json!({
            "method": "POST",
            "url": url,
            "device_key_count": device_keys.len(),
            "level": payload_preview.get("level").cloned().unwrap_or(Value::Null),
            "group": payload_preview.get("group").cloned().unwrap_or(Value::Null),
            "title_preview": payload_preview.get("title").cloned().unwrap_or(Value::Null)
        })),
        response_summary: Some(json!({
            "success_count": results.len().saturating_sub(failed_count),
            "failed_count": failed_count,
            "results": response_results
        })),
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
