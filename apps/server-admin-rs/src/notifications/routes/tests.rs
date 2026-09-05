use super::*;

async fn notification_test_state() -> (tempfile::TempDir, AppState) {
    let directory = tempfile::tempdir().unwrap();
    let mut settings = {
        let _environment = crate::test_support::EnvGuard::new(&[]);
        crate::settings::Settings::from_env()
    };
    settings.runtime_target = "linux".to_string();
    settings.data_dir = directory.path().join("data");
    settings.gateway_config_dir = directory.path().join("gateway");
    settings.sqlite_path = directory.path().join("fn-knock.sqlite3");
    settings.legacy_redis_url.clear();
    settings.go_backend_grpc_addr = "127.0.0.1:1".to_string();
    settings.internal_rpc_token = "notification-test-token".to_string();
    let state = AppState::new(settings).await.unwrap();
    (directory, state)
}

#[tokio::test]
async fn notification_dispatch_drains_burst_after_coalesced_wakeups() {
    let (_directory, state) = notification_test_state().await;
    state
        .storage
        .store
        .set_notification_last_stream_id("0-0")
        .await
        .unwrap();
    save_rule_raw(
        &state,
        &json!({
            "id": "burst-rule", "enabled": true, "event_type": "FN_EVENT_RUNTIME_STARTED",
            "threshold_count": 1, "window_seconds": 60, "cooldown_seconds": 0,
            "targets": [{"id": "disabled-target", "enabled": false}]
        }),
    )
    .await
    .unwrap();
    let count = STREAM_BATCH_SIZE * 2 + 7;
    for index in 0..count {
        state.storage.store.append_system_event(&json!({
            "id": format!("burst-{index}"), "type": "FN_EVENT_RUNTIME_STARTED",
            "source": "RUNTIME_MONITOR", "level": "INFO", "happened_at": time_utils::now_iso(),
            "subject": {"kind": "COMPONENT", "id": "management"},
            "payload": {"component": "management"}
        }), 30, 1000).await.unwrap();
        state.request_notification_dispatch();
    }
    let tail = state
        .storage
        .store
        .latest_system_event_stream_id()
        .await
        .unwrap();
    // No publication/IP-location work runs after this point. All notify calls
    // above collapse to one permit, so progress requires draining full batches.
    let dispatch = notification_dispatch_loop(state.clone());
    let observe = async {
        let result = tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if state
                    .storage
                    .store
                    .get_notification_last_stream_id()
                    .await
                    .unwrap()
                    == tail
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await;
        state.shutdown.cancel();
        assert!(
            result.is_ok(),
            "dispatcher left a burst waiting for an external wakeup"
        );
    };
    tokio::join!(dispatch, observe);
    for index in 0..count {
        let event_id = format!("burst-{index}");
        let trigger_id = create_stable_id("ntftrig", &["burst-rule", &event_id]);
        let delivery_id = create_stable_id("ntfdel", &[&trigger_id, "disabled-target"]);
        assert_eq!(
            load_delivery(&state, &delivery_id).await.unwrap().unwrap()["status"],
            "skipped"
        );
    }
}

#[tokio::test]
async fn notification_dispatch_waits_for_busy_lease_and_retries_without_new_events() {
    let (_directory, state) = notification_test_state().await;
    assert!(
        state
            .storage
            .store
            .acquire_notification_runtime_lease("dispatch", "other-worker", 15)
            .await
            .unwrap()
    );
    assert_eq!(
        notification_dispatch_tick(&state).await.unwrap(),
        DispatchSchedule::RetryAfter(DISPATCH_ERROR_RETRY_DELAY)
    );
    let started = std::time::Instant::now();
    assert!(
        tokio::time::timeout(
            Duration::from_secs(1),
            wait_for_notification_dispatch(
                &state,
                DispatchSchedule::RetryAfter(Duration::from_millis(20))
            )
        )
        .await
        .unwrap()
    );
    assert!(started.elapsed() >= Duration::from_millis(20));
}

#[tokio::test]
async fn notification_dispatch_schedules_ip_location_deadline_without_advancing_cursor() {
    let (_directory, state) = notification_test_state().await;
    state
        .storage
        .store
        .set_notification_last_stream_id("0-0")
        .await
        .unwrap();
    save_rule_raw(
        &state,
        &json!({
            "id": "ip-rule", "enabled": true, "event_type": "FN_EVENT_AUTH_LOGIN_FAILURE",
            "targets": []
        }),
    )
    .await
    .unwrap();
    state
        .storage
        .store
        .append_system_event(
            &json!({
                "id": "await-ip", "type": "FN_EVENT_AUTH_LOGIN_FAILURE", "source": "AUTH",
                "level": "WARN", "happened_at": time_utils::now_iso(), "payload": {"ip": "8.8.4.4"}
            }),
            30,
            1000,
        )
        .await
        .unwrap();
    let schedule = notification_dispatch_tick(&state).await.unwrap();
    assert!(
        matches!(schedule, DispatchSchedule::RetryAfter(delay) if !delay.is_zero()
        && delay <= Duration::from_millis(IP_LOCATION_NOTIFICATION_WAIT_MS as u64))
    );
    assert_eq!(
        state
            .storage
            .store
            .get_notification_last_stream_id()
            .await
            .unwrap()
            .as_deref(),
        Some("0-0")
    );
}

async fn receive_webhook_request(mut stream: tokio::net::TcpStream) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut request = Vec::new();
    loop {
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
            break;
        }
    }
    stream
        .write_all(
            b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{\"ok\":true}",
        )
        .await
        .unwrap();
    String::from_utf8(request).unwrap()
}

fn request_header_value<'a>(request: &'a str, expected_name: &str) -> Option<&'a str> {
    request.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case(expected_name)
            .then_some(value.trim())
    })
}

fn request_body(request: &str) -> &str {
    request
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .unwrap_or("")
}

#[test]
fn notification_trace_filter_normalizes_input_and_supports_snapshot_fallback() {
    let trace_id = "trc_3f93d40a-89ea-4dbe-a04f-67692778d973";
    let legacy_record = json!({
        "id": "delivery-legacy",
        "message_snapshot": { "trace_id": trace_id }
    });
    assert!(matches_optional_trace_id(
        &legacy_record,
        Some(&format!("  {trace_id}  "))
    ));
    assert!(!matches_optional_trace_id(
        &legacy_record,
        Some("trc_00000000-0000-4000-8000-000000000000")
    ));
}

