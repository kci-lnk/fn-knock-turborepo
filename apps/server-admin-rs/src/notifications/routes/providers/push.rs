use super::*;

pub(in crate::notifications::routes) async fn send_serverchan(
    state: &AppState,
    provider: &Value,
    target: &Value,
    message: &Value,
    timeout_seconds: i64,
) -> ProviderTestResult {
    let config = provider_config(provider);
    let target_config = target_config(target);
    let sendkey = config_text(&config, "sendkey");
    if sendkey.is_empty() {
        return missing_config_result(&notification_provider_error_default(
            "serverchan",
            "missingSendKey",
            &[],
        ));
    }
    let base_url = default_string(
        config_text(&config, "server_url"),
        "https://sctapi.ftqq.com",
    );
    let url = format!("{}/{}.send", base_url.trim_end_matches('/'), sendkey);
    let title = truncate_text(&message_title(message), 32);
    let desp = truncate_utf8_bytes(&build_markdown_body(message, ""), 32 * 1024);
    let short = truncate_text(&config_text(&target_config, "short"), 64);
    let channel = config_text(&target_config, "channel");
    let openid = config_text(&target_config, "openid");
    let noip = target_config
        .get("noip")
        .map(value_to_bool)
        .unwrap_or(false);
    let mut form = vec![(
        "title".to_string(),
        default_string(title.clone(), "fn-knock"),
    )];
    push_form_if(&mut form, "desp", desp.clone());
    push_form_if(&mut form, "short", short.clone());
    push_form_if(&mut form, "channel", channel.clone());
    push_form_if(&mut form, "openid", openid.clone());
    if noip {
        form.push(("noip".to_string(), "1".to_string()));
    }
    let request_summary = json!({
        "method": "POST",
        "endpoint": base_url,
        "has_desp": !desp.is_empty(),
        "has_short": !short.is_empty(),
        "channel": empty_to_null(&channel),
        "has_openid": !openid.is_empty(),
        "noip": noip,
        "title_preview": title
    });
    let (status, ok, text, parsed) = post_form(state, &url, &form, timeout_seconds).await;
    provider_result_from_api(
        "ServerChan",
        request_summary,
        status,
        ok,
        text,
        parsed,
        |value| json_i64_any(value, &["code", "errno", "error_code"]).unwrap_or(0) == 0,
        |value| json_text_any(value, &["message", "msg", "error"]),
    )
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
