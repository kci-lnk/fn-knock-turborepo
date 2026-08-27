use super::*;

async fn fpk_lite_runtime_test_state() -> (tempfile::TempDir, AppState) {
    let directory = tempfile::tempdir().expect("create FPK Lite runtime test directory");
    let mut settings = {
        let _environment = crate::test_support::EnvGuard::new(&[]);
        crate::settings::Settings::from_env()
    };
    settings.runtime_target = "fpk-lite".to_string();
    settings.data_dir = directory.path().join("data");
    settings.gateway_config_dir = directory.path().join("gateway");
    settings.sqlite_path = directory.path().join("fn-knock.sqlite3");
    settings.legacy_redis_url = String::new();
    settings.internal_rpc_token = "fpk-lite-runtime-test-token".to_string();
    settings.go_backend_grpc_addr = "127.0.0.1:1".to_string();
    settings.request_timeout = std::time::Duration::from_millis(100);
    let state = AppState::new(settings)
        .await
        .expect("create FPK Lite runtime state");
    (directory, state)
}

async fn response_json(response: Response) -> Value {
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read response body");
    serde_json::from_slice(&body).expect("parse response body")
}

struct SuccessfulFirewallReset;

impl FirewallResetOperation for SuccessfulFirewallReset {
    fn reset<'a>(&'a self, _state: &'a AppState, run_type: i64) -> FirewallResetFuture<'a> {
        Box::pin(async move {
            Ok(json!({
                "runType": run_type,
                "gatewayPort": gateway_port(),
                "exemptPorts": [],
                "whitelistSynced": 0,
            }))
        })
    }
}

struct FailingThenSuccessfulFirewallReset {
    attempts: std::sync::atomic::AtomicUsize,
}

impl FirewallResetOperation for FailingThenSuccessfulFirewallReset {
    fn reset<'a>(&'a self, state: &'a AppState, _run_type: i64) -> FirewallResetFuture<'a> {
        Box::pin(async move {
            let attempt = self
                .attempts
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let config = state
                .storage
                .store
                .get_config()
                .await
                .map_err(|error| error.to_string())?;
            if attempt == 0 {
                assert_eq!(
                    config.get("firewall_additional_ports"),
                    Some(&json!([5666]))
                );
                state
                    .storage
                    .store
                    .set_config_top_level_value("default_route", json!("/concurrent"))
                    .await
                    .map_err(|error| error.to_string())?;
                return Err("apply failed".to_string());
            }
            assert_eq!(
                config.get("firewall_additional_ports"),
                Some(&json!([1234]))
            );
            assert_eq!(config.get("default_route"), Some(&json!("/concurrent")));
            Ok(json!({ "runType": 3 }))
        })
    }
}

#[test]
fn redis_json_keys_match_node_feature_section_store() {
    assert_eq!(CAPTCHA_SETTINGS_KEY, "fn_knock:captcha:settings");
    assert_eq!(LEGACY_CAPTCHA_SETTINGS_KEY, "fn_knock:config:captcha");
    assert_eq!(
        PROTOCOL_MAPPING_FEATURE_KEY,
        "fn_knock:protocol-mapping:feature"
    );
}

#[tokio::test]
async fn boot_migration_enables_gateway_wol_shortcut_once() {
    let (_directory, state) = fpk_lite_runtime_test_state().await;
    let mut config = state.storage.store.get_config().await.expect("load config");
    config["gateway_portal"]["show_wol"] = json!(false);

    let applied = apply_boot_config_migrations(&state, &mut config)
        .await
        .expect("apply config migrations");
    assert!(applied.contains(&"gateway_portal_show_wol_default"));
    assert_eq!(config["gateway_portal"]["show_wol"], json!(true));
    assert_eq!(
        state
            .storage
            .store
            .get_string_value(GATEWAY_PORTAL_SHOW_WOL_DEFAULT_PATCH_FLAG_KEY)
            .await
            .expect("read patch marker")
            .as_deref(),
        Some("1")
    );

    config["gateway_portal"]["show_wol"] = json!(false);
    let applied = apply_boot_config_migrations(&state, &mut config)
        .await
        .expect("reapply config migrations");
    assert!(!applied.contains(&"gateway_portal_show_wol_default"));
    assert_eq!(config["gateway_portal"]["show_wol"], json!(false));
}

#[tokio::test]
async fn boot_migration_raises_previous_throttle_defaults_once() {
    let (_directory, state) = fpk_lite_runtime_test_state().await;
    let mut config = state.storage.store.get_config().await.expect("load config");
    config["reverse_proxy_throttle"] = json!({
        "enabled": false,
        "requests_per_second": 100,
        "burst": 200,
        "block_seconds": 77,
        "future_option": "preserved"
    });

    let applied = apply_boot_config_migrations(&state, &mut config)
        .await
        .expect("apply config migrations");
    assert!(applied.contains(&"reverse_proxy_throttle_default_v2"));
    assert_eq!(config["reverse_proxy_throttle"]["enabled"], json!(false));
    assert_eq!(
        config["reverse_proxy_throttle"]["requests_per_second"],
        json!(500)
    );
    assert_eq!(config["reverse_proxy_throttle"]["burst"], json!(1_000));
    assert_eq!(config["reverse_proxy_throttle"]["block_seconds"], json!(77));
    assert_eq!(
        config["reverse_proxy_throttle"]["future_option"],
        json!("preserved")
    );
    assert_eq!(
        state
            .storage
            .store
            .get_string_value(REVERSE_PROXY_THROTTLE_DEFAULT_V2_PATCH_FLAG_KEY)
            .await
            .expect("read throttle v2 patch marker")
            .as_deref(),
        Some("1")
    );

    let persisted = state
        .storage
        .store
        .get_config()
        .await
        .expect("reload migrated config");
    assert_eq!(
        persisted["reverse_proxy_throttle"],
        config["reverse_proxy_throttle"]
    );

    config["reverse_proxy_throttle"]["requests_per_second"] = json!(100);
    config["reverse_proxy_throttle"]["burst"] = json!(200);
    state
        .storage
        .store
        .save_config(&config)
        .await
        .expect("save explicit post-migration throttle choice");
    let mut config = state
        .storage
        .store
        .get_config()
        .await
        .expect("reload explicit throttle choice");
    let reapplied = apply_boot_config_migrations(&state, &mut config)
        .await
        .expect("reapply config migrations");
    assert!(!reapplied.contains(&"reverse_proxy_throttle_default_v2"));
    assert_eq!(
        config["reverse_proxy_throttle"]["requests_per_second"],
        json!(100)
    );
    assert_eq!(config["reverse_proxy_throttle"]["burst"], json!(200));
    let persisted = state
        .storage
        .store
        .get_config()
        .await
        .expect("reload preserved throttle choice");
    assert_eq!(
        persisted["reverse_proxy_throttle"],
        config["reverse_proxy_throttle"]
    );
}

#[tokio::test]
async fn boot_migration_preserves_partially_custom_throttle_values() {
    let (_directory, state) = fpk_lite_runtime_test_state().await;
    let mut config = state.storage.store.get_config().await.expect("load config");
    config["reverse_proxy_throttle"] = json!({
        "enabled": true,
        "requests_per_second": 101,
        "burst": 200,
        "block_seconds": 30
    });

    let applied = apply_boot_config_migrations(&state, &mut config)
        .await
        .expect("apply config migrations");
    assert!(!applied.contains(&"reverse_proxy_throttle_default_v2"));
    assert_eq!(
        config["reverse_proxy_throttle"]["requests_per_second"],
        json!(101)
    );
    assert_eq!(config["reverse_proxy_throttle"]["burst"], json!(200));
    assert_eq!(
        state
            .storage
            .store
            .get_string_value(REVERSE_PROXY_THROTTLE_DEFAULT_V2_PATCH_FLAG_KEY)
            .await
            .expect("read throttle v2 patch marker")
            .as_deref(),
        Some("1")
    );
}

