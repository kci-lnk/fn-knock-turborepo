use super::*;

#[test]
fn reads_login_backoff_status_like_node_store() {
    let status = login_backoff_status_from_raw(
        "203.0.113.10",
        Some(r#"{"ip":"ignored","attempts":-2,"blockedUntil":1100}"#),
        1000,
    );
    assert_eq!(status.ip, "203.0.113.10");
    assert_eq!(status.attempts, -2);
    assert!(status.blocked);
    assert_eq!(status.retry_after, Some(1));
    assert_eq!(status.blocked_until, Some(1100));

    let expired = login_backoff_status_from_raw(
        "203.0.113.10",
        Some(r#"{"ip":"ignored","attempts":3,"blockedUntil":999}"#),
        1000,
    );
    assert_eq!(expired.attempts, 3);
    assert!(!expired.blocked);
    assert_eq!(expired.retry_after, None);
}

#[test]
fn docker_admin_session_record_accepts_legacy_missing_ttl() {
    let record: DockerAdminSessionRecord = serde_json::from_str(
        r#"{
                "id": "session-1",
                "created_at": "2026-01-01T00:00:00.000Z",
                "updated_at": "2026-01-01T00:00:00.000Z",
                "expires_at": "2026-01-01T12:00:00.000Z",
                "ip": "203.0.113.10",
                "user_agent": "ua"
            }"#,
    )
    .expect("legacy docker admin session");

    assert_eq!(record.ttl_seconds, 0);
    assert!(record.password_revision.is_empty());
}

#[test]
fn traffic_scope_matches_node_uri_encoding() {
    assert_eq!(traffic_scope_segment("global", None, None), "global");
    assert_eq!(traffic_scope_segment("", None, None), "");
    assert_eq!(traffic_scope_segment(" user ", None, None), " user ");
    assert_eq!(
        traffic_scope_segment("global", Some("example.com"), None),
        "global:host:example.com"
    );
    assert_eq!(
        traffic_scope_segment(" user ", Some("example.com"), None),
        " user :host:example.com"
    );
    assert_eq!(
        traffic_scope_segment("u", Some("[2001:db8::1]"), None),
        "u:host:%5B2001%3Adb8%3A%3A1%5D"
    );
    assert_eq!(
        traffic_scope_segment("global", None, Some("tcp/3306")),
        "global:stream:tcp%2F3306"
    );
    assert_eq!(
        traffic_scope_segment("global", Some("example.com"), Some("tcp/3306")),
        "global:host:example.com"
    );
}

#[test]
fn system_event_search_uses_unicode_lowercase_like_node() {
    let event = json!({
        "id": "evt_unicode",
        "type": "FN_EVENT_AUTH_LOGIN_SUCCESS",
        "source": "SERVER_ADMIN",
        "level": "INFO",
        "happened_at": "2026-07-07T00:00:00.000Z",
        "payload": {
            "credential_name": "Älice"
        }
    });

    assert!(system_event_matches_filters(
        &event, "älice", None, None, None
    ));
}

#[tokio::test]
async fn system_event_max_records_keeps_the_newest_entries() {
    let (_dir, store) = open_test_store().await;
    let base = crate::time_utils::now_ms();
    for index in 0..=1000 {
        let event = json!({
            "id": format!("evt_{index:04}"),
            "type": "FN_EVENT_RUNTIME_STARTED",
            "source": "RUNTIME_MONITOR",
            "level": "INFO",
            "happened_at": crate::time_utils::iso_from_ms(base + index),
            "subject": { "kind": "COMPONENT", "id": "management" },
            "payload": { "component": "management" },
        });
        store
            .append_system_event(&event, 30, 1000)
            .await
            .expect("append bounded event");
    }

    let listed = store
        .list_system_events(1, 1, "", None, None, Some("RUNTIME_MONITOR"))
        .await
        .expect("list bounded events");
    assert_eq!(listed.get("total").and_then(Value::as_i64), Some(1000));
    assert_eq!(
        listed.pointer("/events/0/id").and_then(Value::as_str),
        Some("evt_1000")
    );
    let mut conn = store.conn();
    assert!(conn.ttl(EVENTS_INDEX_KEY).await.unwrap() > 0);
    assert!(conn.ttl(EVENTS_STREAM_KEY).await.unwrap() > 0);
}

#[tokio::test]
async fn future_system_event_timestamp_cannot_extend_retention_ttl() {
    let (_dir, store) = open_test_store().await;
    let event = json!({
        "id": "future-event",
        "happened_at": "2099-01-01T00:00:00.000Z",
        "type": "FN_EVENT_RUNTIME_STARTED",
    });
    store.append_system_event(&event, 1, 1_000).await.unwrap();

    let ttl = store
        .ttl_seconds(&system_event_data_key("future-event"))
        .await
        .unwrap();
    assert!(
        ttl > 0 && ttl <= 86_400,
        "unexpected future event TTL: {ttl}"
    );
}

#[tokio::test]
async fn typed_system_events_backfill_and_mutations_stay_in_sync_with_legacy_keyspace() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let event = json!({
        "id": "typed-event",
        "type": "FN_EVENT_RUNTIME_STARTED",
        "source": "RUNTIME_MONITOR",
        "level": "INFO",
        "happened_at": crate::time_utils::now_iso(),
    });
    store
        .append_system_event(&event, 30, 1_000)
        .await
        .expect("append event to both stores");
    assert_eq!(store.typed.typed_events.count().await.unwrap(), 1);

    drop(store);
    let reopened = Store::connect(&path).await.expect("reopen store");
    assert_eq!(reopened.typed.typed_events.count().await.unwrap(), 1);

    reopened
        .delete_system_events(&["typed-event".to_string()])
        .await
        .expect("delete event from both stores");
    assert_eq!(reopened.typed.typed_events.count().await.unwrap(), 0);
    assert_eq!(
        reopened
            .list_system_events(1, 10, "", None, None, None)
            .await
            .unwrap()["total"],
        0
    );

    reopened
        .append_system_event(&event, 30, 1_000)
        .await
        .expect("reappend event");
    assert_eq!(reopened.clear_system_events().await.unwrap(), 1);
    assert_eq!(reopened.typed.typed_events.count().await.unwrap(), 0);
}

