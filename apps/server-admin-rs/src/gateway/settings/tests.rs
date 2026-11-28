use super::*;

#[test]
fn gateway_memory_gc_percent_uses_safe_default_and_valid_range() {
    assert_eq!(
        gateway_memory_gc_percent(&json!({})),
        DEFAULT_GATEWAY_GC_PERCENT
    );
    assert_eq!(
        gateway_memory_gc_percent(&json!({ "gateway_memory": { "gc_percent": 50 } })),
        50
    );
    for invalid in [24, 501] {
        assert_eq!(
            gateway_memory_gc_percent(&json!({ "gateway_memory": { "gc_percent": invalid } })),
            DEFAULT_GATEWAY_GC_PERCENT
        );
    }
}

#[test]
fn gateway_memory_settings_default_to_auto_and_bound_the_limit() {
    assert_eq!(
        gateway_memory_settings(&json!({})),
        GatewayMemorySettings {
            gc_percent: DEFAULT_GATEWAY_GC_PERCENT,
            memory_limit_mib: None,
        }
    );
    assert_eq!(auto_gateway_memory_limit_mib(None), 256);
    assert_eq!(auto_gateway_memory_limit_mib(Some(512 * MIB)), 128);
    assert_eq!(auto_gateway_memory_limit_mib(Some(1024 * MIB)), 256);
    assert_eq!(auto_gateway_memory_limit_mib(Some(16 * 1024 * MIB)), 512);
    assert_eq!(
        gateway_memory_settings(&json!({
            "gateway_memory": { "gc_percent": 75, "memory_limit_mib": 384 }
        })),
        GatewayMemorySettings {
            gc_percent: 75,
            memory_limit_mib: Some(384),
        }
    );
}

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
    settings.go_backend_grpc_addr = "http://127.0.0.1:1".to_string();
    settings.internal_rpc_token = "test-internal-rpc-token".to_string();
    settings.request_timeout = std::time::Duration::from_millis(100);
    let state = AppState::new(settings).await.unwrap();
    (directory, state)
}

