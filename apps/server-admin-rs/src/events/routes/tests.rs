use super::*;

#[test]
fn maps_event_rules_and_default_levels() {
    assert_eq!(
        event_rule_key("FN_EVENT_GATEWAY_THROTTLE_BLOCKED"),
        Some("gateway_throttle_block")
    );
    assert_eq!(
        event_rule_key("FN_EVENT_GATEWAY_VISIBILITY_BLOCKED"),
        Some("gateway_visibility_block")
    );
    assert_eq!(default_event_level("FN_EVENT_AUTH_LOGIN_SUCCESS"), "INFO");
    assert_eq!(default_event_level("FN_EVENT_WAF_BLOCKED"), "WARN");
    assert_eq!(
        default_event_level("FN_EVENT_GATEWAY_VISIBILITY_BLOCKED"),
        "WARN"
    );
    assert_eq!(
        default_event_level("FN_EVENT_TUNNEL_FRP_DISCONNECTED"),
        "WARN"
    );
}

#[test]
fn internal_header_guard_matches_node_truthiness() {
    let mut headers = HeaderMap::new();
    assert!(!has_forbidden_internal_event_headers(&headers));

    headers.insert("x-forwarded-for", axum::http::HeaderValue::from_static(""));
    assert!(!has_forbidden_internal_event_headers(&headers));

    headers.insert("x-forwarded-for", axum::http::HeaderValue::from_static(" "));
    assert!(has_forbidden_internal_event_headers(&headers));

    headers.clear();
    headers.insert("origin", axum::http::HeaderValue::from_static("   "));
    assert!(!has_forbidden_internal_event_headers(&headers));

    headers.insert(
        "origin",
        axum::http::HeaderValue::from_static("https://example.com"),
    );
    assert!(has_forbidden_internal_event_headers(&headers));
}

#[test]
fn validates_subject_kind() {
    assert!(normalize_subject(Some(json!({ "kind": "IP", "id": "1.2.3.4" }))).is_ok());
    assert!(normalize_subject(Some(json!({ "kind": "NOPE", "id": "x" }))).is_err());
    assert!(normalize_subject(Some(json!({ "kind": " IP ", "id": "x" }))).is_err());
    assert_eq!(
        normalize_subject(Some(json!({ "kind": "IP", "id": " 1.2.3.4 " }))).unwrap(),
        Some(json!({ "kind": "IP", "id": " 1.2.3.4 " }))
    );
}

#[test]
fn event_list_page_parser_matches_node_parse_int_edges() {
    assert_eq!(parse_positive_int(None, 1), 1);
    assert_eq!(parse_positive_int(Some("2x"), 1), 2);
    assert_eq!(parse_positive_int(Some("  +3.9"), 1), 3);
    assert_eq!(parse_positive_int(Some("-1"), 1), 1);
    assert_eq!(parse_positive_int(Some(""), 20), 20);
}

#[test]
fn system_event_ip_field_mapping_matches_node_hydration() {
    assert_eq!(
        system_event_ip_fields(Some("FN_EVENT_AUTH_SESSION_IP_DRIFT")),
        &[("from_ip", "from_ip_location"), ("to_ip", "to_ip_location")]
    );
    assert_eq!(
        system_event_ip_fields(Some("FN_EVENT_WAF_BLOCKED")),
        &[("ip", "ip_location")]
    );
    assert_eq!(
        system_event_ip_fields(Some("FN_EVENT_GATEWAY_VISIBILITY_BLOCKED")),
        &[("ip", "ip_location")]
    );
    assert!(system_event_ip_fields(Some("FN_EVENT_DDNS_UPDATE_COMPLETED")).is_empty());
}