#[test]
fn delivery_records_omit_empty_trace_ids_but_keep_real_correlations() {
    let build = |trace_id: &str| {
        build_delivery_value(DeliveryBuildArgs {
            id: "delivery-1".to_string(),
            trace_id: trace_id.to_string(),
            trigger_id: "trigger-1".to_string(),
            rule_id: "rule-1".to_string(),
            target_id: "target-1".to_string(),
            provider_id: "provider-1".to_string(),
            event_id: "event-1".to_string(),
            status: "queued".to_string(),
            reason: None,
            provider_type: "webhook".to_string(),
            message_snapshot: json!({}),
            target_snapshot: json!({}),
            provider_snapshot: json!({}),
            webhook_event_snapshot: None,
            attempt_count: 0,
            triggered_at: "2026-08-29T00:00:00Z".to_string(),
            next_retry_at: None,
        })
    };

    assert!(build("").get("trace_id").is_none());
    assert_eq!(
        build("trc_3f93d40a-89ea-4dbe-a04f-67692778d973").get("trace_id"),
        Some(&json!("trc_3f93d40a-89ea-4dbe-a04f-67692778d973"))
    );
}

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

    let masked = mask_provider(&json!({
        "id": "ntfprov_webhook",
        "name": "Webhook",
        "type": "webhook",
        "connection_config": {
            "url": "https://example.com/hook",
            "custom_headers": [{ "name": "Authorization", "value": "Bearer secret" }],
            "body_config": {
                "mode": "custom",
                "format": "text",
                "template": "private body template"
            }
        }
    }))
    .unwrap();
    assert_eq!(
        masked.pointer("/connection_config_masked/custom_headers"),
        Some(&json!("[configured]"))
    );
    assert_eq!(
        masked.pointer("/connection_config_masked/body_config"),
        Some(&json!("[configured]"))
    );
    assert!(!masked.to_string().contains("Bearer secret"));
    assert!(!masked.to_string().contains("private body template"));
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
fn webhook_header_schema_is_provider_scoped_and_advertises_constraints() {
    let definition = provider_definition("webhook").unwrap();
    let custom_headers = definition
        .connection_schema
        .iter()
        .find(|field| field.key == "custom_headers")
        .unwrap();
    assert_eq!(custom_headers.field_type, "headers");
    assert!(custom_headers.sensitive);
    assert_eq!(
        custom_headers
            .constraints
            .as_ref()
            .and_then(|value| value.get("max_items")),
        Some(&json!(32))
    );
    let constraints = custom_headers.constraints.as_ref().unwrap();
    assert_eq!(constraints.get("max_name_bytes"), Some(&json!(128)));
    assert_eq!(constraints.get("max_value_bytes"), Some(&json!(8 * 1024)));
    assert_eq!(constraints.get("max_total_bytes"), Some(&json!(16 * 1024)));
    assert_eq!(
        constraints
            .get("reserved_names")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(WEBHOOK_RESERVED_HEADER_NAMES.len())
    );
    assert!(
        definition
            .target_schema
            .iter()
            .all(|field| field.key != "extra_headers_json")
    );
    assert!(definition.sensitive_fields.contains(&"custom_headers"));
}

#[test]
fn webhook_body_schema_advertises_scopes_limits_and_hides_legacy_editor() {
    let definition = provider_definition("webhook").unwrap();
    let provider_body = definition
        .connection_schema
        .iter()
        .find(|field| field.key == "body_config")
        .unwrap();
    let target_body = definition
        .target_schema
        .iter()
        .find(|field| field.key == "body_override")
        .unwrap();
    assert_eq!(provider_body.field_type, "webhook_body");
    assert!(provider_body.sensitive);
    assert_eq!(
        provider_body.default_value,
        Some(json!({ "mode": "standard" }))
    );
    assert_eq!(target_body.field_type, "webhook_body");
    assert_eq!(
        target_body.default_value,
        Some(json!({ "mode": "inherit" }))
    );
    assert_eq!(
        provider_body.constraints.as_ref().unwrap().get("scope"),
        Some(&json!("provider"))
    );
    assert_eq!(
        target_body.constraints.as_ref().unwrap().get("scope"),
        Some(&json!("target"))
    );
    assert_eq!(
        provider_body
            .constraints
            .as_ref()
            .unwrap()
            .get("max_template_bytes"),
        Some(&json!(WEBHOOK_MAX_BODY_TEMPLATE_BYTES))
    );
    assert!(
        definition
            .target_schema
            .iter()
            .all(|field| field.key != "extra_body_json")
    );
    assert!(definition.sensitive_fields.contains(&"body_config"));
}