#[tokio::test]
async fn typed_system_event_failure_rolls_back_legacy_write() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let connection = open_fixture_connection(&path);
    connection
        .execute_batch(
            "CREATE TRIGGER fail_typed_system_event_insert
             BEFORE INSERT ON system_event_documents
             BEGIN
               SELECT RAISE(ABORT, 'injected typed event failure');
             END;",
        )
        .unwrap();
    drop(connection);

    let event = json!({
        "id": "rollback-event",
        "type": "FN_EVENT_RUNTIME_STARTED",
        "source": "RUNTIME_MONITOR",
        "level": "INFO",
        "happened_at": crate::time_utils::now_iso(),
    });
    let error = store
        .append_system_event_if_dedupe_available(&event, 30, 1_000, Some("rollback-dedupe"), 60)
        .await
        .expect_err("typed failure must reject the complete event transaction");
    assert!(error.to_string().contains("injected typed event failure"));
    assert_eq!(store.typed.typed_events.count().await.unwrap(), 0);
    assert_eq!(
        store
            .list_system_events(1, 10, "", None, None, None)
            .await
            .unwrap()["total"],
        0
    );
    let mut conn = store.conn();
    assert!(
        conn.xrevrange_count(EVENTS_STREAM_KEY, "+", "-", 1)
            .await
            .unwrap()
            .ids
            .is_empty()
    );
    assert!(
        store
            .get_string_value(&format!("{EVENTS_DEDUPE_PREFIX}rollback-dedupe"))
            .await
            .unwrap()
            .is_none(),
        "failed event transaction must not suppress a retry"
    );
    assert_eq!(store.typed.typed_event_dedupe.count().await.unwrap(), 0);
}

