use super::*;
use serde_json::json;

fn test_settings(directory: &std::path::Path, target: &str) -> Settings {
    let mut settings = Settings::from_env();
    settings.runtime_target = target.to_string();
    settings.data_dir = directory.join("data");
    settings.gateway_config_dir = directory.join("gateway");
    settings.waf_dir = directory.join("waf");
    settings.sqlite_path = directory.join("fn-knock.sqlite3");
    settings.admin_static_path = directory.join("admin");
    settings.auth_static_path = directory.join("auth");
    settings.go_backend_grpc_addr = "127.0.0.1:1".to_string();
    settings.internal_rpc_token = "sqlite-startup-test".to_string();
    settings
}

#[tokio::test]
async fn startup_preserves_sqlite_data_with_historical_redis_migration_markers() {
    // Interrupted/unavailable old imports must never make startup replace
    // existing SQLite data. Cover every old status and an absent marker.
    for target in ["fpk", "fpk-lite", "docker", "synology", "linux"] {
        for status in [
            None,
            Some("unavailable"),
            Some("running"),
            Some("failed"),
            Some("imported"),
            Some("done"),
        ] {
            let directory = tempfile::tempdir().unwrap();
            let settings = test_settings(directory.path(), target);
            let store = Store::connect(&settings.sqlite_path).await.unwrap();
            let config = json!({
                "locale": {"default_locale": "en"},
                "host_mappings": [{"host": "app.example.test", "target": "http://127.0.0.1:8080"}],
                "ddns": {"enabled": true},
                "ssl": {"enabled": true}
            });
            store
                .set_json_value("fn_knock:config", &config)
                .await
                .unwrap();
            store
                .set_string_value("fn_knock:test:account", "existing-account")
                .await
                .unwrap();
            if let Some(status) = status {
                store
                    .set_storage_meta_value("redis_migration_status", status)
                    .await
                    .unwrap();
            }
            let expected_config = store.get_config().await.unwrap();
            let expected_keys = store.scan_keys("fn_knock:", 100).await.unwrap();
            drop(store);

            // Open the same database through the actual startup entry point
            // twice, covering both upgrade startup and subsequent restarts.
            for _ in 0..2 {
                let state = AppState::new(settings.clone()).await.unwrap();
                assert_eq!(
                    state.storage.store.get_config().await.unwrap(),
                    expected_config,
                    "target={target}, status={status:?}"
                );
                assert_eq!(*state.storage.store.config_snapshot(), expected_config);
                assert_eq!(
                    state
                        .storage
                        .store
                        .scan_keys("fn_knock:", 100)
                        .await
                        .unwrap(),
                    expected_keys
                );
                assert_eq!(
                    state
                        .storage
                        .store
                        .get_string_value("fn_knock:test:account")
                        .await
                        .unwrap()
                        .as_deref(),
                    Some("existing-account")
                );
                assert_eq!(
                    state
                        .storage
                        .store
                        .storage_meta_value("redis_migration_status")
                        .await
                        .unwrap()
                        .as_deref(),
                    status
                );
                state
                    .runtime_health
                    .shutdown_operational_log(Duration::from_secs(1))
                    .await;
            }
        }
    }
}

#[tokio::test]
async fn startup_initializes_fresh_sqlite_without_external_storage() {
    let directory = tempfile::tempdir().unwrap();
    let settings = test_settings(directory.path(), "fpk");
    let state = AppState::new(settings).await.unwrap();
    let config = state.storage.store.get_config().await.unwrap();
    assert_eq!(config["host_mappings"], json!([]));
    assert!(
        state
            .storage
            .store
            .storage_meta_value("redis_migration_status")
            .await
            .unwrap()
            .is_none()
    );
    state
        .runtime_health
        .shutdown_operational_log(Duration::from_secs(1))
        .await;
}

#[test]
fn startup_ignores_old_redis_environment_with_a_reachable_source() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let env = crate::test_support::EnvGuard::new(&[
        "FN_KNOCK_DISABLE_REDIS_MIGRATION",
        "FN_KNOCK_LEGACY_REDIS_URL",
        "FN_KNOCK_LEGACY_REDIS_CONNECT_TIMEOUT_MS",
        "FN_KNOCK_LEGACY_REDIS_COMMAND_TIMEOUT_MS",
        "REDIS_HOST",
        "REDIS_PORT",
        "REDIS_PASSWORD",
    ]);
    env.remove("FN_KNOCK_DISABLE_REDIS_MIGRATION");
    env.set("FN_KNOCK_LEGACY_REDIS_CONNECT_TIMEOUT_MS", "20");
    env.set("FN_KNOCK_LEGACY_REDIS_COMMAND_TIMEOUT_MS", "20");
    env.set("REDIS_HOST", "127.0.0.1");
    env.set("REDIS_PORT", address.port().to_string());
    env.set("REDIS_PASSWORD", "obsolete-password");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    // Both the explicit old URL and its REDIS_* fallback must be ignored.
    for explicit_url in [true, false] {
        if explicit_url {
            env.set("FN_KNOCK_LEGACY_REDIS_URL", format!("redis://{address}/"));
        } else {
            env.remove("FN_KNOCK_LEGACY_REDIS_URL");
        }
        let directory = tempfile::tempdir().unwrap();
        let settings = test_settings(directory.path(), "fpk");
        runtime.block_on(async {
            let state = AppState::new(settings).await.unwrap();
            assert!(matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock));
            assert!(state.storage.store.storage_meta_value("redis_migration_status").await.unwrap().is_none());
            assert!(state.runtime_health.shutdown_operational_log(Duration::from_secs(1)).await);
        });
    }
}
