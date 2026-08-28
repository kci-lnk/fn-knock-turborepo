use super::service::{
    apply_recommended_lfi_rule_patch_if_needed, apply_recommended_system_rule_state,
    normalize_fixed_waf_config, should_sync_system_rules_for_restore, waf_drain_schedule,
};
use super::*;
use serde_json::json;

async fn waf_test_state(go_backend_grpc_addr: &str) -> (tempfile::TempDir, AppState) {
    let directory = tempfile::tempdir().unwrap();
    let mut settings = {
        let _environment = crate::test_support::EnvGuard::new(&[]);
        crate::settings::Settings::from_env()
    };
    settings.runtime_target = "linux".to_string();
    settings.data_dir = directory.path().join("data");
    settings.gateway_config_dir = directory.path().join("gateway");
    settings.waf_dir = directory.path().join("waf");
    settings.sqlite_path = directory.path().join("fn-knock.sqlite3");
    settings.legacy_redis_url = String::new();
    settings.go_backend_grpc_addr = go_backend_grpc_addr.to_string();
    settings.internal_rpc_token = "test-internal-rpc-token".to_string();
    settings.request_timeout = std::time::Duration::from_millis(100);
    let state = AppState::new(settings).await.unwrap();
    (directory, state)
}

#[tokio::test]
async fn disabled_waf_has_no_periodic_drain_deadline() {
    let (_directory, state) = waf_test_state("http://127.0.0.1:1").await;
    let mut config = state.storage.store.get_config().await.unwrap();
    config["waf"] = json!({"enabled": false});
    state.storage.store.save_config(&config).await.unwrap();
    assert_eq!(waf_drain_schedule(&state).await, None);

    let mut config = state.storage.store.get_config().await.unwrap();
    config["waf"] = json!({"enabled": true, "drain_interval_seconds": 17});
    state.storage.store.save_config(&config).await.unwrap();
    assert_eq!(waf_drain_schedule(&state).await, Some(17));
}

#[tokio::test]
async fn normalizes_waf_block_behavior_with_backward_compatible_default() {
    let (_directory, state) = waf_test_state("http://127.0.0.1:1").await;

    for input in [json!({}), json!({"block_behavior": "invalid"})] {
        assert_eq!(
            normalize_fixed_waf_config(Some(&input), &state).get("block_behavior"),
            Some(&json!("error_page"))
        );
    }

    assert_eq!(
        normalize_fixed_waf_config(Some(&json!({"block_behavior": "reset_connection"})), &state,)
            .get("block_behavior"),
        Some(&json!("reset_connection"))
    );
}

#[test]
fn sanitizes_initialization_rules_like_node() {
    let event = sanitize_event(json!({
        "trace_id": "t1",
        "action": "log",
        "rules": [
            { "id": 901, "file": "/x/REQUEST-901-INITIALIZATION.conf" },
            { "id": 1001, "file": "/x/rule.conf" }
        ],
        "rule_ids": [901, 1001],
        "interruption": { "rule_id": 901 }
    }))
    .unwrap();

    assert_eq!(event["rules"].as_array().unwrap().len(), 1);
    assert_eq!(event["rule_ids"], json!([1001]));
    assert!(event.get("interruption").is_none());
}

#[test]
fn drops_events_without_rule_or_blocking_signal() {
    assert!(
        sanitize_event(json!({
            "trace_id": "t1",
            "action": "log",
            "rules": []
        }))
        .is_none()
    );
    assert!(
        sanitize_event(json!({
            "trace_id": "t1",
            "action": "block"
        }))
        .is_some()
    );
}

#[test]
fn filters_waf_events_by_query() {
    let event = json!({
        "trace_id": "abc",
        "host": "example.com",
        "client_ip": "1.1.1.1",
        "route_type": "host",
        "mode": "blocking",
        "rule_ids": [1001],
        "path": "/login"
    });
    assert!(event_matches(
        &event,
        &WafLogQuery {
            date: None,
            trace_id: None,
            search: Some("login".to_string()),
            host: Some("EXAMPLE.com".to_string()),
            client_ip: Some("1.1.1.1".to_string()),
            rule_id: Some("1001".to_string()),
            route_type: Some("host".to_string()),
            mode: Some("blocking".to_string()),
            cursor: None,
            limit: None,
        }
    ));
}