#[test]
fn builds_event_envelope_with_node_manager_nullish_semantics() {
    let body = InternalSystemEventBody {
        trace_id: Some("client-forged".to_string()),
        event_type: "FN_EVENT_TUNNEL_CLOUDFLARED_DISCONNECTED".to_string(),
        source: "SERVER_ADMIN".to_string(),
        level: Some(String::new()),
        happened_at: Some("   ".to_string()),
        dedupe_key: None,
        dedupe_ttl_seconds: None,
        subject: None,
        tags: Some(vec![String::new(), " tag ".to_string()]),
        payload: json!({ "message": "edge" }),
    };

    let event = build_event_envelope(body, None, Some("  dedupe  ".to_string()));

    assert_eq!(event.get("level"), Some(&json!("")));
    assert_eq!(event.get("happened_at"), Some(&json!("   ")));
    assert_eq!(event.get("dedupe_key"), Some(&json!("  dedupe  ")));
    assert_eq!(event.get("tags"), Some(&json!(["", " tag "])));
    assert!(event.get("trace_id").is_none());
}

#[test]
fn applies_internal_route_truthiness_before_manager_publish() {
    let mut body = InternalSystemEventBody {
        trace_id: Some("trc_3f93d40a-89ea-4dbe-a04f-67692778d973".to_string()),
        event_type: "FN_EVENT_TUNNEL_CLOUDFLARED_DISCONNECTED".to_string(),
        source: "SERVER_ADMIN".to_string(),
        level: Some(String::new()),
        happened_at: Some(String::new()),
        dedupe_key: Some(String::new()),
        dedupe_ttl_seconds: Some(60.0),
        subject: None,
        tags: Some(vec![String::new()]),
        payload: json!({}),
    };

    apply_internal_event_route_truthiness(&mut body);
    let event = build_event_envelope(body, None, None);
    assert_eq!(
        event.get("trace_id"),
        Some(&json!("trc_3f93d40a-89ea-4dbe-a04f-67692778d973"))
    );

    assert_eq!(event.get("level"), Some(&json!("WARN")));
    assert_ne!(event.get("happened_at"), Some(&json!("")));
    assert!(event.get("dedupe_key").is_none());
    assert_eq!(event.get("tags"), Some(&json!([""])));
}

#[test]
fn standalone_events_do_not_invent_trace_ids() {
    let body = app_update_available_body("2.4.0", "2.4.1", false, "Release notes", "scheduled");
    let event = build_event_envelope(body, None, None);

    assert!(event.get("trace_id").is_none());
    assert_eq!(
        event.get("type"),
        Some(&json!("FN_EVENT_SYSTEM_APP_UPDATE_AVAILABLE"))
    );
}

#[test]
fn honors_event_rule_defaults() {
    let config = EventSystemConfig {
        enabled: true,
        retention_days: 30,
        max_records: 10_000,
        rules: Map::new(),
    };
    assert!(is_event_type_enabled(
        &config,
        "FN_EVENT_GATEWAY_THROTTLE_BLOCKED"
    ));
    assert!(is_event_type_enabled(
        &config,
        "FN_EVENT_GATEWAY_VISIBILITY_BLOCKED"
    ));
    assert!(is_event_type_enabled(&config, "FN_EVENT_AUTH_LOGOUT"));
}

#[test]
fn builds_app_update_available_event_like_node() {
    let body = app_update_available_body("1.8.6", "1.9.0", true, "Release notes", "startup");

    assert_eq!(body.event_type, "FN_EVENT_SYSTEM_APP_UPDATE_AVAILABLE");
    assert_eq!(body.source, "SERVER_ADMIN");
    assert_eq!(body.level.as_deref(), Some("INFO"));
    assert_eq!(body.dedupe_key.as_deref(), Some("system:app-update:1.9.0"));
    assert_eq!(
        body.dedupe_ttl_seconds,
        Some(APP_UPDATE_EVENT_DEDUPE_TTL_SECONDS as f64)
    );
    assert_eq!(
        body.subject,
        Some(json!({ "kind": "APPLICATION", "id": "fn-knock" }))
    );
    assert_eq!(body.payload.get("local_version"), Some(&json!("1.8.6")));
    assert_eq!(body.payload.get("latest_version"), Some(&json!("1.9.0")));
    assert_eq!(body.payload.get("force_update"), Some(&json!(true)));
    assert_eq!(
        body.payload.get("release_notes"),
        Some(&json!("Release notes"))
    );
    assert_eq!(body.payload.get("check_reason"), Some(&json!("startup")));

    let body = app_update_available_body("1.8.6", "1.9.0", false, "  ", "");
    assert!(body.payload.get("release_notes").is_none());
    assert!(body.payload.get("check_reason").is_none());
}