#[tokio::test]
async fn boot_migration_chains_all_historical_throttle_defaults() {
    let (_directory, state) = fpk_lite_runtime_test_state().await;
    let mut config = state.storage.store.get_config().await.expect("load config");
    config["reverse_proxy_throttle"] = json!({
        "enabled": true,
        "requests_per_second": 20,
        "burst": 50,
        "block_seconds": 30
    });

    let applied = apply_boot_config_migrations(&state, &mut config)
        .await
        .expect("apply config migrations");
    assert!(applied.contains(&"legacy_reverse_proxy_throttle"));
    assert!(applied.contains(&"reverse_proxy_throttle_default_v2"));
    assert_eq!(
        config["reverse_proxy_throttle"]["requests_per_second"],
        json!(500)
    );
    assert_eq!(config["reverse_proxy_throttle"]["burst"], json!(1_000));
    assert_eq!(config["reverse_proxy_throttle"]["block_seconds"], json!(30));
}

#[tokio::test]
async fn boot_migration_reenables_unvalidated_stream_mappings() {
    let (_directory, state) = fpk_lite_runtime_test_state().await;
    let mut config = state.storage.store.get_config().await.expect("load config");
    config["stream_mappings"] = json!([
        {"protocol": "tcp", "listen_port": 6001, "validation_mode": "off", "disabled": true},
        {"protocol": "tcp", "listen_port": 6002, "disabled": true},
        {"protocol": "tcp", "listen_port": 6003, "validation_mode": "strict", "disabled": true},
        {"protocol": "tcp", "listen_port": 6004, "validation_mode": "off", "disabled": false}
    ]);

    let applied = apply_boot_config_migrations(&state, &mut config)
        .await
        .expect("apply config migrations");
    assert!(applied.contains(&"unvalidated_stream_mappings_enabled"));
    assert_eq!(config["stream_mappings"][0]["disabled"], false);
    assert_eq!(config["stream_mappings"][1]["disabled"], false);
    assert_eq!(config["stream_mappings"][2]["disabled"], true);
    assert_eq!(config["stream_mappings"][3]["disabled"], false);

    let persisted = state
        .storage
        .store
        .get_config()
        .await
        .expect("reload config");
    assert_eq!(persisted["stream_mappings"], config["stream_mappings"]);
}

#[tokio::test]
async fn protocol_mapping_startup_failure_disables_only_the_feature_and_keeps_booting() {
    let (_directory, state) = fpk_lite_runtime_test_state().await;
    let mappings = json!([{
        "protocol": "tcp",
        "listen_port": 5555,
        "target": "127.0.0.1:5555",
        "use_auth": true,
        "comment": "legacy loop"
    }]);
    let mut config = state.storage.store.get_config().await.expect("load config");
    config["run_type"] = json!(3);
    config["stream_mappings"] = mappings.clone();
    state
        .storage
        .store
        .save_config(&config)
        .await
        .expect("save legacy mapping config");
    save_protocol_mapping_feature(&state, &json!({ "enabled": true }))
        .await
        .expect("enable protocol mappings");

    let mut results = std::collections::VecDeque::from([
        Err(proxy_config::mark_stream_mapping_runtime_error(
            "Stream mapping TCP listen_port 5555 cannot target the same local port 127.0.0.1:5555",
        )),
        Ok(()),
    ]);
    apply_run_type_config_on_boot_with(&state, &config, 3, || {
        std::future::ready(results.pop_front().expect("expected apply attempt"))
    })
    .await
    .expect("continue boot with protocol mappings disabled");

    assert!(results.is_empty());
    assert_eq!(
        load_protocol_mapping_feature(&state, None)
            .await
            .expect("load degraded feature"),
        json!({
            "enabled": false,
            "availability": null,
            "runtime_issue": {
                "code": "local_port_loop",
                "message": "Stream mapping TCP listen_port 5555 cannot target the same local port 127.0.0.1:5555",
                "protocol": "tcp",
                "listen_port": 5555,
                "target": "127.0.0.1:5555"
            }
        })
    );
    assert_eq!(
        state
            .storage
            .store
            .get_config()
            .await
            .expect("reload mapping config")["stream_mappings"],
        mappings
    );
}

#[test]
fn protocol_mapping_startup_failure_identifies_an_occupied_listener_even_in_a_500_response() {
    let config = json!({
        "stream_mappings": [
            {
                "protocol": "udp",
                "listen_port": 5353,
                "target": "192.0.2.10:53"
            },
            {
                "protocol": "tcp",
                "listen_port": 9000,
                "target": "127.0.0.1:9001"
            }
        ]
    });
    let issue = protocol_mapping_runtime_issue(
        &config,
        &proxy_config::mark_stream_mapping_runtime_error(
            "set_stream_rules returned 500 Internal Server Error: listen tcp :9000: bind: address already in use",
        ),
    )
    .expect("classify stream mapping runtime error");

    assert_eq!(
        issue,
        json!({
            "code": "listen_port_in_use",
            "message": "set_stream_rules returned 500 Internal Server Error: listen tcp :9000: bind: address already in use",
            "protocol": "tcp",
            "listen_port": 9000,
            "target": "127.0.0.1:9001"
        })
    );
}

#[tokio::test]
async fn transient_stream_mapping_failure_uses_startup_retry_without_disabling_the_feature() {
    let (_directory, state) = fpk_lite_runtime_test_state().await;
    let config = state.storage.store.get_config().await.expect("load config");
    save_protocol_mapping_feature(&state, &json!({ "enabled": true }))
        .await
        .expect("enable protocol mappings");
    let transient_error = proxy_config::mark_stream_mapping_runtime_error(
        "set_stream_rules returned 503 Service Unavailable",
    );

    let result = apply_run_type_config_on_boot_with(&state, &config, 3, || {
        std::future::ready(Err(transient_error.clone()))
    })
    .await;

    assert_eq!(result, Err(transient_error));
    assert_eq!(
        load_protocol_mapping_feature(&state, None)
            .await
            .expect("load unchanged feature"),
        json!({ "enabled": true, "availability": null })
    );
}

#[tokio::test]
async fn unrelated_startup_failure_does_not_disable_protocol_mappings() {
    let (_directory, state) = fpk_lite_runtime_test_state().await;
    let config = state.storage.store.get_config().await.expect("load config");
    save_protocol_mapping_feature(&state, &json!({ "enabled": true }))
        .await
        .expect("enable protocol mappings");

    let result = apply_run_type_config_on_boot_with(&state, &config, 3, || {
        std::future::ready(Err("host mappings transaction is busy".to_string()))
    })
    .await;

    assert_eq!(result, Err("host mappings transaction is busy".to_string()));
    assert_eq!(
        load_protocol_mapping_feature(&state, None)
            .await
            .expect("load unchanged feature"),
        json!({ "enabled": true, "availability": null })
    );
}

#[tokio::test]
async fn protocol_mapping_cleanup_failure_still_allows_the_admin_backend_to_start() {
    let (_directory, state) = fpk_lite_runtime_test_state().await;
    let mut config = state.storage.store.get_config().await.expect("load config");
    config["run_type"] = json!(3);
    config["stream_mappings"] = json!([{
        "protocol": "tcp",
        "listen_port": 9000,
        "target": "127.0.0.1:9001"
    }]);
    state
        .storage
        .store
        .save_config(&config)
        .await
        .expect("save mapping config");
    save_protocol_mapping_feature(&state, &json!({ "enabled": true }))
        .await
        .expect("enable protocol mappings");
    let mut results = std::collections::VecDeque::from([
        Err(proxy_config::mark_stream_mapping_runtime_error(
            "listen tcp :9000: bind: address already in use",
        )),
        Err("gateway cleanup temporarily unavailable".to_string()),
    ]);

    apply_run_type_config_on_boot_with(&state, &config, 3, || {
        std::future::ready(results.pop_front().expect("expected apply attempt"))
    })
    .await
    .expect("keep the admin backend available for repair");

    let feature = load_protocol_mapping_feature(&state, None)
        .await
        .expect("load degraded feature");
    assert_eq!(feature["enabled"], json!(false));
    assert_eq!(
        feature.pointer("/runtime_issue/code"),
        Some(&json!("listen_port_in_use"))
    );
}

