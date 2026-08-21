use super::*;

fn schema_field<'a>(view: &'a Value, schema: &str, key: &str) -> &'a Value {
    view.get(schema)
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .find(|field| field.get("key").and_then(Value::as_str) == Some(key))
        .unwrap()
}

#[test]
fn masks_sensitive_provider_values_like_node() {
    assert_eq!(mask_sensitive_value(&json!("short")), json!("********"));
    assert_eq!(
        mask_sensitive_value(&json!("abcdefghijkl")),
        json!("ab******")
    );
    assert_eq!(mask_sensitive_value(&json!(true)), json!("[configured]"));
}

#[test]
fn provider_test_result_updates_provider_status_like_node() {
    let mut provider = json!({
        "id": "ntfprov_1",
        "last_test_status": "idle",
        "last_error": "old error"
    });
    apply_provider_test_result(
        &mut provider,
        &ProviderTestResult {
            success: true,
            retryable: false,
            message: "ok".to_string(),
            request_summary: None,
            response_summary: None,
        },
    );
    assert_eq!(provider.get("last_test_status"), Some(&json!("success")));
    assert_eq!(provider.get("last_error"), Some(&Value::Null));
    assert!(
        provider
            .get("last_test_at")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.is_empty())
    );
}

#[test]
fn applies_schema_defaults_and_required_validation() {
    let definition = provider_definition("webhook").unwrap();
    let mut raw = Map::new();
    raw.insert("url".to_string(), json!(" https://example.com/hook "));
    let normalized = normalize_schema_config(&raw, &definition.connection_schema).unwrap();
    assert_eq!(
        normalized.get("url"),
        Some(&json!("https://example.com/hook"))
    );
    assert_eq!(normalized.get("method"), Some(&json!("POST")));
    assert_eq!(normalized.get("timeout_seconds"), Some(&json!(5)));
    validate_required_fields(&normalized, &definition.connection_schema).unwrap();
}

#[test]
fn rejects_invalid_select_values() {
    let definition = provider_definition("webhook").unwrap();
    let mut raw = Map::new();
    raw.insert("method".to_string(), json!("DELETE"));
    assert!(normalize_schema_patch(&raw, &definition.connection_schema).is_err());
}

#[test]
fn schema_boolean_values_follow_node_truthiness() {
    let definition = provider_definition("bark").unwrap();
    let mut raw = Map::new();

    raw.insert("call".to_string(), json!("false"));
    assert_eq!(
        normalize_schema_patch(&raw, &definition.target_schema)
            .unwrap()
            .get("call"),
        Some(&json!(true))
    );

    raw.insert("call".to_string(), json!(""));
    assert_eq!(
        normalize_schema_patch(&raw, &definition.target_schema)
            .unwrap()
            .get("call"),
        Some(&json!(false))
    );

    raw.insert("call".to_string(), json!(0));
    assert_eq!(
        normalize_schema_patch(&raw, &definition.target_schema)
            .unwrap()
            .get("call"),
        Some(&json!(false))
    );

    raw.insert("call".to_string(), json!({}));
    assert_eq!(
        normalize_schema_patch(&raw, &definition.target_schema)
            .unwrap()
            .get("call"),
        Some(&json!(true))
    );
}

#[test]
fn schema_string_values_follow_node_string_coercion() {
    let definition = provider_definition("webhook").unwrap();
    let mut raw = Map::new();
    raw.insert("endpoint_path".to_string(), json!({ "path": "/alerts" }));
    assert_eq!(
        normalize_schema_patch(&raw, &definition.target_schema)
            .unwrap()
            .get("endpoint_path"),
        Some(&json!("[object Object]"))
    );

    raw.insert("endpoint_path".to_string(), json!(["alerts", 1, null]));
    assert_eq!(
        normalize_schema_patch(&raw, &definition.target_schema)
            .unwrap()
            .get("endpoint_path"),
        Some(&json!("alerts,1,"))
    );
}

#[test]
fn json_schema_whitespace_matches_node_parse_behavior() {
    let definition = provider_definition("webhook").unwrap();
    let mut raw = Map::new();

    raw.insert("extra_headers_json".to_string(), json!(""));
    assert!(
        !normalize_schema_patch(&raw, &definition.target_schema)
            .unwrap()
            .contains_key("extra_headers_json")
    );

    raw.insert("extra_headers_json".to_string(), json!("   "));
    assert!(normalize_schema_patch(&raw, &definition.target_schema).is_err());

    raw.insert(
        "extra_headers_json".to_string(),
        json!(" {\"X-Env\":\"prod\"} "),
    );
    assert_eq!(
        normalize_schema_patch(&raw, &definition.target_schema)
            .unwrap()
            .get("extra_headers_json"),
        Some(&json!({ "X-Env": "prod" }))
    );
}