#[test]
fn webhook_body_configs_normalize_validate_and_resolve_precedence() {
    assert_eq!(
        normalize_webhook_body_config(&json!({}), WebhookBodyScope::Provider).unwrap(),
        json!({ "mode": "standard" })
    );
    assert_eq!(
        normalize_webhook_body_config(&json!({}), WebhookBodyScope::Target).unwrap(),
        json!({ "mode": "inherit" })
    );
    let provider_custom = json!({
        "mode": "custom",
        "format": "json",
        "content_type": "application/problem+json",
        "template": "{\"provider\":true}"
    });
    let target_custom = json!({
        "mode": "custom",
        "format": "text",
        "content_type": "text/plain; charset=utf-8",
        "template": "target"
    });
    let provider = Map::from_iter([("body_config".to_string(), provider_custom.clone())]);
    let inherited = Map::from_iter([("body_override".to_string(), json!({ "mode": "inherit" }))]);
    assert_eq!(
        resolve_webhook_body_config(&provider, Some(&inherited))
            .unwrap()
            .unwrap()
            .format,
        WebhookBodyFormat::Json
    );
    let overridden = Map::from_iter([("body_override".to_string(), target_custom)]);
    assert_eq!(
        resolve_webhook_body_config(&provider, Some(&overridden))
            .unwrap()
            .unwrap()
            .format,
        WebhookBodyFormat::Text
    );
    assert!(
        resolve_webhook_body_config(&Map::new(), None)
            .unwrap()
            .is_none()
    );

    for invalid in [
        json!(0),
        json!({ "mode": true }),
        json!({ "mode": "custom", "format": false, "template": "{}" }),
        json!({ "mode": "custom", "format": "text", "template": 42 }),
        json!({ "mode": "custom", "format": "text", "template": "", "content_type": 42 }),
        json!({ "mode": "inherit" }),
        json!({ "mode": "custom", "format": "yaml", "template": "{}" }),
        json!({ "mode": "custom", "format": "json", "template": "{" }),
        json!({ "mode": "custom", "format": "text", "template": "{{secret.value}}" }),
        json!({ "mode": "custom", "format": "text", "template": "{{message.title}}", "content_type": "text/plain\r\nX-Evil: yes" }),
        json!({ "mode": "custom", "format": "text", "template": "{{message.title}}", "content_type": "not a mime" }),
    ] {
        assert!(parse_webhook_body_config(&invalid, WebhookBodyScope::Provider).is_err());
    }

    let max_template = json!({
        "mode": "custom",
        "format": "text",
        "template": "x".repeat(WEBHOOK_MAX_BODY_TEMPLATE_BYTES)
    });
    parse_webhook_body_config(&max_template, WebhookBodyScope::Provider).unwrap();
    let oversized_template = json!({
        "mode": "custom",
        "format": "text",
        "template": "x".repeat(WEBHOOK_MAX_BODY_TEMPLATE_BYTES + 1)
    });
    assert!(parse_webhook_body_config(&oversized_template, WebhookBodyScope::Provider).is_err());

    let max_placeholders = json!({
        "mode": "custom",
        "format": "text",
        "template": "{{message.title}}".repeat(WEBHOOK_MAX_BODY_PLACEHOLDERS)
    });
    parse_webhook_body_config(&max_placeholders, WebhookBodyScope::Provider).unwrap();
    let too_many_placeholders = json!({
        "mode": "custom",
        "format": "text",
        "template": "{{message.title}}".repeat(WEBHOOK_MAX_BODY_PLACEHOLDERS + 1)
    });
    assert!(parse_webhook_body_config(&too_many_placeholders, WebhookBodyScope::Provider).is_err());
}

#[test]
fn webhook_body_renderer_preserves_json_types_paths_escapes_and_missing_values() {
    let context = json!({
        "message": { "title": "Alert" },
        "event": { "payload": { "ip": "192.0.2.10", "items": [1, { "ok": true }] } }
    });
    let json_config = parse_webhook_body_config(
        &json!({
            "mode": "custom",
            "format": "json",
            "template": r#"{"payload":"{{event.payload}}","item":"{{event.payload.items.1}}","embedded":"ip={{event.payload.ip}} items={{event.payload.items}}","missing":"{{event.payload.none}}","missing_text":"x{{event.payload.none}}y","literal":"\\{{message.title}}"}"#
        }),
        WebhookBodyScope::Provider,
    )
    .unwrap();
    let rendered = render_webhook_body(&json_config, &context).unwrap();
    let body: Value = serde_json::from_slice(&rendered.bytes).unwrap();
    assert_eq!(body.pointer("/payload/ip"), Some(&json!("192.0.2.10")));
    assert_eq!(body.pointer("/item/ok"), Some(&json!(true)));
    assert_eq!(
        body.get("embedded"),
        Some(&json!("ip=192.0.2.10 items=[1,{\"ok\":true}]"))
    );
    assert_eq!(body.get("missing"), Some(&Value::Null));
    assert_eq!(body.get("missing_text"), Some(&json!("xy")));
    assert_eq!(body.get("literal"), Some(&json!("{{message.title}}")));
    assert_eq!(rendered.missing_variables, vec!["event.payload.none"]);

    let text_config = parse_webhook_body_config(
        &json!({
            "mode": "custom",
            "format": "text",
            "template": r#"\{{literal}} {{message.title}} {{event.payload.items.1}} {{event.payload.missing}}"#
        }),
        WebhookBodyScope::Provider,
    )
    .unwrap();
    let rendered = render_webhook_body(&text_config, &context).unwrap();
    assert_eq!(
        String::from_utf8(rendered.bytes).unwrap(),
        "{{literal}} Alert {\"ok\":true} "
    );
    assert_eq!(rendered.missing_variables, vec!["event.payload.missing"]);
}