#[tokio::test]
async fn boot_migration_backfills_panel_sync_ids_through_host_mapping_cas() {
    let (_directory, state) = fpk_lite_runtime_test_state().await;
    let initial = vec![
        json!({"host": "one.example.com", "service_role": "app"}),
        json!({"host": "two.example.com", "service_role": "app"}),
    ];
    let mut config = state
        .storage
        .store
        .compare_and_set_host_mappings(&[], &initial)
        .await
        .expect("seed host mappings")
        .expect("host mapping seed should win");

    let applied = apply_boot_config_migrations(&state, &mut config)
        .await
        .expect("apply config migrations");
    assert!(applied.contains(&"host_mapping_sync_ids"));

    let persisted = state
        .storage
        .store
        .get_config()
        .await
        .expect("reload config");
    let mappings = persisted["host_mappings"]
        .as_array()
        .expect("host mappings array");
    assert_eq!(mappings.len(), 2);
    let ids = mappings
        .iter()
        .map(|mapping| {
            let value = mapping["sync_id"].as_str().expect("stable sync id");
            uuid::Uuid::parse_str(value).expect("valid sync UUID");
            value
        })
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(ids.len(), 2);

    let mut persisted = persisted;
    let reapplied = apply_boot_config_migrations(&state, &mut persisted)
        .await
        .expect("reapply config migrations");
    assert!(!reapplied.contains(&"host_mapping_sync_ids"));
}

