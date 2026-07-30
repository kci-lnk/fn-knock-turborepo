use super::*;

#[test]
fn visibility_selection_deduplication_includes_operator() {
    let selections = dedupe_visibility_selection_inputs(Some(&json!([
        { "province": "浙江", "query_city": "杭州", "operator": "移动" },
        { "province": "浙江", "query_city": "杭州", "operator": "移动" },
        { "province": "浙江", "query_city": "杭州", "operator": "电信" },
        { "province": "浙江", "query_city": "杭州" }
    ])))
    .unwrap();
    assert_eq!(selections.len(), 3);
    assert_eq!(selections[0].operator, Some(CidrOperator::Mobile));
    assert_eq!(selections[1].operator, Some(CidrOperator::Telecom));
    assert_eq!(selections[2].operator, None);
}

#[test]
fn visibility_selection_rejects_non_string_operator() {
    assert!(
        dedupe_visibility_selection_inputs(Some(&json!([
            { "province": "浙江", "query_city": "杭州", "operator": 123 }
        ])))
        .is_err()
    );
}

async fn gateway_settings_test_state() -> (tempfile::TempDir, AppState) {
    let directory = tempfile::tempdir().unwrap();
    let mut settings = {
        let _environment = crate::test_support::EnvGuard::new(&[]);
        crate::settings::Settings::from_env()
    };
    settings.runtime_target = "linux".to_string();
    settings.data_dir = directory.path().join("data");
    settings.gateway_config_dir = directory.path().join("gateway");
    settings.sqlite_path = directory.path().join("fn-knock.sqlite3");
    settings.legacy_redis_url = String::new();
    settings.go_backend_grpc_addr = "http://127.0.0.1:1".to_string();
    settings.internal_rpc_token = "test-internal-rpc-token".to_string();
    settings.request_timeout = std::time::Duration::from_millis(100);
    let state = AppState::new(settings).await.unwrap();
    (directory, state)
}

#[tokio::test]
async fn gateway_visibility_allows_enabled_empty_global_rules() {
    let (_directory, state) = gateway_settings_test_state().await;
    let input = json!({
        "enabled": true,
        "selections": [],
        "custom_cidrs": [],
    });

    let compiled = compile_gateway_visibility_config(&state, input.as_object().unwrap())
        .await
        .unwrap();

    assert_eq!(compiled.config["enabled"], Value::Bool(true));
    assert_eq!(compiled.config["selections"], json!([]));
    assert_eq!(compiled.config["custom_cidrs"], json!([]));
    assert_eq!(compiled.runtime["enabled"], Value::Bool(true));
    assert!(compiled.runtime.get("cidrs").is_none());
    assert!(
        compiled.runtime["policy_id"]
            .as_str()
            .unwrap()
            .starts_with("ipset-v1:")
    );
    assert_eq!(compiled.runtime["source_cidr_count"], json!(0));
    assert_eq!(compiled.runtime["range_count"], json!(0));
}