#[test]
fn webhook_sample_context_keeps_provider_authoritative_and_removes_event_trace_id() {
    let provider = json!({ "id": "ntfprov_real", "name": "Real", "type": "webhook" });
    let base = build_webhook_template_context(
        &json!({ "title": "Base" }),
        &json!({
            "id": "evt_base",
            "trace_id": "hidden-base",
            "payload": { "trace_id": "hidden-nested-base" }
        }),
        json!({ "mode": "provider_test" }),
        &json!({}),
        &json!({}),
        &provider,
        json!({}),
    );
    let applied = apply_webhook_sample_context(
        base,
        Some(&json!({
            "event": {
                "id": "evt_sample",
                "trace_id": "hidden-sample",
                "payload": {
                    "ip": "192.0.2.20",
                    "trace_id": "hidden-nested-sample",
                    "items": [{ "waf_trace_id": "hidden-array-sample" }]
                }
            },
            "message": { "title": "Sample", "trace_id": "hidden-message" },
            "provider": { "id": "spoofed", "shared_secret": "secret" },
            "context": { "mode": "spoofed", "delivery_id": "sample-delivery", "secret": "hidden-context" },
            "rule": { "id": "sample-rule", "shared_secret": "hidden-rule" },
            "target": { "id": "sample-target", "endpoint_path": "/hidden" },
            "legacy": { "extra_body": { "kept": true }, "secret": "hidden-legacy" }
        })),
        "target_test",
        &provider,
    )
    .unwrap();
    assert_eq!(applied.pointer("/event/id"), Some(&json!("evt_sample")));
    assert!(applied.pointer("/event/trace_id").is_none());
    assert!(applied.pointer("/event/payload/trace_id").is_none());
    assert!(
        applied
            .pointer("/event/payload/items/0/waf_trace_id")
            .is_none()
    );
    assert!(applied.pointer("/message/trace_id").is_none());
    assert_eq!(
        applied.pointer("/provider/id"),
        Some(&json!("ntfprov_real"))
    );
    assert!(applied.pointer("/provider/shared_secret").is_none());
    assert_eq!(
        applied.pointer("/context/mode"),
        Some(&json!("target_test"))
    );
    assert_eq!(
        applied.pointer("/context/delivery_id"),
        Some(&json!("sample-delivery"))
    );
    for path in [
        "/context/secret",
        "/rule/shared_secret",
        "/target/endpoint_path",
        "/legacy/secret",
    ] {
        assert!(applied.pointer(path).is_none(), "unexpected path {path}");
    }
    assert_eq!(
        applied.pointer("/legacy/extra_body/kept"),
        Some(&json!(true))
    );

    let oversized_sample = json!({ "message": "x".repeat(WEBHOOK_MAX_BODY_SAMPLE_BYTES) });
    assert!(
        apply_webhook_sample_context(
            json!({}),
            Some(&oversized_sample),
            "provider_test",
            &provider
        )
        .is_err()
    );

    let oversized_render = parse_webhook_body_config(
        &json!({
            "mode": "custom",
            "format": "text",
            "template": "{{message}}"
        }),
        WebhookBodyScope::Provider,
    )
    .unwrap();
    assert!(
        render_webhook_body(
            &oversized_render,
            &json!({ "message": "x".repeat(WEBHOOK_MAX_RENDERED_BODY_BYTES + 1) })
        )
        .is_err()
    );

    let duplicate_keys = parse_webhook_body_config(
        &json!({
            "mode": "custom",
            "format": "json",
            "template": "{\"{{message.first}}\":1,\"{{message.second}}\":2}"
        }),
        WebhookBodyScope::Provider,
    )
    .unwrap();
    let duplicate_error = render_webhook_body(
        &duplicate_keys,
        &json!({ "message": { "first": "secret-key-value", "second": "secret-key-value" } }),
    )
    .unwrap_err();
    assert!(!duplicate_error.default_text().contains("secret-key-value"));
}

#[test]
fn webhook_custom_headers_normalize_and_preserve_order() {
    let normalized = normalize_webhook_custom_headers(&json!([
        { "name": " Authorization ", "value": " Bearer token " },
        { "name": "X-Empty", "value": "" }
    ]))
    .unwrap();
    assert_eq!(
        normalized,
        json!([
            { "name": "Authorization", "value": "Bearer token" },
            { "name": "X-Empty", "value": "" }
        ])
    );
}

#[test]
fn webhook_custom_headers_accept_documented_boundaries_and_common_names() {
    let max_count = Value::Array(
        (0..WEBHOOK_MAX_CUSTOM_HEADERS)
            .map(|index| json!({ "name": format!("X-Header-{index}"), "value": "ok" }))
            .collect(),
    );
    assert_eq!(
        parse_webhook_custom_headers(&max_count).unwrap().len(),
        WEBHOOK_MAX_CUSTOM_HEADERS
    );

    let max_name = "X".repeat(WEBHOOK_MAX_HEADER_NAME_BYTES);
    parse_webhook_custom_headers(&json!([{ "name": max_name, "value": "ok" }])).unwrap();
    parse_webhook_custom_headers(&json!([{
        "name": "X-Large",
        "value": "x".repeat(WEBHOOK_MAX_HEADER_VALUE_BYTES)
    }]))
    .unwrap();

    let exact_total = json!([
        { "name": "X-One", "value": "x".repeat(8_187) },
        { "name": "X-Two", "value": "x".repeat(8_187) }
    ]);
    parse_webhook_custom_headers(&exact_total).unwrap();

    parse_webhook_custom_headers(&json!([
        { "name": "Authorization", "value": "Bearer token" },
        { "name": "Cookie", "value": "session=example" },
        { "name": "User-Agent", "value": "fn-knock-test" },
        { "name": "Accept", "value": "application/json" },
        { "name": "X-API-Key", "value": "key" }
    ]))
    .unwrap();
}

#[test]
fn webhook_custom_headers_reject_unsafe_or_oversized_values() {
    for invalid in [
        Value::Null,
        json!({ "Authorization": "Bearer token" }),
        json!([{ "name": "", "value": "value" }]),
        json!([{ "name": "\tX-Token", "value": "value" }]),
        json!([{ "name": "Bad Header", "value": "value" }]),
        json!([{ "name": "Content-Type", "value": "text/plain" }]),
        json!([
            { "name": "X-Token", "value": "one" },
            { "name": "x-token", "value": "two" }
        ]),
        json!([{ "name": "X-Token", "value": "line\nbreak" }]),
        json!([{ "name": "X-Token", "value": "\ttrim-bypass" }]),
        json!([{ "name": "X-Token", "value": "trim-bypass\t" }]),
        json!([{ "name": "X-Token", "value": "control\u{0085}" }]),
        json!([{ "name": "X-Token", "value": 42 }]),
        json!([{ "name": "X".repeat(WEBHOOK_MAX_HEADER_NAME_BYTES + 1), "value": "value" }]),
        json!([{ "name": "X-Token", "value": "x".repeat(WEBHOOK_MAX_HEADER_VALUE_BYTES + 1) }]),
    ] {
        assert!(normalize_webhook_custom_headers(&invalid).is_err());
    }

    let too_many = Value::Array(
        (0..=WEBHOOK_MAX_CUSTOM_HEADERS)
            .map(|index| json!({ "name": format!("X-Header-{index}"), "value": "ok" }))
            .collect(),
    );
    assert!(normalize_webhook_custom_headers(&too_many).is_err());

    let too_large = json!([
        { "name": "X-One", "value": "x".repeat(6000) },
        { "name": "X-Two", "value": "x".repeat(6000) },
        { "name": "X-Three", "value": "x".repeat(6000) }
    ]);
    assert!(normalize_webhook_custom_headers(&too_large).is_err());

    for reserved in WEBHOOK_RESERVED_HEADER_NAMES {
        assert!(
            normalize_webhook_custom_headers(&json!([{
                "name": reserved.to_ascii_uppercase(),
                "value": "blocked"
            }]))
            .is_err(),
            "reserved header {reserved} was accepted"
        );
    }
}