#[test]
fn waf_query_number_parsers_match_node_parse_int_edges() {
    assert_eq!(normalize_limit(Some("10x")), 10);
    assert_eq!(normalize_limit(Some("  +3.9")), 3);
    assert_eq!(normalize_limit(Some("-1")), 50);
    assert_eq!(normalize_limit(Some("300")), 200);
    assert_eq!(normalize_limit(Some("0x10")), 50);

    assert_eq!(normalize_cursor(Some("12x")), 12);
    assert_eq!(normalize_cursor(Some("  +3.9")), 3);
    assert_eq!(normalize_cursor(Some("-1")), 0);
    assert_eq!(normalize_cursor(Some("0x10")), 0);
}

#[test]
fn waf_event_filters_match_node_unicode_and_rule_id_prefixes() {
    let event = json!({
        "trace_id": "abc",
        "host": "Ä.example",
        "client_ip": "1.1.1.1",
        "route_type": "host",
        "mode": "blocking",
        "rule_ids": [1001],
        "path": "/Älice"
    });

    assert!(event_matches(
        &event,
        &WafLogQuery {
            date: None,
            trace_id: None,
            search: Some("älice".to_string()),
            host: Some("ä.example".to_string()),
            client_ip: None,
            rule_id: Some("1001x".to_string()),
            route_type: None,
            mode: None,
            cursor: None,
            limit: None,
        }
    ));

    assert!(!event_matches(
        &event,
        &WafLogQuery {
            date: None,
            trace_id: None,
            search: None,
            host: None,
            client_ip: None,
            rule_id: Some("nope".to_string()),
            route_type: None,
            mode: None,
            cursor: None,
            limit: None,
        }
    ));
}

#[test]
fn normalizes_waf_rule_filenames_like_node() {
    assert_eq!(
        safe_rule_filename("../custom rule.conf").unwrap(),
        "custom-rule.conf"
    );
    assert!(safe_rule_filename("../secret.txt").is_err());
    assert!(safe_rule_filename("..").is_err());
}

#[test]
fn localizes_waf_route_and_service_errors() {
    let translator = Translator::new("zh-CN");

    assert_eq!(
        waf_text(&translator, "detailsLoadFailed"),
        "读取 WAF 详情失败"
    );
    assert_eq!(
        localize_waf_error(&translator, "WAF manifest is empty"),
        "系统规则清单为空"
    );
    assert_eq!(
        localize_waf_error(&translator, "Duplicate WAF bundle file: REQUEST.conf"),
        "系统规则包内存在重复文件: REQUEST.conf"
    );
    assert_eq!(
        localize_waf_error(&translator, "WAF rule file is too large: custom.conf"),
        "custom.conf 超过 1MB"
    );
    assert_eq!(
        localize_waf_error(&translator, "Invalid WAF rule source"),
        "规则来源不正确"
    );
    assert_eq!(
        localize_waf_error(&translator, "invalid date, expected YYYY-MM-DD"),
        "日期格式不正确，应为 YYYY-MM-DD"
    );
}

#[test]
fn blocks_filesystem_directives_in_uploaded_rules() {
    assert!(contains_blocked_directive("  Include /tmp/*.conf"));
    assert!(contains_blocked_directive("SecAuditLog /tmp/audit.log"));
    assert!(!contains_blocked_directive(
        "SecRule ARGS attack \"id:1001\""
    ));
}

#[test]
fn defaults_high_noise_system_rules_to_disabled() {
    assert!(is_system_rule_enabled_by_default(
        "REQUEST-901-INITIALIZATION.conf"
    ));
    assert!(!is_system_rule_enabled_by_default(
        "REQUEST-942-APPLICATION-ATTACK-SQLI.conf"
    ));
    assert!(is_system_rule_enabled_by_default(
        "REQUEST-930-APPLICATION-ATTACK-LFI.conf"
    ));
    assert!(is_system_rule_enabled_by_default(
        "REQUEST-949-BLOCKING-EVALUATION.conf"
    ));
}