#[test]
fn notification_number_fields_follow_node_number_coercion() {
    assert_eq!(
        number_field(
            &json!({ "window_seconds": "" }),
            "window_seconds",
            60,
            1,
            86400
        ),
        1
    );
    assert_eq!(
        number_field(
            &json!({ "threshold_count": null }),
            "threshold_count",
            9,
            1,
            9999
        ),
        1
    );
    assert_eq!(
        number_field(
            &json!({ "cooldown_seconds": false }),
            "cooldown_seconds",
            60,
            0,
            86400
        ),
        0
    );
    assert_eq!(
        number_field(
            &json!({ "window_seconds": "2.9" }),
            "window_seconds",
            60,
            1,
            86400
        ),
        2
    );
    assert_eq!(
        number_field(
            &json!({ "window_seconds": "2x" }),
            "window_seconds",
            60,
            1,
            86400
        ),
        60
    );
    assert_eq!(
        number_field(
            &json!({ "window_seconds": "0x10" }),
            "window_seconds",
            60,
            1,
            86400
        ),
        16
    );
    assert_eq!(
        number_field(
            &json!({ "window_seconds": "0b10" }),
            "window_seconds",
            60,
            1,
            86400
        ),
        2
    );
    assert_eq!(
        number_field(
            &json!({ "window_seconds": "0o10" }),
            "window_seconds",
            60,
            1,
            86400
        ),
        8
    );
    assert_eq!(
        number_field(
            &json!({ "window_seconds": ["4.9"] }),
            "window_seconds",
            60,
            1,
            86400
        ),
        4
    );
}

#[test]
fn notification_group_keys_coerce_payload_values_like_node() {
    assert_eq!(
        build_notification_group_key(&json!({ "payload": { "ip": 123 } }), "IP"),
        "123"
    );
    assert_eq!(
        build_notification_group_key(&json!({ "payload": { "ip": false } }), "IP"),
        "false"
    );
    assert_eq!(
        build_notification_group_key(
            &json!({ "payload": { "provider": ["cf", 1, null] } }),
            "PROVIDER"
        ),
        "cf,1,"
    );
    assert_eq!(
        build_notification_group_key(
            &json!({
                "payload": { "ip": "   ", "to_ip": "203.0.113.9" },
                "subject": { "kind": "IP", "id": "subject-ip" }
            }),
            "IP"
        ),
        "subject-ip"
    );
    assert_eq!(
        build_notification_group_key(
            &json!({
                "payload": { "ip": "", "to_ip": "203.0.113.9" },
                "subject": { "kind": "IP", "id": "subject-ip" }
            }),
            "IP"
        ),
        "203.0.113.9"
    );
}

#[test]
fn gateway_visibility_event_matches_rules_and_builds_localized_notification() {
    let event = json!({
        "id": "evt_visibility_1",
        "type": "FN_EVENT_GATEWAY_VISIBILITY_BLOCKED",
        "source": "GO_REAUTH_PROXY",
        "level": "WARN",
        "happened_at": "2026-07-27T10:11:12Z",
        "dedupe_key": "gateway-visibility:global",
        "subject": { "kind": "IP", "id": "203.0.113.8" },
        "payload": {
            "ip": "203.0.113.8",
            "blocked_at": "2026-07-27T10:11:12Z",
            "method": "GET",
            "scheme": "https",
            "host": "app.example.test",
            "path": "/private",
            "route_type": "host_rule",
            "route_key": "app.example.test",
            "visibility_scope": "host",
            "visibility_mode": "custom",
            "status": 499
        }
    });
    let rule = json!({
        "id": "ntfrule_visibility",
        "enabled": true,
        "event_type": "FN_EVENT_GATEWAY_VISIBILITY_BLOCKED",
        "window_seconds": 60,
        "threshold_count": 1,
        "group_by": "GLOBAL"
    });

    assert!(SYSTEM_EVENT_TYPES.contains(&"FN_EVENT_GATEWAY_VISIBILITY_BLOCKED"));
    assert!(event_matches_notification_rule(&event, &rule));
    assert_eq!(build_notification_group_key(&event, "GLOBAL"), "global");
    assert_eq!(
        notification_event_label_key("FN_EVENT_GATEWAY_VISIBILITY_BLOCKED"),
        Some("events.gatewayVisibilityBlocked")
    );

    let translator = Translator::new("zh-CN");
    let details = build_notification_details(&event, &rule, 1, &translator);
    assert!(details.summary.contains("203.0.113.8"));
    assert!(details.summary.contains("app.example.test"));
    assert!(details.body_text.contains("/private"));
    assert!(details.body_text.contains("当前域名"));
    assert!(details.body_text.contains("自定义"));
    assert!(
        details
            .facts
            .iter()
            .any(|fact| fact.get("value") == Some(&json!("https")))
    );
    assert!(
        details
            .facts
            .iter()
            .any(|fact| fact.get("value") == Some(&json!("499")))
    );
    let aggregated_message = build_notification_message(&event, &rule, 3, "global", &translator);
    assert_eq!(
        aggregated_message.get("dedupe_key"),
        Some(&json!("ntfrule_visibility:global"))
    );
    assert_eq!(
        aggregated_message.pointer("/metadata/matched_count"),
        Some(&json!(3))
    );
    assert!(
        aggregated_message
            .get("body_text")
            .and_then(Value::as_str)
            .is_some_and(|value| value.contains("聚合 3 条相似事件"))
    );

    let malicious_event = json!({
        "type": "FN_EVENT_GATEWAY_VISIBILITY_BLOCKED",
        "source": "GO_REAUTH_PROXY",
        "level": "WARN",
        "payload": {
            "ip": "203.0.113.8",
            "method": "GET",
            "host": "app.example.test",
            "path": "/[open](https://evil.test)\n# injected",
            "visibility_scope": "gateway",
            "visibility_mode": "inherit",
            "status": 499
        }
    });
    let malicious_details = build_notification_details(&malicious_event, &rule, 1, &translator);
    assert!(
        malicious_details
            .body_markdown
            .contains("/\\[open\\]\\(https://evil.test\\) \\# injected")
    );
    assert!(!malicious_details.body_markdown.contains("[open]("));
    assert!(!malicious_details.body_markdown.contains("\n# injected"));
}