#[tokio::test]
async fn fpk_lite_privileged_runtime_handlers_return_forbidden() {
    let (_directory, state) = fpk_lite_runtime_test_state().await;

    assert_eq!(
        update_run_type(State(state.clone()), Json(json!({ "run_type": 0 })))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        update_terminal_feature(State(state.clone()), Json(json!({ "enabled": true })))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        update_smart_connect(
            State(state.clone()),
            Json(json!({ "enabled": true, "selected_ipv4": "192.168.1.2" }))
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        update_auto_https(State(state.clone()), Json(json!({ "enabled": true })))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        update_auto_manage_firewall(
            State(state.clone()),
            Json(json!({ "auto_manage_firewall": true }))
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        get_firewall_additional_ports(State(state.clone()))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        update_firewall_additional_ports(State(state.clone()), Json(json!({ "ports": [5666] })))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        super::fnos_connect_waf::update_fnos_connect_waf(
            State(state.clone()),
            Json(json!({ "enabled": true })),
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        clear_firewall(State(state)).await.status(),
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn smart_connect_sync_failure_disables_feature_before_run_type_apply() {
    let (_directory, state) = fpk_lite_runtime_test_state().await;
    let mut config = state.storage.store.get_config().await.expect("load config");
    config["run_type"] = json!(3);
    config["smart_connect"] = json!({
        "enabled": true,
        "selected_ipv4": "192.168.1.20"
    });
    state
        .storage
        .store
        .save_config(&config)
        .await
        .expect("save target run type config");
    let attempts = std::sync::atomic::AtomicUsize::new(0);

    let disabled = reconcile_smart_connect_for_run_type_change(&state, &mut config, |state, _| {
        let attempt = attempts.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        async move {
            if attempt == 0 {
                state
                    .storage
                    .store
                    .set_config_top_level_value("default_route", json!("/concurrent"))
                    .await
                    .map_err(|error| error.to_string())?;
                Err("smart connect sync failed".to_string())
            } else {
                Ok(())
            }
        }
    })
    .await
    .expect("degrade smart connect without blocking the mode change");

    assert!(disabled);
    assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 2);
    assert_eq!(config["smart_connect"]["enabled"], json!(false));
    assert_eq!(config["default_route"], json!("/concurrent"));
    assert_eq!(
        state
            .storage
            .store
            .get_config()
            .await
            .expect("reload degraded config")["smart_connect"]["enabled"],
        json!(false)
    );
    assert_eq!(
        state
            .storage
            .store
            .get_config()
            .await
            .expect("reload concurrent config")["default_route"],
        json!("/concurrent")
    );
}

#[tokio::test]
async fn successful_smart_connect_sync_preserves_enabled_feature() {
    let (_directory, state) = fpk_lite_runtime_test_state().await;
    let mut config = state.storage.store.get_config().await.expect("load config");
    config["run_type"] = json!(3);
    config["smart_connect"] = json!({
        "enabled": true,
        "selected_ipv4": "192.168.1.20"
    });
    state
        .storage
        .store
        .save_config(&config)
        .await
        .expect("save target run type config");
    let attempts = std::sync::atomic::AtomicUsize::new(0);

    let disabled = reconcile_smart_connect_for_run_type_change(&state, &mut config, |_, _| {
        attempts.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        async { Ok(()) }
    })
    .await
    .expect("sync smart connect");

    assert!(!disabled);
    assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(config["smart_connect"]["enabled"], json!(true));
}

#[tokio::test]
async fn failed_smart_connect_cleanup_does_not_rewrite_disabled_feature() {
    let (_directory, state) = fpk_lite_runtime_test_state().await;
    let mut config = state.storage.store.get_config().await.expect("load config");
    config["run_type"] = json!(3);
    config["smart_connect"] = json!({
        "enabled": false,
        "selected_ipv4": "192.168.1.20"
    });
    state
        .storage
        .store
        .save_config(&config)
        .await
        .expect("save target run type config");
    let attempts = std::sync::atomic::AtomicUsize::new(0);

    let disabled = reconcile_smart_connect_for_run_type_change(&state, &mut config, |_, _| {
        attempts.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        async { Err("smart connect cleanup failed".to_string()) }
    })
    .await
    .expect("keep the disabled feature non-blocking");

    assert!(!disabled);
    assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(config["smart_connect"]["enabled"], json!(false));
}

#[tokio::test]
async fn protocol_mapping_feature_update_never_mutates_mapping_config() {
    let (_directory, state) = fpk_lite_runtime_test_state().await;
    let mappings = json!([{
        "protocol": "tcp",
        "listen_port": 2222,
        "target": "127.0.0.1:22",
        "use_auth": true,
        "comment": "SSH"
    }]);
    let mut config = state.storage.store.get_config().await.expect("load config");
    config
        .as_object_mut()
        .expect("config object")
        .insert("stream_mappings".to_string(), mappings.clone());
    state
        .storage
        .store
        .save_config(&config)
        .await
        .expect("save config");
    save_protocol_mapping_feature(
        &state,
        &json!({
            "enabled": true,
            "availability": {
                "enabled": true,
                "start_time": "22:00",
                "end_time": "06:00"
            }
        }),
    )
    .await
    .expect("enable protocol mapping feature");
    assert!(disable_stream_rules(&state).await.is_err());

    let response =
        update_protocol_mapping_feature(State(state.clone()), Json(json!({ "enabled": false })))
            .await;

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    assert_eq!(
        state
            .storage
            .store
            .get_config()
            .await
            .expect("reload config")
            .get("stream_mappings"),
        Some(&mappings)
    );
    assert_eq!(
        load_protocol_mapping_feature(&state, None)
            .await
            .expect("reload feature"),
        json!({
            "enabled": true,
            "availability": {
                "enabled": true,
                "start_time": "22:00",
                "end_time": "06:00"
            }
        })
    );
}

#[tokio::test]
async fn protocol_mapping_enable_rejects_local_port_loops_without_losing_config() {
    let (_directory, state) = fpk_lite_runtime_test_state().await;
    let mappings = json!([{
        "protocol": "tcp",
        "listen_port": 5555,
        "target": "127.0.0.1:5555",
        "use_auth": true,
        "comment": "Needs repair"
    }]);
    let mut config = state.storage.store.get_config().await.expect("load config");
    config
        .as_object_mut()
        .expect("config object")
        .insert("stream_mappings".to_string(), mappings.clone());
    state
        .storage
        .store
        .save_config(&config)
        .await
        .expect("save config");
    save_protocol_mapping_feature(&state, &json!({ "enabled": false }))
        .await
        .expect("disable protocol mapping feature");

    let response =
        update_protocol_mapping_feature(State(state.clone()), Json(json!({ "enabled": true })))
            .await;

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let response_body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read response body");
    let response_json: Value = serde_json::from_slice(&response_body).expect("parse response body");
    assert_eq!(
        response_json.get("message"),
        Some(&json!(
            "TCP 监听端口 5555 不能转发到本机同一端口（127.0.0.1:5555），否则会形成循环；请修改对外端口或目标端口"
        ))
    );
    assert_eq!(
        state
            .storage
            .store
            .get_config()
            .await
            .expect("reload config")
            .get("stream_mappings"),
        Some(&mappings)
    );
    assert_eq!(
        load_protocol_mapping_feature(&state, None)
            .await
            .expect("reload feature"),
        json!({
            "enabled": false,
            "availability": null,
            "runtime_issue": {
                "code": "local_port_loop",
                "message": "Stream mapping TCP listen_port 5555 cannot target the same local port 127.0.0.1:5555",
                "protocol": "tcp",
                "listen_port": 5555,
                "target": "127.0.0.1:5555"
            }
        })
    );
}

#[tokio::test]
async fn protocol_mapping_feature_rejects_invalid_availability_before_mutation() {
    let (_directory, state) = fpk_lite_runtime_test_state().await;
    save_protocol_mapping_feature(
        &state,
        &json!({
            "enabled": false,
            "availability": {
                "enabled": true,
                "start_time": "09:00",
                "end_time": "18:00"
            }
        }),
    )
    .await
    .expect("save initial protocol mapping schedule");

    let response = update_protocol_mapping_feature(
        State(state.clone()),
        Json(json!({
            "availability": {
                "enabled": true,
                "start_time": "09:00",
                "end_time": "09:00"
            }
        })),
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        load_protocol_mapping_feature(&state, None)
            .await
            .expect("reload feature"),
        json!({
            "enabled": false,
            "availability": {
                "enabled": true,
                "start_time": "09:00",
                "end_time": "18:00"
            }
        })
    );
}

#[tokio::test]
async fn protocol_mapping_feature_rejects_invalid_patch_shapes_before_mutation() {
    let (_directory, state) = fpk_lite_runtime_test_state().await;
    let initial = json!({
        "enabled": false,
        "availability": {
            "enabled": true,
            "start_time": "09:00",
            "end_time": "18:00"
        }
    });
    save_protocol_mapping_feature(&state, &initial)
        .await
        .expect("save initial protocol mapping schedule");

    for patch in [
        json!([]),
        json!({ "enabled": "true" }),
        json!({ "availability": [] }),
        json!({ "availability": { "enabled": false } }),
        json!({
            "runtime_issue": {
                "code": "runtime_sync_failed",
                "message": "client-controlled"
            }
        }),
    ] {
        let response = update_protocol_mapping_feature(State(state.clone()), Json(patch)).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            load_protocol_mapping_feature(&state, None)
                .await
                .expect("reload feature"),
            initial
        );
    }
}

#[tokio::test]
async fn protocol_mapping_feature_update_waits_for_the_shared_transaction_lock() {
    let (_directory, state) = fpk_lite_runtime_test_state().await;
    let mut config = state.storage.store.get_config().await.expect("load config");
    config["run_type"] = json!(0);
    state
        .storage
        .store
        .save_config(&config)
        .await
        .expect("save config");

    let guard = state.gateway.protocol_mapping_update_lock.lock().await;
    let task_state = state.clone();
    let mut task = tokio::spawn(async move {
        update_protocol_mapping_feature(State(task_state), Json(json!({ "enabled": true }))).await
    });

    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(50), &mut task)
            .await
            .is_err(),
        "feature update must wait while the shared transaction lock is held"
    );
    drop(guard);
    let response = tokio::time::timeout(std::time::Duration::from_secs(1), task)
        .await
        .expect("feature update should finish after releasing the lock")
        .expect("feature update task");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn firewall_additional_ports_successfully_save_apply_and_clear() {
    let (_directory, state) = fpk_lite_runtime_test_state().await;
    let mut config = state.storage.store.get_config().await.expect("load config");
    config["run_type"] = json!(0);
    config["auto_manage_firewall"] = json!(false);
    config["firewall_additional_ports"] = json!([1234]);
    state
        .storage
        .store
        .save_config(&config)
        .await
        .expect("save config");

    let response = update_firewall_additional_ports_transaction_with_reset(
        &state,
        vec![53, 5666],
        &SuccessfulFirewallReset,
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(
        body.pointer("/data/additionalPorts"),
        Some(&json!([53, 5666]))
    );
    assert_eq!(body.pointer("/data/runType"), Some(&json!(0)));
    assert_eq!(body.pointer("/data/appliedNow"), Some(&json!(true)));
    assert!(
        body.pointer("/data/effectivePorts")
            .and_then(Value::as_array)
            .is_some_and(|ports| ports.contains(&json!(gateway_port()))
                && ports.contains(&json!(53))
                && ports.contains(&json!(5666)))
    );
    assert_eq!(
        state
            .storage
            .store
            .get_config()
            .await
            .expect("reload config")
            .get("firewall_additional_ports"),
        Some(&json!([53, 5666]))
    );

    let response = update_firewall_additional_ports_transaction_with_reset(
        &state,
        Vec::new(),
        &SuccessfulFirewallReset,
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body.pointer("/data/additionalPorts"), Some(&json!([])));
    assert_eq!(
        state
            .storage
            .store
            .get_config()
            .await
            .expect("reload config")
            .get("firewall_additional_ports"),
        Some(&json!([]))
    );
}

#[tokio::test]
async fn firewall_additional_ports_failure_restores_rules_without_overwriting_other_config() {
    let (_directory, state) = fpk_lite_runtime_test_state().await;
    let mut config = state.storage.store.get_config().await.expect("load config");
    config["firewall_additional_ports"] = json!([1234]);
    config["default_route"] = json!("/before");
    state
        .storage
        .store
        .save_config(&config)
        .await
        .expect("save config");

    let reset = FailingThenSuccessfulFirewallReset {
        attempts: std::sync::atomic::AtomicUsize::new(0),
    };

    let response =
        update_firewall_additional_ports_transaction_with_reset(&state, vec![5666], &reset).await;

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    assert_eq!(reset.attempts.load(std::sync::atomic::Ordering::SeqCst), 2);
    assert_eq!(
        state
            .storage
            .store
            .get_config()
            .await
            .expect("reload config")
            .get("firewall_additional_ports"),
        Some(&json!([1234]))
    );
    assert_eq!(
        state
            .storage
            .store
            .get_config()
            .await
            .expect("reload concurrent config")
            .get("default_route"),
        Some(&json!("/concurrent"))
    );
    assert!(
        response_json(response)
            .await
            .get("message")
            .and_then(Value::as_str)
            .is_some_and(|message| message.contains("已恢复之前的配置和防火墙"))
    );
}

#[tokio::test]
async fn firewall_additional_ports_wait_for_the_shared_transaction_lock() {
    let (_directory, state) = fpk_lite_runtime_test_state().await;
    let guard = state.gateway.protocol_mapping_update_lock.lock().await;
    let task_state = state.clone();
    let mut task = tokio::spawn(async move {
        update_firewall_additional_ports_transaction(&task_state, vec![5666]).await
    });

    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(50), &mut task)
            .await
            .is_err(),
        "firewall port update must wait while the shared transaction lock is held"
    );
    drop(guard);
    let response = tokio::time::timeout(std::time::Duration::from_secs(1), task)
        .await
        .expect("firewall port update should finish after releasing the lock")
        .expect("firewall port update task");
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
}

#[tokio::test]
async fn manual_reset_and_clear_wait_for_the_shared_transaction_lock() {
    let (_directory, state) = fpk_lite_runtime_test_state().await;

    let guard = state.gateway.protocol_mapping_update_lock.lock().await;
    let reset_state = state.clone();
    let mut reset_task =
        tokio::spawn(async move { reset_firewall_with_transaction_lock(&reset_state, 1).await });
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(50), &mut reset_task)
            .await
            .is_err(),
        "manual reset must wait while the shared transaction lock is held"
    );
    drop(guard);
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(1), reset_task)
            .await
            .expect("manual reset should finish after releasing the lock")
            .expect("manual reset task")
            .is_err()
    );

    let guard = state.gateway.protocol_mapping_update_lock.lock().await;
    let clear_state = state.clone();
    let mut clear_task =
        tokio::spawn(async move { clear_firewall_with_transaction_lock(&clear_state).await });
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(50), &mut clear_task)
            .await
            .is_err(),
        "manual clear must wait while the shared transaction lock is held"
    );
    drop(guard);
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(1), clear_task)
            .await
            .expect("manual clear should finish after releasing the lock")
            .expect("manual clear task")
            .is_err()
    );
}