#[tokio::test]
async fn recommended_lfi_patch_enables_legacy_rule_once() {
    let (_directory, state) = waf_test_state("127.0.0.1:1").await;
    let legacy = WafRulesState {
        system_enabled: BTreeMap::from([
            (INITIALIZATION_RULE_FILENAME.to_string(), true),
            (LFI_RULE_FILENAME.to_string(), false),
        ]),
        custom_enabled: BTreeMap::from([("custom.conf".to_string(), true)]),
    };
    write_rules_state(&state, &legacy).await.unwrap();

    assert!(
        apply_recommended_lfi_rule_patch_if_needed(&state)
            .await
            .unwrap()
    );
    let patched = read_rules_state(&state).await.unwrap();
    assert_eq!(patched.system_enabled.get(LFI_RULE_FILENAME), Some(&true));
    assert_eq!(patched.custom_enabled, legacy.custom_enabled);
    assert_eq!(
        state
            .storage
            .store
            .get_string_value(RECOMMENDED_LFI_RULE_PATCH_FLAG_KEY)
            .await
            .unwrap()
            .as_deref(),
        Some("1")
    );

    let mut user_updated = patched;
    user_updated
        .system_enabled
        .insert(LFI_RULE_FILENAME.to_string(), false);
    write_rules_state(&state, &user_updated).await.unwrap();

    assert!(
        !apply_recommended_lfi_rule_patch_if_needed(&state)
            .await
            .unwrap()
    );
    assert_eq!(
        read_rules_state(&state)
            .await
            .unwrap()
            .system_enabled
            .get(LFI_RULE_FILENAME),
        Some(&false)
    );
}

#[tokio::test]
async fn recommended_lfi_patch_marks_pre_enabled_rule_without_rewriting_choice() {
    let (_directory, state) = waf_test_state("127.0.0.1:1").await;
    let current = WafRulesState {
        system_enabled: BTreeMap::from([
            (INITIALIZATION_RULE_FILENAME.to_string(), true),
            (LFI_RULE_FILENAME.to_string(), true),
        ]),
        custom_enabled: BTreeMap::new(),
    };
    write_rules_state(&state, &current).await.unwrap();

    assert!(
        !apply_recommended_lfi_rule_patch_if_needed(&state)
            .await
            .unwrap()
    );
    assert_eq!(
        read_rules_state(&state).await.unwrap().system_enabled,
        current.system_enabled
    );
    assert_eq!(
        state
            .storage
            .store
            .get_string_value(RECOMMENDED_LFI_RULE_PATCH_FLAG_KEY)
            .await
            .unwrap()
            .as_deref(),
        Some("1")
    );
}

#[test]
fn recommended_preset_resets_system_rules_and_preserves_custom_rules() {
    let mut state = WafRulesState {
        system_enabled: BTreeMap::from([
            ("REQUEST-942-APPLICATION-ATTACK-SQLI.conf".to_string(), true),
            ("REQUEST-949-BLOCKING-EVALUATION.conf".to_string(), false),
            ("REMOVED-SYSTEM-RULE.conf".to_string(), true),
        ]),
        custom_enabled: BTreeMap::from([("custom.conf".to_string(), true)]),
    };

    apply_recommended_system_rule_state(
        &mut state,
        vec![
            "REQUEST-942-APPLICATION-ATTACK-SQLI.conf".to_string(),
            "REQUEST-949-BLOCKING-EVALUATION.conf".to_string(),
        ],
    );

    assert_eq!(
        state
            .system_enabled
            .get("REQUEST-942-APPLICATION-ATTACK-SQLI.conf"),
        Some(&false)
    );
    assert_eq!(
        state
            .system_enabled
            .get("REQUEST-949-BLOCKING-EVALUATION.conf"),
        Some(&true)
    );
    assert_eq!(state.custom_enabled.get("custom.conf"), Some(&true));
    assert!(
        !state
            .system_enabled
            .contains_key("REMOVED-SYSTEM-RULE.conf")
    );
    assert_eq!(
        state.system_enabled.get(INITIALIZATION_RULE_FILENAME),
        Some(&true)
    );
}