#[tokio::test]
async fn concurrent_system_event_dedupe_claim_and_event_write_are_one_transaction() {
    let (_dir, store) = open_test_store().await;
    let first = json!({
        "id": "dedupe-event-first",
        "type": "FN_EVENT_RUNTIME_STARTED",
        "source": "RUNTIME_MONITOR",
        "level": "INFO",
        "happened_at": crate::time_utils::now_iso(),
    });
    let second = json!({
        "id": "dedupe-event-second",
        "type": "FN_EVENT_RUNTIME_STARTED",
        "source": "RUNTIME_MONITOR",
        "level": "INFO",
        "happened_at": crate::time_utils::now_iso(),
    });
    let (first_result, second_result) = tokio::join!(
        store.append_system_event_if_dedupe_available(
            &first,
            30,
            1_000,
            Some("concurrent-dedupe"),
            60,
        ),
        store.append_system_event_if_dedupe_available(
            &second,
            30,
            1_000,
            Some("concurrent-dedupe"),
            60,
        ),
    );
    assert_ne!(first_result.unwrap(), second_result.unwrap());
    assert_eq!(store.typed.typed_events.count().await.unwrap(), 1);
    assert_eq!(
        store
            .list_system_events(1, 10, "", None, None, None)
            .await
            .unwrap()["total"],
        1
    );
    assert_eq!(
        store
            .get_string_value(&format!("{EVENTS_DEDUPE_PREFIX}concurrent-dedupe"))
            .await
            .unwrap()
            .as_deref(),
        Some("1")
    );
    let typed = store
        .typed
        .typed_event_dedupe
        .load("concurrent-dedupe")
        .await
        .unwrap()
        .expect("typed event dedupe lease");
    assert!(typed.expires_at_ms > crate::time_utils::now_ms());
}

#[tokio::test]
async fn typed_event_dedupe_failure_rolls_back_lease_and_event() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let connection = open_fixture_connection(&path);
    connection
        .execute_batch(
            "CREATE TRIGGER fail_typed_event_dedupe_insert
             BEFORE INSERT ON system_event_dedupe_leases
             BEGIN
               SELECT RAISE(FAIL, 'forced typed event-dedupe failure');
             END;",
        )
        .unwrap();
    drop(connection);
    let event = json!({
        "id": "typed-dedupe-rollback-event",
        "type": "FN_EVENT_RUNTIME_STARTED",
        "source": "RUNTIME_MONITOR",
        "level": "INFO",
        "happened_at": crate::time_utils::now_iso(),
    });
    assert!(
        store
            .append_system_event_if_dedupe_available(
                &event,
                30,
                1_000,
                Some("typed-dedupe-rollback"),
                60,
            )
            .await
            .is_err()
    );
    assert!(
        store
            .get_string_value(&format!("{EVENTS_DEDUPE_PREFIX}typed-dedupe-rollback"))
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(store.typed.typed_event_dedupe.count().await.unwrap(), 0);
    assert_eq!(store.typed.typed_events.count().await.unwrap(), 0);
}

#[tokio::test]
async fn system_event_dedupe_uses_legacy_authority_and_repairs_typed_shadow() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let event = json!({
        "id": "dedupe-shadow-first",
        "type": "FN_EVENT_RUNTIME_STARTED",
        "source": "RUNTIME_MONITOR",
        "level": "INFO",
        "happened_at": crate::time_utils::now_iso(),
    });
    assert!(
        store
            .append_system_event_if_dedupe_available(&event, 30, 1_000, Some("shadow-repair"), 60,)
            .await
            .unwrap()
    );
    let connection = open_fixture_connection(&path);
    connection
        .execute(
            "UPDATE system_event_dedupe_leases SET expires_at_ms = 1 WHERE dedupe_key = 'shadow-repair'",
            [],
        )
        .unwrap();
    drop(connection);

    let duplicate = json!({
        "id": "dedupe-shadow-duplicate",
        "type": "FN_EVENT_RUNTIME_STARTED",
        "source": "RUNTIME_MONITOR",
        "level": "INFO",
        "happened_at": crate::time_utils::now_iso(),
    });
    assert!(
        !store
            .append_system_event_if_dedupe_available(
                &duplicate,
                30,
                1_000,
                Some("shadow-repair"),
                60,
            )
            .await
            .unwrap()
    );
    let typed = store
        .typed
        .typed_event_dedupe
        .load("shadow-repair")
        .await
        .unwrap()
        .unwrap();
    assert!(typed.expires_at_ms > crate::time_utils::now_ms());
    let status = store.typed_event_dedupe_shadow_status();
    assert!(!status.healthy);
    assert_eq!(status.mismatch_count, 1);
    assert_eq!(store.typed.typed_events.count().await.unwrap(), 1);
}

