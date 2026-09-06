use super::*;

#[tokio::test]
async fn drain_settings_preserve_defaults_and_retention_normalization() {
    let (_directory, state) = waf_test_state("http://127.0.0.1:1").await;
    for waf in [
        Value::Null,
        json!({}),
        json!({"enabled": true, "log_retention_days": "14"}),
        json!({"enabled": true, "log_retention_days": -1, "drain_interval_seconds": 0}),
        json!({"enabled": true, "log_retention_days": 999, "drain_interval_seconds": 999}),
        json!({"enabled": "true", "log_retention_days": "bad", "drain_interval_seconds": "9"}),
    ] {
        state
            .storage
            .store
            .set_config_top_level_value("waf", waf.clone())
            .await
            .unwrap();
        let settings = waf_drain_settings(&state);
        let normalized = load_waf_config(&state).await.unwrap();
        assert_eq!(json!(settings.enabled), normalized["enabled"]);
        assert_eq!(
            json!(settings.retention_days),
            normalized["log_retention_days"]
        );
        assert_eq!(
            json!(settings.interval_seconds),
            normalized["drain_interval_seconds"]
        );
    }
}

#[tokio::test]
async fn drain_reads_submit_no_sqlite_work_with_large_config() {
    let (_directory, state) = waf_test_state("http://127.0.0.1:1").await;
    let mut config = state.storage.store.get_config().await.unwrap();
    config["host_mappings"] = large_host_mappings();
    config["waf"] = json!({"enabled": true, "drain_interval_seconds": 2, "log_retention_days": 14});
    state.storage.store.replace_config(&config).await.unwrap();
    let recorder = state.storage.store.diagnostics();
    let generation = recorder.start();
    for _ in 0..100 {
        assert_eq!(waf_drain_schedule(&state), Some(2));
        assert_eq!(waf_drain_settings(&state).retention_days, 14);
    }
    recorder.stop(generation);
    assert!(recorder.snapshot().operations.is_empty());

    let generation = recorder.start();
    // Enabled drains reach the unavailable gateway without first submitting a
    // config read. The gateway failure must still propagate to the caller.
    assert!(drain_waf_events_now(&state).await.is_err());
    recorder.stop(generation);
    assert!(
        recorder
            .snapshot()
            .operations
            .iter()
            .all(|operation| operation.kind == "task")
    );

    state
        .storage
        .store
        .set_config_top_level_value("waf", json!({"enabled": false}))
        .await
        .unwrap();
    let generation = recorder.start();
    let result = drain_waf_events_now(&state).await.unwrap();
    recorder.stop(generation);
    assert_eq!(result["skipped_reason"], "waf_disabled");
    assert!(
        recorder
            .snapshot()
            .operations
            .iter()
            .all(|operation| operation.kind == "task")
    );
}

#[tokio::test]
async fn drain_settings_follow_migration_compatibility_restore_and_reopen() {
    let (_directory, state) = waf_test_state("http://127.0.0.1:1").await;
    let store = &state.storage.store;
    let expected = store.get_config().await.unwrap();
    let mut migrated = expected.clone();
    migrated["waf"] =
        json!({"enabled": true, "drain_interval_seconds": 17, "log_retention_days": 30});
    store
        .compare_and_set_config_migration(&expected, &migrated)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(waf_drain_schedule(&state), Some(17));
    assert_eq!(waf_drain_settings(&state).retention_days, 30);

    let restored =
        json!({"waf": {"enabled": true, "drain_interval_seconds": 5, "log_retention_days": 3}});
    store.replace_backup_entries_by_prefix("fn_knock:", &[json!({
        "key": "fn_knock:config", "type": "string", "ttl_ms": null, "value": restored.to_string()
    })], 200).await.unwrap();
    assert_eq!(waf_drain_schedule(&state), Some(5));
    assert_eq!(waf_drain_settings(&state).retention_days, 3);

    // A separately opened Store still performs its initialization reconciliation.
    let reopened = crate::store::Store::connect(store.path()).await.unwrap();
    assert_eq!(reopened.config_snapshot()["waf"], restored["waf"]);
    drop(reopened);

    store
        .set_json_value("fn_knock:config", &json!({"waf": {"enabled": false}}))
        .await
        .unwrap();
    assert_eq!(waf_drain_schedule(&state), None);
    store
        .set_string_value("fn_knock:config", &restored.to_string())
        .await
        .unwrap();
    assert_eq!(waf_drain_schedule(&state), Some(5));
    store.delete_key("fn_knock:config").await.unwrap();
    assert_eq!(waf_drain_schedule(&state), None);
}