#[tokio::test]
async fn gateway_visibility_replay_and_updates_share_a_transaction_lock() {
    use std::time::Duration;

    let (_directory, state) = gateway_settings_test_state().await;
    let guard = state.gateway.gateway_visibility_update_lock.lock().await;
    let replay = sync_gateway_visibility_runtime_from_store(&state);
    let update = handlers::update_gateway_visibility(
        axum::extract::State(state.clone()),
        axum::Json(json!({"enabled": true})),
    );
    tokio::pin!(replay, update);
    assert!(
        tokio::time::timeout(Duration::from_millis(20), &mut replay)
            .await
            .is_err()
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(20), &mut update)
            .await
            .is_err()
    );
    drop(guard);
    assert!(replay.await.is_err()); // The fixture has no gateway.
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
            .starts_with("ipset-v2:")
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
        default_reverse_proxy_throttle(),
        json!({
            "enabled": true,
            "requests_per_second": 500,
            "burst": 1_000,
            "block_seconds": 30
        })
    );
    assert_eq!(
        normalize_gateway_unmatched_route(&json!({})),
        json!({
            "behavior": "error_page",
            "upstream_error_detail": "less",
            "disabled_host_behavior": "error_page"
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
            "select_page_enabled": true,
            "display_style": "title",
            "show_app_icon": true,
            "show_wol": true,
            "icon_drag_mode": "corners",
            "version": "v1",
            "navigation_mode": "internet",
            "smart_lan_detection": false
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

#[test]
fn effective_gateway_portal_masks_wol_without_overwriting_preference() {
    let disabled = json!({
        "wol_feature": { "enabled": false },
        "gateway_portal": { "show_wol": true }
    });
    assert_eq!(
        super::runtime::effective_gateway_portal(&disabled)["show_wol"],
        json!(false)
    );
    assert_eq!(disabled["gateway_portal"]["show_wol"], json!(true));

    let enabled = json!({
        "wol_feature": { "enabled": true },
        "gateway_portal": { "show_wol": true }
    });
    assert_eq!(
        super::runtime::effective_gateway_portal(&enabled)["show_wol"],
        json!(true)
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
    assert_eq!(
        response.pointer("/unmatched_route/disabled_host_behavior"),
        Some(&json!("error_page"))
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

#[test]
fn gateway_portal_navigation_patch_preserves_fixed_mode_when_smart_changes() {
    let mut config = json!({});
    apply_gateway_patch(
        &mut config,
        json!({ "portal": { "navigation_mode": "lan" } })
            .as_object()
            .unwrap(),
    );
    apply_gateway_patch(
        &mut config,
        json!({ "portal": { "smart_lan_detection": true } })
            .as_object()
            .unwrap(),
    );
    assert_eq!(config["gateway_portal"]["navigation_mode"], json!("lan"));
    assert_eq!(config["gateway_portal"]["smart_lan_detection"], json!(true));
    apply_gateway_patch(
        &mut config,
        json!({ "portal": { "smart_lan_detection": false } })
            .as_object()
            .unwrap(),
    );
    assert_eq!(config["gateway_portal"]["navigation_mode"], json!("lan"));
    assert_eq!(
        normalize_gateway_portal(&json!({ "navigation_mode": "unknown" }))["navigation_mode"],
        json!("internet")
    );
}

#[test]
fn gateway_portal_navigation_runtime_echo_rejects_legacy_backend() {
    for patch in [
        json!({ "navigation_mode": "lan" }),
        json!({ "smart_lan_detection": true }),
    ] {
        let error = super::runtime::ensure_gateway_portal_applied(
            &patch,
            json!({ "success": true, "data": {} }),
        )
        .unwrap_err();
        assert!(error.contains("upgrade the gateway backend"));
    }
    assert!(
        super::runtime::ensure_gateway_portal_applied(
            &json!({}),
            json!({ "success": true, "data": {} })
        )
        .is_ok()
    );
}

#[test]
fn disabled_host_behavior_normalizes_and_preserves_partial_updates() {
    for value in [json!({}), json!({"disabled_host_behavior": "invalid"})] {
        assert_eq!(
            normalize_gateway_unmatched_route(&value)["disabled_host_behavior"],
            "error_page"
        );
    }
    let mut config = json!({"gateway_unmatched_route": {
        "behavior": "reset_connection", "upstream_error_detail": "more"
    }});
    apply_gateway_patch(
        &mut config,
        json!({"unmatched_route": {
            "disabled_host_behavior": "reset_connection"
        }})
        .as_object()
        .unwrap(),
    );
    assert_eq!(
        config["gateway_unmatched_route"],
        json!({
            "behavior": "reset_connection", "upstream_error_detail": "more",
            "disabled_host_behavior": "reset_connection"
        })
    );
    apply_gateway_patch(
        &mut config,
        json!({"unmatched_route": {
            "behavior": "error_page"
        }})
        .as_object()
        .unwrap(),
    );
    assert_eq!(
        config["gateway_unmatched_route"]["disabled_host_behavior"],
        "reset_connection"
    );
}

#[test]
fn disabled_host_behavior_requires_gateway_acknowledgement() {
    let requested = json!({"disabled_host_behavior": "reset_connection"});
    let applied = json!({"success": true, "data": requested.clone()});
    assert!(super::runtime::ensure_gateway_unmatched_route_applied(&requested, applied).is_ok());
    let legacy = json!({"success": true, "data": {"behavior": "error_page"}});
    let error = super::runtime::ensure_gateway_unmatched_route_applied(&requested, legacy.clone())
        .unwrap_err();
    assert!(error.contains("upgrade the gateway backend"));
    assert!(error.contains("disabled_host_behavior"));
    assert!(super::runtime::ensure_gateway_unmatched_route_applied(&json!({}), legacy).is_ok());
}

#[test]
fn select_page_defaults_and_partial_updates_are_independent() {
    assert_eq!(
        normalize_gateway_portal(&json!({}))["select_page_enabled"],
        true
    );
    let mut config = json!({"gateway_portal": {"enabled": false}});
    apply_gateway_patch(
        &mut config,
        json!({"portal": {"select_page_enabled": false}})
            .as_object()
            .unwrap(),
    );
    assert_eq!(config["gateway_portal"]["enabled"], false);
    apply_gateway_patch(
        &mut config,
        json!({"portal": {"enabled": true, "version": "v2"}})
            .as_object()
            .unwrap(),
    );
    assert_eq!(config["gateway_portal"]["select_page_enabled"], false);
    assert_eq!(
        super::runtime::effective_gateway_portal(&config)["select_page_enabled"],
        false
    );
    assert_eq!(config["gateway_portal"]["enabled"], true);
}

#[test]
fn select_page_runtime_echo_requires_explicit_disable_acknowledgement() {
    let requested = json!({"select_page_enabled": false});
    assert!(
        super::runtime::ensure_gateway_portal_applied(
            &requested,
            json!({"success": true, "data": requested})
        )
        .is_ok()
    );
    for applied in [json!({}), json!({"select_page_enabled": true})] {
        let error = super::runtime::ensure_gateway_portal_applied(
            &json!({"select_page_enabled": false}),
            json!({"success": true, "data": applied}),
        )
        .unwrap_err();
        assert!(error.contains("upgrade the gateway backend"));
        assert!(error.contains("select_page_enabled"));
    }
    assert!(
        super::runtime::ensure_gateway_portal_applied(
            &json!({"select_page_enabled": true}),
            json!({"success": true, "data": {}})
        )
        .is_ok()
    );
}

#[tokio::test]
async fn select_page_update_rolls_back_persistence_when_gateway_is_unavailable() {
    let (_directory, state) = gateway_settings_test_state().await;
    let previous = state.storage.store.get_config().await.unwrap();
    let response = super::handlers::update_gateway(
        State(state.clone()),
        axum::Json(json!({"portal": {"select_page_enabled": false}})),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let restored = state.storage.store.get_config().await.unwrap();
    assert_eq!(restored, previous);
}

#[tokio::test]
async fn select_page_persisted_preference_is_used_by_boot_runtime_config() {
    let (_directory, state) = gateway_settings_test_state().await;
    let mut config = state.storage.store.get_config().await.unwrap();
    apply_gateway_patch(
        &mut config,
        json!({"portal": {"select_page_enabled": false}})
            .as_object()
            .unwrap(),
    );
    state.storage.store.save_config(&config).await.unwrap();
    let restored = state.storage.store.get_config().await.unwrap();
    assert_eq!(
        super::runtime::effective_gateway_portal(&restored)["select_page_enabled"],
        false
    );
}
