use super::*;

pub(in crate::notifications::routes) fn bark_definition() -> ProviderDefinition {
    ProviderDefinition {
        provider_type: "bark",
        label: "Bark",
        description: "Send notifications through Bark.",
        connection_schema: vec![
            string_schema(
                "server_url",
                "Server URL",
                true,
                false,
                Some("https://api.day.app"),
            )
            .placeholder("https://api.day.app"),
            string_schema("device_key", "Device Key", true, true, None)
                .placeholder("ynJ5Ft4atkMkWeo2PAvFhF"),
            number_schema("timeout_seconds", "Timeout seconds", true, Some(5)).bounds(1, 30),
        ],
        target_schema: vec![
            select_schema(
                "level",
                "Level",
                false,
                Some("active"),
                &["active", "timeSensitive", "passive", "critical"],
            ),
            string_schema("group", "Group", false, false, None).placeholder("fn-knock"),
            string_schema("sound", "Sound", false, false, None).placeholder("alarm"),
            string_schema("url", "URL", false, false, None)
                .placeholder("https://example.com/events/123"),
            string_schema("icon", "Icon", false, false, None)
                .placeholder("https://day.app/assets/images/avatar.jpg"),
            number_schema("badge", "Badge", false, None).bounds(0, 99999),
            bool_schema("call", "Call", false, Some(false)),
        ],
        sensitive_fields: vec!["device_key"],
        supports_markdown: false,
        supports_actions: true,
        supports_mentions: false,
        supports_provider_dedupe_key: false,
    }
}

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