#[test]
fn notification_delivery_ready_time_matches_node_fallbacks() {
    let next_retry = "2026-07-07T10:00:00.000Z";
    let triggered = "2026-07-07T09:00:00.000Z";
    assert_eq!(
        resolve_delivery_ready_at_ms(&json!({
            "next_retry_at": next_retry,
            "triggered_at": triggered
        })),
        time_utils::parse_iso_ms(next_retry).unwrap()
    );
    assert_eq!(
        resolve_delivery_ready_at_ms(&json!({
            "next_retry_at": "bad",
            "triggered_at": triggered
        })),
        time_utils::parse_iso_ms(triggered).unwrap()
    );
}

#[test]
fn notification_provider_retryability_matches_node_business_codes() {
    assert!(provider_api_failure_retryable("Webhook", 500, &json!({})));
    assert!(provider_api_failure_retryable("Webhook", 429, &json!({})));
    assert!(!provider_api_failure_retryable("Webhook", 400, &json!({})));
    assert!(provider_api_failure_retryable(
        "PushPlus",
        200,
        &json!({ "code": 999 })
    ));
    assert!(provider_api_failure_retryable(
        "PushPlus",
        200,
        &json!({ "code": "500" })
    ));
    assert!(provider_api_failure_retryable(
        "Feishu",
        200,
        &json!({ "code": 11232 })
    ));
    assert!(provider_api_failure_retryable(
        "Telegram",
        400,
        &json!({ "error_code": 429 })
    ));
    assert!(!provider_api_failure_retryable(
        "PushDeer",
        200,
        &json!({ "code": 1 })
    ));

    let network_error = provider_result_from_api(
        "PushPlus",
        json!({ "method": "POST" }),
        599,
        false,
        "connection refused".to_string(),
        None,
        |_| false,
        |_| None,
    );
    assert!(network_error.retryable);
    assert_eq!(network_error.response_summary, None);
}

#[test]
fn notification_provider_payload_helpers_match_node_edges() {
    let message = json!({
        "summary": " 概览 ",
        "body_text": " 第一行 \n 第二行 ",
        "facts": [{ "label": "状态", "value": "异常" }],
        "actions": [{ "label": "查看", "url": "https://example.com/a" }],
    });

    assert_eq!(message_title(&json!({})), "fn-knock 通知");
    assert_eq!(build_markdown_body(&json!({}), ""), "");
    assert!(build_pushplus_markdown_content(&message).contains("- **状态**：异常"));

    let pushplus_html = build_pushplus_html_content(&message);
    assert!(!pushplus_html.contains("<h2>"));
    assert!(pushplus_html.contains("<strong>状态</strong>：异常"));

    let wxpusher_html = build_wxpusher_html_content(&message);
    assert!(wxpusher_html.contains("<h2>概览</h2>"));
    assert!(wxpusher_html.contains("<strong>状态</strong>：异常"));

    assert_eq!(magicpush_facts_object(&message), json!({ "状态": "异常" }));
    assert_eq!(
        build_bark_payload(&message, &json!({ "target_config": { "badge": 0 } })).get("badge"),
        Some(&json!(0))
    );

    let untrusted = json!({
        "summary": "[fake](https://evil.test)",
        "facts": [{ "label": "Path", "value": "/[open](https://evil.test)\n# injected" }],
    });
    let rendered = build_markdown_body(&untrusted, "");
    assert!(rendered.contains("\\[fake\\]\\(https://evil.test\\)"));
    assert!(rendered.contains("/\\[open\\]\\(https://evil.test\\) \\# injected"));
    assert!(!rendered.contains("[fake]("));
    assert!(!rendered.contains("\n# injected"));
}

