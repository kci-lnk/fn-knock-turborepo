use super::*;

#[tokio::test]
async fn notification_runtime_lease_has_one_winner_and_typed_failure_rolls_back() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let mut tasks = Vec::new();
    for index in 0..16 {
        let store = store.clone();
        tasks.push(tokio::spawn(async move {
            let token = format!("owner-{index}");
            let acquired = store
                .acquire_notification_runtime_lease("concurrent", &token, 60)
                .await?;
            Ok::<_, crate::storage::StorageError>((token, acquired))
        }));
    }
    let mut winner = None;
    for task in tasks {
        let (token, acquired) = task.await.unwrap().unwrap();
        if acquired {
            assert!(winner.replace(token).is_none());
        }
    }
    let winner = winner.expect("one lease winner");
    let typed = store
        .typed
        .typed_notification_runtime
        .load_lease("concurrent")
        .await
        .unwrap()
        .expect("typed notification lease");
    assert_eq!(typed.token, winner);
    assert!(typed.expires_at_ms > crate::time_utils::now_ms());

    let connection = open_fixture_connection(&path);
    connection
        .execute_batch(
            "CREATE TRIGGER fail_typed_notification_lease_insert
             BEFORE INSERT ON notification_runtime_leases
             BEGIN
               SELECT RAISE(FAIL, 'forced typed notification lease failure');
             END;",
        )
        .unwrap();
    drop(connection);
    assert!(
        store
            .acquire_notification_runtime_lease("rollback", "owner", 60)
            .await
            .is_err()
    );
    assert!(
        store
            .get_string_value(&notification_runtime_lock_key("rollback"))
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .typed
            .typed_notification_runtime
            .load_lease("rollback")
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn notification_window_is_atomic_repairs_shadow_and_rolls_back_typed_failure() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let happened_at_ms = crate::time_utils::now_ms();
    let mut tasks = Vec::new();
    for index in 0..16 {
        let store = store.clone();
        tasks.push(tokio::spawn(async move {
            store
                .append_notification_window_hit(
                    "rule-concurrent",
                    "global",
                    &format!("event-{index}"),
                    happened_at_ms,
                    60,
                )
                .await
        }));
    }
    for task in tasks {
        assert!((1..=16).contains(&task.await.unwrap().unwrap()));
    }
    let key = notification_window_key("rule-concurrent", "global");
    assert_eq!(
        store
            .typed
            .typed_notification_runtime
            .load_window(&key)
            .await
            .unwrap()
            .len(),
        16
    );

    let connection = open_fixture_connection(&path);
    connection
        .execute(
            "UPDATE notification_runtime_window_hits SET happened_at_ms = 0
             WHERE runtime_key = ?1 AND event_id = 'event-0'",
            [&key],
        )
        .unwrap();
    drop(connection);
    assert_eq!(
        store
            .append_notification_window_hit(
                "rule-concurrent",
                "global",
                "event-16",
                happened_at_ms,
                60,
            )
            .await
            .unwrap(),
        17
    );
    assert_eq!(
        store
            .typed
            .typed_notification_runtime
            .load_window(&key)
            .await
            .unwrap()
            .len(),
        17
    );
    let status = store.typed_notification_runtime_shadow_status();
    assert!(!status.healthy);
    assert_eq!(status.mismatch_count, 1);

    let connection = open_fixture_connection(&path);
    connection
        .execute_batch(
            "CREATE TRIGGER fail_typed_notification_window_insert
             BEFORE INSERT ON notification_runtime_window_hits
             WHEN new.runtime_key LIKE '%rule-rollback%'
             BEGIN
               SELECT RAISE(FAIL, 'forced typed notification window failure');
             END;",
        )
        .unwrap();
    drop(connection);
    let rollback_key = notification_window_key("rule-rollback", "global");
    assert!(
        store
            .append_notification_window_hit(
                "rule-rollback",
                "global",
                "event-rollback",
                happened_at_ms,
                60,
            )
            .await
            .is_err()
    );
    assert!(
        store
            .zrevrange_strings(&rollback_key)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        store
            .typed
            .typed_notification_runtime
            .load_window(&rollback_key)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn notification_cooldown_and_ready_queue_repair_and_rollback_atomically() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let cooldown_key = notification_cooldown_key("rule", "group");
    let until = crate::time_utils::iso_after_seconds(60);
    store
        .set_notification_cooldown_until("rule", "group", &until, 60)
        .await
        .unwrap();
    let connection = open_fixture_connection(&path);
    connection
        .execute(
            "UPDATE notification_runtime_cooldowns SET until_iso = 'corrupt'
             WHERE runtime_key = ?1",
            [&cooldown_key],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE notification_delivery_ready_queue SET ready_at_ms = 999
             WHERE delivery_id = 'ready-repair'",
            [],
        )
        .unwrap();
    drop(connection);
    assert_eq!(
        store
            .get_notification_cooldown_until("rule", "group")
            .await
            .unwrap()
            .as_deref(),
        Some(until.as_str())
    );
    assert_eq!(
        store
            .typed
            .typed_notification_runtime
            .load_cooldown(&cooldown_key)
            .await
            .unwrap()
            .unwrap()
            .until_iso,
        until
    );

    store
        .enqueue_notification_delivery("ready-repair", 10)
        .await
        .unwrap();
    let connection = open_fixture_connection(&path);
    connection
        .execute(
            "UPDATE notification_delivery_ready_queue SET ready_at_ms = 999
             WHERE delivery_id = 'ready-repair'",
            [],
        )
        .unwrap();
    drop(connection);
    assert_eq!(
        store
            .pull_ready_notification_delivery_ids(10, 20)
            .await
            .unwrap(),
        vec!["ready-repair".to_string()]
    );
    assert!(
        store
            .typed
            .typed_notification_runtime
            .load_ready_queue()
            .await
            .unwrap()
            .is_empty()
    );

    store
        .enqueue_notification_delivery("ready-rollback", 10)
        .await
        .unwrap();
    let connection = open_fixture_connection(&path);
    connection
        .execute_batch(
            "CREATE TRIGGER fail_typed_notification_ready_delete
             BEFORE DELETE ON notification_delivery_ready_queue
             WHEN old.delivery_id = 'ready-rollback'
             BEGIN
               SELECT RAISE(FAIL, 'forced typed notification ready failure');
             END;",
        )
        .unwrap();
    drop(connection);
    assert!(
        store
            .pull_ready_notification_delivery_ids(10, 20)
            .await
            .is_err()
    );
    assert_eq!(
        store
            .zrevrange_strings(NOTIFICATION_DELIVERIES_READY_KEY)
            .await
            .unwrap(),
        vec!["ready-rollback".to_string()]
    );
    assert_eq!(
        store
            .typed
            .typed_notification_runtime
            .load_ready_queue()
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn notification_cooldown_and_ready_enqueue_roll_back_on_typed_failure() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let connection = open_fixture_connection(&path);
    connection
        .execute_batch(
            "CREATE TRIGGER fail_typed_notification_cooldown_insert
             BEFORE INSERT ON notification_runtime_cooldowns
             BEGIN
               SELECT RAISE(FAIL, 'forced typed notification cooldown failure');
             END;
             CREATE TRIGGER fail_typed_notification_ready_insert
             BEFORE INSERT ON notification_delivery_ready_queue
             BEGIN
               SELECT RAISE(FAIL, 'forced typed notification ready failure');
             END;",
        )
        .unwrap();
    drop(connection);
    let cooldown_key = notification_cooldown_key("rollback-rule", "global");
    assert!(
        store
            .set_notification_cooldown_until(
                "rollback-rule",
                "global",
                &crate::time_utils::iso_after_seconds(60),
                60,
            )
            .await
            .is_err()
    );
    assert!(
        store
            .get_string_value(&cooldown_key)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .enqueue_notification_delivery("enqueue-rollback", 10)
            .await
            .is_err()
    );
    assert!(
        store
            .zrevrange_strings(NOTIFICATION_DELIVERIES_READY_KEY)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn notification_ready_queue_claims_each_delivery_once_under_concurrency() {
    let (_dir, store) = open_test_store().await;
    for index in 0..32 {
        store
            .enqueue_notification_delivery(&format!("delivery-{index:02}"), 10)
            .await
            .unwrap();
    }
    let mut tasks = Vec::new();
    for _ in 0..8 {
        let store = store.clone();
        tasks.push(tokio::spawn(async move {
            store.pull_ready_notification_delivery_ids(8, 20).await
        }));
    }
    let mut claimed = Vec::new();
    for task in tasks {
        claimed.extend(task.await.unwrap().unwrap());
    }
    assert_eq!(claimed.len(), 32);
    claimed.sort();
    claimed.dedup();
    assert_eq!(claimed.len(), 32);
    assert!(
        store
            .zrevrange_strings(NOTIFICATION_DELIVERIES_READY_KEY)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        store
            .typed
            .typed_notification_runtime
            .load_ready_queue()
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn notification_delivery_queue_recovers_non_terminal_history_after_crash() {
    let (_dir, store) = open_test_store().await;
    for (id, status) in [
        ("delivery-queued", "queued"),
        ("delivery-sending", "sending"),
        ("delivery-success", "success"),
    ] {
        let delivery = json!({
            "id": id,
            "status": status,
            "triggered_at": "2020-01-01T00:00:00.000Z"
        });
        store
            .save_notification_delivery(id, &delivery, crate::time_utils::now_ms(), 60, false)
            .await
            .unwrap();
    }
    assert!(
        store
            .zrevrange_strings(NOTIFICATION_DELIVERIES_READY_KEY)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        store
            .rebuild_notification_delivery_ready_queue()
            .await
            .unwrap(),
        2
    );
    let mut recovered = store
        .pull_ready_notification_delivery_ids(10, crate::time_utils::now_ms())
        .await
        .unwrap();
    recovered.sort();
    assert_eq!(
        recovered,
        vec![
            "delivery-queued".to_string(),
            "delivery-sending".to_string()
        ]
    );
}

#[tokio::test]
async fn notification_runtime_direct_restore_and_clear_rebuild_all_typed_state() {
    let source_dir = tempfile::tempdir().expect("create source temp dir");
    let source = Store::connect(source_dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open source store");
    source
        .acquire_notification_runtime_lease("restore", "owner", 60)
        .await
        .unwrap();
    source
        .append_notification_window_hit(
            "restore-rule",
            "global",
            "restore-event",
            crate::time_utils::now_ms(),
            60,
        )
        .await
        .unwrap();
    source
        .set_notification_cooldown_until(
            "restore-rule",
            "global",
            &crate::time_utils::iso_after_seconds(60),
            60,
        )
        .await
        .unwrap();
    source
        .enqueue_notification_delivery("restore-delivery", crate::time_utils::now_ms())
        .await
        .unwrap();
    let entries = source
        .export_backup_entries_by_prefix_limited("fn_knock:notifications:", 1_000_000, |_| true)
        .await
        .unwrap();

    let target_dir = tempfile::tempdir().expect("create target temp dir");
    let target_path = target_dir.path().join("fn-knock.sqlite3");
    let target = Store::connect(&target_path)
        .await
        .expect("open target store");
    target
        .replace_backup_entries_by_prefix("fn_knock:", &entries, 200)
        .await
        .unwrap();
    assert!(
        target
            .typed
            .typed_notification_runtime
            .load_lease("restore")
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(
        target
            .typed
            .typed_notification_runtime
            .load_window(&notification_window_key("restore-rule", "global"))
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(
        target
            .typed
            .typed_notification_runtime
            .load_cooldown(&notification_cooldown_key("restore-rule", "global"))
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(
        target
            .typed
            .typed_notification_runtime
            .load_ready_queue()
            .await
            .unwrap()
            .len(),
        1
    );

    target.clear_all_keys().await.unwrap();
    let connection = open_fixture_connection(&target_path);
    let remaining: i64 = connection
        .query_row(
            "SELECT
               (SELECT COUNT(*) FROM notification_runtime_leases) +
               (SELECT COUNT(*) FROM notification_runtime_cooldowns) +
               (SELECT COUNT(*) FROM notification_runtime_window_hits) +
               (SELECT COUNT(*) FROM notification_delivery_ready_queue)",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(remaining, 0);
}

#[tokio::test]
async fn notification_runtime_expiry_never_leaves_typed_state_authoritative() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let lease_key = notification_runtime_lock_key("expiry");
    let cooldown_key = notification_cooldown_key("expiry-rule", "global");
    let window_key = notification_window_key("expiry-rule", "global");
    assert!(
        store
            .acquire_notification_runtime_lease("expiry", "old-owner", 60)
            .await
            .unwrap()
    );
    store
        .set_notification_cooldown_until(
            "expiry-rule",
            "global",
            &crate::time_utils::iso_after_seconds(60),
            60,
        )
        .await
        .unwrap();
    store
        .append_notification_window_hit(
            "expiry-rule",
            "global",
            "old-event",
            crate::time_utils::now_ms(),
            60,
        )
        .await
        .unwrap();
    store
        .enqueue_notification_delivery("expired-ready", 1)
        .await
        .unwrap();

    let connection = open_fixture_connection(&path);
    for key in [
        lease_key.as_str(),
        cooldown_key.as_str(),
        window_key.as_str(),
        NOTIFICATION_DELIVERIES_READY_KEY,
    ] {
        connection
            .execute("UPDATE kv_keys SET expires_at_ms = 0 WHERE key = ?1", [key])
            .unwrap();
    }
    drop(connection);

    assert!(
        store
            .acquire_notification_runtime_lease("expiry", "new-owner", 60)
            .await
            .unwrap()
    );
    assert_eq!(
        store
            .typed
            .typed_notification_runtime
            .load_lease("expiry")
            .await
            .unwrap()
            .unwrap()
            .token,
        "new-owner"
    );
    assert!(
        store
            .get_notification_cooldown_until("expiry-rule", "global")
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .typed
            .typed_notification_runtime
            .load_cooldown(&cooldown_key)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        store
            .append_notification_window_hit(
                "expiry-rule",
                "global",
                "new-event",
                crate::time_utils::now_ms(),
                60,
            )
            .await
            .unwrap(),
        1
    );
    let hits = store
        .typed
        .typed_notification_runtime
        .load_window(&window_key)
        .await
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].event_id, "new-event");
    assert!(
        store
            .pull_ready_notification_delivery_ids(10, crate::time_utils::now_ms())
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        store
            .typed
            .typed_notification_runtime
            .load_ready_queue()
            .await
            .unwrap()
            .is_empty()
    );
}