#[tokio::test]
async fn drain_waiter_uses_settings_published_while_waiting_for_lock() {
    let (_directory, state) = waf_test_state("http://127.0.0.1:1").await;
    state
        .storage
        .store
        .set_config_top_level_value("waf", json!({"enabled": true}))
        .await
        .unwrap();
    let guard = state.security.waf_event_drain_lock.lock().await;
    let drain = drain_waf_events_now(&state);
    tokio::pin!(drain);
    // Poll until the drain is waiting for the held lock, without sleeps.
    assert!(
        std::future::poll_fn(|cx| std::task::Poll::Ready(drain.as_mut().poll(cx)))
            .await
            .is_pending()
    );
    state
        .storage
        .store
        .set_config_top_level_value("waf", json!({"enabled": false}))
        .await
        .unwrap();
    drop(guard);
    assert_eq!(drain.await.unwrap()["skipped_reason"], "waf_disabled");
}

#[tokio::test]
async fn drain_snapshot_follows_full_read_reconciliation_of_external_writes() {
    let (_directory, state) = waf_test_state("http://127.0.0.1:1").await;
    let store = &state.storage.store;
    let writer = crate::store::Store::connect(store.path()).await.unwrap();
    let updated =
        json!({"waf": {"enabled": true, "drain_interval_seconds": 13, "log_retention_days": 9}});
    writer
        .set_json_value("fn_knock:config", &updated)
        .await
        .unwrap();
    // As with other runtime snapshot consumers, another Store is observed by
    // an explicit full read/reconciliation, not by querying SQLite per tick.
    let observed = store.get_config().await.unwrap();
    assert_eq!(observed["waf"], updated["waf"]);
    assert_eq!(waf_drain_schedule(&state), Some(13));
    assert_eq!(waf_drain_settings(&state).retention_days, 9);
}

fn large_host_mappings() -> Value {
    Value::Array(
        (0..70)
            .map(|index| {
                json!({
                    "host": format!("host-{index}.example.invalid"),
                    "target": "http://127.0.0.1:8080",
                    "favicon": "x".repeat(15_440),
                    "waf_enabled": index % 2 == 0
                })
            })
            .collect(),
    )
}

#[tokio::test]
async fn committed_waf_config_notifies_even_when_response_construction_fails() {
    let (_directory, state) = waf_test_state("http://127.0.0.1:1").await;
    // A file where the rules directory should be makes get_waf_details fail
    // after the configuration commit, without relying on a network failure.
    fs::write(waf_root_dir(&state), b"not a directory")
        .await
        .unwrap();
    let mut updates = state.storage.store.subscribe_config_snapshot();
    let result =
        apply_waf_config(&state, &json!({"system_rules_auto_update_enabled": false})).await;
    assert!(result.is_err());
    assert!(updates.has_changed().unwrap());
    updates.changed().await.unwrap();
    assert_eq!(
        state.storage.store.config_snapshot()["waf"]["system_rules_auto_update_enabled"],
        false
    );
}

#[tokio::test]
async fn disabled_drain_wakes_on_publication_without_handler_notification() {
    let (_directory, state) = waf_test_state("http://127.0.0.1:1").await;
    let mut updates = state.storage.store.subscribe_config_snapshot();
    let wait = wait_for_waf_drain(&state, &mut updates);
    tokio::pin!(wait);
    assert!(
        std::future::poll_fn(|cx| std::task::Poll::Ready(wait.as_mut().poll(cx)))
            .await
            .is_pending()
    );

    // A compatibility write or an API whose response construction subsequently
    // fails has no route-level notification. Publication alone must wake it.
    state
        .storage
        .store
        .set_config_top_level_value("waf", json!({"enabled": true, "drain_interval_seconds": 1}))
        .await
        .unwrap();
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(3), wait)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn drain_wait_observes_updates_before_registration_and_interval_changes() {
    let (_directory, state) = waf_test_state("http://127.0.0.1:1").await;
    let mut updates = state.storage.store.subscribe_config_snapshot();
    state
        .storage
        .store
        .set_config_top_level_value(
            "waf",
            json!({"enabled": true, "drain_interval_seconds": 60}),
        )
        .await
        .unwrap();
    let wait = wait_for_waf_drain(&state, &mut updates);
    tokio::pin!(wait);
    assert!(
        std::future::poll_fn(|cx| std::task::Poll::Ready(wait.as_mut().poll(cx)))
            .await
            .is_pending()
    );
    state
        .storage
        .store
        .set_config_top_level_value(
            "waf",
            json!({"enabled": true, "drain_interval_seconds": "1"}),
        )
        .await
        .unwrap();
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(3), wait)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn unrelated_publications_do_not_starve_a_drain_deadline() {
    let (_directory, state) = waf_test_state("http://127.0.0.1:1").await;
    state
        .storage
        .store
        .set_config_top_level_value("waf", json!({"enabled": true, "drain_interval_seconds": 1}))
        .await
        .unwrap();
    let mut updates = state.storage.store.subscribe_config_snapshot();
    let writes = async {
        for index in 0..30 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            state
                .storage
                .store
                .set_config_top_level_value("locale", json!({"test_sequence": index}))
                .await
                .unwrap();
        }
    };
    tokio::pin!(writes);
    tokio::select! {
        result = wait_for_waf_drain(&state, &mut updates) => assert!(result),
        _ = &mut writes => panic!("unrelated configuration writes kept postponing the deadline"),
    }
}