#[test]
fn webhook_header_validation_uses_the_active_server_locale() {
    let definition = provider_definition("webhook").unwrap();
    let config = Map::from_iter([(
        "custom_headers".to_string(),
        json!([{ "name": "X-Token", "value": "bad\r\nvalue" }]),
    )]);
    let error = validate_provider_connection_patch(&definition, &config, &Translator::new("en"))
        .unwrap_err();
    match error {
        NotifyError::BadRequest(message) => {
            assert_eq!(message, "The value for header X-Token is invalid");
        }
        NotifyError::Storage(_) => panic!("unexpected storage error"),
    }
}

#[test]
fn webhook_headers_switch_from_legacy_target_to_provider_configuration() {
    let malformed_legacy_target =
        Map::from_iter([("extra_headers_json".to_string(), json!("not-an-object"))]);
    assert!(resolve_webhook_headers(&Map::new(), Some(&malformed_legacy_target)).is_err());
    let unsafe_legacy_target = Map::from_iter([(
        "extra_headers_json".to_string(),
        json!({ "X-Legacy": "\ttrim-bypass" }),
    )]);
    assert!(resolve_webhook_headers(&Map::new(), Some(&unsafe_legacy_target)).is_err());

    let legacy_target = Map::from_iter([(
        "extra_headers_json".to_string(),
        json!({ "Authorization": "Bearer legacy", "X-Fn-Knock-Trace-Id": "hidden" }),
    )]);
    let legacy = resolve_webhook_headers(&Map::new(), Some(&legacy_target)).unwrap();
    assert_eq!(
        legacy,
        vec![WebhookHeader {
            name: "Authorization".to_string(),
            value: "Bearer legacy".to_string(),
        }]
    );

    let switched_empty = Map::from_iter([("custom_headers".to_string(), json!([]))]);
    assert!(
        resolve_webhook_headers(&switched_empty, Some(&legacy_target))
            .unwrap()
            .is_empty()
    );

    let switched = Map::from_iter([(
        "custom_headers".to_string(),
        json!([{ "name": "Authorization", "value": "Bearer provider" }]),
    )]);
    assert_eq!(
        resolve_webhook_headers(&switched, Some(&legacy_target)).unwrap()[0].value,
        "Bearer provider"
    );
}

#[tokio::test]
async fn webhook_provider_save_switches_and_rule_save_cleans_legacy_headers() {
    let (_directory, state) = notification_test_state().await;
    let provider_id = "ntfprov_legacy_webhook";
    save_provider_raw(
        &state,
        &json!({
            "id": provider_id,
            "name": "Legacy webhook",
            "type": "webhook",
            "enabled": true,
            "connection_config": {
                "url": "https://example.com/hook",
                "method": "POST",
                "timeout_seconds": 5
            },
            "created_at": "2026-09-02T00:00:00Z",
            "updated_at": "2026-09-02T00:00:00Z"
        }),
    )
    .await
    .unwrap();

    let raw_targets = json!([{
        "id": "ntftarget_legacy",
        "provider_id": provider_id,
        "target_config": {}
    }]);
    let current_targets = vec![json!({
        "id": "ntftarget_legacy",
        "provider_id": provider_id,
        "target_config": {
            "extra_headers_json": { "Authorization": "Bearer legacy" }
        }
    })];
    let translator = Translator::new("en");
    let preserved =
        normalize_rule_targets(&state, Some(&raw_targets), &current_targets, &translator)
            .await
            .unwrap();
    assert_eq!(
        preserved[0].pointer("/target_config/extra_headers_json/Authorization"),
        Some(&json!("Bearer legacy"))
    );

    let second_provider_id = "ntfprov_second_legacy_webhook";
    save_provider_raw(
        &state,
        &json!({
            "id": second_provider_id,
            "name": "Second legacy webhook",
            "type": "webhook",
            "enabled": true,
            "connection_config": { "url": "https://second.example.com/hook" },
            "created_at": "2026-09-02T00:00:00Z",
            "updated_at": "2026-09-02T00:00:00Z"
        }),
    )
    .await
    .unwrap();
    let switched_provider_target = json!([{
        "id": "ntftarget_legacy",
        "provider_id": second_provider_id,
        "target_config": {}
    }]);
    let provider_changed = normalize_rule_targets(
        &state,
        Some(&switched_provider_target),
        &current_targets,
        &translator,
    )
    .await
    .unwrap();
    assert!(
        provider_changed[0]
            .pointer("/target_config/extra_headers_json")
            .is_none()
    );

    update_provider_value(
        &state,
        provider_id,
        json!({ "name": "Saved legacy webhook" }),
    )
    .await
    .unwrap();
    let saved = load_provider(&state, provider_id).await.unwrap().unwrap();
    assert_eq!(
        saved.pointer("/connection_config/custom_headers"),
        Some(&json!([]))
    );

    let cleaned = normalize_rule_targets(&state, Some(&raw_targets), &current_targets, &translator)
        .await
        .unwrap();
    assert!(
        cleaned[0]
            .pointer("/target_config/extra_headers_json")
            .is_none()
    );
}