#[test]
fn builds_proxy_protocol_force_payload_from_go_envelopes() {
    assert_eq!(
        proxy_protocol_force_payload(
            &json!({ "success": true, "data": { "proxy_protocol_force": true } }),
            false
        ),
        json!({ "proxy_protocol_force": true })
    );
    assert_eq!(
        proxy_protocol_force_payload(&json!({ "proxy_protocol_force": false }), true),
        json!({ "proxy_protocol_force": false })
    );
    assert_eq!(
        proxy_protocol_force_payload(&json!({ "success": true }), true),
        json!({ "proxy_protocol_force": true })
    );
}

#[test]
fn normalizes_terminal_feature_like_node() {
    assert_eq!(
        normalize_terminal_feature(Some(&json!({
            "enabled": true,
            "default_cwd": "",
            "max_sessions": 20,
            "idle_timeout_seconds": 30,
            "allow_mobile_toolbar": false
        }))),
        json!({
            "enabled": true,
            "default_cwd": "~",
            "max_sessions": 12,
            "idle_timeout_seconds": 60,
            "resume_backend": "tmux",
            "allow_mobile_toolbar": false,
            "dangerously_run_as_current_user": true,
        })
    );
    assert_eq!(
        normalize_terminal_feature(Some(&json!({
            "default_cwd": "/usr/local/etc/fn-knock/"
        })))
        .get("default_cwd"),
        Some(&json!("~"))
    );
}

#[test]
fn wol_feature_defaults_disabled_and_accepts_only_boolean_enablement() {
    assert_eq!(normalize_wol_feature(None), json!({ "enabled": false }));
    assert_eq!(
        normalize_wol_feature(Some(&json!({ "enabled": true }))),
        json!({ "enabled": true })
    );
    assert_eq!(
        normalize_wol_feature(Some(&json!({ "enabled": "true" }))),
        json!({ "enabled": false })
    );
}

#[test]
fn normalizes_gateway_logging_like_node_parse_int_without_upper_cap() {
    assert_eq!(
        normalize_gateway_logging(Some(&json!({
            "enabled": true,
            "record_localhost": true,
            "max_days": "2x",
        }))),
        json!({ "enabled": true, "record_localhost": true, "max_days": 2 })
    );
    assert_eq!(
        normalize_gateway_logging(None),
        json!({ "enabled": false, "record_localhost": false, "max_days": 7 })
    );
    assert_eq!(
        normalize_gateway_logging(Some(&json!({
            "max_days": 3.9,
        })))
        .get("max_days"),
        Some(&json!(3))
    );
    assert_eq!(
        normalize_gateway_logging(Some(&json!({
            "max_days": ["4x"],
        })))
        .get("max_days"),
        Some(&json!(4))
    );
    assert_eq!(
        normalize_gateway_logging(Some(&json!({
            "max_days": 999,
        })))
        .get("max_days"),
        Some(&json!(999))
    );
}

#[test]
fn normalizes_captcha_settings() {
    assert_eq!(
        normalize_captcha_settings(Some(&json!({
            "provider": "turnstile",
            "turnstile": { "site_key": " site ", "secret_key": " secret " }
        }))),
        json!({
            "provider": "turnstile",
            "widget_mode": "normal",
            "pow": {
                "base_max_number": 100000,
                "uncommon_location": { "enabled": false, "max_number": 300000 }
            },
            "turnstile": { "site_key": "site", "secret_key": "secret" }
        })
    );
}

#[test]
fn normalizes_pow_difficulty_and_repairs_legacy_values() {
    assert_eq!(
        normalize_captcha_settings(Some(&json!({
            "provider": "pow",
            "pow": {
                "base_max_number": 250000,
                "uncommon_location": { "enabled": true, "max_number": 200000 }
            }
        }))),
        json!({
            "provider": "pow",
            "widget_mode": "normal",
            "pow": {
                "base_max_number": 250000,
                "uncommon_location": { "enabled": true, "max_number": 300000 }
            },
            "turnstile": { "site_key": "", "secret_key": "" }
        })
    );

    let normalized = normalize_captcha_settings(Some(&json!({
        "pow": {
            "base_max_number": 9999,
            "uncommon_location": { "enabled": "yes", "max_number": 1000001 }
        }
    })));
    assert_eq!(normalized["pow"]["base_max_number"], 100000);
    assert_eq!(normalized["pow"]["uncommon_location"]["enabled"], false);
    assert_eq!(normalized["pow"]["uncommon_location"]["max_number"], 300000);

    let valid_custom = normalize_captcha_settings(Some(&json!({
        "pow": {
            "base_max_number": 100000,
            "uncommon_location": { "max_number": 200000 }
        }
    })));
    assert_eq!(
        valid_custom["pow"]["uncommon_location"]["max_number"],
        200000
    );

    let invalid_steps = normalize_captcha_settings(Some(&json!({
        "pow": {
            "base_max_number": 15000,
            "uncommon_location": { "max_number": 305000 }
        }
    })));
    assert_eq!(invalid_steps["pow"]["base_max_number"], 100000);
    assert_eq!(
        invalid_steps["pow"]["uncommon_location"]["max_number"],
        300000
    );
}

