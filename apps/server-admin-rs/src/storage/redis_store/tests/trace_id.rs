use super::*;

#[tokio::test]
async fn system_event_trace_lookup_scans_all_retained_legacy_records() {
    let (_dir, store) = open_test_store().await;
    let trace_id = "trc_3f93d40a-89ea-4dbe-a04f-67692778d973";
    let base = crate::time_utils::now_ms();
    for index in 0..101 {
        store
            .append_system_event(
                &json!({
                    "id": format!("trace-event-{index:03}"),
                    "trace_id": trace_id,
                    "type": "FN_EVENT_RUNTIME_STARTED",
                    "source": "RUNTIME_MONITOR",
                    "level": "INFO",
                    "happened_at": crate::time_utils::iso_from_ms(base + index),
                }),
                30,
                1_000,
            )
            .await
            .expect("append traced event");
    }

    let events = store
        .find_system_events_by_trace(trace_id)
        .await
        .expect("find every traced event");
    assert_eq!(events.len(), 101);
}

#[tokio::test]
async fn system_event_trace_lookup_repairs_valid_same_id_shadow_mismatch() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let trace_id = "trc_3f93d40a-89ea-4dbe-a04f-67692778d973";
    let event = json!({
        "id": "trace-shadow-event",
        "trace_id": trace_id,
        "type": "FN_EVENT_RUNTIME_STARTED",
        "source": "RUNTIME_MONITOR",
        "level": "INFO",
        "happened_at": crate::time_utils::now_iso(),
    });
    store
        .append_system_event(&event, 30, 1_000)
        .await
        .expect("seed traced event");
    let mut corrupted = event.clone();
    corrupted["source"] = json!("CORRUPTED_TYPED_SHADOW");
    let connection = open_fixture_connection(&path);
    connection
        .execute(
            "UPDATE system_event_documents SET event_json = ?2 WHERE id = ?1",
            tokio_rusqlite::rusqlite::params!["trace-shadow-event", corrupted.to_string()],
        )
        .unwrap();
    drop(connection);

    assert_eq!(
        store.find_system_events_by_trace(trace_id).await.unwrap(),
        vec![event.clone()]
    );
    let repaired = store
        .typed
        .typed_events
        .load_by_trace(trace_id)
        .await
        .unwrap();
    assert_eq!(repaired, vec![event]);
}

#[tokio::test]
async fn typed_trace_indexes_keep_legacy_missing_ids_null() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let event = json!({
        "id": "legacy-event-without-trace",
        "type": "FN_EVENT_RUNTIME_STARTED",
        "source": "RUNTIME_MONITOR",
        "level": "INFO",
        "happened_at": crate::time_utils::now_iso(),
    });
    let trigger = json!({
        "id": "legacy-trigger-without-trace",
        "created_at": crate::time_utils::now_iso(),
        "status": "created",
    });
    store.append_system_event(&event, 30, 1_000).await.unwrap();
    store
        .save_notification_trigger(
            "legacy-trigger-without-trace",
            &trigger,
            crate::time_utils::now_ms(),
            60,
            false,
        )
        .await
        .unwrap();

    let connection = open_fixture_connection(&path);
    let event_trace: Option<String> = connection
        .query_row(
            "SELECT trace_id FROM system_event_documents WHERE id = ?1",
            ["legacy-event-without-trace"],
            |row| row.get(0),
        )
        .unwrap();
    let trigger_trace: Option<String> = connection
        .query_row(
            "SELECT trace_id FROM notification_history_documents WHERE kind = 'trigger' AND id = ?1",
            ["legacy-trigger-without-trace"],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(event_trace, None);
    assert_eq!(trigger_trace, None);
}