#[tokio::test]
async fn webhook_tests_and_deliveries_share_headers_without_leaking_values() {
    let (_directory, state) = notification_test_state().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let receiver = tokio::spawn(async move {
        let mut requests = Vec::new();
        for _ in 0..2 {
            let (stream, _) = listener.accept().await.unwrap();
            requests.push(receive_webhook_request(stream).await);
        }
        requests
    });
    let provider = json!({
        "id": "ntfprov_webhook_headers",
        "type": "webhook",
        "connection_config": {
            "url": url,
            "method": "PUT",
            "timeout_seconds": 5,
            "shared_secret": "shared-secret-value",
            "custom_headers": [
                { "name": "Authorization", "value": "Bearer provider-secret" },
                { "name": "X-API-Key", "value": "api-key-secret" },
                { "name": "X-Empty", "value": "" }
            ]
        }
    });
    let translator = Translator::new("en");

    let test_result = send_webhook_test(&state, &provider, &translator)
        .await
        .unwrap();
    assert!(test_result.success);

    let delivery_result = send_webhook_delivery(
        &state,
        &provider,
        &json!({
            "id": "ntftarget_webhook_headers",
            "target_config": {
                "extra_headers_json": { "Authorization": "Bearer ignored-legacy" }
            }
        }),
        &json!({
            "id": "ntfdelivery_webhook_headers",
            "event_id": "event-webhook-headers",
            "message_snapshot": { "title": "Alert", "severity": "warn" }
        }),
        &json!({ "id": "ntftrigger_webhook_headers" }),
        &json!({ "id": "ntfrule_webhook_headers" }),
        5,
        &translator,
    )
    .await;
    assert!(delivery_result.success);

    let requests = receiver.await.unwrap();
    for request in requests {
        assert!(request.starts_with("PUT "));
        assert_eq!(
            request_header_value(&request, "authorization"),
            Some("Bearer provider-secret")
        );
        assert_eq!(
            request_header_value(&request, "x-api-key"),
            Some("api-key-secret")
        );
        assert_eq!(request_header_value(&request, "x-empty"), Some(""));
        assert_eq!(
            request_header_value(&request, "x-fn-knock-signature"),
            Some("shared-secret-value")
        );
        assert_eq!(
            request_header_value(&request, "x-fn-knock-provider"),
            Some("webhook")
        );
        assert_eq!(
            request_header_value(&request, "content-type"),
            Some("application/json")
        );
        assert!(!request.contains("ignored-legacy"));
    }

    for result in [&test_result, &delivery_result] {
        let summary = result.request_summary.as_ref().unwrap();
        let serialized = summary.to_string();
        assert!(serialized.contains("Authorization"));
        assert!(serialized.contains("X-API-Key"));
        assert!(!serialized.contains("provider-secret"));
        assert!(!serialized.contains("api-key-secret"));
        assert!(!serialized.contains("shared-secret-value"));
    }

    let invalid_provider = json!({
        "type": "webhook",
        "connection_config": {
            "url": "http://127.0.0.1:1",
            "custom_headers": [{ "name": "X-Token", "value": "bad\r\nvalue" }]
        }
    });
    let invalid_result = send_webhook_delivery(
        &state,
        &invalid_provider,
        &json!({ "target_config": {} }),
        &json!({ "message_snapshot": {} }),
        &json!({}),
        &json!({}),
        5,
        &translator,
    )
    .await;
    assert!(!invalid_result.success);
    assert!(!invalid_result.retryable);
    assert!(invalid_result.request_summary.is_none());
    assert_eq!(
        invalid_result.message,
        "The value for header X-Token is invalid"
    );
}

#[tokio::test]
async fn webhook_custom_bodies_match_preview_test_and_delivery_without_leaking_content() {
    let (_directory, state) = notification_test_state().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let receiver = tokio::spawn(async move {
        let mut requests = Vec::new();
        for _ in 0..3 {
            let (stream, _) = listener.accept().await.unwrap();
            requests.push(receive_webhook_request(stream).await);
        }
        requests
    });
    let provider = json!({
        "id": "ntfprov_webhook_body",
        "name": "Body webhook",
        "type": "webhook",
        "connection_config": {
            "url": url,
            "method": "POST",
            "timeout_seconds": 5,
            "body_config": {
                "mode": "custom",
                "format": "json",
                "content_type": "application/problem+json",
                "template": "{\"title\":\"{{message.title}}\",\"event\":\"{{event}}\",\"missing\":\"{{event.payload.missing}}\"}"
            }
        }
    });
    let translator = Translator::new("en");

    let provider_options = WebhookTestOptions {
        sample_context: Some(json!({
            "message": { "title": "provider-body-secret" },
            "event": { "id": "evt_provider", "trace_id": "must-not-render", "payload": {} }
        })),
        ..WebhookTestOptions::default()
    };
    let preview = preview_webhook_body(&provider, &translator, provider_options.clone()).unwrap();
    assert_eq!(
        preview.get("content_type"),
        Some(&json!("application/problem+json"))
    );
    assert_eq!(
        preview.get("missing_variables"),
        Some(&json!(["event.payload.missing"]))
    );
    assert!(!preview.to_string().contains("must-not-render"));
    let provider_result =
        send_webhook_test_with_options(&state, &provider, &translator, provider_options)
            .await
            .unwrap();
    assert!(provider_result.success);

    let target_config = Map::from_iter([
        (
            "body_override".to_string(),
            json!({
                "mode": "custom",
                "format": "text",
                "content_type": "text/plain; charset=utf-8",
                "template": r#"{{message.title}}|{{event.payload.ip}}|\{{literal}}|{{legacy.extra_body}}"#
            }),
        ),
        ("extra_body_json".to_string(), json!({ "legacy": true })),
    ]);
    let target_result = send_webhook_test_with_options(
        &state,
        &provider,
        &translator,
        WebhookTestOptions {
            target_config: Some(target_config.clone()),
            sample_context: Some(json!({
                "message": { "title": "target-body-secret" },
                "event": { "trace_id": "must-not-render", "payload": { "ip": "192.0.2.30" } }
            })),
        },
    )
    .await
    .unwrap();
    assert!(target_result.success);

    let delivery_result = send_webhook_delivery(
        &state,
        &provider,
        &json!({
            "id": "ntftarget_webhook_body",
            "provider_id": "ntfprov_webhook_body",
            "target_config": target_config
        }),
        &json!({
            "id": "ntfdelivery_webhook_body",
            "event_id": "evt_delivery_body",
            "message_snapshot": { "title": "delivery-body-secret" },
            "webhook_event_snapshot": {
                "id": "evt_delivery_body",
                "trace_id": "must-not-render",
                "payload": { "ip": "192.0.2.40" }
            }
        }),
        &json!({ "id": "ntftrigger_webhook_body" }),
        &json!({ "id": "ntfrule_webhook_body" }),
        5,
        &translator,
    )
    .await;
    assert!(delivery_result.success);

    let requests = receiver.await.unwrap();
    assert_eq!(
        request_header_value(&requests[0], "content-type"),
        Some("application/problem+json")
    );
    let provider_body: Value = serde_json::from_str(request_body(&requests[0])).unwrap();
    assert_eq!(
        provider_body.get("title"),
        Some(&json!("provider-body-secret"))
    );
    assert!(provider_body.get("event").unwrap().is_object());
    assert_eq!(provider_body.get("missing"), Some(&Value::Null));
    assert!(!request_body(&requests[0]).contains("must-not-render"));

    assert_eq!(
        request_header_value(&requests[1], "content-type"),
        Some("text/plain; charset=utf-8")
    );
    assert_eq!(
        request_body(&requests[1]),
        "target-body-secret|192.0.2.30|{{literal}}|{\"legacy\":true}"
    );
    assert_eq!(
        request_body(&requests[2]),
        "delivery-body-secret|192.0.2.40|{{literal}}|{\"legacy\":true}"
    );
    assert!(!request_body(&requests[2]).contains("must-not-render"));

    for result in [&provider_result, &target_result, &delivery_result] {
        let summary = result.request_summary.as_ref().unwrap().to_string();
        assert!(summary.contains("body_format"));
        assert!(summary.contains("body_bytes"));
        assert!(!summary.contains("body-secret"));
        assert!(!summary.contains("template"));
    }

    let invalid_result = send_webhook_delivery(
        &state,
        &json!({
            "type": "webhook",
            "connection_config": {
                "url": "http://127.0.0.1:1",
                "body_config": { "mode": "custom", "format": "json", "template": "{" }
            }
        }),
        &json!({ "target_config": {} }),
        &json!({ "message_snapshot": {} }),
        &json!({}),
        &json!({}),
        5,
        &translator,
    )
    .await;
    assert!(!invalid_result.success);
    assert!(!invalid_result.retryable);
    assert!(invalid_result.request_summary.is_none());
}