#[test]
fn app_update_event_includes_only_the_latest_release_notes_section() {
    let release_notes = r#"
[用户协议与隐私政策](https://www.fnknock.cn/legal)

# fn-knock 2.1.3

- 修复协议映射回环配置
- 停用后仍可管理已有配置

---

# fn-knock 2.1.2

- 历史版本内容不应进入事件推送
"#;

    let body = app_update_available_body("2.1.2", "2.1.3", false, release_notes, "scheduled");
    assert_eq!(
        body.payload.get("release_notes"),
        Some(&json!(
            "# fn-knock 2.1.3\n\n- 修复协议映射回环配置\n- 停用后仍可管理已有配置"
        ))
    );

    let plain_notes =
        app_update_available_body("2.1.2", "2.1.3", false, "普通更新说明", "scheduled");
    assert_eq!(
        plain_notes.payload.get("release_notes"),
        Some(&json!("普通更新说明"))
    );
}

#[test]
fn dedupe_ttl_seconds_matches_node_number_ceiling() {
    assert_eq!(normalize_dedupe_ttl_seconds(Some(1.2)), 2);
    assert_eq!(normalize_dedupe_ttl_seconds(Some(1.0)), 1);
    assert_eq!(normalize_dedupe_ttl_seconds(Some(0.0)), 0);
    assert_eq!(normalize_dedupe_ttl_seconds(Some(f64::NAN)), 0);
    assert_eq!(normalize_dedupe_ttl_seconds(None), 0);
}

#[test]
fn gateway_visibility_event_enforces_global_minute_dedupe() {
    let body = InternalSystemEventBody {
        trace_id: None,
        event_type: "FN_EVENT_GATEWAY_VISIBILITY_BLOCKED".to_string(),
        source: "GO_REAUTH_PROXY".to_string(),
        level: Some("WARN".to_string()),
        happened_at: None,
        dedupe_key: Some("producer-specific-key".to_string()),
        dedupe_ttl_seconds: Some(5.0),
        subject: Some(json!({ "kind": "IP", "id": "203.0.113.8" })),
        tags: Some(vec![
            "gateway".to_string(),
            "visibility".to_string(),
            "security".to_string(),
        ]),
        payload: json!({ "ip": "203.0.113.8", "status": 499 }),
    };

    assert_eq!(
        resolve_system_event_dedupe(&body),
        (
            Some(GATEWAY_VISIBILITY_EVENT_DEDUPE_KEY.to_string()),
            GATEWAY_VISIBILITY_EVENT_DEDUPE_TTL_SECONDS
        )
    );
}

#[test]
fn builds_auth_login_failure_event_like_node() {
    let body = auth_login_failure_body(json!({
        "ip": "203.0.113.10",
        "attempts": 3,
        "retry_after_seconds": 8,
        "blocked_until": "2026-01-01T00:00:00Z",
        "method": "PASSKEY",
        "credential_name": "MacBook",
        "linked_totp_name": Value::Null,
        "user_agent": "Browser",
    }));

    assert_eq!(body.event_type, "FN_EVENT_AUTH_LOGIN_FAILURE");
    assert_eq!(body.source, "SERVER_ADMIN");
    assert_eq!(body.level.as_deref(), Some("WARN"));
    assert_eq!(
        body.subject,
        Some(json!({ "kind": "IP", "id": "203.0.113.10" }))
    );
    assert_eq!(body.payload.get("attempts"), Some(&json!(3)));
    assert_eq!(body.payload.get("retry_after_seconds"), Some(&json!(8)));
    assert_eq!(body.payload.get("method"), Some(&json!("PASSKEY")));
    assert_eq!(body.payload.get("credential_name"), Some(&json!("MacBook")));
    assert!(body.payload.get("linked_totp_name").is_none());

    let oidc_body = auth_login_failure_body(json!({
        "ip": "203.0.113.11",
        "attempts": 1,
        "method": "OIDC",
        "provider_id": "oidc_provider_123",
        "auth_provider_name": "QQ",
        "credential_name": "QQ",
    }));
    assert_eq!(
        oidc_body.payload.get("provider_id"),
        Some(&json!("oidc_provider_123"))
    );
    assert_eq!(
        oidc_body.payload.get("auth_provider_name"),
        Some(&json!("QQ"))
    );
    assert_eq!(oidc_body.payload.get("credential_name"), Some(&json!("QQ")));
}

#[test]
fn resolves_oidc_provider_id_from_current_and_legacy_failure_events() {
    assert_eq!(
        oidc_failure_provider_id(&json!({
            "type": "FN_EVENT_AUTH_LOGIN_FAILURE",
            "payload": {
                "method": "OIDC",
                "provider_id": "oidc_provider_current",
                "credential_name": "QQ"
            }
        })),
        Some("oidc_provider_current".to_string())
    );
    assert_eq!(
        oidc_failure_provider_id(&json!({
            "type": "FN_EVENT_AUTH_LOGIN_FAILURE",
            "payload": {
                "method": "OIDC",
                "credential_name": "oidc_provider_legacy"
            }
        })),
        Some("oidc_provider_legacy".to_string())
    );
    assert_eq!(
        oidc_failure_provider_id(&json!({
            "type": "FN_EVENT_AUTH_LOGIN_FAILURE",
            "payload": { "method": "PASSKEY", "credential_name": "oidc_provider_nope" }
        })),
        None
    );
}

#[test]
fn builds_auth_login_success_event_like_node_optional_payload_shape() {
    let body = auth_login_success_body(json!({
        "session_id": " session-1 ",
        "auth_method": "TOTP",
        "auth_provider_name": "",
        "credential_id": "cred-1",
        "credential_name": "Token",
        "linked_totp_name": Value::Null,
        "session_comment": "",
        "grant_type": "browser_session",
        "post_login_ip_grant_mode": Value::Null,
        "whitelist_record_id": Value::Null,
        "ip": "203.0.113.10",
        "ip_location": "",
        "user_agent": "Browser",
        "remember_me": false,
        "expires_at": "2026-01-01T00:00:00Z",
    }));

    assert_eq!(body.event_type, "FN_EVENT_AUTH_LOGIN_SUCCESS");
    assert_eq!(
        body.subject,
        Some(json!({ "kind": "SESSION", "id": "session-1" }))
    );
    assert!(body.payload.get("auth_provider_name").is_none());
    assert!(body.payload.get("linked_totp_name").is_none());
    assert!(body.payload.get("session_comment").is_none());
    assert!(body.payload.get("ip_location").is_none());
    assert_eq!(
        body.payload.get("post_login_ip_grant_mode"),
        Some(&Value::Null)
    );
    assert_eq!(body.payload.get("whitelist_record_id"), Some(&Value::Null));
    assert_eq!(body.payload.get("remember_me"), Some(&json!(false)));
}

#[test]
fn builds_auth_logout_and_drift_events_like_node_optional_payload_shape() {
    let logout = auth_logout_body(json!({
        "session_id": "session-1",
        "auth_method": "TOTP",
        "credential_id": "cred-1",
        "credential_name": "Token",
        "linked_totp_name": "",
        "session_comment": Value::Null,
        "ip": "203.0.113.10",
        "ip_location": "",
        "user_agent": "Browser",
        "login_time": "",
        "logout_source": "admin_session_delete",
    }));
    assert!(logout.payload.get("linked_totp_name").is_none());
    assert!(logout.payload.get("session_comment").is_none());
    assert!(logout.payload.get("ip_location").is_none());
    assert!(logout.payload.get("login_time").is_none());

    let drift = auth_session_ip_drift_body(json!({
        "session_id": "session-1",
        "auth_method": "TOTP",
        "credential_id": "cred-1",
        "credential_name": "Token",
        "linked_totp_name": "",
        "session_comment": "",
        "drift_source": "proxy-session",
        "from_ip": "203.0.113.10",
        "from_ip_location": "",
        "to_ip": "203.0.113.11",
        "to_ip_location": Value::Null,
        "login_time": "",
    }));
    assert!(drift.payload.get("linked_totp_name").is_none());
    assert!(drift.payload.get("session_comment").is_none());
    assert!(drift.payload.get("from_ip_location").is_none());
    assert!(drift.payload.get("to_ip_location").is_none());
    assert!(drift.payload.get("login_time").is_none());
}

#[test]
fn builds_waf_blocked_event_like_node_helper_truthiness() {
    let body = waf_blocked_body(&json!({
        "trace_id": " trace-1 ",
        "client_ip": "",
        "remote_addr": " 203.0.113.9 ",
        "mode": "block",
        "action": "",
        "status": 0,
        "host": "   ",
        "path": "",
        "rule_ids": [1001, "skip"]
    }))
    .unwrap();

    assert_eq!(body.event_type, "FN_EVENT_WAF_BLOCKED");
    assert_eq!(body.level.as_deref(), Some("WARN"));
    assert_eq!(body.happened_at, None);
    assert_eq!(body.dedupe_key.as_deref(), Some("waf: trace-1 "));
    assert_eq!(
        body.subject,
        Some(json!({ "kind": "IP", "id": " 203.0.113.9 " }))
    );
    assert_eq!(body.payload.get("ip"), Some(&json!(" 203.0.113.9 ")));
    assert_eq!(body.payload.get("trace_id"), Some(&json!(" trace-1 ")));
    assert_eq!(body.payload.get("action"), Some(&json!("deny")));
    assert_eq!(body.payload.get("host"), Some(&json!("   ")));
    assert!(body.payload.get("path").is_none());
    assert!(body.payload.get("status").is_none());
    assert!(body.payload.get("blocked_at").is_none());
    assert_eq!(body.payload.get("rule_ids"), Some(&json!([1001])));
}

#[test]
fn builds_tunnel_connectivity_event_like_node() {
    let body = tunnel_connectivity_body(TunnelConnectivityEvent {
        tunnel: "frp",
        connected: false,
        pid: Some(1234),
        message: Some("Primary: session shutdown"),
        instance_id: Some("primary"),
        instance_name: Some("Primary"),
        is_primary: Some(true),
        happened_at: Some("2026-08-06T12:34:56Z"),
    });

    assert_eq!(body.event_type, "FN_EVENT_TUNNEL_FRP_DISCONNECTED");
    assert_eq!(body.source, "SERVER_ADMIN");
    assert_eq!(body.level.as_deref(), Some("ERROR"));
    assert_eq!(
        body.subject,
        Some(json!({ "kind": "TUNNEL", "id": "frp:primary" }))
    );
    assert_eq!(body.payload.get("tunnel"), Some(&json!("frp")));
    assert_eq!(body.payload.get("status"), Some(&json!("disconnected")));
    assert_eq!(body.payload.get("pid"), Some(&json!(1234)));
    assert_eq!(body.payload.get("instance_id"), Some(&json!("primary")));
    assert_eq!(body.payload.get("instance_name"), Some(&json!("Primary")));
    assert_eq!(body.payload.get("is_primary"), Some(&json!(true)));
    assert_eq!(body.happened_at.as_deref(), Some("2026-08-06T12:34:56Z"));
    assert_eq!(
        body.payload.get("message"),
        Some(&json!("Primary: session shutdown"))
    );

    let body = tunnel_connectivity_body(TunnelConnectivityEvent {
        tunnel: "cloudflared",
        connected: true,
        pid: None,
        message: None,
        instance_id: None,
        instance_name: None,
        is_primary: None,
        happened_at: None,
    });
    assert_eq!(body.event_type, "FN_EVENT_TUNNEL_CLOUDFLARED_CONNECTED");
    assert_eq!(body.level.as_deref(), Some("INFO"));
    assert_eq!(
        body.subject,
        Some(json!({ "kind": "TUNNEL", "id": "cloudflared" }))
    );
    assert!(body.happened_at.is_none());
    assert!(body.payload.get("pid").is_none());
}

#[test]
fn localizes_system_event_route_text() {
    let zh = Translator::new("zh-CN");
    assert_eq!(
        system_event_route_text(&zh, "unsupportedEventLevel"),
        "不支持的事件级别"
    );
    assert_eq!(
        system_event_route_text(&zh, "clearEventsFailed"),
        "清空系统事件失败"
    );
}
