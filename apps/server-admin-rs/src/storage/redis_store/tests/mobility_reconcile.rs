use super::*;

#[tokio::test]
async fn matched_session_authority_read_bypasses_the_primary_executor() {
    let (_dir, store) = open_test_store().await;
    let session_id = "typed-mobility-auth-reader";
    let session = new_login_session(session_id, "Auth reader", "192.0.2.90", "test", 3_600);
    store
        .add_session(session_id, &session, 3_600)
        .await
        .expect("seed matched session");

    let manager = store.manager.clone();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let blocker = manager.call(move |_conn| {
        let _ = started_tx.send(());
        release_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .map_err(|error| {
                crate::storage::storage_error(format!("release auth blocker: {error}"))
            })?;
        Ok(())
    });
    let read = async {
        started_rx.await.expect("primary executor started");
        let loaded = tokio::time::timeout(
            std::time::Duration::from_millis(250),
            store.get_session(session_id),
        )
        .await
        .expect("matched auth read must bypass primary storage")
        .expect("load session")
        .expect("session exists");
        assert_eq!(loaded.ip, session.ip);
        release_tx.send(()).expect("release primary executor");
    };
    let (blocker_result, ()) = tokio::join!(blocker, read);
    blocker_result.expect("primary blocker result");
}

#[tokio::test]
async fn queued_session_repair_returns_the_latest_authoritative_value() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let session_id = "typed-mobility-queued-repair";
    let initial = new_login_session(session_id, "Initial", "192.0.2.91", "test", 3_600);
    store
        .add_session(session_id, &initial, 3_600)
        .await
        .expect("seed matched session");

    let queued_snapshot = new_login_session(session_id, "Queued", "192.0.2.92", "test", 3_600);
    let connection = open_fixture_connection(&path);
    connection
        .execute(
            "UPDATE kv_strings SET value = ?2 WHERE key = ?1",
            tokio_rusqlite::rusqlite::params![
                crate::auth_session_keys::session_key(session_id),
                serde_json::to_string(&queued_snapshot).unwrap()
            ],
        )
        .expect("create session shadow mismatch");
    drop(connection);

    let blocker_manager = store.manager.clone();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let blocker = blocker_manager.call(move |_conn| {
        let _ = started_tx.send(());
        release_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .map_err(|error| {
                crate::storage::storage_error(format!("release session repair blocker: {error}"))
            })?;
        Ok(())
    });
    let load = store.get_session(session_id);
    let latest = new_login_session(session_id, "Latest", "192.0.2.93", "test", 3_600);
    let replace_while_queued = async {
        started_rx.await.expect("primary blocker started");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while store.primary_queue_status().queue_depth == 0 {
            assert!(
                std::time::Instant::now() < deadline,
                "session repair did not queue behind primary blocker"
            );
            tokio::task::yield_now().await;
        }
        let connection = open_fixture_connection(&path);
        connection
            .execute(
                "UPDATE kv_strings SET value = ?2 WHERE key = ?1",
                tokio_rusqlite::rusqlite::params![
                    crate::auth_session_keys::session_key(session_id),
                    serde_json::to_string(&latest).unwrap()
                ],
            )
            .expect("replace authority while repair is queued");
        drop(connection);
        release_tx.send(()).expect("release primary blocker");
    };
    let (blocker_result, loaded, ()) = tokio::join!(blocker, load, replace_while_queued);
    blocker_result.expect("primary blocker result");
    let loaded = loaded
        .expect("load repaired session")
        .expect("repaired session exists");
    assert_eq!(loaded.ip, "192.0.2.93");
    assert_eq!(loaded.credential_name, "Latest");
}