#[test]
fn gateway_response_uses_node_defaults() {
    let config = json!({
        "subdomain_mode": {},
        "host_mappings": [
            { "host": "app.example.com", "target": "http://127.0.0.1:8080", "title": "App" },
            { "host": "auth.example.com", "target": "http://127.0.0.1:7997", "service_role": "auth" }
        ],
        "gateway_proxy_headers": { "disabled_hosts": ["app.example.com"] }
    });
    let host_mappings = config
        .get("host_mappings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap();
    let visible = visible_host_mappings(&host_mappings);
    let proxy_items =
        build_gateway_proxy_header_items(&visible, config.get("gateway_proxy_headers").unwrap());
    assert_eq!(
        normalize_reverse_proxy_throttle(&json!({})),
        default_reverse_proxy_throttle()
    );
    assert_eq!(
        normalize_gateway_unmatched_route(&json!({})),
        json!({
            "behavior": "error_page",
            "upstream_error_detail": "less"
        })
    );
    assert_eq!(
        normalize_gateway_unmatched_route(&json!({
            "upstream_error_detail": "reset_connection"
        }))
        .pointer("/upstream_error_detail"),
        Some(&json!("reset_connection"))
    );
    assert_eq!(
        normalize_gateway_portal(&json!({})),
        json!({
            "enabled": true,
            "display_style": "title",
            "show_app_icon": true,
            "icon_drag_mode": "corners",
            "version": "v1"
        })
    );
    assert_eq!(
        normalize_gateway_portal(&json!({ "version": "v2" }))["version"],
        json!("v2")
    );
    assert_eq!(
        normalize_gateway_portal(&json!({ "version": "future" }))["version"],
        json!("v1")
    );
    assert_eq!(visible.len(), 1);
    assert_eq!(
        proxy_items[0]
            .get("send_proxy_headers")
            .and_then(Value::as_bool),
        Some(false)
    );
}

#[tokio::test]
async fn gateway_response_includes_default_unmatched_route_behavior() {
    let (_directory, state) = gateway_settings_test_state().await;
    let response = build_gateway_settings_response(&state).await.unwrap();
    assert_eq!(
        response.pointer("/unmatched_route/behavior"),
        Some(&json!("error_page"))
    );
    assert_eq!(
        response.pointer("/unmatched_route/upstream_error_detail"),
        Some(&json!("less"))
    );
    assert_eq!(response.pointer("/portal/version"), Some(&json!("v1")));
}

#[test]
fn gateway_patch_merges_and_normalizes_sections() {
    let mut config = json!({
        "subdomain_mode": { "auth_cache_ttl_seconds": 1 },
        "reverse_proxy_throttle": { "enabled": true, "requests_per_second": 10, "burst": 20, "block_seconds": 30 },
        "gateway_portal": { "enabled": true, "display_style": "title", "show_app_icon": true, "icon_drag_mode": "corners", "version": "v1" }
    });
    let patch = json!({
        "auth_cache_ttl_seconds": 8,
        "reverse_proxy_throttle": { "burst": 250 },
        "portal": { "display_style": "domain", "show_app_icon": false, "version": "v2" },
        "unmatched_route": {
            "behavior": "reset_connection",
            "upstream_error_detail": "reset_connection"
        },
        "crawler_blocker": { "enabled": true }
    });
    apply_gateway_patch(&mut config, patch.as_object().unwrap());
    assert_eq!(
        config.pointer("/subdomain_mode/auth_cache_ttl_seconds"),
        Some(&Value::Number(8.into()))
    );
    assert_eq!(
        config.pointer("/reverse_proxy_throttle/requests_per_second"),
        Some(&Value::Number(10.into()))
    );
    assert_eq!(
        config.pointer("/reverse_proxy_throttle/burst"),
        Some(&Value::Number(250.into()))
    );
    assert_eq!(
        config
            .pointer("/gateway_portal/display_style")
            .and_then(Value::as_str),
        Some("domain")
    );
    assert_eq!(
        config
            .pointer("/gateway_portal/version")
            .and_then(Value::as_str),
        Some("v2")
    );
    assert_eq!(
        config
            .pointer("/gateway_unmatched_route/behavior")
            .and_then(Value::as_str),
        Some("reset_connection")
    );
    assert_eq!(
        config
            .pointer("/gateway_unmatched_route/upstream_error_detail")
            .and_then(Value::as_str),
        Some("reset_connection")
    );
    assert_eq!(
        config
            .pointer("/gateway_crawler_blocker/enabled")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert!(
        config
            .pointer("/gateway_crawler_blocker/updated_at")
            .and_then(Value::as_str)
            .is_some()
    );
}

#[test]
fn gateway_portal_runtime_echo_accepts_disabled_config() {
    let portal = json!({
        "enabled": false,
        "display_style": "title",
        "show_app_icon": true,
        "icon_drag_mode": "free",
        "version": "v2",
    });
    let response = json!({
        "success": true,
        "data": portal.clone(),
    });

    assert!(super::runtime::ensure_gateway_portal_applied(&portal, response).is_ok());
}

#[test]
fn gateway_portal_runtime_echo_rejects_reenabled_config() {
    let portal = json!({
        "enabled": false,
        "display_style": "title",
        "show_app_icon": true,
        "icon_drag_mode": "free",
    });
    let response = json!({
        "success": true,
        "data": {
            "enabled": true,
            "display_style": "title",
            "show_app_icon": true,
            "icon_drag_mode": "free",
            "version": "v2",
        },
    });

    let error = super::runtime::ensure_gateway_portal_applied(&portal, response).unwrap_err();
    assert!(error.contains("did not apply gateway portal config"));
    assert!(error.contains(r#""enabled":false"#));
    assert!(error.contains(r#""enabled":true"#));
}

#[test]
fn gateway_portal_runtime_echo_accepts_legacy_backend_for_v1() {
    let portal = json!({
        "enabled": true,
        "display_style": "title",
        "show_app_icon": true,
        "icon_drag_mode": "corners",
        "version": "v1",
    });
    let response = json!({
        "success": true,
        "data": {
            "enabled": true,
            "display_style": "title",
            "show_app_icon": true,
            "icon_drag_mode": "corners",
        },
    });

    assert!(super::runtime::ensure_gateway_portal_applied(&portal, response).is_ok());
}

#[test]
fn gateway_portal_runtime_echo_rejects_legacy_backend_for_v2() {
    let portal = json!({
        "enabled": true,
        "display_style": "title",
        "show_app_icon": true,
        "icon_drag_mode": "corners",
        "version": "v2",
    });
    let response = json!({
        "success": true,
        "data": {
            "enabled": true,
            "display_style": "title",
            "show_app_icon": true,
            "icon_drag_mode": "corners",
        },
    });

    let error = super::runtime::ensure_gateway_portal_applied(&portal, response).unwrap_err();
    assert!(error.contains("upgrade the gateway backend"));
    assert!(error.contains(r#""version":"v2""#));
    assert!(error.contains(r#""version":"v1""#));
}

#[test]
fn gateway_unmatched_route_runtime_echo_accepts_applied_reset() {
    let requested = json!({
        "behavior": "error_page",
        "upstream_error_detail": "reset_connection",
    });
    let response = json!({
        "success": true,
        "data": requested.clone(),
    });

    assert!(super::runtime::ensure_gateway_unmatched_route_applied(&requested, response).is_ok());
}

#[test]
fn gateway_unmatched_route_runtime_echo_rejects_legacy_fallback() {
    let requested = json!({
        "behavior": "error_page",
        "upstream_error_detail": "reset_connection",
    });
    let response = json!({
        "success": true,
        "data": {
            "behavior": "error_page",
            "upstream_error_detail": "less",
        },
    });

    let error =
        super::runtime::ensure_gateway_unmatched_route_applied(&requested, response).unwrap_err();
    assert!(error.contains("upgrade the gateway backend"));
    assert!(error.contains(r#""upstream_error_detail":"reset_connection""#));
    assert!(error.contains(r#""upstream_error_detail":"less""#));
}

#[test]
fn gateway_unmatched_route_invalid_behavior_falls_back_to_error_page() {
    let mut config = json!({
        "gateway_unmatched_route": { "behavior": "reset_connection" }
    });
    apply_gateway_patch(
        &mut config,
        json!({ "unmatched_route": { "behavior": "drop" } })
            .as_object()
            .unwrap(),
    );
    assert_eq!(
        config.pointer("/gateway_unmatched_route/behavior"),
        Some(&json!("error_page"))
    );
    assert_eq!(
        config.pointer("/gateway_unmatched_route/upstream_error_detail"),
        Some(&json!("less"))
    );
}

#[test]
fn gateway_number_normalizers_match_node_parse_int_for_strings() {
    assert_eq!(
        normalize_reverse_proxy_throttle(&json!({
            "requests_per_second": "12px",
            "burst": "1.9",
            "block_seconds": "  +30s"
        })),
        json!({
            "enabled": true,
            "requests_per_second": 12,
            "burst": 1,
            "block_seconds": 30
        })
    );
}

#[test]
fn gateway_target_configs_filter_auth_targets_and_stale_hosts() {
    let config = json!({
        "run_type": 3,
        "host_mappings": [
            { "host": "app.example.com", "target": "http://127.0.0.1:8080", "title": "App" },
            { "host": "auth.example.com", "target": "http://127.0.0.1:7997", "title": "Auth" }
        ],
    });
    let requested = json!({
        "disabled_hosts": ["APP.EXAMPLE.COM", "missing.example.com", "auth.example.com"]
    });
    let compiled = compile_gateway_proxy_headers_state(&config, &requested);

    assert_eq!(
        compiled.config["disabled_hosts"],
        json!(["app.example.com"])
    );
    assert_eq!(
        compiled.runtime["omit_targets"],
        json!(["http://127.0.0.1:8080"])
    );
}

#[test]
fn gateway_target_runtime_supports_reverse_proxy_subdomain_mode() {
    let config = json!({
        "run_type": 1,
        "reverse_proxy_submode": "subdomain",
        "host_mappings": [
            { "host": "app.example.com", "target": "http://127.0.0.1:8080", "title": "App" }
        ],
    });
    let compiled = compile_gateway_host_response_state(&config, &json!({ "disabled_hosts": [] }));

    assert_eq!(compiled.runtime["enabled"], Value::Bool(true));
    assert_eq!(compiled.runtime["omit_targets"], json!([]));
}

#[test]
fn localizes_gateway_settings_route_errors() {
    let translator = Translator::new("zh-CN");

    assert_eq!(
        gateway_route_text(&translator, "loadGatewaySettingsFailed"),
        "加载网关设置失败"
    );
    assert_eq!(
        localize_gateway_route_message(&translator, "Gateway payload must be an object"),
        "网关请求内容必须是对象"
    );
    assert_eq!(
        localize_gateway_route_message(&translator, "Gateway visibility payload must be an object"),
        "网关请求内容必须是对象"
    );
    assert_eq!(
        localize_gateway_route_message(&translator, GO_BACKEND_UNSUCCESSFUL_RESPONSE),
        "上游服务不可用"
    );
    assert_eq!(
        rollback_message(
            &translator,
            "Gateway payload must be an object",
            None,
            "server.admin.gatewayVisibility.updateFailedRolledBack",
        ),
        "网关请求内容必须是对象"
    );
    assert_eq!(
        gateway_route_text_params(
            &translator,
            "syncGatewaySettingsFailed",
            &[("message", "网关不可用".to_string())],
        ),
        "同步网关设置失败：网关不可用"
    );
}

#[test]
fn gateway_visibility_cidr_validation_matches_node_shape() {
    let translator = Translator::new("en");
    let cidrs = validate_gateway_custom_cidrs(
        vec![
            Value::String(" 203.0.113.0/24 ".to_string()),
            Value::String("203.0.113.0/24".to_string()),
            Value::String("2001:db8::/32".to_string()),
        ],
        &translator,
    )
    .unwrap();
    assert_eq!(cidrs, vec!["203.0.113.0/24", "2001:db8::/32"]);

    let error =
        validate_gateway_custom_cidrs(vec![Value::String("10.0.0.0/33".to_string())], &translator)
            .unwrap_err();
    assert!(error.contains("10.0.0.0/33"));
}