#[tokio::test]
async fn disabling_a_waiting_drain_removes_its_deadline_and_shutdown_wakes_it() {
    let (_directory, state) = waf_test_state("http://127.0.0.1:1").await;
    state
        .storage
        .store
        .set_config_top_level_value("waf", json!({"enabled": true, "drain_interval_seconds": 1}))
        .await
        .unwrap();
    let mut updates = state.storage.store.subscribe_config_snapshot();
    let wait = wait_for_waf_drain(&state, &mut updates);
    tokio::pin!(wait);
    assert!(
        std::future::poll_fn(|cx| std::task::Poll::Ready(wait.as_mut().poll(cx)))
            .await
            .is_pending()
    );
    state
        .storage
        .store
        .set_config_top_level_value("waf", json!({"enabled": false}))
        .await
        .unwrap();
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(1200), wait.as_mut())
            .await
            .is_err()
    );
    state.shutdown.cancel();
    assert!(!wait.await);
}

// Run explicitly in an optimized build. Synthetic data only; no submitted
// database, credentials or hostnames are embedded in this benchmark.
#[tokio::test]
#[ignore = "manual WAF config polling A/B benchmark"]
async fn waf_config_polling_ab() {
    use std::{hint::black_box, time::Instant};
    let (_directory, state) = waf_test_state("http://127.0.0.1:1").await;
    let mut config = state.storage.store.get_config().await.unwrap();
    config["host_mappings"] = large_host_mappings();
    config["waf"] = json!({"enabled": true, "drain_interval_seconds": 2, "log_retention_days": 7});
    state.storage.store.replace_config(&config).await.unwrap();
    let recorder = state.storage.store.diagnostics();
    let iterations = 300;
    // Warm both paths and alternate order between rounds. A reproduces both
    // pre-fix config reads per cycle, excluding shared gateway/persistence work.
    black_box(load_waf_config(&state).await.unwrap());
    black_box(waf_drain_settings(&state));
    for round in 0..4 {
        for baseline in if round % 2 == 0 {
            [true, false]
        } else {
            [false, true]
        } {
            let generation = recorder.start();
            let started = Instant::now();
            for _ in 0..iterations {
                if baseline {
                    let config = state.storage.store.get_config().await.unwrap();
                    let waf = config.get("waf");
                    black_box(waf.and_then(|v| v.get("enabled")).and_then(Value::as_bool));
                    black_box(
                        waf.and_then(|v| v.get("drain_interval_seconds"))
                            .and_then(Value::as_i64)
                            .unwrap_or(2)
                            .clamp(1, 60),
                    );
                    drop(config);
                    black_box(load_waf_config(&state).await.unwrap());
                } else {
                    black_box(waf_drain_schedule(&state));
                    black_box(waf_drain_settings(&state));
                }
            }
            let elapsed = started.elapsed();
            recorder.stop(generation);
            let stats = recorder.snapshot();
            let sqlite_calls: u64 = stats
                .operations
                .iter()
                .filter(|s| s.kind.starts_with("sqlite_"))
                .map(|s| s.calls)
                .sum();
            assert_eq!(sqlite_calls, if baseline { iterations * 2 } else { 0 });
            println!(
                "WAF_CONFIG_AB round={round} path={} cycles={iterations} config_bytes={} ns_per_cycle={} sqlite_calls={sqlite_calls}",
                if baseline { "baseline" } else { "snapshot" },
                serde_json::to_vec(&config).unwrap().len(),
                elapsed.as_nanos() / u128::from(iterations)
            );
        }
    }
}