#[test]
fn notification_provider_parsers_follow_node_edges() {
    assert_eq!(value_as_i64(&json!("200 OK")), Some(200));
    assert_eq!(value_as_i64(&json!("0x10")), Some(0));
    assert_eq!(value_as_i64(&json!("  -12x")), Some(-12));

    let topic_value = json!("+1,01,abc");
    let (topic_ids, invalid_topic_ids) = parse_topic_ids(Some(&topic_value));
    assert_eq!(topic_ids, vec![1]);
    assert_eq!(invalid_topic_ids, vec!["+1", "abc"]);

    assert_eq!(
        resolve_pushplus_url("https://push.example.com/BatchSend"),
        "https://push.example.com/BatchSend"
    );
    assert_eq!(
        resolve_magicpush_url("https://push.example.com/API/PUSH/token", "other", "push"),
        "https://push.example.com/API/PUSH/token"
    );
    assert_eq!(
        resolve_magicpush_url("https://push.example.com/API/INBOUND", "a b", "inbound"),
        "https://push.example.com/API/INBOUND/a+b"
    );
}

#[test]
fn harmonyosmeow_catalog_and_masking_match_provider_contract() {
    let definition = provider_definition("harmonyosmeow").unwrap();
    assert_eq!(definition.label, "HarmonyOSMeoW");
    assert!(definition.target_schema.is_empty());
    assert_eq!(definition.sensitive_fields, vec!["nickname"]);

    let zh = provider_definition_view(&definition, &Translator::new("zh-CN"));
    assert_eq!(zh.get("label"), Some(&json!("鸿蒙MeoW")));
    assert_eq!(
        schema_field(&zh, "connection_schema", "server_url").get("default_value"),
        Some(&json!("https://api.chuckfang.com"))
    );
    assert_eq!(
        schema_field(&zh, "connection_schema", "nickname").get("sensitive"),
        Some(&json!(true))
    );
    assert_eq!(
        zh.pointer("/capabilities/supports_markdown"),
        Some(&json!(true))
    );
    assert_eq!(
        zh.pointer("/capabilities/supports_actions"),
        Some(&json!(true))
    );
    assert_eq!(
        zh.pointer("/capabilities/supports_mentions"),
        Some(&json!(false))
    );
    assert_eq!(
        zh.pointer("/capabilities/max_body_length"),
        Some(&Value::Null)
    );

    let en = provider_definition_view(&definition, &Translator::new("en"));
    assert_eq!(en.get("label"), Some(&json!("HarmonyOSMeoW")));

    let masked = mask_provider(&json!({
        "id": "ntfprov_meow",
        "name": "MeoW",
        "type": "harmonyosmeow",
        "connection_config": {
            "server_url": "https://api.chuckfang.com",
            "nickname": "JohnDoe",
            "timeout_seconds": 5
        }
    }))
    .unwrap();
    assert_eq!(
        masked.pointer("/connection_config_masked/nickname"),
        Some(&json!("********"))
    );
}

#[test]
fn harmonyosmeow_url_and_markdown_body_are_safe_and_deterministic() {
    let resolved = resolve_harmonyosmeow_url(
        "https://api.example.com/base/?old=1#fragment",
        "张 三测试",
        "告警 / A",
    )
    .unwrap();
    assert_eq!(
        resolved,
        "https://api.example.com/base/%E5%BC%A0%20%E4%B8%89%E6%B5%8B%E8%AF%95/%E5%91%8A%E8%AD%A6%20%2F%20A?msgType=markdown"
    );
    assert_eq!(
        resolve_harmonyosmeow_url("https://api.example.com/", "JohnDoe", "Title").unwrap(),
        "https://api.example.com/JohnDoe/Title?msgType=markdown"
    );
    assert!(resolve_harmonyosmeow_url("ftp://api.example.com", "JohnDoe", "Title").is_err());
    assert!(resolve_harmonyosmeow_url("not a url", "JohnDoe", "Title").is_err());
    assert!(resolve_harmonyosmeow_url("https://api.example.com", "John/Doe", "Title").is_err());

    let definition = provider_definition("harmonyosmeow").unwrap();
    assert!(
        validate_provider_connection_config(
            &definition,
            &Map::from_iter([
                ("server_url".to_string(), json!("https://api.example.com")),
                ("nickname".to_string(), json!("John/Doe")),
                ("timeout_seconds".to_string(), json!(5)),
            ]),
        )
        .is_err()
    );

    let body = build_harmonyosmeow_body(&json!({
        "title": "告警",
        "summary": "服务异常",
        "body_markdown": "请检查 **api** 服务。",
        "facts": [{ "label": "状态", "value": "异常" }],
        "actions": [{ "label": "查看详情", "url": "https://example.com/events/1" }]
    }));
    assert!(body.contains("请检查 **api** 服务。"));
    assert!(body.contains("- **状态**：异常"));
    assert!(body.contains("- [查看详情](https://example.com/events/1)"));
    assert_eq!(build_harmonyosmeow_body(&json!({})), "fn-knock 通知");
}