#[test]
fn validates_pow_difficulty_patch() {
    let current = normalize_captcha_settings(None);
    assert!(validate_pow_captcha_patch(&current, &json!({})).is_ok());
    assert!(
        validate_pow_captcha_patch(
            &current,
            &json!({"pow": {"base_max_number": 10000, "uncommon_location": {"max_number": 1000000}}})
        )
        .is_ok()
    );
    assert!(
        validate_pow_captcha_patch(
            &current,
            &json!({"pow": {"base_max_number": 200000, "uncommon_location": {"enabled": true, "max_number": 400000}}})
        )
        .is_ok()
    );
    assert_eq!(
        validate_pow_captcha_patch(&current, &json!({"pow": {"base_max_number": 400000}})),
        Err("captcha.powUncommonDifficultyTooLow")
    );
    assert_eq!(
        validate_pow_captcha_patch(&current, &json!({"pow": {"base_max_number": 10000.5}})),
        Err("captcha.powDifficultyInvalid")
    );
    assert_eq!(
        validate_pow_captcha_patch(&current, &json!({"pow": {"base_max_number": 15000}})),
        Err("captcha.powDifficultyInvalid")
    );
    assert_eq!(
        validate_pow_captcha_patch(&current, &json!({"pow": {"uncommon_location": []}})),
        Err("captcha.powDifficultyInvalid")
    );
}

#[tokio::test]
async fn captcha_updates_deep_merge_provider_subconfigs() {
    let (_directory, state) = fpk_lite_runtime_test_state().await;
    state
        .storage
        .store
        .set_json_value(
            CAPTCHA_SETTINGS_KEY,
            &json!({
                "provider": "turnstile",
                "pow": {
                    "base_max_number": 100000,
                    "uncommon_location": { "enabled": false, "max_number": 300000 }
                },
                "turnstile": { "site_key": "site", "secret_key": "secret" }
            }),
        )
        .await
        .expect("seed captcha settings");

    let updated = update_captcha_settings(
        &state,
        &json!({
            "provider": "pow",
            "pow": { "uncommon_location": { "enabled": true } }
        }),
    )
    .await
    .expect("update nested PoW config");
    assert_eq!(updated["pow"]["base_max_number"], 100000);
    assert_eq!(updated["pow"]["uncommon_location"]["enabled"], true);
    assert_eq!(updated["pow"]["uncommon_location"]["max_number"], 300000);
    assert_eq!(updated["turnstile"]["site_key"], "site");
    assert_eq!(updated["turnstile"]["secret_key"], "secret");

    let updated =
        update_captcha_settings(&state, &json!({ "turnstile": { "site_key": "new-site" } }))
            .await
            .expect("update nested Turnstile config");
    assert_eq!(updated["turnstile"]["site_key"], "new-site");
    assert_eq!(updated["turnstile"]["secret_key"], "secret");
    assert_eq!(updated["pow"]["uncommon_location"]["enabled"], true);
}

#[tokio::test]
async fn captcha_update_waits_for_the_shared_transaction_lock() {
    let (_directory, state) = fpk_lite_runtime_test_state().await;
    let guard = state.security.captcha_settings_update_lock.lock().await;
    let task_state = state.clone();
    let mut task = tokio::spawn(async move {
        update_captcha(
            State(task_state),
            Json(json!({
                "provider": "pow",
                "pow": {
                    "base_max_number": 100000,
                    "uncommon_location": { "enabled": false, "max_number": 300000 }
                }
            })),
        )
        .await
    });

    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(50), &mut task)
            .await
            .is_err(),
        "captcha update must wait while the shared transaction lock is held"
    );
    drop(guard);
    let response = tokio::time::timeout(std::time::Duration::from_secs(1), task)
        .await
        .expect("captcha update should finish after releasing the lock")
        .expect("captcha update task");
    assert_eq!(response.status(), StatusCode::OK);
}

#[test]
fn normalizes_fnos_share_bypass_bounds() {
    assert_eq!(
        normalize_fnos_share_bypass(Some(&json!({
            "enabled": true,
            "upstream_timeout_ms": 100,
            "session_ttl_seconds": 99999
        }))),
        json!({
            "enabled": true,
            "upstream_timeout_ms": 500,
            "validation_cache_ttl_seconds": 30,
            "validation_lock_ttl_seconds": 5,
            "session_ttl_seconds": 3600,
        })
    );
}

#[test]
fn normalizes_fnos_port_icon_hijack_like_node() {
    assert_eq!(
        normalize_fnos_port_icon_hijack(Some(&json!({
            "enabled": true,
            "updated_at": " 2026-07-05T01:02:03.000Z "
        }))),
        json!({
            "enabled": true,
            "updated_at": "2026-07-05T01:02:03.000Z"
        })
    );
    assert_eq!(
        normalize_fnos_port_icon_hijack(Some(&json!({
            "enabled": false,
            "updated_at": ""
        }))),
        json!({
            "enabled": false,
            "updated_at": Value::Null
        })
    );
}

#[test]
fn normalizes_auto_manage_firewall_like_node() {
    assert!(normalize_auto_manage_firewall(Some(&json!(true))));
    assert!(!normalize_auto_manage_firewall(Some(&json!(false))));
    assert!(normalize_auto_manage_firewall(Some(&json!("false"))));
    assert!(normalize_auto_manage_firewall(None));
}

#[test]
fn validates_and_normalizes_firewall_additional_ports() {
    assert_eq!(normalize_firewall_additional_ports(None), Vec::<i64>::new());
    assert_eq!(
        normalize_firewall_additional_ports(Some(&json!([5666, 53, 5666, 0, 65536, "7999"]))),
        vec![53, 5666]
    );
    assert_eq!(
        parse_firewall_additional_ports(&json!({ "ports": [5666, 53, 5666] })),
        Ok(vec![53, 5666])
    );
    assert_eq!(
        parse_firewall_additional_ports(&json!({ "ports": ["5666"] })),
        Err("portIntegerRequired")
    );
    assert_eq!(
        parse_firewall_additional_ports(&json!({ "ports": [0] })),
        Err("portOutOfRange")
    );
    assert_eq!(
        parse_firewall_additional_ports(
            &json!({ "ports": (1..=MAX_FIREWALL_ADDITIONAL_PORTS + 1).collect::<Vec<_>>() })
        ),
        Err("tooManyPorts")
    );
}

#[test]
fn normalizes_runtime_mode_feature_configs_like_node() {
    assert_eq!(normalize_run_type(Some(&json!(0))), Some(0));
    assert_eq!(normalize_run_type(Some(&json!(1))), Some(1));
    assert_eq!(normalize_run_type(Some(&json!(3))), Some(3));
    assert_eq!(normalize_run_type(Some(&json!(2))), None);
    assert_eq!(
        normalize_protocol_mapping_feature(Some(&json!({ "enabled": true }))),
        json!({ "enabled": true, "availability": null })
    );
    assert_eq!(
        normalize_protocol_mapping_feature(Some(&json!({
            "enabled": true,
            "availability": {
                "enabled": true,
                "start_time": " 22:00 ",
                "end_time": "06:00"
            }
        }))),
        json!({
            "enabled": true,
            "availability": {
                "enabled": true,
                "start_time": "22:00",
                "end_time": "06:00"
            }
        })
    );
    assert_eq!(
        normalize_protocol_mapping_feature(Some(&json!({
            "enabled": true,
            "availability": {
                "enabled": true,
                "start_time": "invalid",
                "end_time": "06:00"
            }
        }))),
        json!({ "enabled": true, "availability": null })
    );
    assert_eq!(
        normalize_protocol_mapping_feature(Some(&json!({
            "enabled": false,
            "runtime_issue": {
                "code": "listen_port_in_use",
                "message": "  listen tcp :9000: bind: address already in use  ",
                "protocol": "TCP",
                "listen_port": 9000,
                "target": " 127.0.0.1:9001 "
            }
        }))),
        json!({
            "enabled": false,
            "availability": null,
            "runtime_issue": {
                "code": "listen_port_in_use",
                "message": "listen tcp :9000: bind: address already in use",
                "protocol": "tcp",
                "listen_port": 9000,
                "target": "127.0.0.1:9001"
            }
        })
    );
    assert_eq!(
        normalize_smart_connect_config(Some(&json!({
            "enabled": true,
            "selected_ipv4": " 192.168.1.20 "
        }))),
        json!({ "enabled": true, "selected_ipv4": "192.168.1.20" })
    );
}