#[tokio::test]
async fn typed_mobility_rebuilds_after_backup_restore_and_clear() {
    let source_dir = tempfile::tempdir().expect("create source temp dir");
    let source = Store::connect(source_dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open source store");
    let session_id = "typed-mobility-backup";
    let session = new_login_session(
        session_id,
        "Typed mobility backup",
        "192.0.2.91",
        "test",
        3_600,
    );
    source
        .add_session(session_id, &session, 3_600)
        .await
        .expect("seed source session");
    assert!(
        source
            .add_auth_mobility_pending_whitelist(
                session_id,
                "whitelist:backup",
                "fn_knock:test:backup-owner",
                3_600,
            )
            .await
            .expect("seed source pending whitelist")
    );
    let entries = source
        .export_backup_entries_by_prefix_limited("fn_knock:", 1_000_000, |_| true)
        .await
        .expect("export compatibility backup");

    let target_dir = tempfile::tempdir().expect("create target temp dir");
    let target = Store::connect(target_dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open target store");
    target
        .replace_backup_entries_by_prefix("fn_knock:", &entries, 200)
        .await
        .expect("restore compatibility backup");
    let restored = target
        .typed
        .typed_mobility
        .load_session(session_id)
        .await
        .expect("load restored aggregate")
        .expect("restored aggregate exists");
    assert!(restored.session.is_some());
    assert_eq!(restored.pending_whitelist.len(), 1);

    target.clear_all_keys().await.expect("clear restored store");
    assert_eq!(target.typed.typed_mobility.counts().await.unwrap(), (0, 0));
}

#[tokio::test]
async fn typed_mobility_expiry_and_legacy_rewrite_are_reconciled() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let session_id = "typed-mobility-legacy-rewrite";
    let store = Store::connect(&path).await.expect("open store");
    let initial = new_login_session(
        session_id,
        "Before legacy rewrite",
        "192.0.2.92",
        "test",
        3_600,
    );
    store
        .add_session(session_id, &initial, 3_600)
        .await
        .expect("seed session");
    drop(store);

    let rewritten = new_login_session(
        session_id,
        "After legacy rewrite",
        "192.0.2.93",
        "legacy-test",
        3_600,
    );
    let connection = open_fixture_connection(&path);
    connection
        .execute(
            "UPDATE kv_strings SET value = ?2 WHERE key = ?1",
            tokio_rusqlite::rusqlite::params![
                crate::auth_session_keys::session_key(session_id),
                serde_json::to_string(&rewritten).unwrap()
            ],
        )
        .unwrap();
    drop(connection);

    let reopened = Store::connect(&path)
        .await
        .expect("reopen after legacy rewrite");
    let typed = reopened
        .typed
        .typed_mobility
        .load_session(session_id)
        .await
        .expect("load reconciled aggregate")
        .expect("reconciled aggregate exists");
    assert_eq!(
        typed
            .session
            .as_ref()
            .and_then(|session| session.value.get("credentialName"))
            .and_then(Value::as_str),
        Some("After legacy rewrite")
    );

    let connection = open_fixture_connection(&path);
    connection
        .execute(
            "UPDATE kv_keys SET expires_at_ms = ?2 WHERE key = ?1",
            tokio_rusqlite::rusqlite::params![
                crate::auth_session_keys::session_key(session_id),
                crate::time_utils::now_ms() - 1
            ],
        )
        .unwrap();
    drop(connection);
    assert!(
        reopened
            .get_session(session_id)
            .await
            .expect("read expired authoritative session")
            .is_none()
    );
    assert!(
        reopened
            .typed
            .typed_mobility
            .load_session(session_id)
            .await
            .expect("load expired aggregate")
            .is_none()
    );
    assert_eq!(reopened.purge_expired_keys().await.unwrap(), 0);
}

#[tokio::test]
async fn typed_mobility_reconcile_does_not_rewrite_unchanged_aggregates() {
    let (_dir, store) = open_test_store().await;
    for session_id in ["typed-mobility-changed", "typed-mobility-unchanged"] {
        let session = new_login_session(session_id, session_id, "192.0.2.94", "test", 3_600);
        store
            .add_session(session_id, &session, 3_600)
            .await
            .expect("seed typed mobility session");
    }
    let changed_before = store
        .typed
        .typed_mobility
        .aggregate_updated_at_ms("typed-mobility-changed")
        .await
        .unwrap()
        .unwrap();
    let unchanged_before = store
        .typed
        .typed_mobility
        .aggregate_updated_at_ms("typed-mobility-unchanged")
        .await
        .unwrap()
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;

    assert!(
        store
            .add_auth_mobility_pending_whitelist(
                "typed-mobility-changed",
                "whitelist:changed",
                "fn_knock:test:changed-owner",
                3_600,
            )
            .await
            .expect("mutate one typed mobility aggregate")
    );

    let changed_after = store
        .typed
        .typed_mobility
        .aggregate_updated_at_ms("typed-mobility-changed")
        .await
        .unwrap()
        .unwrap();
    let unchanged_after = store
        .typed
        .typed_mobility
        .aggregate_updated_at_ms("typed-mobility-unchanged")
        .await
        .unwrap()
        .unwrap();
    assert!(changed_after > changed_before);
    assert_eq!(unchanged_after, unchanged_before);
}

#[tokio::test]
async fn typed_mobility_incremental_sync_does_not_touch_unrelated_aggregates() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    for session_id in ["typed-mobility-target", "typed-mobility-unrelated"] {
        let session = new_login_session(session_id, session_id, "192.0.2.96", "test", 3_600);
        store
            .add_session(session_id, &session, 3_600)
            .await
            .expect("seed incremental session");
    }
    let mut conn = store.conn();
    conn.set(
        auth_mobility_binding_key("malformed", "unrelated"),
        "{not-json",
    )
    .await
    .expect("seed malformed unrelated compatibility binding");
    let connection = open_fixture_connection(&path);
    connection
        .execute(
            "UPDATE mobility_session_aggregates SET aggregate_json = 'unrelated-marker' WHERE session_id = 'typed-mobility-unrelated'",
            [],
        )
        .unwrap();
    drop(connection);

    assert!(
        store
            .add_auth_mobility_pending_whitelist(
                "typed-mobility-target",
                "whitelist:incremental-target",
                "fn_knock:test:incremental-target-owner",
                3_600,
            )
            .await
            .expect("incrementally mutate target session")
    );
    let connection = open_fixture_connection(&path);
    let unrelated_raw = connection
        .query_row(
            "SELECT aggregate_json FROM mobility_session_aggregates WHERE session_id = 'typed-mobility-unrelated'",
            [],
            |row| row.get::<_, String>(0),
        )
        .unwrap();
    assert_eq!(unrelated_raw, "unrelated-marker");
    drop(connection);

    assert!(
        store
            .add_auth_mobility_pending_whitelist(
                "typed-mobility-unrelated",
                "whitelist:incremental-repair",
                "fn_knock:test:incremental-repair-owner",
                3_600,
            )
            .await
            .expect("targeted corruption falls back to full repair")
    );
    let repaired = store
        .typed
        .typed_mobility
        .load_session("typed-mobility-unrelated")
        .await
        .expect("load repaired target")
        .expect("repaired target exists");
    assert_eq!(repaired.pending_whitelist.len(), 1);
}