#[test]
fn harmonyosmeow_response_status_and_messages_drive_delivery_result() {
    let success = harmonyosmeow_result(
        json!({ "method": "POST" }),
        200,
        true,
        r#"{"status":200,"message":"推送成功"}"#.to_string(),
        Some(json!({ "status": 200, "message": "推送成功" })),
    );
    assert!(success.success);
    assert!(!success.retryable);

    let bad_request = harmonyosmeow_result(
        json!({}),
        200,
        true,
        r#"{"status":400,"msg":"昵称不存在"}"#.to_string(),
        Some(json!({ "status": 400, "msg": "昵称不存在" })),
    );
    assert!(!bad_request.success);
    assert!(!bad_request.retryable);
    assert_eq!(bad_request.message, "昵称不存在");

    let server_error = harmonyosmeow_result(
        json!({}),
        200,
        true,
        r#"{"status":500,"error":"服务异常"}"#.to_string(),
        Some(json!({ "status": 500, "error": "服务异常" })),
    );
    assert!(!server_error.success);
    assert!(server_error.retryable);
    assert_eq!(server_error.message, "服务异常");

    let rate_limited = harmonyosmeow_result(
        json!({}),
        429,
        false,
        r#"{"status":400,"message":"请求过多"}"#.to_string(),
        Some(json!({ "status": 400, "message": "请求过多" })),
    );
    assert!(rate_limited.retryable);

    let network_error = harmonyosmeow_result(
        json!({}),
        599,
        false,
        "connection refused".to_string(),
        None,
    );
    assert!(network_error.retryable);
    assert_eq!(network_error.message, "connection refused");
    assert_eq!(network_error.response_summary, None);
}

#[tokio::test]
async fn harmonyosmeow_text_request_matches_api_contract() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        let (header_end, content_length) = loop {
            let mut buffer = [0_u8; 4096];
            let read = stream.read(&mut buffer).await.unwrap();
            assert!(read > 0, "client closed before request body completed");
            request.extend_from_slice(&buffer[..read]);
            let Some(header_end) = request.windows(4).position(|value| value == b"\r\n\r\n") else {
                continue;
            };
            let headers = String::from_utf8_lossy(&request[..header_end]);
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap_or_default();
            if request.len() >= header_end + 4 + content_length {
                break (header_end, content_length);
            }
        };

        let headers = String::from_utf8_lossy(&request[..header_end]);
        let request_line = headers.lines().next().unwrap_or_default().to_string();
        let content_type = headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-type")
                    .then(|| value.trim().to_string())
            })
            .unwrap_or_default();
        let body =
            String::from_utf8(request[header_end + 4..header_end + 4 + content_length].to_vec())
                .unwrap();

        let response_body = r#"{"status":200,"message":"ok"}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        stream.write_all(response.as_bytes()).await.unwrap();
        (request_line, content_type, body)
    });

    let url = format!("http://{address}/JohnDoe/Alert?msgType=markdown");
    let client = reqwest::Client::new();
    let (status, ok, _text, parsed) =
        send_prepared_text(client.post(url), "## Alert\n\nService recovered.", 5).await;
    assert_eq!(status, 200);
    assert!(ok);
    assert_eq!(parsed, Some(json!({ "status": 200, "message": "ok" })));

    let (request_line, content_type, body) = server.await.unwrap();
    assert_eq!(
        request_line,
        "POST /JohnDoe/Alert?msgType=markdown HTTP/1.1"
    );
    assert_eq!(content_type, "text/plain; charset=utf-8");
    assert_eq!(body, "## Alert\n\nService recovered.");
}

#[test]
fn notification_page_parser_matches_node_parse_int_edges() {
    assert_eq!(parse_positive_int(None, 1, i64::MAX), 1);
    assert_eq!(parse_positive_int(Some(""), 20, 100), 20);
    assert_eq!(parse_positive_int(Some("2x"), 1, 100), 2);
    assert_eq!(parse_positive_int(Some("  +3.9"), 1, 100), 3);
    assert_eq!(parse_positive_int(Some("-1"), 1, 100), 1);
    assert_eq!(parse_positive_int(Some("0x10"), 7, 100), 7);
    assert_eq!(parse_positive_int(Some("999"), 20, 100), 100);
    assert_eq!(
        parse_positive_int(Some("999999999999999999999999"), 20, 100),
        100
    );
}