#[tokio::test]
async fn system_event_dedupe_expiry_backup_and_clear_keep_typed_shadow_exact() {
    let source_dir = tempfile::tempdir().expect("create source temp dir");
    let source_path = source_dir.path().join("fn-knock.sqlite3");
    let source = Store::connect(&source_path)
        .await
        .expect("open source store");
    let event = json!({
        "id": "dedupe-backup-event",
        "type": "FN_EVENT_RUNTIME_STARTED",
        "source": "RUNTIME_MONITOR",
        "level": "INFO",
        "happened_at": crate::time_utils::now_iso(),
    });
    source
        .append_system_event_if_dedupe_available(&event, 30, 1_000, Some("backup-dedupe"), 60)
        .await
        .unwrap();
    let entries = source
        .export_backup_entries_by_prefix_limited(EVENTS_DEDUPE_PREFIX, 1_000_000, |_| true)
        .await
        .expect("export event dedupe lease");
    let target_dir = tempfile::tempdir().expect("create target temp dir");
    let target = Store::connect(target_dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open target store");
    target
        .replace_backup_entries_by_prefix("fn_knock:", &entries, 200)
        .await
        .expect("restore event dedupe lease");
    assert!(
        target
            .typed
            .typed_event_dedupe
            .load("backup-dedupe")
            .await
            .unwrap()
            .is_some()
    );

    let connection = open_fixture_connection(&source_path);
    connection
        .execute(
            "UPDATE kv_keys SET expires_at_ms = 0 WHERE key = ?1",
            [format!("{EVENTS_DEDUPE_PREFIX}backup-dedupe")],
        )
        .unwrap();
    drop(connection);
    assert!(
        !source
            .typed
            .typed_event_dedupe
            .verify_and_repair("backup-dedupe")
            .await
            .unwrap()
    );
    assert!(
        source
            .typed
            .typed_event_dedupe
            .load("backup-dedupe")
            .await
            .unwrap()
            .is_none()
    );
    target
        .clear_all_keys()
        .await
        .expect("clear target keyspace");
    assert_eq!(target.typed.typed_event_dedupe.count().await.unwrap(), 0);
}

#[tokio::test]
async fn typed_system_events_rebuild_after_legacy_backup_restore() {
    let source_dir = tempfile::tempdir().expect("create source temp dir");
    let source = Store::connect(source_dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open source store");
    let event = json!({
        "id": "restored-event",
        "type": "FN_EVENT_RUNTIME_STARTED",
        "source": "RUNTIME_MONITOR",
        "level": "INFO",
        "happened_at": crate::time_utils::now_iso(),
    });
    source
        .append_system_event(&event, 30, 1_000)
        .await
        .expect("seed source event");
    let entries = source
        .export_backup_entries_by_prefix_limited("fn_knock:events:", 1_000_000, |_| true)
        .await
        .expect("export legacy event entries");
    assert!(!entries.is_empty());

    let target_dir = tempfile::tempdir().expect("create target temp dir");
    let target = Store::connect(target_dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open target store");
    target
        .replace_backup_entries_by_prefix("fn_knock:", &entries, 200)
        .await
        .expect("restore legacy backup entries");
    assert_eq!(target.typed.typed_events.count().await.unwrap(), 1);
    assert_eq!(
        target
            .list_system_events(1, 10, "", None, None, None)
            .await
            .unwrap()
            .pointer("/events/0/id")
            .and_then(Value::as_str),
        Some("restored-event")
    );
}

#[tokio::test]
async fn typed_system_event_mismatch_falls_back_to_legacy_and_repairs_primary() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let now = crate::time_utils::now_ms();
    let event = json!({
        "id": "shadow-event",
        "type": "FN_EVENT_RUNTIME_STARTED",
        "source": "RUNTIME_MONITOR",
        "level": "INFO",
        "happened_at": crate::time_utils::iso_from_ms(now),
    });
    store
        .append_system_event(&event, 30, 1_000)
        .await
        .expect("seed event");
    let connection = open_fixture_connection(&path);
    connection
        .execute(
            "UPDATE system_event_documents SET event_json = 'not-json' WHERE id = ?1",
            ["shadow-event"],
        )
        .unwrap();
    drop(connection);

    let listed = store
        .list_system_events(1, 10, "", None, None, None)
        .await
        .expect("legacy fallback list");
    assert_eq!(
        listed.pointer("/events/0/id").and_then(Value::as_str),
        Some("shadow-event")
    );
    assert_eq!(store.typed_event_shadow_mismatch_count(), 1);
    let repaired = store
        .typed
        .typed_events
        .load_active()
        .await
        .expect("typed event repaired from legacy fallback");
    assert_eq!(repaired.len(), 1);
    assert_eq!(repaired[0].event["id"], "shadow-event");

    let ranged = store
        .list_system_events_by_range(now.saturating_sub(1), now.saturating_add(1), &[])
        .await
        .expect("typed primary range after repair");
    assert_eq!(ranged.len(), 1);
    assert_eq!(ranged[0].0["id"], "shadow-event");
}

#[tokio::test]
async fn system_event_document_update_preserves_typed_and_legacy_consistency() {
    let (_dir, store) = open_test_store().await;
    let mut event = json!({
        "id": "location-event",
        "type": "FN_EVENT_AUTH_LOGIN_FAILURE",
        "source": "SERVER_ADMIN",
        "level": "WARN",
        "happened_at": crate::time_utils::now_iso(),
        "payload": {
            "ip": "203.0.113.8"
        }
    });
    store
        .append_system_event(&event, 30, 1_000)
        .await
        .expect("seed event");
    event["payload"]["ip_location"] = json!("上海|上海|联通");

    assert!(
        store
            .update_system_event_document(&event)
            .await
            .expect("update event document")
    );
    let listed = store
        .list_system_events(1, 10, "", None, None, None)
        .await
        .expect("list consistent event views");
    assert_eq!(
        listed.pointer("/events/0/payload/ip_location"),
        Some(&json!("上海|上海|联通"))
    );
    assert_eq!(store.typed_event_shadow_mismatch_count(), 0);
    assert_eq!(
        store
            .get_json_value("fn_knock:events:data:location-event")
            .await
            .unwrap()
            .and_then(|value| value.pointer("/payload/ip_location").cloned()),
        Some(json!("上海|上海|联通"))
    );
}

#[tokio::test]
async fn concurrent_system_event_writes_preserve_typed_and_legacy_history() {
    let (_dir, store) = open_test_store().await;
    const WRITERS: usize = 16;
    const READERS: usize = 8;
    let start = Arc::new(tokio::sync::Barrier::new(WRITERS + READERS));
    let now = crate::time_utils::now_ms();
    let mut writes = Vec::new();
    for index in 0..WRITERS {
        let writer = store.clone();
        let start = start.clone();
        writes.push(tokio::spawn(async move {
            start.wait().await;
            writer
                .append_system_event(
                    &json!({
                        "id": format!("concurrent-event-{index:02}"),
                        "type": "FN_EVENT_RUNTIME_STARTED",
                        "source": "RUNTIME_MONITOR",
                        "level": "INFO",
                        "happened_at": crate::time_utils::iso_from_ms(now + index as i64),
                    }),
                    30,
                    1_000,
                )
                .await
        }));
    }
    let mut reads = Vec::new();
    for _ in 0..READERS {
        let reader = store.clone();
        let start = start.clone();
        reads.push(tokio::spawn(async move {
            start.wait().await;
            for _ in 0..16 {
                reader
                    .list_system_events(1, 100, "", None, None, None)
                    .await
                    .expect("concurrent event read");
                tokio::task::yield_now().await;
            }
        }));
    }
    for write in writes {
        write.await.expect("join event writer").unwrap();
    }
    for read in reads {
        read.await.expect("join event reader");
    }
    let listed = store
        .list_system_events(1, 100, "", None, None, None)
        .await
        .expect("load final event history");
    assert_eq!(listed["total"], WRITERS as i64);
    assert_eq!(
        store.typed.typed_events.count().await.unwrap(),
        WRITERS as i64
    );
    assert_eq!(
        store
            .list_system_events_by_range(now.saturating_sub(1), now + WRITERS as i64, &[])
            .await
            .unwrap()
            .len(),
        WRITERS
    );
}

#[tokio::test]
async fn typed_notification_provider_and_rule_writes_are_atomic_and_rebuild_on_startup() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let provider = json!({ "id": "provider-1", "name": "Provider", "updated_at": now_iso() });
    let rule = json!({ "id": "rule-1", "name": "Rule", "updated_at": now_iso() });
    store
        .save_notification_provider("provider-1", &provider, 10)
        .await
        .expect("save provider atomically");
    store
        .save_notification_rule("rule-1", &rule, 20)
        .await
        .expect("save rule atomically");
    assert_eq!(
        store
            .typed
            .typed_notifications
            .count_kind("provider")
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        store
            .typed
            .typed_notifications
            .count_kind("rule")
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        store
            .get_json_value("fn_knock:notifications:providers:data:provider-1")
            .await
            .unwrap(),
        Some(provider.clone())
    );

    drop(store);
    let reopened = Store::connect(&path).await.expect("reopen store");
    assert_eq!(
        reopened
            .typed
            .typed_notifications
            .count_kind("provider")
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        reopened
            .typed
            .typed_notifications
            .count_kind("rule")
            .await
            .unwrap(),
        1
    );
    reopened
        .delete_notification_provider("provider-1")
        .await
        .expect("delete provider atomically");
    reopened
        .delete_notification_rule("rule-1")
        .await
        .expect("delete rule atomically");
    assert_eq!(
        reopened
            .typed
            .typed_notifications
            .count_kind("provider")
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        reopened
            .typed
            .typed_notifications
            .count_kind("rule")
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn typed_notification_write_failure_rolls_back_legacy_record_and_index() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let connection = open_fixture_connection(&path);
    connection
        .execute_batch(
            "CREATE TRIGGER fail_typed_notification_insert
             BEFORE INSERT ON notification_documents
             BEGIN
               SELECT RAISE(ABORT, 'injected typed notification failure');
             END;",
        )
        .unwrap();
    drop(connection);
    let provider = json!({ "id": "provider-fail", "updated_at": now_iso() });
    let error = store
        .save_notification_provider("provider-fail", &provider, 10)
        .await
        .expect_err("typed failure rejects entire provider write");
    assert!(
        error
            .to_string()
            .contains("injected typed notification failure")
    );
    assert!(
        store
            .get_json_value("fn_knock:notifications:providers:data:provider-fail")
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .zrevrange_strings("fn_knock:notifications:providers:index")
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn typed_notification_read_mismatch_falls_back_to_legacy_and_repairs() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let provider =
        json!({ "id": "provider-shadow", "name": "Legacy Provider", "updated_at": now_iso() });
    store
        .save_notification_provider("provider-shadow", &provider, 10)
        .await
        .expect("seed provider");
    let connection = open_fixture_connection(&path);
    connection
        .execute(
            "UPDATE notification_documents SET document_json = 'not-json' WHERE kind = 'provider' AND id = ?1",
            ["provider-shadow"],
        )
        .unwrap();
    drop(connection);
    let providers = store
        .load_notification_providers()
        .await
        .expect("legacy fallback provider list");
    assert_eq!(providers, vec![provider.clone()]);
    assert_eq!(
        store
            .typed
            .typed_notifications
            .load_one("provider", "provider-shadow")
            .await
            .expect("typed provider repaired"),
        Some(provider)
    );
}

#[tokio::test]
async fn typed_notification_history_writes_are_atomic_and_rebuild_on_startup() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let trigger = json!({
        "id": "trigger-1",
        "created_at": now_iso(),
        "status": "pending"
    });
    let delivery = json!({
        "id": "delivery-1",
        "triggered_at": now_iso(),
        "status": "pending"
    });
    store
        .save_notification_trigger(
            "trigger-1",
            &trigger,
            crate::time_utils::now_ms(),
            60,
            false,
        )
        .await
        .expect("save trigger atomically");
    store
        .save_notification_delivery(
            "delivery-1",
            &delivery,
            crate::time_utils::now_ms(),
            60,
            false,
        )
        .await
        .expect("save delivery atomically");
    assert_eq!(
        store
            .typed
            .typed_notifications
            .count_history("trigger")
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        store
            .typed
            .typed_notifications
            .count_history("delivery")
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        store.load_notification_trigger("trigger-1").await.unwrap(),
        Some(trigger.clone())
    );

    drop(store);
    let reopened = Store::connect(&path).await.expect("reopen store");
    assert_eq!(
        reopened.load_notification_history("trigger").await.unwrap(),
        vec![trigger]
    );
    assert_eq!(
        reopened
            .load_notification_history("delivery")
            .await
            .unwrap(),
        vec![delivery]
    );
}

#[tokio::test]
async fn typed_notification_history_rebuilds_after_legacy_backup_restore() {
    let source_dir = tempfile::tempdir().expect("create source temp dir");
    let source = Store::connect(source_dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open source store");
    let trigger = json!({
        "id": "restored-trigger",
        "created_at": now_iso(),
        "status": "completed"
    });
    source
        .save_notification_trigger(
            "restored-trigger",
            &trigger,
            crate::time_utils::now_ms(),
            600,
            false,
        )
        .await
        .expect("seed source trigger");
    let entries = source
        .export_backup_entries_by_prefix_limited("fn_knock:notifications:", 1_000_000, |_| true)
        .await
        .expect("export legacy notification entries");
    assert!(!entries.is_empty());

    let target_dir = tempfile::tempdir().expect("create target temp dir");
    let target = Store::connect(target_dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open target store");
    target
        .replace_backup_entries_by_prefix("fn_knock:", &entries, 200)
        .await
        .expect("restore legacy notification entries");
    assert_eq!(
        target
            .typed
            .typed_notifications
            .count_history("trigger")
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        target.load_notification_history("trigger").await.unwrap(),
        vec![trigger]
    );
}

#[tokio::test]
async fn concurrent_notification_history_reads_and_writes_preserve_both_views() {
    let (_dir, store) = open_test_store().await;
    const WRITERS: usize = 16;
    const READERS: usize = 8;
    let start = Arc::new(tokio::sync::Barrier::new(WRITERS + READERS));
    let now = crate::time_utils::now_ms();
    let mut writers = Vec::new();
    for index in 0..WRITERS {
        let writer = store.clone();
        let start = start.clone();
        writers.push(tokio::spawn(async move {
            start.wait().await;
            let timestamp = now + index as i64;
            writer
                .save_notification_delivery(
                    &format!("concurrent-delivery-{index:02}"),
                    &json!({
                        "id": format!("concurrent-delivery-{index:02}"),
                        "triggered_at": crate::time_utils::iso_from_ms(timestamp),
                        "status": "pending"
                    }),
                    timestamp,
                    600,
                    false,
                )
                .await
        }));
    }
    let mut readers = Vec::new();
    for _ in 0..READERS {
        let reader = store.clone();
        let start = start.clone();
        readers.push(tokio::spawn(async move {
            start.wait().await;
            for _ in 0..16 {
                reader
                    .load_notification_history("delivery")
                    .await
                    .expect("concurrent notification history read");
                tokio::task::yield_now().await;
            }
        }));
    }
    for writer in writers {
        writer.await.expect("join history writer").unwrap();
    }
    for reader in readers {
        reader.await.expect("join history reader");
    }
    assert_eq!(
        store
            .load_notification_history("delivery")
            .await
            .unwrap()
            .len(),
        WRITERS
    );
    assert_eq!(
        store
            .typed
            .typed_notifications
            .count_history("delivery")
            .await
            .unwrap(),
        WRITERS as i64
    );
}

#[tokio::test]
async fn typed_notification_history_nx_preserves_existing_record_and_repairs_index() {
    let (_dir, store) = open_test_store().await;
    let initial = json!({
        "id": "trigger-nx",
        "created_at": now_iso(),
        "status": "initial"
    });
    let duplicate = json!({
        "id": "trigger-nx",
        "created_at": now_iso(),
        "status": "duplicate"
    });
    assert!(
        store
            .save_notification_trigger(
                "trigger-nx",
                &initial,
                crate::time_utils::now_ms(),
                60,
                true,
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
                true,
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
    connection
        .execute_batch(
            "CREATE TRIGGER fail_typed_notification_history_insert
             BEFORE INSERT ON notification_history_documents
             BEGIN
               SELECT RAISE(ABORT, 'injected typed notification history failure');
             END;",
        )
        .unwrap();
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
    let delivery = json!({
        "id": "delivery-shadow",
        "triggered_at": now_iso(),
        "status": "pending"
    });
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
    connection
        .execute(
            "UPDATE notification_history_documents SET document_json = 'not-json'
             WHERE kind = 'delivery' AND id = ?1",
            ["delivery-shadow"],
        )
        .unwrap();
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