#[test]
fn smart_connect_domains_prioritize_auth_and_dedupe_hosts() {
    let config = json!({
        "host_mappings": [
            { "host": "app.example.com", "service_role": "app" },
            { "host": "https://AUTH.example.com/path", "service_role": "auth" },
            { "host": "app.example.com.", "service_role": "app" }
        ]
    });

    assert_eq!(
        list_smart_connect_domains(&config),
        vec![
            "auth.example.com".to_string(),
            "app.example.com".to_string()
        ]
    );
}

#[test]
fn smart_connect_host_normalizer_strips_only_alpha_scheme_like_node() {
    assert_eq!(normalize_host("HTTP://Example.COM./path"), "example.com");
    assert_eq!(normalize_host("1://Example.COM/path"), "1:");
}

#[test]
fn builds_smart_connect_managed_config_like_node() {
    let config = build_smart_connect_managed_config(
        " 192.168.1.20 ",
        &[
            "Beta.Example.com".to_string(),
            "alpha.example.com".to_string(),
            "beta.example.com".to_string(),
        ],
    );

    assert_eq!(
        config,
        [
            "# Managed by fn-knock smart connect. Do not edit manually.",
            "local-ttl=30",
            "listen-address=127.0.0.1,192.168.1.20",
            "bind-interfaces",
            "address=/beta.example.com/192.168.1.20",
            "local=/beta.example.com/",
            "address=/alpha.example.com/192.168.1.20",
            "local=/alpha.example.com/",
            "",
        ]
        .join("\n")
    );
}

#[test]
fn smart_connect_cleanup_deactivates_dnsmasq_even_without_managed_config() {
    let directory = tempfile::tempdir().unwrap();
    let managed_config = directory.path().join("missing-smart-connect.conf");
    let calls = std::cell::Cell::new(0);

    clear_smart_connect_managed_config_at(&managed_config, || {
        calls.set(calls.get() + 1);
        Ok(())
    })
    .expect("deactivate dnsmasq without managed config");

    assert_eq!(calls.get(), 1);
    assert!(!managed_config.exists());
}

#[test]
fn smart_connect_cleanup_removes_managed_config_and_propagates_deactivation_failure() {
    let directory = tempfile::tempdir().unwrap();
    let managed_config = directory.path().join("fn-knock-smart-connect.conf");
    std::fs::write(&managed_config, "address=/example.com/192.168.1.20\n").unwrap();

    let error = clear_smart_connect_managed_config_at(&managed_config, || {
        Err("failed to disable dnsmasq on boot: permission denied".to_string())
    })
    .expect_err("propagate dnsmasq deactivation failure");

    assert_eq!(
        error,
        "failed to disable dnsmasq on boot: permission denied"
    );
    assert!(!managed_config.exists());
}

#[test]
fn gateway_port_matches_node_parse_int_fallback() {
    assert_eq!(gateway_port_from_env(None), 7999);
    assert_eq!(gateway_port_from_env(Some(String::new())), 7999);
    assert_eq!(gateway_port_from_env(Some("   ".to_string())), 7999);
    assert_eq!(gateway_port_from_env(Some(" 8000x ".to_string())), 8000);
    assert_eq!(gateway_port_from_env(Some("0x10".to_string())), 7999);
}

#[test]
fn firewall_exempt_ports_include_stream_and_smart_connect_ports() {
    let config = json!({
        "firewall_additional_ports": [5666, 2222, 70000],
        "smart_connect": { "enabled": true, "selected_ipv4": "192.168.1.20" },
        "stream_mappings": [
            { "listen_port": 2222 },
            { "listen_port": 70000 }
        ]
    });
    let ports = exempt_ports(&config, true, 3);

    assert!(ports.contains(&gateway_port().to_string()));
    assert!(ports.contains(&"2222".to_string()));
    assert!(ports.contains(&"53".to_string()));
    assert!(ports.contains(&"5666".to_string()));
    assert!(!ports.contains(&"70000".to_string()));

    let disabled_ports = exempt_ports(&config, false, 3);
    assert!(disabled_ports.contains(&gateway_port().to_string()));
    assert!(disabled_ports.contains(&"53".to_string()));
    assert!(disabled_ports.contains(&"2222".to_string()));
    assert!(disabled_ports.contains(&"5666".to_string()));

    let direct_ports = exempt_ports(&config, true, 0);
    assert!(direct_ports.contains(&gateway_port().to_string()));
    assert!(direct_ports.contains(&"2222".to_string()));
    assert!(direct_ports.contains(&"5666".to_string()));
    assert!(!direct_ports.contains(&"53".to_string()));
    assert!(exempt_ports(&config, true, 1).is_empty());
}

#[test]
fn direct_mode_auth_entry_route_matches_node_payload() {
    assert_eq!(
        auth_entry_route_payload(7997),
        json!([{
            "path": "/auth",
            "target": "http://127.0.0.1:7997",
            "rewrite_html": false,
            "use_auth": false,
            "use_root_mode": false,
            "strip_path": false,
        }])
    );
}

#[test]
fn fpk_lite_retargets_legacy_auth_service_without_touching_unrelated_mappings() {
    let mut config = json!({
        "subdomain_mode": {
            "auth_host": "auth.example.com",
            "auth_target": "http://127.0.0.1:7997"
        },
        "host_mappings": [
            {
                "host": "auth.example.com",
                "target": "http://localhost:7997",
                "service_role": "auth"
            },
            {
                "host": "full.example.com",
                "target": "http://127.0.0.1:7997",
                "service_role": "app"
            }
        ]
    });

    assert!(retarget_fpk_lite_auth_service(
        &mut config,
        "http://127.0.0.1:8997"
    ));
    assert_eq!(
        config.pointer("/subdomain_mode/auth_target"),
        Some(&json!("http://127.0.0.1:8997"))
    );
    assert_eq!(
        config.pointer("/host_mappings/0/target"),
        Some(&json!("http://127.0.0.1:8997"))
    );
    assert_eq!(
        config.pointer("/host_mappings/1/target"),
        Some(&json!("http://127.0.0.1:7997"))
    );
}

#[test]
fn normalizes_fnos_network_tuning_like_node() {
    assert_eq!(
        normalize_fnos_network_tuning(Some(&json!({
            "bbr_enabled": true,
            "mtu_probing_enabled": true,
            "previous_tcp_congestion_control": " cubic ",
            "previous_default_qdisc": "",
            "previous_tcp_mtu_probing": "0",
            "updated_at": " now ",
            "last_error": ""
        }))),
        json!({
            "bbr_enabled": true,
            "mtu_probing_enabled": true,
            "previous_tcp_congestion_control": "cubic",
            "previous_default_qdisc": Value::Null,
            "previous_tcp_mtu_probing": "0",
            "updated_at": "now",
            "last_error": Value::Null,
        })
    );
}

#[test]
fn normalizes_fnos_network_tuning_from_camel_case_persisted_config() {
    assert_eq!(
        normalize_fnos_network_tuning(Some(&json!({
            "bbrEnabled": true,
            "mtuProbingEnabled": true,
            "previousTcpCongestionControl": " cubic ",
            "previousDefaultQdisc": " fq_codel ",
            "previousTcpMtuProbing": "0",
            "updatedAt": " now ",
            "lastError": " failed "
        }))),
        json!({
            "bbr_enabled": true,
            "mtu_probing_enabled": true,
            "previous_tcp_congestion_control": "cubic",
            "previous_default_qdisc": "fq_codel",
            "previous_tcp_mtu_probing": "0",
            "updated_at": "now",
            "last_error": "failed",
        })
    );
}