#[test]
fn builds_sequential_names() {
    let names = vec!["Webhook 1".to_string(), "Webhook 3".to_string()];
    assert_eq!(build_next_sequential_name("Webhook", &names), "Webhook 2");
    assert_eq!(
        build_next_sequential_name("", &["未命名 1".to_string()]),
        "未命名 2"
    );
}

#[test]
fn provider_catalog_view_localizes_schema_text() {
    let definition = provider_definition("email").unwrap();
    let view = provider_definition_view(&definition, &Translator::new("zh-CN"));
    assert_eq!(view.get("label"), Some(&json!("邮件")));
    assert!(
        view.get("description")
            .and_then(Value::as_str)
            .is_some_and(|value| value.contains("SMTP"))
    );
    let smtp_host = view
        .get("connection_schema")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .find(|field| field.get("key").and_then(Value::as_str) == Some("smtp_host"))
        .unwrap();
    assert_eq!(smtp_host.get("label"), Some(&json!("SMTP 主机")));
    assert!(
        smtp_host
            .get("description")
            .and_then(Value::as_str)
            .is_some_and(|value| value.contains("邮件发送服务器"))
    );

    let pushplus = provider_definition_view(
        &provider_definition("pushplus").unwrap(),
        &Translator::new("zh-CN"),
    );
    assert_eq!(pushplus.get("label"), Some(&json!("PushPlus 推送")));
    let token = schema_field(&pushplus, "connection_schema", "token");
    assert_eq!(token.get("label"), Some(&json!("令牌")));

    let dingtalk = provider_definition_view(
        &provider_definition("dingtalk").unwrap(),
        &Translator::new("zh-CN"),
    );
    let webhook_url = schema_field(&dingtalk, "connection_schema", "webhook_url");
    assert_eq!(webhook_url.get("label"), Some(&json!("Webhook 地址")));

    let bark = provider_definition_view(
        &provider_definition("bark").unwrap(),
        &Translator::new("zh-CN"),
    );
    let level = schema_field(&bark, "target_schema", "level");
    assert_eq!(level.get("label"), Some(&json!("通知级别")));
    assert!(
        level
            .get("options")
            .and_then(Value::as_array)
            .unwrap()
            .iter()
            .any(|option| option == &json!({"label": "时效性通知", "value": "timeSensitive"}))
    );

    let telegram = provider_definition_view(
        &provider_definition("telegram").unwrap(),
        &Translator::new("zh-CN"),
    );
    let chat_id = schema_field(&telegram, "connection_schema", "chat_id");
    assert_eq!(chat_id.get("label"), Some(&json!("聊天 ID")));
}

#[test]
fn provider_default_names_use_localized_label() {
    let definition = provider_definition("email").unwrap();
    let zh = Translator::new("zh-CN");
    let base = provider_definition_label(&definition, &zh);

    assert_eq!(base, "邮件");
    assert_eq!(
        build_next_sequential_name(&base, &["邮件 1".to_string()]),
        "邮件 2"
    );
}

#[test]
fn provider_catalog_view_includes_node_schema_metadata() {
    let translator = Translator::new("zh-CN");

    let email = provider_definition_view(&provider_definition("email").unwrap(), &translator);
    let smtp_host = schema_field(&email, "connection_schema", "smtp_host");
    assert_eq!(
        smtp_host.get("placeholder"),
        Some(&json!("smtp.example.com"))
    );
    let smtp_port = schema_field(&email, "connection_schema", "smtp_port");
    assert_eq!(smtp_port.get("min"), Some(&json!(1)));
    assert_eq!(smtp_port.get("max"), Some(&json!(65535)));

    let wxpusher = provider_definition_view(&provider_definition("wxpusher").unwrap(), &translator);
    let default_uids = schema_field(&wxpusher, "connection_schema", "uids");
    assert_eq!(default_uids.get("label"), Some(&json!("默认 UID 列表")));
    assert_eq!(
        default_uids.get("placeholder"),
        Some(&json!("UID_xxx,UID_yyy"))
    );
    let target_verify = schema_field(&wxpusher, "target_schema", "verify_pay_type");
    assert_eq!(
        target_verify.get("default_value"),
        Some(&json!("__inherit__"))
    );

    let wecom = provider_definition_view(&provider_definition("wecom").unwrap(), &translator);
    assert_eq!(
        schema_field(&wecom, "connection_schema", "webhook_url").get("placeholder"),
        Some(&json!(
            "https://qyapi.weixin.qq.com/cgi-bin/webhook/send?key=xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
        ))
    );
    assert!(
        wecom
            .get("connection_schema")
            .and_then(Value::as_array)
            .unwrap()
            .iter()
            .all(|field| field.get("key").and_then(Value::as_str) != Some("secret"))
    );
}