#[tokio::test]
async fn webhook_provider_test_honors_timeout_while_reading_the_response() {
    use tokio::io::AsyncReadExt;

    let (_directory, state) = notification_test_state().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let receiver = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buffer = [0_u8; 4096];
        let _ = stream.read(&mut buffer).await.unwrap();
        time::sleep(Duration::from_millis(1_500)).await;
    });
    let started_at = std::time::Instant::now();
    let result = send_webhook_test(
        &state,
        &json!({
            "type": "webhook",
            "connection_config": {
                "url": url,
                "method": "POST",
                "timeout_seconds": 1,
                "custom_headers": []
            }
        }),
        &Translator::new("en"),
    )
    .await
    .unwrap();
    assert!(!result.success);
    assert!(result.retryable);
    assert!(result.request_summary.is_some());
    assert!(started_at.elapsed() < Duration::from_millis(1_400));
    receiver.abort();
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
fn webhook_target_schema_hides_legacy_extra_body_and_normalizes_body_override() {
    let definition = provider_definition("webhook").unwrap();
    let mut raw = Map::new();
    raw.insert(
        "extra_body_json".to_string(),
        json!({ "preserved_only_by_rule_service": true }),
    );
    raw.insert(
        "body_override".to_string(),
        json!({
            "mode": "custom",
            "format": "text",
            "template": "{{message.title}}"
        }),
    );
    let normalized = normalize_schema_patch(&raw, &definition.target_schema).unwrap();
    assert!(
        !normalized.contains_key("extra_body_json"),
        "legacy body must not be exposed through the catalog schema"
    );
    assert_eq!(
        normalized.get("body_override"),
        Some(&json!({
            "mode": "custom",
            "format": "text",
            "content_type": "text/plain; charset=utf-8",
            "template": "{{message.title}}"
        }))
    );
}