#[tokio::test]
async fn recommended_preset_rolls_back_state_when_gateway_reload_fails() {
    let (_directory, state) = waf_test_state("127.0.0.1:1").await;
    ensure_waf_directories(&state).await.unwrap();
    fs::write(
        system_dir(&state).join("REQUEST-949-BLOCKING-EVALUATION.conf"),
        b"SecAction \"id:949001,phase:1,pass\"\n",
    )
    .await
    .unwrap();
    fs::write(
        custom_dir(&state).join("custom.conf"),
        b"SecAction \"id:100001,phase:1,pass\"\n",
    )
    .await
    .unwrap();
    write_json_file(
        &manifest_cache_path(&state),
        &json!({
            "manifest": {},
            "cached_at": time_utils::now_iso(),
            "last_checked_at": time_utils::now_iso(),
            "last_error": null
        }),
    )
    .await
    .unwrap();
    let previous = WafRulesState {
        system_enabled: BTreeMap::from([
            (INITIALIZATION_RULE_FILENAME.to_string(), true),
            ("REQUEST-949-BLOCKING-EVALUATION.conf".to_string(), false),
        ]),
        custom_enabled: BTreeMap::from([("custom.conf".to_string(), true)]),
    };
    write_rules_state(&state, &previous).await.unwrap();
    state
        .storage
        .store
        .save_config(&json!({"waf": {"enabled": true, "mode": "block"}}))
        .await
        .unwrap();

    assert!(set_recommended_system_rules(&state).await.is_err());

    let restored = read_rules_state(&state).await.unwrap();
    assert_eq!(restored.system_enabled, previous.system_enabled);
    assert_eq!(restored.custom_enabled, previous.custom_enabled);
}

#[test]
fn derives_disabled_waf_hosts_from_business_mappings() {
    let config = json!({
        "host_mappings": [
            {"host": "Z.EXAMPLE.COM", "service_role": "app", "waf_enabled": false},
            {"host": " App.Example.COM:443 ", "service_role": "app", "waf_enabled": false},
            {"host": "app.example.com", "service_role": "app", "waf_enabled": false},
            {"host": "enabled.example.com", "service_role": "app", "waf_enabled": true},
            {"host": "auth.example.com", "service_role": "auth", "waf_enabled": false},
            {"host": "legacy-auth.example.com", "target": "http://localhost:7997", "waf_enabled": false}
        ]
    });

    assert_eq!(
        disabled_hosts_for_config(&config),
        vec!["app.example.com".to_string(), "z.example.com".to_string()]
    );
}

#[test]
fn disabled_waf_hosts_follow_mapping_renames_and_removals() {
    let previous = json!({
        "host_mappings": [
            {"host": "B.Example.COM", "waf_enabled": false},
            {"host": "a.example.com", "waf_enabled": false}
        ]
    });
    let reordered = json!({
        "host_mappings": [
            {"host": "a.example.com", "waf_enabled": false},
            {"host": "b.example.com", "waf_enabled": false}
        ]
    });
    let renamed = json!({
        "host_mappings": [
            {"host": "renamed.example.com", "waf_enabled": false},
            {"host": "a.example.com", "waf_enabled": false}
        ]
    });
    let removed = json!({"host_mappings": []});

    assert_eq!(
        disabled_hosts_for_config(&previous),
        disabled_hosts_for_config(&reordered)
    );
    assert_eq!(
        disabled_hosts_for_config(&renamed),
        vec![
            "a.example.com".to_string(),
            "renamed.example.com".to_string()
        ]
    );
    assert!(disabled_hosts_for_config(&removed).is_empty());
}

#[test]
fn backup_restore_syncs_rules_only_when_enabled_and_missing() {
    assert!(should_sync_system_rules_for_restore(
        &json!({"enabled": true}),
        false
    ));
    assert!(!should_sync_system_rules_for_restore(
        &json!({"enabled": true}),
        true
    ));
    assert!(!should_sync_system_rules_for_restore(
        &json!({"enabled": false}),
        false
    ));
}

#[test]
fn validates_waf_bundle_paths() {
    assert_eq!(
        safe_bundle_entry_path("REQUEST-920-PROTOCOL-ENFORCEMENT.conf").unwrap(),
        "REQUEST-920-PROTOCOL-ENFORCEMENT.conf"
    );
    assert!(safe_bundle_entry_path("../evil.conf").is_err());
    assert!(safe_bundle_entry_path("/absolute.conf").is_err());
    assert!(safe_bundle_entry_path("nested//rule.conf").is_err());
}

#[test]
fn resolves_waf_download_urls_without_cache_busting() {
    assert_eq!(
        resolve_waf_url("https://cor.fnknock.cn/waf/manifest.json", None).unwrap(),
        "https://cor.fnknock.cn/waf/manifest.json"
    );
    assert_eq!(
        resolve_waf_url(
            "rules/system.zip",
            Some("https://cor.fnknock.cn/waf/manifest.json")
        )
        .unwrap(),
        "https://cor.fnknock.cn/waf/rules/system.zip"
    );
}