#[test]
fn provider_catalog_view_matches_node_capabilities() {
    let translator = Translator::new("zh-CN");

    let magicpush =
        provider_definition_view(&provider_definition("magicpush").unwrap(), &translator);
    assert_eq!(
        magicpush.pointer("/capabilities/supports_markdown"),
        Some(&json!(false))
    );
    assert_eq!(
        magicpush.pointer("/capabilities/supports_actions"),
        Some(&json!(false))
    );

    let bark = provider_definition_view(&provider_definition("bark").unwrap(), &translator);
    assert_eq!(
        bark.pointer("/capabilities/supports_markdown"),
        Some(&json!(false))
    );
    assert_eq!(
        bark.pointer("/capabilities/supports_actions"),
        Some(&json!(true))
    );

    let feishu = provider_definition_view(&provider_definition("feishu").unwrap(), &translator);
    assert_eq!(
        feishu.pointer("/capabilities/supports_markdown"),
        Some(&json!(false))
    );
    assert_eq!(
        feishu.pointer("/capabilities/supports_actions"),
        Some(&json!(true))
    );
    assert_eq!(
        feishu.pointer("/capabilities/max_body_length"),
        Some(&json!(20480))
    );

    let wecom = provider_definition_view(&provider_definition("wecom").unwrap(), &translator);
    assert_eq!(
        wecom.pointer("/capabilities/supports_mentions"),
        Some(&json!(true))
    );
    assert_eq!(
        wecom.pointer("/capabilities/max_body_length"),
        Some(&json!(4096))
    );

    let serverchan =
        provider_definition_view(&provider_definition("serverchan").unwrap(), &translator);
    assert_eq!(
        serverchan.pointer("/capabilities/max_body_length"),
        Some(&json!(32768))
    );

    let telegram = provider_definition_view(&provider_definition("telegram").unwrap(), &translator);
    assert_eq!(
        telegram.pointer("/capabilities/max_body_length"),
        Some(&json!(4096))
    );

    let webhook = provider_definition_view(&provider_definition("webhook").unwrap(), &translator);
    assert_eq!(
        webhook.pointer("/capabilities/max_body_length"),
        Some(&Value::Null)
    );
}

#[test]
fn localizes_provider_test_builtin_messages() {
    let zh = Translator::new("zh-CN");
    assert_eq!(
        parse_json_body(&Bytes::from_static(b"{"), &zh).expect_err("invalid json body should fail"),
        "请求体必须是合法 JSON"
    );
    assert_eq!(
        notification_service_text(&zh, "providerTestName", &[("provider", "Webhook".into())]),
        "Webhook 测试"
    );
    assert_eq!(
        localize_provider_test_message(&zh, "Notification provider test sent successfully"),
        "测试发送成功"
    );
    assert_eq!(
        localize_provider_test_message(&zh, "Webhook request returned status 503"),
        "Webhook 请求返回状态 503"
    );
    assert_eq!(
        localize_provider_test_result(
            ProviderTestResult {
                success: false,
                retryable: true,
                message: "Telegram request returned status 429".to_string(),
                request_summary: None,
                response_summary: None,
            },
            &zh,
        )
        .message,
        "Telegram 请求返回状态 429"
    );
    assert_eq!(
        localize_provider_test_message(&zh, "Bark failed for 1/2 target(s)"),
        "Bark 1/2 个目标发送失败"
    );
    assert_eq!(
        localize_provider_test_message(&zh, "Invalid WxPusher topic id(s): abc"),
        "Topic ID 格式不正确：abc"
    );

    let en = Translator::new("en");
    assert_eq!(
        localize_provider_test_message(&en, "缺少 Webhook URL"),
        "Missing Webhook URL"
    );
    assert_eq!(
        localize_provider_test_message(&en, "测试发送成功"),
        "Test send succeeded"
    );
    assert_eq!(
        localize_provider_test_message(&en, "Topic ID 格式不正确：abc"),
        "Invalid Topic ID format: abc"
    );
    assert_eq!(
        localize_provider_test_message(&en, "缺少 MeoW 接收昵称"),
        "Missing MeoW recipient nickname"
    );
    assert_eq!(
        localize_provider_test_message(&en, "MeoW 接收昵称不能包含斜杠"),
        "MeoW recipient nickname cannot contain a slash"
    );
}

#[test]
fn deleted_provider_snapshot_uses_config_locale() {
    let snapshot = deleted_provider_snapshot(
        "provider-1",
        "2026-01-02T03:04:05Z",
        &Translator::new("zh-CN"),
    );
    assert_eq!(snapshot.get("name"), Some(&json!("已删除提供商")));
}

