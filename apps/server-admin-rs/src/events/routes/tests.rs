use super::*;

#[test]
fn maps_event_rules_and_default_levels() {
    assert_eq!(
        event_rule_key("FN_EVENT_GATEWAY_THROTTLE_BLOCKED"),
        Some("gateway_throttle_block")
    );
    assert_eq!(default_event_level("FN_EVENT_AUTH_LOGIN_SUCCESS"), "INFO");
    assert_eq!(default_event_level("FN_EVENT_WAF_BLOCKED"), "WARN");
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
    assert!(system_event_ip_fields(Some("FN_EVENT_DDNS_UPDATE_COMPLETED")).is_empty());
}

#[test]
fn builds_event_envelope_with_node_manager_nullish_semantics() {
    let body = InternalSystemEventBody {
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
}

#[test]
fn applies_internal_route_truthiness_before_manager_publish() {
    let mut body = InternalSystemEventBody {
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

    assert_eq!(event.get("level"), Some(&json!("WARN")));
    assert_ne!(event.get("happened_at"), Some(&json!("")));
    assert!(event.get("dedupe_key").is_none());
    assert_eq!(event.get("tags"), Some(&json!([""])));
}

#[test]
fn honors_event_rule_defaults() {
    let config = EventSystemConfig {
        enabled: true,
        retention_days: 30,
        rules: Map::new(),
    };
    assert!(is_event_type_enabled(
        &config,
        "FN_EVENT_GATEWAY_THROTTLE_BLOCKED"
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
fn dedupe_ttl_seconds_matches_node_number_ceiling() {
    assert_eq!(normalize_dedupe_ttl_seconds(Some(1.2)), 2);
    assert_eq!(normalize_dedupe_ttl_seconds(Some(1.0)), 1);
    assert_eq!(normalize_dedupe_ttl_seconds(Some(0.0)), 0);
    assert_eq!(normalize_dedupe_ttl_seconds(Some(f64::NAN)), 0);
    assert_eq!(normalize_dedupe_ttl_seconds(None), 0);
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
    let body = tunnel_connectivity_body(
        "frp",
        false,
        Some(1234),
        Some("Primary: session shutdown"),
        Some("primary"),
        Some("Primary"),
        Some(true),
    );

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
    assert_eq!(
        body.payload.get("message"),
        Some(&json!("Primary: session shutdown"))
    );

    let body = tunnel_connectivity_body("cloudflared", true, None, None, None, None, None);
    assert_eq!(body.event_type, "FN_EVENT_TUNNEL_CLOUDFLARED_CONNECTED");
    assert_eq!(body.level.as_deref(), Some("INFO"));
    assert_eq!(
        body.subject,
        Some(json!({ "kind": "TUNNEL", "id": "cloudflared" }))
    );
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