#[tokio::test]
async fn typed_mobility_incrementally_reconciles_binding_and_owner_moves() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let first_session = "typed-mobility-owner-a";
    let second_session = "typed-mobility-owner-b";
    for session_id in [first_session, second_session] {
        let session = new_login_session(session_id, session_id, "192.0.2.95", "test", 3_600);
        store
            .add_session(session_id, &session, 3_600)
            .await
            .expect("seed owner session");
    }

    let subject = "incremental-owner-subject";
    let first_binding = json!({
        "ownerSessionId": first_session,
        "whitelistRecordId": "whitelist:incremental-binding"
    });
    assert!(
        store
            .save_auth_mobility_owned_binding(
                "fnos-token",
                subject,
                &first_binding,
                first_session,
                3_600,
                Some(3_600),
            )
            .await
            .expect("save first binding owner")
    );
    let second_binding = json!({
        "ownerSessionId": second_session,
        "whitelistRecordId": "whitelist:incremental-binding"
    });
    assert!(
        store
            .save_auth_mobility_owned_binding(
                "fnos-token",
                subject,
                &second_binding,
                second_session,
                3_600,
                Some(3_600),
            )
            .await
            .expect("move binding owner")
    );
    let subject_hash = auth_mobility_subject_hash("fnos-token", subject);
    let binding_key = auth_mobility_binding_key("fnos-token", &subject_hash);
    store
        .remove_auth_mobility_session_bindings(first_session, std::slice::from_ref(&binding_key))
        .await
        .expect("remove stale first owner index");
    let first_typed = store
        .typed
        .typed_mobility
        .load_session(first_session)
        .await
        .unwrap()
        .unwrap();
    let second_typed = store
        .typed
        .typed_mobility
        .load_session(second_session)
        .await
        .unwrap()
        .unwrap();
    assert!(first_typed.bindings.is_empty());
    assert_eq!(second_typed.bindings.len(), 1);

    assert!(
        store
            .save_auth_mobility_orphaned_binding(
                "fnos-token",
                subject,
                &json!({ "whitelistRecordId": "whitelist:incremental-binding" }),
                second_session,
            )
            .await
            .expect("orphan moved binding")
    );
    assert_eq!(store.typed.typed_mobility.counts().await.unwrap(), (2, 1));
    assert!(
        store
            .typed
            .typed_mobility
            .load_session(second_session)
            .await
            .unwrap()
            .unwrap()
            .bindings
            .is_empty()
    );
    assert!(
        store
            .save_auth_mobility_owned_binding(
                "fnos-token",
                subject,
                &second_binding,
                second_session,
                3_600,
                Some(3_600),
            )
            .await
            .expect("reclaim orphan binding")
    );
    assert_eq!(store.typed.typed_mobility.counts().await.unwrap(), (2, 0));

    let owner_record_id = "whitelist:incremental-owner";
    assert!(
        store
            .set_auth_mobility_whitelist_owner(owner_record_id, first_session, 3_600)
            .await
            .expect("set first whitelist owner")
    );
    assert!(
        store
            .set_auth_mobility_whitelist_owner(owner_record_id, second_session, 3_600)
            .await
            .expect("move whitelist owner")
    );
    let first_typed = store
        .typed
        .typed_mobility
        .load_session(first_session)
        .await
        .unwrap()
        .unwrap();
    let second_typed = store
        .typed
        .typed_mobility
        .load_session(second_session)
        .await
        .unwrap()
        .unwrap();
    assert!(first_typed.whitelist_owners.is_empty());
    assert_eq!(second_typed.whitelist_owners.len(), 1);

    let connection = open_fixture_connection(&path);
    connection
        .execute(
            "UPDATE mobility_session_aggregates SET aggregate_json = 'not-json' WHERE session_id = ?1",
            [first_session],
        )
        .unwrap();
    drop(connection);
    let mut conn = store.conn();
    conn.set("fn_knock:auth_mobility:future-key:opaque", "future-value")
        .await
        .expect("unknown mobility key uses full reconcile fallback");
    assert!(
        store
            .typed
            .typed_mobility
            .load_session(first_session)
            .await
            .expect("load aggregate repaired by fallback")
            .is_some()
    );
}