#[test]
fn localizes_rule_names_and_fallback_messages() {
    let zh = Translator::new("zh-CN");
    assert_eq!(
        build_notification_rule_name("FN_EVENT_AUTH_LOGIN_SUCCESS", &zh),
        "登录成功 通知"
    );
    let message = build_notification_message(
        &json!({
            "id": "evt_1",
            "type": "FN_EVENT_AUTH_LOGIN_SUCCESS",
            "level": "WARN",
            "source": "GO_REAUTH_PROXY",
            "happened_at": "2026-07-06T00:00:00.000Z",
            "dedupe_key": "auth-login"
        }),
        &json!({
            "id": "rule_1",
            "window_seconds": 60
        }),
        2,
        "global",
        &zh,
    );

    assert_eq!(message.get("title"), Some(&json!("敲门 Knock 登录成功 x2")));
    assert!(
        message
            .get("body_text")
            .and_then(Value::as_str)
            .is_some_and(|value| value.contains("本次通知已在 60 秒窗口内聚合 2 条相似事件"))
    );
    let facts = message.get("facts").and_then(Value::as_array).unwrap();
    assert!(
        facts
            .iter()
            .any(|fact| fact.get("label") == Some(&json!("事件类型")))
    );
    assert!(
        facts
            .iter()
            .any(|fact| fact.get("label") == Some(&json!("风险级别")))
    );
    assert!(!serde_json::to_string(&message).unwrap().contains("Matched"));
}

#[test]
fn notification_copy_includes_ip_location_without_generic_advice() {
    let translator = Translator::new("zh-CN");
    let details = build_notification_details(
        &json!({
            "type": "FN_EVENT_AUTH_LOGIN_FAILURE",
            "source": "SERVER_ADMIN",
            "level": "WARN",
            "payload": {
                "ip": "203.0.113.8",
                "ip_location": "上海|上海|联通",
                "attempts": 3
            }
        }),
        &json!({ "window_seconds": 60 }),
        1,
        &translator,
    );

    assert!(details.summary.contains("203.0.113.8（上海|上海|联通）"));
    assert!(details.body_text.contains("203.0.113.8（上海|上海|联通）"));
    assert!(!details.body_text.contains("如非本人操作"));
    assert!(!details.body_markdown.contains("**事件概述**"));

    let drift_summary = format!(
        "{} -> {}",
        format_notification_ip_with_location("203.0.113.8", "上海|上海|联通", &translator,),
        format_notification_ip_with_location("203.0.113.9", "上海|上海|联通", &translator,),
    );
    assert_eq!(
        drift_summary,
        "203.0.113.8（上海|上海|联通） -> 203.0.113.9（上海|上海|联通）"
    );

    let waf_details = build_notification_details(
        &json!({
            "type": "FN_EVENT_WAF_BLOCKED",
            "source": "GO_REAUTH_PROXY",
            "level": "WARN",
            "payload": {
                "ip": "203.0.113.8",
                "ip_location": "上海|上海|联通",
                "path": "/api/203.0.113.8",
                "action": "deny",
                "mode": "blocking"
            }
        }),
        &json!({ "window_seconds": 60 }),
        1,
        &translator,
    );
    assert!(
        waf_details
            .summary
            .contains("203.0.113.8（上海|上海|联通）")
    );
    assert!(waf_details.body_text.contains("/api/203.0.113.8"));
    assert!(!waf_details.body_text.contains("/api/203.0.113.8（"));
}

#[test]
fn notification_ip_location_wait_uses_stream_receive_time() {
    let now = 1_000_000_i64;
    assert!(should_wait_for_ip_location("999999-0", now));
    assert!(!should_wait_for_ip_location(
        &format!("{}-0", now - IP_LOCATION_NOTIFICATION_WAIT_MS),
        now,
    ));
    assert!(!should_wait_for_ip_location("invalid", now));
}

#[test]
fn localizes_email_address_validation_errors() {
    let zh = Translator::new("zh-CN");
    assert_eq!(
        parse_mailboxes("bad-address", "to_addresses", &zh)
            .expect_err("invalid mailbox should fail"),
        "收件人 中包含无效邮箱地址: bad-address"
    );
    assert_eq!(
        build_from_mailbox("bad-address", "", &zh).expect_err("invalid from should fail"),
        "发件邮箱格式不正确"
    );
    assert!(
        build_email_plain_text_body(
            &json!({
                "body_text": "正文",
                "severity": "info",
                "event_id": "evt_1",
                "occurred_at": "2026-07-06T00:00:00.000Z"
            }),
            &zh
        )
        .contains("发生时间: 2026-07-06T00:00:00.000Z")
    );
}
