use super::*;

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
    assert_eq!(compiled.runtime["cidrs"], json!([]));
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
    assert_eq!(visible.len(), 1);
    assert_eq!(
        proxy_items[0]
            .get("send_proxy_headers")
            .and_then(Value::as_bool),
        Some(false)
    );
}

#[test]
fn gateway_patch_merges_and_normalizes_sections() {
    let mut config = json!({
        "subdomain_mode": { "auth_cache_ttl_seconds": 1 },
        "reverse_proxy_throttle": { "enabled": true, "requests_per_second": 10, "burst": 20, "block_seconds": 30 },
        "gateway_portal": { "enabled": true, "display_style": "title", "show_app_icon": true, "icon_drag_mode": "corners" }
    });
    let patch = json!({
        "auth_cache_ttl_seconds": 8,
        "reverse_proxy_throttle": { "burst": 250 },
        "portal": { "display_style": "domain", "show_app_icon": false },
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
