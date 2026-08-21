use super::*;

#[tokio::test]
async fn typed_notification_history_nx_preserves_existing_record_and_repairs_index() {
    let (_dir, store) = open_test_store().await;
    let initial = json!({ "id": "trigger-nx", "created_at": now_iso(), "status": "initial" });
    let duplicate = json!({ "id": "trigger-nx", "created_at": now_iso(), "status": "duplicate" });
    assert!(
        store
            .save_notification_trigger(
                "trigger-nx",
                &initial,
                crate::time_utils::now_ms(),
                60,
                true
            )
            .await
            .unwrap()
    );
    store
        .zrem_string_member("fn_knock:notifications:triggers:index", "trigger-nx")
        .await
        .unwrap();
    assert!(
        !store
            .save_notification_trigger(
                "trigger-nx",
                &duplicate,
                crate::time_utils::now_ms(),
                60,
                true
            )
            .await
            .unwrap()
    );
    assert_eq!(
        store.load_notification_trigger("trigger-nx").await.unwrap(),
        Some(initial)
    );
    assert_eq!(
        store
            .zrevrange_strings("fn_knock:notifications:triggers:index")
            .await
            .unwrap(),
        vec!["trigger-nx".to_string()]
    );
}

#[tokio::test]
async fn typed_notification_history_failure_rolls_back_legacy_write() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let connection = open_fixture_connection(&path);
    connection.execute_batch("CREATE TRIGGER fail_typed_notification_history_insert BEFORE INSERT ON notification_history_documents BEGIN SELECT RAISE(ABORT, 'injected typed notification history failure'); END;").unwrap();
    drop(connection);
    let trigger = json!({ "id": "trigger-fail", "created_at": now_iso() });
    let error = store
        .save_notification_trigger(
            "trigger-fail",
            &trigger,
            crate::time_utils::now_ms(),
            60,
            false,
        )
        .await
        .expect_err("typed failure rejects the entire history write");
    assert!(
        error
            .to_string()
            .contains("injected typed notification history failure")
    );
    assert!(
        store
            .get_json_value("fn_knock:notifications:triggers:data:trigger-fail")
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .zrevrange_strings("fn_knock:notifications:triggers:index")
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn typed_notification_history_read_mismatch_falls_back_and_repairs() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let delivery =
        json!({ "id": "delivery-shadow", "triggered_at": now_iso(), "status": "pending" });
    store
        .save_notification_delivery(
            "delivery-shadow",
            &delivery,
            crate::time_utils::now_ms(),
            60,
            false,
        )
        .await
        .expect("seed delivery");
    let connection = open_fixture_connection(&path);
    connection.execute("UPDATE notification_history_documents SET document_json = 'not-json' WHERE kind = 'delivery' AND id = ?1", ["delivery-shadow"]).unwrap();
    drop(connection);
    assert_eq!(
        store
            .load_notification_delivery("delivery-shadow")
            .await
            .expect("fallback to legacy delivery"),
        Some(delivery.clone())
    );
    assert_eq!(
        store
            .typed
            .typed_notifications
            .load_history_one("delivery", "delivery-shadow")
            .await
            .expect("typed delivery repaired"),
        Some(delivery)
    );
}

#[tokio::test]
async fn deleting_typed_delivery_history_also_removes_ready_queue_member() {
    let (_dir, store) = open_test_store().await;
    let delivery = json!({ "id": "delivery-delete", "triggered_at": now_iso() });
    store
        .save_notification_delivery(
            "delivery-delete",
            &delivery,
            crate::time_utils::now_ms(),
            60,
            false,
        )
        .await
        .unwrap();
    store
        .zadd_string_member(
            NOTIFICATION_DELIVERIES_READY_KEY,
            "delivery-delete",
            crate::time_utils::now_ms(),
        )
        .await
        .unwrap();
    store
        .delete_notification_deliveries(&["delivery-delete".to_string()])
        .await
        .unwrap();
    assert!(
        store
            .load_notification_delivery("delivery-delete")
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .zrevrange_strings(NOTIFICATION_DELIVERIES_READY_KEY)
            .await
            .unwrap()
            .is_empty()
    );
}