#[tokio::test]
async fn concurrent_mobility_writes_preserve_legacy_and_typed_aggregates() {
    const WRITERS: usize = 8;
    const READERS: usize = 4;
    let (_dir, store) = open_test_store().await;
    let start = Arc::new(tokio::sync::Barrier::new(WRITERS + READERS));
    let mut writers = Vec::new();
    for index in 0..WRITERS {
        let writer = store.clone();
        let start = start.clone();
        writers.push(tokio::spawn(async move {
            start.wait().await;
            let session_id = format!("typed-mobility-concurrent-{index}");
            let session = new_login_session(
                &session_id,
                &format!("Concurrent {index}"),
                &format!("192.0.2.{}", index + 100),
                "test",
                3_600,
            );
            writer.add_session(&session_id, &session, 3_600).await?;
            writer
                .add_auth_mobility_pending_whitelist(
                    &session_id,
                    &format!("whitelist:concurrent:{index}"),
                    &format!("fn_knock:test:concurrent-owner:{index}"),
                    3_600,
                )
                .await?;
            writer
                .save_auth_mobility_active_ip_detail(
                    &session_id,
                    &format!("192.0.2.{}", index + 100),
                    index as i64,
                    &json!({ "whitelistRecordId": format!("whitelist:concurrent:{index}") }),
                    3_600,
                )
                .await?;
            Ok::<(), crate::storage::StorageError>(())
        }));
    }
    let mut readers = Vec::new();
    for reader_index in 0..READERS {
        let reader = store.clone();
        let start = start.clone();
        readers.push(tokio::spawn(async move {
            start.wait().await;
            for iteration in 0..16 {
                let session_id = format!(
                    "typed-mobility-concurrent-{}",
                    (reader_index + iteration) % WRITERS
                );
                reader
                    .list_auth_mobility_session_whitelist_ids(&session_id)
                    .await?;
                tokio::task::yield_now().await;
            }
            Ok::<(), crate::storage::StorageError>(())
        }));
    }
    for writer in writers {
        writer.await.expect("join mobility writer").unwrap();
    }
    for reader in readers {
        reader.await.expect("join mobility reader").unwrap();
    }

    assert_eq!(store.typed.typed_mobility.counts().await.unwrap(), (8, 0));
    for index in 0..WRITERS {
        let session_id = format!("typed-mobility-concurrent-{index}");
        let expected = vec![format!("whitelist:concurrent:{index}")];
        assert_eq!(
            store
                .list_auth_mobility_session_whitelist_ids(&session_id)
                .await
                .unwrap(),
            expected
        );
        let typed = store
            .typed
            .typed_mobility
            .load_session(&session_id)
            .await
            .unwrap()
            .expect("typed concurrent aggregate");
        assert!(typed.session.is_some());
        assert_eq!(typed.pending_whitelist.len(), 1);
        assert_eq!(typed.active_ips.len(), 1);
    }
}