#[test]
fn fnos_network_tuning_disable_keeps_previous_runtime_values() {
    let previous = normalize_fnos_network_tuning(Some(&json!({
        "bbr_enabled": true,
        "mtu_probing_enabled": true,
        "previous_tcp_congestion_control": "cubic",
        "previous_default_qdisc": "fq_codel",
        "previous_tcp_mtu_probing": "0",
    })));
    let before = json!({
        "tcp_congestion_control": "bbr",
        "default_qdisc": "fq",
        "tcp_mtu_probing": "1",
    });
    let next = build_next_fnos_network_tuning_config(
        &previous,
        &json!({
            "bbr_enabled": false,
            "mtu_probing_enabled": false,
        }),
        &before,
    );

    assert_eq!(
        next.get("bbr_enabled").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        next.get("mtu_probing_enabled").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        next.get("previous_tcp_congestion_control")
            .and_then(Value::as_str),
        Some("cubic")
    );
    assert_eq!(
        next.get("previous_default_qdisc").and_then(Value::as_str),
        Some("fq_codel")
    );
    assert_eq!(
        next.get("previous_tcp_mtu_probing").and_then(Value::as_str),
        Some("0")
    );
}

#[test]
fn fnos_network_tuning_disable_requires_zero_even_when_previous_was_one() {
    let translator = Translator::new("zh-CN");
    let config = normalize_fnos_network_tuning(Some(&json!({
        "bbr_enabled": false,
        "mtu_probing_enabled": false,
        "previous_tcp_mtu_probing": "1",
    })));
    let patch = json!({ "mtu_probing_enabled": false });
    let targets = FnosNetworkTuningTransitionTargets::default();

    assert!(
        verify_fnos_network_tuning_state(
            &config,
            &patch,
            &json!({ "tcp_mtu_probing": "0" }),
            &targets,
            &translator,
        )
        .is_ok()
    );
    assert!(
        verify_fnos_network_tuning_state(
            &config,
            &patch,
            &json!({ "tcp_mtu_probing": "1" }),
            &targets,
            &translator,
        )
        .is_err()
    );
}

#[test]
fn fnos_network_tuning_managed_config_writes_explicit_mtu_zero() {
    let config = normalize_fnos_network_tuning(Some(&json!({
        "bbr_enabled": true,
        "mtu_probing_enabled": false,
    })));
    let content = render_fnos_network_tuning_sysctl_config(&config).join("\n");

    assert!(content.contains("net.core.default_qdisc=fq"));
    assert!(content.contains("net.ipv4.tcp_congestion_control=bbr"));
    assert!(content.contains("net.ipv4.tcp_mtu_probing=0"));
}

#[test]
fn fnos_network_tuning_managed_config_removes_bbr_when_disabled() {
    let config = normalize_fnos_network_tuning(Some(&json!({
        "bbr_enabled": false,
        "mtu_probing_enabled": false,
    })));
    let content = render_fnos_network_tuning_sysctl_config(&config).join("\n");

    assert!(!content.contains("net.core.default_qdisc=fq"));
    assert!(!content.contains("net.ipv4.tcp_congestion_control=bbr"));
    assert!(content.contains("net.ipv4.tcp_mtu_probing=0"));
}

#[test]
fn fnos_network_tuning_success_clears_previous_last_error_like_node() {
    let mut next = normalize_fnos_network_tuning(Some(&json!({
        "bbr_enabled": true,
        "mtu_probing_enabled": false,
        "last_error": "previous failure"
    })));

    clear_fnos_network_tuning_last_error(&mut next);

    assert_eq!(next.get("last_error"), Some(&Value::Null));
}

#[test]
fn fnos_network_tuning_import_failure_restores_local_desired_state() {
    let previous = normalize_fnos_network_tuning(Some(&json!({
        "bbr_enabled": false,
        "mtu_probing_enabled": true,
        "last_error": null
    })));
    let failed = build_fnos_network_tuning_import_failure(&previous, "apply failed");

    assert_eq!(failed["bbr_enabled"], json!(false));
    assert_eq!(failed["mtu_probing_enabled"], json!(true));
    assert_eq!(failed["last_error"], json!("apply failed"));
    assert!(failed["updated_at"].as_str().is_some());
}

#[test]
fn fnos_network_tuning_mtu_active_semantics_match_node() {
    assert!(fnos_mtu_probing_active(Some("1")));
    assert!(!fnos_mtu_probing_active(Some("0")));
    assert!(!fnos_mtu_probing_active(Some("2")));
    assert!(!fnos_mtu_probing_active(None));
}

#[test]
fn fnos_network_tuning_module_loaded_reads_proc_modules_like_node() {
    assert!(bbr_module_loaded_from_proc_modules(
        "tcp_bbr 20480 0 - Live 0\nveth 32768 0 - Live 0\n"
    ));
    assert!(!bbr_module_loaded_from_proc_modules(
        "tcp_cubic 20480 1 - Live 0\ntcp_bbr_extra 20480 0 - Live 0\n"
    ));
}

#[test]
fn fnos_network_tuning_available_depends_on_runtime_block_only() {
    assert!(fnos_network_tuning_available(None));
    assert!(!fnos_network_tuning_available(Some("deployment")));
    assert!(!fnos_network_tuning_available(Some("platform")));
    assert!(!fnos_network_tuning_available(Some("permission")));
}

#[test]
fn fnos_network_tuning_patch_accepts_camel_case_aliases() {
    let translator = Translator::new("zh-CN");
    assert_eq!(
        normalize_fnos_network_tuning_patch(
            &json!({
                "bbrEnabled": true,
                "mtuProbingEnabled": false,
            }),
            &translator,
        )
        .expect("normalized patch"),
        json!({
            "bbr_enabled": true,
            "mtu_probing_enabled": false,
        })
    );
}

#[test]
fn fnos_network_tuning_patch_rejects_empty_payload() {
    let translator = Translator::new("zh-CN");
    assert_eq!(
        normalize_fnos_network_tuning_patch(&json!({ "enabled": true }), &translator)
            .expect_err("empty patch should fail"),
        "请至少修改一个飞牛 FPK 网络优化选项"
    );
}

#[test]
fn fnos_network_tuning_import_only_applies_changed_switches() {
    let previous = json!({
        "bbr_enabled": true,
        "mtu_probing_enabled": false,
    });
    let next = json!({
        "bbr_enabled": true,
        "mtu_probing_enabled": true,
    });
    assert_eq!(
        fnos_network_tuning_import_patch(&previous, &next),
        json!({ "mtu_probing_enabled": true })
    );
    assert_eq!(fnos_network_tuning_import_patch(&next, &next), json!({}));
}

#[test]
fn localizes_runtime_config_route_and_fnos_network_errors() {
    let zh = Translator::new("zh-CN");
    assert_eq!(
        runtime_config_route_text(&zh, "loadAutoHttpsFailed"),
        "加载自动 HTTPS 配置失败"
    );
    assert_eq!(
        localize_runtime_config_error(&zh, GO_BACKEND_UNSUCCESSFUL_RESPONSE),
        "上游服务不可用"
    );
    assert_eq!(
        localize_runtime_config_error(
            &zh,
            r#"go backend gRPC request failed: {"message":"failed to set stream rules: cannot target the same local listen_port 5555"}"#
        ),
        "监听端口 5555 不能转发到本机同一端口，否则会形成循环；请进入协议映射修改对外端口或目标端口"
    );
    assert_eq!(
        admin_text_params(
            &zh,
            "fnosNetworkTuning.errors.setSysctlFailed",
            &[("key", "net.ipv4.tcp_mtu_probing".to_string())],
        ),
        "设置 net.ipv4.tcp_mtu_probing 失败"
    );
}