#[test]
fn webhook_target_test_options_keep_hidden_legacy_data() {
    let options = webhook_test_options_from_body(
        &json!({
            "target_config": {
                "extra_headers_json": { "Authorization": "Bearer legacy" },
                "extra_body_json": { "legacy": true },
                "body_override": { "mode": "inherit" }
            }
        }),
        &Translator::new("en"),
    )
    .unwrap();
    let target = options.target_config.unwrap();
    assert_eq!(
        target.get("extra_headers_json"),
        Some(&json!({ "Authorization": "Bearer legacy" }))
    );
    assert_eq!(
        target.get("extra_body_json"),
        Some(&json!({ "legacy": true }))
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
            &Translator::new("en"),
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
            "trace_id": "trc_3f93d40a-89ea-4dbe-a04f-67692778d973",
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
    assert!(message.get("trace_id").is_none());
    assert!(message.pointer("/metadata/trace_id").is_none());
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
    let trace_id = "trc_3f93d40a-89ea-4dbe-a04f-67692778d973";
    let rendered_channels = [
        build_pushplus_text_content(&message),
        build_pushplus_markdown_content(&message),
        build_pushplus_html_content(&message),
        build_pushplus_json_content(&message),
        build_wxpusher_html_content(&message),
        build_wecom_markdown_content(&message, &[]),
        build_wecom_text_content(&message),
        serde_json::to_string(&build_feishu_post_content(&message, &[])).unwrap(),
        build_magicpush_content(&message),
        build_telegram_text(&message),
        build_harmonyosmeow_body(&message),
        build_email_plain_text_body(&message, &zh),
    ];
    for rendered in rendered_channels {
        let visible = rendered.replace('\\', "");
        assert!(
            !visible.contains(trace_id),
            "notification representation leaked the Trace ID: {rendered}"
        );
    }

    let mut oversized_message = message.clone();
    oversized_message["body_text"] = json!("x".repeat(40_000));
    oversized_message["body_markdown"] = json!("x".repeat(40_000));
    let oversized_markdown = build_markdown_body(&oversized_message, "");
    for limit in [2048, 4096, 32 * 1024] {
        let rendered = truncate_utf8_bytes(&oversized_markdown, limit);
        assert!(rendered.len() <= limit);
        assert!(!rendered.contains(trace_id));
    }
    let telegram = build_telegram_text(&oversized_message);
    assert!(telegram.encode_utf16().count() <= 4096);
    assert!(!telegram.contains(trace_id));
    assert!(!serde_json::to_string(&message).unwrap().contains("Matched"));
}

#[test]
fn sanitizes_legacy_notification_snapshots_before_display_or_delivery() {
    let trace_id = "trc_3f93d40a-89ea-4dbe-a04f-67692778d973";
    let record = json!({
        "id": "delivery_legacy",
        "trace_id": trace_id,
        "webhook_event_snapshot": {
            "id": "evt_internal",
            "payload": { "secret": "internal-only" }
        },
        "message_snapshot": {
            "title": "Legacy notification",
            "summary": "A stored notification from before the fix",
            "body_text": "Body",
            "body_markdown": "Body",
            "facts": [
                { "label": "Trace ID", "value": trace_id },
                { "label": "Event type", "value": "Login failure" }
            ],
            "trace_id": trace_id,
            "waf_trace_id": trace_id,
            "metadata": {
                "trace_id": trace_id,
                "waf_trace_id": trace_id,
                "event_type": "FN_EVENT_AUTH_LOGIN_FAILURE"
            }
        }
    });

    let sanitized = sanitize_notification_record(record);
    assert_eq!(sanitized.get("trace_id"), Some(&json!(trace_id)));
    assert!(sanitized.get("webhook_event_snapshot").is_none());
    let message = sanitized.get("message_snapshot").unwrap();
    assert!(message.get("trace_id").is_none());
    assert!(message.get("waf_trace_id").is_none());
    assert!(message.pointer("/metadata/trace_id").is_none());
    assert!(message.pointer("/metadata/waf_trace_id").is_none());
    assert_eq!(
        message.pointer("/metadata/event_type"),
        Some(&json!("FN_EVENT_AUTH_LOGIN_FAILURE"))
    );
    assert_eq!(
        message.get("facts").and_then(Value::as_array).map(Vec::len),
        Some(1)
    );
    for rendered in [
        build_text_body(message),
        build_markdown_body(message, ""),
        build_pushplus_json_content(message),
        build_telegram_text(message),
    ] {
        assert!(!rendered.contains(trace_id));
    }
}

#[test]
fn app_update_notification_keeps_release_notes_and_trace_internal() {
    let trace_id = "trc_3f93d40a-89ea-4dbe-a04f-67692778d973";
    let translator = Translator::new("zh-CN");
    let message = build_notification_message(
        &json!({
            "id": "evt_update_1",
            "trace_id": trace_id,
            "type": "FN_EVENT_SYSTEM_APP_UPDATE_AVAILABLE",
            "source": "SERVER_ADMIN",
            "level": "INFO",
            "happened_at": "2026-08-29T00:00:00.000Z",
            "payload": {
                "local_version": "2.4.0",
                "latest_version": "2.4.1",
                "force_update": false,
                "release_notes": "修复通知与认证桥接问题",
                "check_reason": "scheduled"
            }
        }),
        &json!({
            "id": "rule_update",
            "window_seconds": 60,
            "threshold_count": 1
        }),
        1,
        "global",
        &translator,
    );

    assert!(message.get("trace_id").is_none());
    assert!(message.pointer("/metadata/trace_id").is_none());
    assert!(!serde_json::to_string(&message).unwrap().contains(trace_id));
    assert!(
        message
            .get("body_text")
            .and_then(Value::as_str)
            .is_some_and(|value| value.contains("修复通知与认证桥接问题"))
    );
    assert!(
        message
            .get("body_markdown")
            .and_then(Value::as_str)
            .is_some_and(|value| value.contains("修复通知与认证桥接问题"))
    );

    let rendered_channels = [
        build_text_body(&message),
        build_markdown_body(&message, ""),
        build_pushplus_text_content(&message),
        build_pushplus_markdown_content(&message),
        build_pushplus_html_content(&message),
        build_pushplus_json_content(&message),
        build_wxpusher_html_content(&message),
        build_wecom_markdown_content(&message, &[]),
        build_wecom_text_content(&message),
        serde_json::to_string(&build_feishu_post_content(&message, &[])).unwrap(),
        build_magicpush_content(&message),
        build_telegram_text(&message),
        build_harmonyosmeow_body(&message),
        build_email_plain_text_body(&message, &translator),
        build_bark_payload(&message, &json!({ "target_config": {} }))["body"]
            .as_str()
            .unwrap()
            .to_string(),
    ];
    for rendered in rendered_channels {
        assert!(
            !rendered.replace('\\', "").contains(trace_id),
            "app update notification leaked the Trace ID: {rendered}"
        );
        assert!(
            rendered.contains("修复通知与认证桥接问题"),
            "app update notification omitted release notes: {rendered}"
        );
    }
}

#[test]
fn terminal_audit_notifications_are_localized_and_descriptive() {
    let translator = Translator::new("zh-CN");
    let event = json!({
        "id": "evt_terminal_1",
        "type": "FN_EVENT_TERMINAL_AUDIT",
        "source": "SERVER_ADMIN",
        "level": "WARN",
        "happened_at": "2026-08-29T00:00:00.000Z",
        "payload": {
            "action": "session_creation_failed",
            "target_id": "target-1",
            "session_id": "session-1",
            "error_code": "connect_timeout"
        }
    });
    let rule = json!({ "id": "rule_terminal", "window_seconds": 60 });
    let message = build_notification_message(&event, &rule, 1, "session-1", &translator);

    assert_eq!(
        notification_event_label_key("FN_EVENT_TERMINAL_AUDIT"),
        Some("events.terminalAudit")
    );
    assert_eq!(message.get("title"), Some(&json!("敲门 Knock 终端审计")));
    assert!(
        message
            .get("summary")
            .and_then(Value::as_str)
            .is_some_and(|value| value.contains("终端会话创建失败"))
    );
    let facts = message.get("facts").and_then(Value::as_array).unwrap();
    assert!(facts.iter().any(|fact| {
        fact.get("label") == Some(&json!("终端操作"))
            && fact.get("value") == Some(&json!("终端会话创建失败"))
    }));
    assert!(facts.iter().any(|fact| {
        fact.get("label") == Some(&json!("SSH 会话"))
            && fact.get("value") == Some(&json!("session-1"))
    }));
    assert!(!build_text_body(&message).contains("FN_EVENT_TERMINAL_AUDIT"));
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
                "trace_id": "trc_3f93d40a-89ea-4dbe-a04f-67692778d973",
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
    assert!(
        !serde_json::to_string(&waf_details.facts)
            .unwrap()
            .contains("trc_3f93d40a-89ea-4dbe-a04f-67692778d973")
    );
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
