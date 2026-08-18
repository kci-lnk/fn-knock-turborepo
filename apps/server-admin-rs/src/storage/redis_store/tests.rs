use super::*;
use tokio_rusqlite::OptionalExtension;

#[tokio::test]
async fn passkey_usage_updates_the_persisted_webauthn_credential_monotonically() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("passkey-counter.sqlite3"))
        .await
        .expect("open store");
    store
        .add_passkey(&json!({
            "id": "credential-1",
            "counter": 3,
            "backupEligible": true,
            "backupState": false,
            "webauthnCredential": {
                "counter": 7,
                "backup_eligible": true,
                "backup_state": false
            }
        }))
        .await
        .expect("seed passkey");

    assert!(
        store
            .update_passkey_counter(
                "credential-1",
                5,
                "2026-08-06T00:00:00Z",
                Some(false),
                Some(true),
            )
            .await
            .expect("update passkey")
    );
    let updated = store.get_passkeys().await.expect("load updated passkey");
    assert_eq!(updated[0]["counter"], json!(7));
    assert_eq!(updated[0]["backupEligible"], json!(true));
    assert_eq!(updated[0]["backupState"], json!(true));
    assert_eq!(updated[0]["webauthnCredential"]["counter"], json!(7));
    assert_eq!(
        updated[0]["webauthnCredential"]["backup_eligible"],
        json!(true)
    );
    assert_eq!(
        updated[0]["webauthnCredential"]["backup_state"],
        json!(true)
    );

    let lower = store.clone();
    let higher = store.clone();
    let (lower_result, higher_result) = tokio::join!(
        lower.update_passkey_counter(
            "credential-1",
            8,
            "2026-08-06T00:00:01Z",
            Some(true),
            Some(true),
        ),
        higher.update_passkey_counter(
            "credential-1",
            12,
            "2026-08-06T00:00:02Z",
            Some(true),
            Some(true),
        )
    );
    assert!(lower_result.expect("lower concurrent update"));
    assert!(higher_result.expect("higher concurrent update"));
    let updated = store.get_passkeys().await.expect("load final passkey");
    assert_eq!(updated[0]["counter"], json!(12));
    assert_eq!(updated[0]["webauthnCredential"]["counter"], json!(12));

    let first_add = store.clone();
    let second_add = store.clone();
    let second_credential = json!({
        "id": "credential-2",
        "counter": 0,
        "webauthnCredential": { "counter": 0 }
    });
    let third_credential = json!({
        "id": "credential-3",
        "counter": 0,
        "webauthnCredential": { "counter": 0 }
    });
    let (first_add_result, second_add_result) = tokio::join!(
        first_add.add_passkey(&second_credential),
        second_add.add_passkey(&third_credential)
    );
    first_add_result.expect("first concurrent insertion");
    second_add_result.expect("second concurrent insertion");
    let inserted = store.get_passkeys().await.expect("load inserted passkeys");
    assert_eq!(inserted.len(), 3);

    let deleting = store.clone();
    let updating = store.clone();
    let (delete_result, update_result) = tokio::join!(
        deleting.delete_passkey("credential-1"),
        updating.update_passkey_counter(
            "credential-1",
            15,
            "2026-08-06T00:00:03Z",
            Some(true),
            Some(true),
        )
    );
    assert!(delete_result.expect("concurrent deletion"));
    let _ = update_result.expect("concurrent update during deletion");
    let remaining = store.get_passkeys().await.expect("load remaining passkeys");
    assert_eq!(remaining.len(), 2);
    assert!(
        remaining
            .iter()
            .all(|passkey| passkey["id"] != json!("credential-1"))
    );
}

#[test]
fn sorts_backup_strings_like_node_locale_compare() {
    let mut values = [
        "fn_knock:a",
        "fn_knock:Z",
        "fn_knock:A",
        "fn_knock:z",
        "fn_knock:_",
        "fn_knock:-",
        "fn_knock:2",
        "fn_knock:10",
        "fn_knock:á",
        "fn_knock:ä",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<Vec<_>>();

    values.sort_by(|left, right| node_locale_compare_ordering(left, right));

    assert_eq!(
        values,
        vec![
            "fn_knock:_",
            "fn_knock:-",
            "fn_knock:10",
            "fn_knock:2",
            "fn_knock:a",
            "fn_knock:A",
            "fn_knock:á",
            "fn_knock:ä",
            "fn_knock:z",
            "fn_knock:Z",
        ]
    );
    assert_eq!(node_locale_compare_ordering("a", "Z"), Ordering::Less);
    assert_eq!(node_locale_compare_ordering("😀", "0"), Ordering::Less);
    assert_eq!(node_locale_compare_ordering("中", "z"), Ordering::Greater);
}

#[tokio::test]
async fn clear_all_keys_removes_the_complete_keyspace_and_preserves_storage_metadata() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    store
        .set_storage_meta_value("redis_migration_status", "done")
        .await
        .expect("seed storage metadata");

    let mut conn = store.conn();
    let _: () = redis::cmd("SET")
        .arg("fn_knock:test:string")
        .arg("value")
        .query_async(&mut conn)
        .await
        .expect("seed string");
    let _: () = redis::cmd("HSET")
        .arg("fn_knock:test:hash")
        .arg("field")
        .arg("value")
        .query_async(&mut conn)
        .await
        .expect("seed hash");
    let _: () = redis::cmd("RPUSH")
        .arg("fn_knock:test:list")
        .arg("value")
        .query_async(&mut conn)
        .await
        .expect("seed list");
    let _: () = redis::cmd("SADD")
        .arg("other:test:set")
        .arg("value")
        .query_async(&mut conn)
        .await
        .expect("seed set outside app prefix");
    let _: () = redis::cmd("ZADD")
        .arg("other:test:zset")
        .arg(1)
        .arg("value")
        .query_async(&mut conn)
        .await
        .expect("seed zset outside app prefix");
    let _: String = redis::cmd("XADD")
        .arg("other:test:stream")
        .arg("1-0")
        .arg("field")
        .arg("value")
        .query_async(&mut conn)
        .await
        .expect("seed stream outside app prefix");

    let cleared = store.clear_all_keys().await.expect("clear keyspace");

    assert_eq!(cleared, 6);
    assert!(store.scan_keys("", 100).await.unwrap().is_empty());
    assert_eq!(
        store
            .storage_meta_value("redis_migration_status")
            .await
            .unwrap()
            .as_deref(),
        Some("done")
    );
    let typed = store
        .typed_config
        .load()
        .await
        .expect("load typed config after clearing")
        .expect("typed config exists after clearing");
    assert_eq!(typed.document, default_config());
    assert_eq!(typed.host_mappings_generation, 0);
}

#[tokio::test]
async fn typed_config_bootstraps_idempotently_without_advancing_the_legacy_schema() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");

    let initial = store
        .typed_config
        .load()
        .await
        .expect("load initial typed config")
        .expect("initial typed config exists");
    let mut initial_legacy = store
        .get_config()
        .await
        .expect("load initial legacy config");
    strip_internal_config_metadata(&mut initial_legacy);
    assert_eq!(initial.document, initial_legacy);
    assert_eq!(initial.host_mappings_generation, 0);

    let mut updated = store
        .set_config_top_level_value("typed_repository_test", json!({ "enabled": true }))
        .await
        .expect("dual-write config");
    strip_internal_config_metadata(&mut updated);
    let after_write = store
        .typed_config
        .load()
        .await
        .expect("load typed config after write")
        .expect("typed config exists after write");
    assert_eq!(after_write.document, updated);
    assert_eq!(after_write.host_mappings_generation, 0);
    assert!(after_write.revision > initial.revision);
    let revision_after_write = after_write.revision;

    let schema_versions = store
        .manager
        .call(|conn| {
            let legacy =
                conn.query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
                    row.get::<_, Option<i64>>(0)
                })?;
            let typed = conn.query_row(
                "SELECT MAX(version) FROM typed_schema_migrations",
                [],
                |row| row.get::<_, Option<i64>>(0),
            )?;
            Ok((legacy, typed))
        })
        .await
        .expect("load schema versions");
    assert_eq!(schema_versions, (Some(2), Some(1)));
    drop(store);

    let reopened = Store::connect(&path).await.expect("reopen store");
    let after_reopen = reopened
        .typed_config
        .load()
        .await
        .expect("load typed config after reopen")
        .expect("typed config exists after reopen");
    assert_eq!(after_reopen.document, updated);
    assert_eq!(after_reopen.revision, revision_after_write);
}

#[tokio::test]
async fn stale_typed_revision_cannot_replace_a_newer_in_memory_config_snapshot() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    let stale = store.get_config().await.expect("load stale candidate");
    let stale_revision = store
        .typed_config
        .load()
        .await
        .unwrap()
        .expect("typed config exists")
        .revision;

    let fresh = store
        .set_config_top_level_value("newer_snapshot", json!(true))
        .await
        .expect("publish newer config");
    assert_eq!(store.config_snapshot()["newer_snapshot"], json!(true));

    store.publish_config_snapshot(stale, stale_revision);
    assert_eq!(store.config_snapshot().as_ref(), &fresh);
}

#[tokio::test]
async fn typed_config_mismatch_falls_back_to_legacy_and_repairs_the_typed_primary() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    let mut expected = store
        .set_config_top_level_value("legacy_authoritative", json!(true))
        .await
        .expect("seed config");
    strip_internal_config_metadata(&mut expected);

    store
        .manager
        .call(|conn| {
            conn.execute(
                "UPDATE config_documents SET document_json = ?1 WHERE singleton = 1",
                [r#"{"typed_only":true}"#],
            )?;
            Ok(())
        })
        .await
        .expect("inject typed shadow mismatch");

    let mut loaded = store.get_config().await.expect("load authoritative config");
    strip_internal_config_metadata(&mut loaded);
    assert_eq!(loaded, expected);
    assert_eq!(store.typed_config_shadow_mismatch_count(), 1);
    let mismatched_status = store.typed_config_shadow_status();
    assert_eq!(mismatched_status.phase, "typed_primary");
    assert!(mismatched_status.healthy);
    let repaired = store
        .typed_config
        .load()
        .await
        .expect("load repaired typed config")
        .expect("repaired typed config exists");
    assert_eq!(repaired.document, expected);
    let _ = store.get_config().await.expect("verify repaired shadow");
    assert_eq!(store.typed_config_shadow_mismatch_count(), 1);
    assert!(store.typed_config_shadow_status().healthy);
}

#[tokio::test]
async fn corrupt_typed_config_falls_back_to_legacy_and_recovers_the_typed_primary() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    let mut expected = store
        .set_config_top_level_value("legacy_survives_typed_corruption", json!(true))
        .await
        .expect("seed config");
    strip_internal_config_metadata(&mut expected);

    store
        .manager
        .call(|conn| {
            conn.execute(
                "UPDATE config_documents SET document_json = 'not-json' WHERE singleton = 1",
                [],
            )?;
            Ok(())
        })
        .await
        .expect("corrupt typed document");

    let mut loaded = store
        .get_config()
        .await
        .expect("legacy read remains available");
    strip_internal_config_metadata(&mut loaded);
    assert_eq!(loaded, expected);
    assert_eq!(store.typed_config_shadow_mismatch_count(), 1);
    assert!(store.typed_config_shadow_status().healthy);
    let repaired = store
        .typed_config
        .load()
        .await
        .expect("load repaired typed config")
        .expect("typed config is repaired from legacy fallback");
    assert_eq!(repaired.document, expected);
}

#[tokio::test]
async fn missing_typed_document_after_bootstrap_is_observable_and_repaired() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    let mut expected = store
        .set_config_top_level_value("typed_document_recovery", json!(true))
        .await
        .expect("seed config");
    strip_internal_config_metadata(&mut expected);

    store
        .manager
        .call(|conn| {
            conn.execute("DELETE FROM config_documents WHERE singleton = 1", [])?;
            Ok(())
        })
        .await
        .expect("remove typed primary document");

    let mut loaded = store
        .get_config()
        .await
        .expect("legacy fallback remains available");
    strip_internal_config_metadata(&mut loaded);
    assert_eq!(loaded, expected);
    assert_eq!(store.typed_config_shadow_mismatch_count(), 1);
    assert!(store.typed_config_shadow_status().healthy);
    let repaired = store
        .typed_config
        .load()
        .await
        .expect("load repaired typed document")
        .expect("typed document is restored");
    assert_eq!(repaired.document, expected);
}

#[tokio::test]
async fn typed_config_failure_rolls_back_the_legacy_config_transaction() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    let legacy_before = store.get_string_value(CONFIG_KEY).await.unwrap();
    let generation_before = store
        .get_string_value(HOST_MAPPINGS_GENERATION_KEY)
        .await
        .unwrap();
    let typed_before = store
        .typed_config
        .load()
        .await
        .expect("load typed config before failure")
        .expect("typed config exists before failure");

    store
        .manager
        .call(|conn| {
            conn.execute_batch(
                "CREATE TRIGGER fail_typed_config_update
                 BEFORE UPDATE ON config_documents
                 BEGIN
                   SELECT RAISE(ABORT, 'injected typed config failure');
                 END;",
            )?;
            Ok(())
        })
        .await
        .expect("install failure trigger");

    let error = store
        .set_config_top_level_value("must_roll_back", json!(true))
        .await
        .expect_err("typed write failure must reject the whole config transaction");
    assert!(error.to_string().contains("injected typed config failure"));
    assert_eq!(
        store.get_string_value(CONFIG_KEY).await.unwrap(),
        legacy_before
    );
    assert_eq!(
        store
            .get_string_value(HOST_MAPPINGS_GENERATION_KEY)
            .await
            .unwrap(),
        generation_before
    );
    let typed_after_failure = store
        .typed_config
        .load()
        .await
        .expect("load typed config after failure")
        .expect("typed config exists after failure");
    assert_eq!(typed_after_failure.document, typed_before.document);
    assert_eq!(typed_after_failure.revision, typed_before.revision);

    store
        .manager
        .call(|conn| {
            conn.execute_batch("DROP TRIGGER fail_typed_config_update;")?;
            Ok(())
        })
        .await
        .expect("remove failure trigger");
    let updated = store
        .set_config_top_level_value("must_roll_back", json!(true))
        .await
        .expect("retry config write");
    assert_eq!(updated["must_roll_back"], json!(true));
}

#[tokio::test]
async fn concurrent_config_writes_keep_typed_and_legacy_documents_in_sync() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    let initial_revision = store
        .typed_config
        .load()
        .await
        .unwrap()
        .expect("initial typed config exists")
        .revision;

    const WRITERS: usize = 16;
    const READERS: usize = 8;
    let start = Arc::new(tokio::sync::Barrier::new(WRITERS + READERS));
    let mut writes = Vec::new();
    for index in 0..WRITERS {
        let writer = store.clone();
        let start = start.clone();
        writes.push(tokio::spawn(async move {
            start.wait().await;
            let key = format!("concurrent_typed_write_{index}");
            writer.set_config_top_level_value(&key, json!(index)).await
        }));
    }
    let mut reads = Vec::new();
    for _ in 0..READERS {
        let reader = store.clone();
        let start = start.clone();
        reads.push(tokio::spawn(async move {
            start.wait().await;
            for _ in 0..64 {
                reader.get_config().await.expect("concurrent shadow read");
                tokio::task::yield_now().await;
            }
        }));
    }
    for write in writes {
        write.await.expect("join concurrent writer").unwrap();
    }
    for read in reads {
        read.await.expect("join concurrent reader");
    }

    let mut legacy = store.get_config().await.expect("load final legacy config");
    strip_internal_config_metadata(&mut legacy);
    for index in 0..WRITERS {
        assert_eq!(
            legacy[format!("concurrent_typed_write_{index}")],
            json!(index)
        );
    }
    let typed = store
        .typed_config
        .load()
        .await
        .expect("load final typed config")
        .expect("final typed config exists");
    assert_eq!(typed.document, legacy);
    assert_eq!(typed.host_mappings_generation, 0);
    assert!(typed.revision >= initial_revision + WRITERS as u64);
    assert_eq!(store.typed_config_shadow_mismatch_count(), 0);
}

#[tokio::test]
async fn startup_reconciles_writes_made_by_a_legacy_binary() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let typed_revision = store
        .typed_config
        .load()
        .await
        .unwrap()
        .expect("typed config exists")
        .revision;
    let mut legacy = store.get_config().await.expect("load legacy config");
    strip_internal_config_metadata(&mut legacy);
    legacy["legacy_binary_write"] = json!({ "survives": true });
    let legacy_raw = serde_json::to_string(&legacy).expect("serialize legacy config");
    let mut conn = store.conn();
    let _: () = redis::cmd("SET")
        .arg(CONFIG_KEY)
        .arg(legacy_raw)
        .query_async(&mut conn)
        .await
        .expect("simulate old binary write");
    drop(conn);
    drop(store);

    let reopened = Store::connect(&path).await.expect("reopen upgraded store");
    let typed = reopened
        .typed_config
        .load()
        .await
        .expect("load reconciled typed config")
        .expect("reconciled typed config exists");
    assert_eq!(typed.document, legacy);
    assert!(typed.revision > typed_revision);
    let mut loaded = reopened.get_config().await.expect("load upgraded config");
    strip_internal_config_metadata(&mut loaded);
    assert_eq!(loaded, legacy);
}

#[tokio::test]
async fn backup_restore_roundtrips_stream_field_order_and_duplicates() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    let entry = json!({
        "key": "fn_knock:test:stream",
        "type": "stream",
        "ttl_ms": null,
        "value": [
            {
                "id": "1-0",
                "fields": ["z", "last", "a", "first", "z", "again"]
            }
        ]
    });

    let cleared = store
        .replace_backup_entries_by_prefix("fn_knock:", &[entry], 200)
        .await
        .expect("restore entry");
    assert_eq!(cleared, 0);

    let exported = store
        .export_backup_entry("fn_knock:test:stream")
        .await
        .expect("export entry")
        .expect("entry exists");
    assert_eq!(
        exported["value"][0]["fields"],
        json!(["z", "last", "a", "first", "z", "again"])
    );
}

#[tokio::test]
async fn backup_prefix_replace_ignores_imported_host_generation_and_sets_trusted_value() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    store
        .save_config(&json!({
            "host_mappings": [{ "host": "before.example.com" }]
        }))
        .await
        .expect("seed config and generation");
    let restored_config = json!({
        "host_mapping_groups": [{
            "id": "11111111-1111-4111-8111-111111111111",
            "name": "Restored"
        }],
        "host_mappings": [{
            "host": "restored.example.com",
            "group_id": "11111111-1111-4111-8111-111111111111"
        }],
        "unrelated": { "restored": true }
    });
    let entries = vec![
        json!({
            "key": CONFIG_KEY,
            "type": "string",
            "ttl_ms": null,
            "value": restored_config.to_string()
        }),
        json!({
            "key": HOST_MAPPINGS_GENERATION_KEY,
            "type": "string",
            "ttl_ms": null,
            "value": "999999"
        }),
    ];

    store
        .replace_backup_entries_by_prefix("fn_knock:", &entries, 200)
        .await
        .expect("replace backup prefix");

    assert_eq!(
        store
            .get_string_value(HOST_MAPPINGS_GENERATION_KEY)
            .await
            .unwrap()
            .as_deref(),
        Some("2")
    );
    let restored = store.get_config().await.unwrap();
    assert_eq!(restored["host_mappings"], restored_config["host_mappings"]);
    assert_eq!(
        restored["host_mapping_groups"],
        restored_config["host_mapping_groups"]
    );
    assert_eq!(restored["unrelated"], restored_config["unrelated"]);
    let typed = store
        .typed_config
        .load()
        .await
        .expect("load typed config after backup restore")
        .expect("typed config exists after backup restore");
    assert_eq!(typed.document, restored_config);
    assert_eq!(typed.host_mappings_generation, 2);
}

#[tokio::test]
async fn poll_log_buffer_recovers_when_sequence_lags_existing_items() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    let key = "fn_knock:test:logs";
    let seq_key = format!("{key}:seq");
    let mut conn = store.conn();
    let _: () = redis::cmd("RPUSH")
        .arg(key)
        .arg("old")
        .arg("middle")
        .arg("latest")
        .query_async(&mut conn)
        .await
        .expect("seed log list");
    conn.set(&seq_key, 2).await.expect("seed stale seq");

    let result = store
        .poll_log_buffer(key, Some("2"))
        .await
        .expect("poll logs");

    assert_eq!(result["cursor"], json!(3));
    assert_eq!(result["reset"], json!(false));
    assert_eq!(result["items"], json!(["latest"]));
}

#[tokio::test]
async fn append_log_buffer_continues_sequence_from_existing_items_without_seq() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    let key = "fn_knock:test:logs-without-seq";
    let mut conn = store.conn();
    let _: () = redis::cmd("RPUSH")
        .arg(key)
        .arg("old")
        .query_async(&mut conn)
        .await
        .expect("seed log list without seq");

    store
        .append_log_buffer(key, &["new-1".to_string(), "new-2".to_string()], 60, 100)
        .await
        .expect("append logs");

    let result = store
        .poll_log_buffer(key, Some("1"))
        .await
        .expect("poll appended logs");
    assert_eq!(result["cursor"], json!(3));
    assert_eq!(result["reset"], json!(false));
    assert_eq!(result["items"], json!(["new-1", "new-2"]));

    let empty = store
        .poll_log_buffer(key, Some("3"))
        .await
        .expect("poll after current cursor");
    assert_eq!(empty["cursor"], json!(3));
    assert_eq!(empty["items"], json!([]));
}

#[tokio::test]
async fn json_locks_refresh_and_release_only_for_the_owner() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    let key = "fn_knock:test:json-lock";
    let initial = json!({ "lockId": "owner", "createdAt": "initial" });
    assert!(
        store
            .set_json_value_nx_ex(key, &initial, 30)
            .await
            .expect("acquire JSON lock")
    );

    assert!(
        !store
            .set_json_lock_if_owned_ex(
                key,
                "other",
                &json!({ "lockId": "other", "createdAt": "wrong" }),
                120,
            )
            .await
            .expect("reject wrong owner refresh")
    );
    assert_eq!(
        store.get_json_value(key).await.unwrap(),
        Some(initial.clone())
    );

    let refreshed = json!({ "lockId": "owner", "createdAt": "refreshed" });
    assert!(
        store
            .set_json_lock_if_owned_ex(key, "owner", &refreshed, 120)
            .await
            .expect("refresh owned lock")
    );
    assert_eq!(store.get_json_value(key).await.unwrap(), Some(refreshed));
    let mut conn = store.conn();
    assert!(conn.ttl(key).await.expect("read refreshed TTL") > 30);

    assert!(
        !store
            .delete_lock_if_owned(key, "other")
            .await
            .expect("reject wrong owner release")
    );
    assert!(store.get_json_value(key).await.unwrap().is_some());
    assert!(
        store
            .delete_lock_if_owned(key, "owner")
            .await
            .expect("release owned lock")
    );
    assert_eq!(store.get_json_value(key).await.unwrap(), None);
    assert!(
        !store
            .delete_lock_if_owned(key, "owner")
            .await
            .expect("repeat release")
    );

    store
        .set_string_value(key, "not-json")
        .await
        .expect("seed invalid JSON lock");
    assert!(
        !store
            .delete_lock_if_owned(key, "owner")
            .await
            .expect("invalid JSON is not owned")
    );
    assert_eq!(
        store.get_string_value(key).await.unwrap().as_deref(),
        Some("not-json")
    );
}

#[tokio::test]
async fn host_mapping_section_cas_requires_an_exact_array_and_preserves_other_sections() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    let structurally_invalid = json!({
        "host_mappings": {},
        "unrelated": { "generation": 7 }
    });
    store
        .save_config(&structurally_invalid)
        .await
        .expect("seed non-array host mappings");

    assert!(
        store
            .compare_and_set_host_mappings(&[], &[json!({ "host": "a.example.com" })])
            .await
            .expect("compare structural mismatch")
            .is_none()
    );
    let mut stored = store.get_config().await.unwrap();
    strip_internal_config_metadata(&mut stored);
    assert_eq!(stored, structurally_invalid);

    let expected = vec![json!({
        "host": "a.example.com",
        "target": "http://127.0.0.1:8080"
    })];
    store
        .replace_config(&json!({
            "host_mappings": expected,
            "unrelated": { "generation": 8 }
        }))
        .await
        .expect("seed valid host mappings");
    let expected = store.get_config().await.unwrap()["host_mappings"]
        .as_array()
        .cloned()
        .unwrap();
    let mut forbidden_full_writer = store.get_config().await.unwrap();
    forbidden_full_writer["host_mappings"] = json!([{
        "host": "forbidden.example.com"
    }]);
    assert!(store.save_config(&forbidden_full_writer).await.is_err());
    assert_eq!(
        store.get_config().await.unwrap()["host_mappings"],
        json!(expected)
    );
    let replacement = vec![json!({
        "host": "a.example.com",
        "target": "http://127.0.0.1:8080",
        "protocol_mode": "http1"
    })];
    let updated = store
        .compare_and_set_host_mappings(&expected, &replacement)
        .await
        .expect("apply exact host mappings CAS")
        .expect("expected value matched");

    assert_eq!(updated["host_mappings"], json!(replacement));
    assert_eq!(updated["unrelated"]["generation"], json!(8));
    assert_eq!(store.get_config().await.unwrap(), updated);

    let emptied = store
        .compare_and_set_host_mappings(&replacement, &[])
        .await
        .expect("replace host mappings with an empty array")
        .expect("non-empty expected value matched");
    assert_eq!(emptied["host_mappings"], json!([]));
    assert!(emptied["host_mappings"].is_array());
    assert_eq!(emptied["unrelated"]["generation"], json!(8));
    assert_eq!(store.get_config().await.unwrap(), emptied);
}

#[tokio::test]
async fn host_mapping_policy_cas_keeps_advanced_auth_references_and_restores_old_policies() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    let old_mappings = vec![json!({
        "host": "app.example.com",
        "visibility": { "policy_id": "visibility-old" },
        "advanced_auth": {
            "groups": [{
                "conditions": [{ "policy_id": "advanced-old" }]
            }]
        }
    })];
    let old_policies = json!({
        "visibility-old": { "format_version": 2 },
        "advanced-old": { "format_version": 2 },
        "unreferenced": { "format_version": 2 }
    });
    store
        .save_config(&json!({
            "host_mappings": old_mappings,
            "visibility_policies": old_policies,
        }))
        .await
        .expect("seed config");

    let new_mappings = vec![json!({
        "host": "app.example.com",
        "visibility": { "policy_id": "visibility-old" },
        "advanced_auth": {
            "groups": [{
                "conditions": [{ "policy_id": "advanced-new" }]
            }]
        }
    })];
    let supplied = json!({
        "visibility-old": { "format_version": 2 },
        "advanced-new": { "format_version": 2 }
    })
    .as_object()
    .cloned()
    .unwrap();
    let updated = store
        .compare_and_set_host_mappings_with_visibility_policies(
            &old_mappings,
            &new_mappings,
            &supplied,
        )
        .await
        .expect("update mappings and policies")
        .expect("old mappings matched");
    assert!(
        updated["visibility_policies"]
            .get("visibility-old")
            .is_some()
    );
    assert!(updated["visibility_policies"].get("advanced-new").is_some());
    assert!(updated["visibility_policies"].get("advanced-old").is_none());
    assert!(updated["visibility_policies"].get("unreferenced").is_none());

    let old_policies = old_policies.as_object().cloned().unwrap();
    let restored = store
        .compare_and_set_host_mappings_with_visibility_policies(
            &new_mappings,
            &old_mappings,
            &old_policies,
        )
        .await
        .expect("rollback mappings and policies")
        .expect("new mappings matched");
    assert!(
        restored["visibility_policies"]
            .get("visibility-old")
            .is_some()
    );
    assert!(
        restored["visibility_policies"]
            .get("advanced-old")
            .is_some()
    );
    assert!(
        restored["visibility_policies"]
            .get("advanced-new")
            .is_none()
    );
}

#[tokio::test]
async fn config_snapshot_is_published_immediately_after_save_and_host_cas() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    let before = store.config_snapshot();
    assert!(before.get("snapshot_test").is_none());

    let mut saved = (*before).clone();
    saved["snapshot_test"] = json!({"generation": 1});
    store.save_config(&saved).await.expect("save config");
    let after_save = store.config_snapshot();
    assert_eq!(
        after_save.pointer("/snapshot_test/generation"),
        Some(&json!(1))
    );
    assert!(!Arc::ptr_eq(&before, &store.config_snapshot()));

    let replacement = vec![json!({
        "host": "snapshot.example.com",
        "target": "http://127.0.0.1:8080"
    })];
    store
        .compare_and_set_host_mappings(&[], &replacement)
        .await
        .expect("host CAS")
        .expect("host CAS matched");
    let after_cas = store.config_snapshot();
    assert_eq!(after_cas["host_mappings"], json!(replacement));
    assert_eq!(
        after_cas.pointer("/snapshot_test/generation"),
        Some(&json!(1))
    );
}

#[tokio::test]
async fn host_mapping_catalog_cas_updates_both_sections_atomically() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    let mappings = vec![json!({
        "host": "app.example.com",
        "target": "http://127.0.0.1:8080",
        "group_id": null
    })];
    store
        .save_config(&json!({
            "host_mappings": mappings,
            "host_mapping_groups": [],
            "unrelated": { "preserved": true }
        }))
        .await
        .expect("seed config");
    let groups = vec![json!({
        "id": "11111111-1111-4111-8111-111111111111",
        "name": "Media"
    })];
    let grouped = vec![json!({
        "host": "app.example.com",
        "target": "http://127.0.0.1:8080",
        "group_id": "11111111-1111-4111-8111-111111111111"
    })];
    let updated = store
        .compare_and_set_host_mapping_catalog(&mappings, &[], false, &grouped, &groups, true)
        .await
        .expect("catalog CAS")
        .expect("catalog matched");
    assert_eq!(updated["host_mappings"], json!(grouped));
    assert_eq!(updated["host_mapping_groups"], json!(groups));
    assert_eq!(updated["host_mapping_grouped_view"], json!(true));
    assert_eq!(updated["unrelated"]["preserved"], json!(true));

    assert!(
        store
            .compare_and_set_host_mapping_catalog(&mappings, &[], false, &[], &[], false)
            .await
            .expect("stale catalog CAS")
            .is_none()
    );
}

#[tokio::test]
async fn gateway_target_section_merge_preserves_newer_config_and_section_writes() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let stale_store = Store::connect(&path).await.expect("open stale store");
    let newer_store = Store::connect(&path).await.expect("open newer store");
    stale_store
        .save_config(&json!({
            "run_type": 3,
            "host_mappings": [{
                "host": "video.example.com",
                "target": "http://127.0.0.1:8080",
                "protocol_mode": "http1"
            }],
            "gateway_proxy_headers": { "disabled_hosts": [] },
            "gateway_host_response": { "disabled_hosts": [] },
            "unrelated": { "generation": 1 }
        }))
        .await
        .expect("seed config");

    let stale = stale_store.get_config().await.expect("load stale config");
    let expected_proxy_headers = stale.get("gateway_proxy_headers").cloned();
    let expected_host_response = stale.get("gateway_host_response").cloned();

    let mut newer = newer_store.get_config().await.expect("load newer config");
    newer["run_type"] = json!(0);
    newer["gateway_proxy_headers"] = json!({
        "disabled_hosts": ["video.example.com"]
    });
    newer["unrelated"]["generation"] = json!(2);
    newer_store
        .save_config(&newer)
        .await
        .expect("save interleaved config update");

    let merged = stale_store
        .merge_gateway_target_config_sections(
            expected_proxy_headers.as_ref(),
            &json!({ "disabled_hosts": [] }),
            expected_host_response.as_ref(),
            &json!({ "disabled_hosts": ["video.example.com"] }),
        )
        .await
        .expect("merge stale target sections");

    assert_eq!(merged["run_type"], json!(0));
    assert_eq!(merged["unrelated"]["generation"], json!(2));
    assert_eq!(
        merged["gateway_proxy_headers"],
        json!({ "disabled_hosts": ["video.example.com"] }),
        "a newer write to the same section must win"
    );
    assert_eq!(
        merged["gateway_host_response"],
        json!({ "disabled_hosts": ["video.example.com"] }),
        "an unchanged section may receive the compiled replacement"
    );
    assert_eq!(stale_store.get_config().await.unwrap(), merged);
}

#[tokio::test]
async fn json_rewrite_preserves_ttl_without_resurrecting_expired_keys() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    let key = "fn_knock:test:preserve-ttl";
    store
        .set_json_value_ex(key, &json!({ "version": 1 }), 120)
        .await
        .unwrap();
    let (_, ttl) = store.get_json_value_with_ttl(key).await.unwrap();
    store
        .set_json_value_preserve_ttl(key, &json!({ "version": 2 }), ttl)
        .await
        .unwrap();
    let (_, rewritten_ttl) = store.get_json_value_with_ttl(key).await.unwrap();
    assert!(rewritten_ttl > 0 && rewritten_ttl <= ttl);

    store
        .set_json_value_preserve_ttl(key, &json!({ "version": 3 }), -2)
        .await
        .unwrap();
    assert!(store.get_json_value(key).await.unwrap().is_none());
}

#[tokio::test]
async fn object_field_merge_preserves_concurrent_scan_discovery_writes() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let target_store = Store::connect(&path).await.expect("open target store");
    let intensity_store = Store::connect(&path).await.expect("open intensity store");
    target_store
        .save_config(&json!({
            "host_mappings": [],
            "scan_discovery": {
                "custom_cidrs": [],
                "selected_cidrs": [],
                "intensity_mode": "auto",
                "intensity_level": "medium"
            }
        }))
        .await
        .expect("seed scan discovery config");

    let targets = target_store.merge_config_object_fields(
        "scan_discovery",
        [
            ("custom_cidrs".to_string(), json!(["10.0.0.0/24"])),
            ("selected_cidrs".to_string(), json!(["192.168.1.0/24"])),
        ]
        .into_iter()
        .collect(),
    );
    let intensity = intensity_store.merge_config_object_fields(
        "scan_discovery",
        [
            ("intensity_mode".to_string(), json!("manual")),
            ("intensity_level".to_string(), json!("high")),
        ]
        .into_iter()
        .collect(),
    );
    let (targets_result, intensity_result) = tokio::join!(targets, intensity);
    targets_result.expect("merge target fields");
    intensity_result.expect("merge intensity fields");

    let merged = target_store.get_config().await.expect("load merged config");
    assert_eq!(
        merged["scan_discovery"]["custom_cidrs"],
        json!(["10.0.0.0/24"])
    );
    assert_eq!(
        merged["scan_discovery"]["selected_cidrs"],
        json!(["192.168.1.0/24"])
    );
    assert_eq!(merged["scan_discovery"]["intensity_mode"], "manual");
    assert_eq!(merged["scan_discovery"]["intensity_level"], "high");
}

#[tokio::test]
async fn gateway_target_section_merge_distinguishes_absent_from_present() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let stale_store = Store::connect(&path).await.expect("open stale store");
    let newer_store = Store::connect(&path).await.expect("open newer store");
    stale_store
        .save_config(&json!({
            "run_type": 3,
            "host_mappings": []
        }))
        .await
        .expect("seed config without target sections");

    let mut newer = newer_store.get_config().await.expect("load newer config");
    newer["gateway_proxy_headers"] = json!({ "disabled_hosts": ["new.example.com"] });
    newer_store
        .save_config(&newer)
        .await
        .expect("add proxy section concurrently");

    let merged = stale_store
        .merge_gateway_target_config_sections(
            None,
            &json!({ "disabled_hosts": [] }),
            None,
            &json!({ "disabled_hosts": ["compiled.example.com"] }),
        )
        .await
        .expect("merge sections with absent preconditions");

    assert_eq!(
        merged["gateway_proxy_headers"],
        json!({ "disabled_hosts": ["new.example.com"] }),
        "present does not match an absent expected section"
    );
    assert_eq!(
        merged["gateway_host_response"],
        json!({ "disabled_hosts": ["compiled.example.com"] }),
        "a section that remains absent may be inserted"
    );
}

#[tokio::test]
async fn config_generation_fence_handles_missing_reset_and_explicit_full_replacements() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    let host_a = vec![json!({ "host": "a.example.com" })];
    let host_b = vec![json!({ "host": "b.example.com" })];
    let host_c = vec![json!({ "host": "c.example.com" })];
    let host_d = vec![json!({ "host": "d.example.com" })];
    let groups_d = vec![json!({
        "id": "11111111-1111-4111-8111-111111111111",
        "name": "Imported"
    })];

    store
        .save_config(&json!({
            "host_mappings": host_a,
            "unrelated": { "generation": 1 }
        }))
        .await
        .expect("seed explicit full config");
    // A marker-free value is an intentional full replacement and may update
    // host_mappings while advancing the companion generation.
    store
        .replace_config(&json!({
            "host_mappings": host_b,
            "unrelated": { "generation": 2 }
        }))
        .await
        .expect("explicit full replacement");
    assert_eq!(
        store
            .get_string_value(HOST_MAPPINGS_GENERATION_KEY)
            .await
            .unwrap()
            .as_deref(),
        Some("2")
    );

    // A snapshot from before a companion-key reset must fail closed even when
    // its host fingerprint still matches the stored config.
    let mut ahead_of_reset = store.get_config().await.unwrap();
    store
        .delete_key(HOST_MAPPINGS_GENERATION_KEY)
        .await
        .unwrap();
    ahead_of_reset["unrelated"]["generation"] = json!(99);
    assert!(store.save_config(&ahead_of_reset).await.is_err());
    assert_eq!(
        store.get_config().await.unwrap()["unrelated"]["generation"],
        json!(2)
    );

    // A fresh read after a missing generation is represented as generation
    // zero. Its host fingerprint still fences the snapshot if the companion
    // key is reset independently after a host change.
    let mut stale_full_config = store.get_config().await.unwrap();
    assert_eq!(
        stale_full_config.pointer(&format!("/{CONFIG_GENERATION_MARKER}/generation")),
        Some(&json!(0))
    );
    store
        .compare_and_set_host_mappings(&host_b, &host_c)
        .await
        .unwrap()
        .expect("host CAS after missing generation");
    store
        .delete_key(HOST_MAPPINGS_GENERATION_KEY)
        .await
        .unwrap();
    stale_full_config["unrelated"]["generation"] = json!(3);
    let stale_save = store.save_config(&stale_full_config).await;
    assert!(stale_save.is_err());
    let after_reset = store.get_config().await.unwrap();
    assert_eq!(after_reset["host_mappings"], json!(host_c));
    assert_eq!(after_reset["unrelated"]["generation"], json!(2));
    assert_eq!(
        store
            .get_string_value(HOST_MAPPINGS_GENERATION_KEY)
            .await
            .unwrap()
            .as_deref(),
        None
    );

    store
        .replace_config(&json!({
            "host_mappings": host_d,
            "host_mapping_groups": groups_d,
            "unrelated": { "generation": 4 }
        }))
        .await
        .expect("marker-free import remains an explicit replacement");
    let imported = store.get_config().await.unwrap();
    assert_eq!(imported["host_mappings"], json!(host_d));
    assert_eq!(imported["host_mapping_groups"], json!(groups_d));
    assert_eq!(imported["unrelated"]["generation"], json!(4));
    assert_eq!(
        store
            .get_string_value(HOST_MAPPINGS_GENERATION_KEY)
            .await
            .unwrap()
            .as_deref(),
        Some("1")
    );
    let persisted_raw = store.get_string_value(CONFIG_KEY).await.unwrap().unwrap();
    assert!(!persisted_raw.contains(CONFIG_GENERATION_MARKER));
}

#[tokio::test]
async fn every_application_eval_operation_runs_on_sqlite() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");

    assert_eq!(
        store
            .increment_counter_with_ttl("fn_knock:test:counter", 60)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        store
            .increment_counter_with_ttl("fn_knock:test:counter", 60)
            .await
            .unwrap(),
        2
    );
    let mut counter_conn = store.conn();
    assert!(counter_conn.ttl("fn_knock:test:counter").await.unwrap() > 0);

    assert!(
        store
            .set_expiring_string_with_zset_limit(
                "fn_knock:test:limited:one",
                "one",
                60,
                "fn_knock:test:limited:index",
                100,
                160,
                2,
            )
            .await
            .unwrap()
    );
    assert!(
        store
            .set_expiring_string_with_zset_limit(
                "fn_knock:test:limited:two",
                "two",
                60,
                "fn_knock:test:limited:index",
                100,
                160,
                2,
            )
            .await
            .unwrap()
    );
    assert!(
        !store
            .set_expiring_string_with_zset_limit(
                "fn_knock:test:limited:three",
                "three",
                60,
                "fn_knock:test:limited:index",
                100,
                160,
                2,
            )
            .await
            .unwrap()
    );
    assert!(
        store
            .set_expiring_string_with_zset_limit(
                "fn_knock:test:limited:one",
                "renewed",
                60,
                "fn_knock:test:limited:index",
                100,
                180,
                2,
            )
            .await
            .unwrap()
    );
    assert_eq!(
        store
            .get_string_value("fn_knock:test:limited:one")
            .await
            .unwrap()
            .as_deref(),
        Some("renewed")
    );
    assert_eq!(
        store
            .get_string_value("fn_knock:test:limited:three")
            .await
            .unwrap(),
        None
    );
    assert!(
        store
            .set_expiring_string_with_zset_limit(
                "fn_knock:test:limited:three",
                "after-expiry",
                60,
                "fn_knock:test:limited:index",
                181,
                241,
                2,
            )
            .await
            .unwrap()
    );

    store
        .set_string_value("fn_knock:test:compare", "owner")
        .await
        .unwrap();
    store
        .delete_key_if_value("fn_knock:test:compare", "other")
        .await
        .unwrap();
    assert_eq!(
        store
            .get_string_value("fn_knock:test:compare")
            .await
            .unwrap()
            .as_deref(),
        Some("owner")
    );
    store
        .delete_key_if_value("fn_knock:test:compare", "owner")
        .await
        .unwrap();
    assert_eq!(
        store
            .get_string_value("fn_knock:test:compare")
            .await
            .unwrap(),
        None
    );

    store
        .set_json_value("fn_knock:test:consume", &json!({ "value": 1 }))
        .await
        .unwrap();
    assert_eq!(
        store
            .consume_json_value("fn_knock:test:consume")
            .await
            .unwrap(),
        Some(json!({ "value": 1 }))
    );
    assert_eq!(
        store
            .consume_json_value("fn_knock:test:consume")
            .await
            .unwrap(),
        None
    );

    store
        .set_json_value(
            "fn_knock:test:ldap:invite",
            &json!({ "provider_id": "provider", "totp_id": "one" }),
        )
        .await
        .unwrap();
    assert!(
        store
            .claim_ldap_binding_and_consume_invite(LdapBindingClaim {
                invite_key: "fn_knock:test:ldap:invite",
                subject_key: "fn_knock:test:ldap:subject",
                binding_key: "fn_knock:test:ldap:binding:one",
                bindings_index_key: "fn_knock:test:ldap:index",
                binding_id: "one",
                binding: &json!({ "id": "one", "totp_id": "one" }),
                provider_id: "provider",
                totp_id: "one",
                score: 42,
            })
            .await
            .unwrap()
    );
    assert_eq!(
        store
            .get_json_value("fn_knock:test:ldap:invite")
            .await
            .unwrap(),
        None
    );
    store
        .set_json_value(
            "fn_knock:test:ldap:invite:replay",
            &json!({ "provider_id": "provider", "totp_id": "two" }),
        )
        .await
        .unwrap();
    assert!(
        !store
            .claim_ldap_binding_and_consume_invite(LdapBindingClaim {
                invite_key: "fn_knock:test:ldap:invite:replay",
                subject_key: "fn_knock:test:ldap:subject",
                binding_key: "fn_knock:test:ldap:binding:two",
                bindings_index_key: "fn_knock:test:ldap:index",
                binding_id: "two",
                binding: &json!({ "id": "two", "totp_id": "two" }),
                provider_id: "provider",
                totp_id: "two",
                score: 43,
            })
            .await
            .unwrap()
    );

    let backoff = store
        .register_login_backoff_failure("192.0.2.1")
        .await
        .unwrap();
    assert_eq!(backoff.attempts, 1);

    store
        .set_passkey_challenge("challenge", "auth", 60)
        .await
        .unwrap();
    assert!(
        !store
            .consume_passkey_challenge("challenge", "register")
            .await
            .unwrap()
    );
    assert!(
        store
            .consume_passkey_challenge("challenge", "auth")
            .await
            .unwrap()
    );
    assert!(
        !store
            .consume_passkey_challenge("challenge", "auth")
            .await
            .unwrap()
    );

    let bind_token = store.create_passkey_bind_token("totp", 60).await.unwrap();
    assert_eq!(
        store
            .get_passkey_bind_token_totp_id(&bind_token)
            .await
            .unwrap()
            .as_deref(),
        Some("totp")
    );
    assert_eq!(
        store
            .consume_passkey_bind_token(&bind_token)
            .await
            .unwrap()
            .as_deref(),
        Some("totp")
    );
    assert_eq!(
        store.consume_passkey_bind_token(&bind_token).await.unwrap(),
        None
    );
    assert_eq!(
        store
            .get_passkey_bind_token_totp_id(&bind_token)
            .await
            .unwrap(),
        None
    );

    assert!(
        store
            .acquire_notification_runtime_lease("test", "owner", 60)
            .await
            .unwrap()
    );
    store
        .release_notification_runtime_lease("test", "other")
        .await
        .unwrap();
    assert!(
        !store
            .acquire_notification_runtime_lease("test", "new", 60)
            .await
            .unwrap()
    );
    store
        .release_notification_runtime_lease("test", "owner")
        .await
        .unwrap();
    assert!(
        store
            .acquire_notification_runtime_lease("test", "new", 60)
            .await
            .unwrap()
    );

    store
        .enqueue_notification_delivery("ready", 10)
        .await
        .unwrap();
    store
        .enqueue_notification_delivery("future", 30)
        .await
        .unwrap();
    assert!(
        store
            .conn()
            .ttl(NOTIFICATION_DELIVERIES_READY_KEY)
            .await
            .unwrap()
            > 0
    );
    assert_eq!(
        store
            .pull_ready_notification_delivery_ids(10, 20)
            .await
            .unwrap(),
        vec!["ready".to_string()]
    );
    assert!(
        store
            .pull_ready_notification_delivery_ids(10, 20)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        store
            .pull_ready_notification_delivery_ids(10, 30)
            .await
            .unwrap(),
        vec!["future".to_string()]
    );
}

#[tokio::test]
async fn ldap_binding_claim_is_atomic_under_concurrency() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    store
        .set_json_value(
            "fn_knock:test:ldap:race:invite",
            &json!({ "provider_id": "provider", "totp_id": "shared" }),
        )
        .await
        .unwrap();

    let left_store = store.clone();
    let right_store = store.clone();
    let left_binding = json!({ "id": "left", "totp_id": "shared" });
    let right_binding = json!({ "id": "right", "totp_id": "shared" });
    let (left, right) = tokio::join!(
        left_store.claim_ldap_binding_and_consume_invite(LdapBindingClaim {
            invite_key: "fn_knock:test:ldap:race:invite",
            subject_key: "fn_knock:test:ldap:race:subject",
            binding_key: "fn_knock:test:ldap:race:binding:left",
            bindings_index_key: "fn_knock:test:ldap:race:index",
            binding_id: "left",
            binding: &left_binding,
            provider_id: "provider",
            totp_id: "shared",
            score: 1,
        }),
        right_store.claim_ldap_binding_and_consume_invite(LdapBindingClaim {
            invite_key: "fn_knock:test:ldap:race:invite",
            subject_key: "fn_knock:test:ldap:race:subject",
            binding_key: "fn_knock:test:ldap:race:binding:right",
            bindings_index_key: "fn_knock:test:ldap:race:index",
            binding_id: "right",
            binding: &right_binding,
            provider_id: "provider",
            totp_id: "shared",
            score: 2,
        }),
    );
    assert_ne!(left.unwrap(), right.unwrap());
    let winner = store
        .get_string_value("fn_knock:test:ldap:race:subject")
        .await
        .unwrap()
        .expect("subject is claimed");
    assert!(matches!(winner.as_str(), "left" | "right"));
    assert_eq!(
        store
            .zrevrange_strings("fn_knock:test:ldap:race:index")
            .await
            .unwrap(),
        vec![winner]
    );
}

#[tokio::test]
async fn ldap_binding_claim_checks_invite_target_and_revocation_wins_updates() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let invite_key = "fn_knock:test:ldap:verified-invite";
    let subject_key = "fn_knock:test:ldap:verified-subject";
    let binding_key = "fn_knock:test:ldap:verified-binding";
    let index_key = "fn_knock:test:ldap:verified-index";
    let binding = json!({ "id": "binding", "provider_id": "provider", "totp_id": "totp" });
    store
        .set_json_value(
            invite_key,
            &json!({ "provider_id": "provider", "totp_id": "totp" }),
        )
        .await
        .unwrap();

    assert!(
        !store
            .claim_ldap_binding_and_consume_invite(LdapBindingClaim {
                invite_key,
                subject_key,
                binding_key,
                bindings_index_key: index_key,
                binding_id: "binding",
                binding: &binding,
                provider_id: "other-provider",
                totp_id: "totp",
                score: 1,
            })
            .await
            .unwrap()
    );
    assert!(store.get_json_value(invite_key).await.unwrap().is_some());

    assert!(
        store
            .claim_ldap_binding_and_consume_invite(LdapBindingClaim {
                invite_key,
                subject_key,
                binding_key,
                bindings_index_key: index_key,
                binding_id: "binding",
                binding: &binding,
                provider_id: "provider",
                totp_id: "totp",
                score: 2,
            })
            .await
            .unwrap()
    );
    let updated = json!({ "id": "binding", "provider_id": "provider", "totp_id": "totp", "last_used_at": "now" });
    assert!(
        store
            .update_binding_if_owned(OwnedBindingUpdate {
                subject_key,
                binding_key,
                bindings_index_key: index_key,
                binding_id: "binding",
                binding: &updated,
                score: 3,
            })
            .await
            .unwrap()
    );

    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "CREATE TRIGGER fail_owned_binding_delete
             BEFORE DELETE ON kv_strings
             WHEN OLD.key = 'fn_knock:test:ldap:verified-binding'
             BEGIN SELECT RAISE(ABORT, 'injected owned binding delete failure'); END;",
        )
        .unwrap();
    drop(connection);
    let error = store
        .delete_binding_if_owned(OwnedBindingDelete {
            subject_key,
            binding_key,
            bindings_index_key: index_key,
            binding_id: "binding",
        })
        .await
        .expect_err("binding delete failure must roll back owner, document, and index");
    assert!(
        error
            .to_string()
            .contains("injected owned binding delete failure")
    );
    assert_eq!(
        store
            .get_string_value(subject_key)
            .await
            .unwrap()
            .as_deref(),
        Some("binding")
    );
    assert_eq!(
        store.get_json_value(binding_key).await.unwrap(),
        Some(updated.clone())
    );
    assert_eq!(
        store.zrevrange_strings(index_key).await.unwrap(),
        vec!["binding"]
    );

    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch("DROP TRIGGER fail_owned_binding_delete")
        .unwrap();
    drop(connection);
    store
        .set_string_value(subject_key, "replacement-binding")
        .await
        .unwrap();
    assert!(
        store
            .delete_binding_if_owned(OwnedBindingDelete {
                subject_key,
                binding_key,
                bindings_index_key: index_key,
                binding_id: "binding",
            })
            .await
            .unwrap()
    );
    assert_eq!(
        store
            .get_string_value(subject_key)
            .await
            .unwrap()
            .as_deref(),
        Some("replacement-binding")
    );
    assert!(store.get_json_value(binding_key).await.unwrap().is_none());
    assert!(store.zrevrange_strings(index_key).await.unwrap().is_empty());
    assert!(
        !store
            .update_binding_if_owned(OwnedBindingUpdate {
                subject_key,
                binding_key,
                bindings_index_key: index_key,
                binding_id: "binding",
                binding: &updated,
                score: 4,
            })
            .await
            .unwrap()
    );
    assert!(store.get_json_value(binding_key).await.unwrap().is_none());
}

#[tokio::test]
async fn session_merge_is_atomic_preserves_absolute_expiry_and_never_recreates() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let session_key = "fn_knock:session:atomic-merge";
    let mut conn = store.conn();
    conn.set_ex(
        session_key,
        json!({
            "ip": "192.0.2.1",
            "userAgent": "before",
            "accessScopes": [],
            "subdomainAccess": { "mode": "custom", "items": [] },
            "shapeSentinel": [[], {}, { "nested": [] }]
        })
        .to_string(),
        600,
    )
    .await
    .expect("seed session");

    let expiry_before = sqlite_key_expiry_at_ms(&path, session_key)
        .await
        .expect("session expiry");
    let mut updates = Map::new();
    updates.insert("ip".to_string(), json!("192.0.2.2"));
    let updated = store
        .update_session_value("atomic-merge", updates)
        .await
        .expect("atomic session merge")
        .expect("live session");
    assert_eq!(updated["ip"], json!("192.0.2.2"));
    assert_eq!(updated["userAgent"], json!("before"));
    assert_eq!(updated["accessScopes"], json!([]));
    assert_eq!(
        updated["subdomainAccess"],
        json!({ "mode": "custom", "items": [] })
    );
    assert_eq!(updated["shapeSentinel"], json!([[], {}, { "nested": [] }]));
    let stored = store
        .get_session_value("atomic-merge")
        .await
        .expect("stored merged session")
        .expect("stored live session");
    assert_eq!(stored["accessScopes"], json!([]));
    assert_eq!(stored["subdomainAccess"]["items"], json!([]));
    assert_eq!(stored["shapeSentinel"], json!([[], {}, { "nested": [] }]));
    assert_eq!(
        sqlite_key_expiry_at_ms(&path, session_key).await,
        Some(expiry_before),
        "the absolute millisecond deadline must not be rounded or extended"
    );

    for round in 0..16 {
        let session_id = format!("atomic-delete-{round}");
        let key = crate::auth_session_keys::session_key(&session_id);
        let mut conn = store.conn();
        conn.set_ex(&key, json!({ "round": round }).to_string(), 600)
            .await
            .expect("seed raced session");
        let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(3));
        let update_store = store.clone();
        let update_barrier = std::sync::Arc::clone(&barrier);
        let update_id = session_id.clone();
        let updater = tokio::spawn(async move {
            update_barrier.wait().await;
            let mut updates = Map::new();
            updates.insert("updated".to_string(), Value::Bool(true));
            update_store.update_session_value(&update_id, updates).await
        });
        let delete_store = store.clone();
        let delete_barrier = std::sync::Arc::clone(&barrier);
        let delete_id = session_id.clone();
        let deleter = tokio::spawn(async move {
            delete_barrier.wait().await;
            delete_store.delete_session(&delete_id).await
        });
        barrier.wait().await;
        updater.await.expect("updater task").expect("update result");
        deleter.await.expect("deleter task").expect("delete result");
        assert!(
            store
                .get_session_value(&session_id)
                .await
                .expect("final session lookup")
                .is_none(),
            "round {round} recreated a deleted session"
        );
    }

    let mut missing_update = Map::new();
    missing_update.insert("ip".to_string(), json!("192.0.2.99"));
    assert!(
        store
            .update_session_value("does-not-exist", missing_update)
            .await
            .expect("missing update")
            .is_none()
    );
    assert!(
        store
            .get_string_value("fn_knock:session:does-not-exist")
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn docker_admin_session_refresh_never_recreates_a_revoked_session() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");

    for round in 0..16 {
        let now = crate::time_utils::now_iso();
        let record = DockerAdminSessionRecord {
            id: format!("docker-admin-race-{round}"),
            created_at: now.clone(),
            updated_at: now,
            expires_at: crate::time_utils::iso_after_seconds(600),
            ttl_seconds: 600,
            password_revision: "password-revision".to_string(),
            ip: "192.0.2.1".to_string(),
            user_agent: "test".to_string(),
        };
        store
            .set_docker_admin_session(&record)
            .await
            .expect("seed docker admin session");

        let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(3));
        let refresh_store = store.clone();
        let refresh_barrier = std::sync::Arc::clone(&barrier);
        let refresh_record = record.clone();
        let refresher = tokio::spawn(async move {
            refresh_barrier.wait().await;
            refresh_store
                .refresh_docker_admin_session_if_exists(&refresh_record)
                .await
        });
        let delete_store = store.clone();
        let delete_barrier = std::sync::Arc::clone(&barrier);
        let delete_id = record.id.clone();
        let deleter = tokio::spawn(async move {
            delete_barrier.wait().await;
            delete_store.delete_docker_admin_session(&delete_id).await
        });
        barrier.wait().await;
        refresher.await.expect("refresher task").expect("refresh");
        deleter.await.expect("deleter task").expect("delete");

        assert!(
            store
                .docker_admin_session(&record.id)
                .await
                .expect("final session lookup")
                .is_none(),
            "round {round} recreated a revoked docker admin session"
        );
    }

    let missing = DockerAdminSessionRecord {
        id: "missing-docker-admin-session".to_string(),
        created_at: crate::time_utils::now_iso(),
        updated_at: crate::time_utils::now_iso(),
        expires_at: crate::time_utils::iso_after_seconds(600),
        ttl_seconds: 600,
        password_revision: "password-revision".to_string(),
        ip: "192.0.2.1".to_string(),
        user_agent: "test".to_string(),
    };
    assert!(
        !store
            .refresh_docker_admin_session_if_exists(&missing)
            .await
            .expect("missing session refresh")
    );
}

#[tokio::test]
async fn docker_admin_login_failures_increment_atomically_under_concurrency() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    let ip = "192.0.2.151";
    let mut tasks = Vec::new();
    for _ in 0..16 {
        let store = store.clone();
        tasks.push(tokio::spawn(async move {
            store.register_docker_admin_login_failure(ip).await
        }));
    }
    for task in tasks {
        let (retry_after, blocked_until) = task
            .await
            .expect("join docker admin failure")
            .expect("register docker admin failure");
        assert!(retry_after >= 2);
        assert!(blocked_until > 0);
    }
    let record = store
        .docker_admin_login_attempt(ip)
        .await
        .expect("load docker admin login attempt")
        .expect("docker admin login attempt exists");
    assert_eq!(record.attempts, 16);
    assert_eq!(record.ip, ip);
    assert!(!record.last_attempt_at.is_empty());
    assert!(record.blocked_until > crate::time_utils::now_ms());
    let typed = store
        .typed_docker_admin
        .load_login_backoff(ip)
        .await
        .expect("load typed Docker admin backoff")
        .expect("typed Docker admin backoff exists");
    assert_eq!(
        serde_json::from_str::<Value>(&typed.document_json).unwrap()["attempts"],
        json!(16)
    );
}

#[tokio::test]
async fn docker_admin_security_shadow_uses_legacy_authority_and_repairs_mismatches() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let session = DockerAdminSessionRecord {
        id: "typed-docker-session".to_string(),
        created_at: crate::time_utils::now_iso(),
        updated_at: crate::time_utils::now_iso(),
        expires_at: crate::time_utils::iso_after_seconds(600),
        ttl_seconds: 600,
        password_revision: "password-revision".to_string(),
        ip: "192.0.2.152".to_string(),
        user_agent: "test".to_string(),
    };
    store
        .set_docker_admin_session(&session)
        .await
        .expect("seed Docker admin session");
    store
        .register_docker_admin_login_failure(&session.ip)
        .await
        .expect("seed Docker admin backoff");
    assert_eq!(store.typed_docker_admin.counts().await.unwrap(), (1, 1));

    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
    let mut corrupt_session = serde_json::to_value(&session).unwrap();
    corrupt_session["user_agent"] = json!("typed-only-user-agent");
    connection
        .execute(
            "UPDATE docker_admin_session_documents SET session_json = ?2 WHERE session_id = ?1",
            tokio_rusqlite::rusqlite::params![session.id, corrupt_session.to_string()],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE docker_admin_login_backoff_attempts SET attempt_json = ?2 WHERE ip = ?1",
            tokio_rusqlite::rusqlite::params![
                session.ip,
                json!({
                    "ip": session.ip,
                    "attempts": 999,
                    "last_attempt_at": crate::time_utils::now_iso(),
                    "blocked_until": 9_999_999_999_999_i64
                })
                .to_string()
            ],
        )
        .unwrap();
    drop(connection);

    let legacy_session = store
        .docker_admin_session(&session.id)
        .await
        .expect("load legacy-authoritative session")
        .expect("legacy session exists");
    assert_eq!(legacy_session.user_agent, "test");
    let legacy_attempt = store
        .docker_admin_login_attempt(&session.ip)
        .await
        .expect("load legacy-authoritative attempt")
        .expect("legacy attempt exists");
    assert_eq!(legacy_attempt.attempts, 1);
    let shadow = store.typed_docker_admin_shadow_status();
    assert!(!shadow.healthy);
    assert_eq!(shadow.mismatch_count, 2);
    assert_eq!(
        serde_json::from_str::<Value>(
            &store
                .typed_docker_admin
                .load_session(&session.id)
                .await
                .unwrap()
                .unwrap()
                .document_json
        )
        .unwrap()["user_agent"],
        json!("test")
    );

    store
        .delete_docker_admin_session(&session.id)
        .await
        .expect("delete Docker admin session");
    store
        .reset_docker_admin_login_attempt(&session.ip)
        .await
        .expect("reset Docker admin backoff");
    assert_eq!(store.typed_docker_admin.counts().await.unwrap(), (0, 0));
}

#[tokio::test]
async fn docker_admin_typed_failures_roll_back_and_lazy_expiry_removes_shadows() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let session = DockerAdminSessionRecord {
        id: "typed-docker-rollback".to_string(),
        created_at: crate::time_utils::now_iso(),
        updated_at: crate::time_utils::now_iso(),
        expires_at: crate::time_utils::iso_after_seconds(600),
        ttl_seconds: 600,
        password_revision: "password-revision".to_string(),
        ip: "192.0.2.153".to_string(),
        user_agent: "test".to_string(),
    };
    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "CREATE TRIGGER fail_typed_docker_session_insert
             BEFORE INSERT ON docker_admin_session_documents
             BEGIN SELECT RAISE(FAIL, 'forced typed Docker session failure'); END;
             CREATE TRIGGER fail_typed_docker_backoff_insert
             BEFORE INSERT ON docker_admin_login_backoff_attempts
             BEGIN SELECT RAISE(FAIL, 'forced typed Docker backoff failure'); END;",
        )
        .unwrap();
    drop(connection);
    assert!(store.set_docker_admin_session(&session).await.is_err());
    assert!(
        store
            .register_docker_admin_login_failure(&session.ip)
            .await
            .is_err()
    );
    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
    let legacy_count = connection
        .query_row(
            "SELECT COUNT(*) FROM kv_keys WHERE key IN (?1, ?2)",
            tokio_rusqlite::rusqlite::params![
                format!("{DOCKER_ADMIN_SESSION_PREFIX}{}", session.id),
                format!("{DOCKER_ADMIN_LOGIN_BACKOFF_PREFIX}{}", session.ip)
            ],
            |row| row.get::<_, i64>(0),
        )
        .unwrap();
    assert_eq!(legacy_count, 0);
    connection
        .execute_batch(
            "DROP TRIGGER fail_typed_docker_session_insert;
             DROP TRIGGER fail_typed_docker_backoff_insert;",
        )
        .unwrap();
    drop(connection);

    store
        .set_docker_admin_session(&session)
        .await
        .expect("seed expiring Docker session");
    store
        .register_docker_admin_login_failure(&session.ip)
        .await
        .expect("seed expiring Docker backoff");
    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE kv_keys SET expires_at_ms = 0 WHERE key IN (?1, ?2)",
            tokio_rusqlite::rusqlite::params![
                format!("{DOCKER_ADMIN_SESSION_PREFIX}{}", session.id),
                format!("{DOCKER_ADMIN_LOGIN_BACKOFF_PREFIX}{}", session.ip)
            ],
        )
        .unwrap();
    drop(connection);
    assert!(
        store
            .docker_admin_session(&session.id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .docker_admin_login_attempt(&session.ip)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(store.typed_docker_admin.counts().await.unwrap(), (0, 0));
}

#[tokio::test]
async fn docker_admin_security_shadow_rebuilds_after_backup_restore_and_clear() {
    let source_dir = tempfile::tempdir().expect("create source temp dir");
    let source = Store::connect(source_dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open source store");
    let session = DockerAdminSessionRecord {
        id: "typed-docker-backup".to_string(),
        created_at: crate::time_utils::now_iso(),
        updated_at: crate::time_utils::now_iso(),
        expires_at: crate::time_utils::iso_after_seconds(600),
        ttl_seconds: 600,
        password_revision: "password-revision".to_string(),
        ip: "192.0.2.154".to_string(),
        user_agent: "test".to_string(),
    };
    source
        .set_docker_admin_session(&session)
        .await
        .expect("seed source Docker session");
    source
        .register_docker_admin_login_failure(&session.ip)
        .await
        .expect("seed source Docker backoff");
    let entries = source
        .export_backup_entries_by_prefix_limited("fn_knock:docker_admin:", 1_000_000, |_| true)
        .await
        .expect("export Docker admin backup");

    let target_dir = tempfile::tempdir().expect("create target temp dir");
    let target = Store::connect(target_dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open target store");
    target
        .replace_backup_entries_by_prefix("fn_knock:", &entries, 200)
        .await
        .expect("restore Docker admin backup");
    assert_eq!(target.typed_docker_admin.counts().await.unwrap(), (1, 1));
    target.clear_all_keys().await.expect("clear restored store");
    assert_eq!(target.typed_docker_admin.counts().await.unwrap(), (0, 0));
}

#[tokio::test]
async fn docker_admin_password_rotation_and_reset_are_atomic_with_security_state() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let old_password = DockerAdminPasswordRecord {
        algorithm: "scrypt".to_string(),
        salt: "00".repeat(16),
        hash: "old-password-hash".to_string(),
        n: 16_384,
        r: 8,
        p: 1,
        key_length: 32,
        created_at: crate::time_utils::now_iso(),
        updated_at: crate::time_utils::now_iso(),
    };
    store
        .set_docker_admin_password(&old_password)
        .await
        .expect("seed old Docker password");
    let make_session = |id: &str, ip: &str| DockerAdminSessionRecord {
        id: id.to_string(),
        created_at: crate::time_utils::now_iso(),
        updated_at: crate::time_utils::now_iso(),
        expires_at: crate::time_utils::iso_after_seconds(600),
        ttl_seconds: 600,
        password_revision: "old-password-revision".to_string(),
        ip: ip.to_string(),
        user_agent: "test".to_string(),
    };
    let first_session = make_session("atomic-docker-session-1", "192.0.2.155");
    let second_session = make_session("atomic-docker-session-2", "192.0.2.156");
    store
        .set_docker_admin_session(&first_session)
        .await
        .expect("seed first Docker session");
    store
        .set_docker_admin_session(&second_session)
        .await
        .expect("seed second Docker session");
    store
        .register_docker_admin_login_failure(&first_session.ip)
        .await
        .expect("seed Docker backoff");

    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "CREATE TRIGGER fail_typed_docker_session_delete
             BEFORE DELETE ON docker_admin_session_documents
             BEGIN SELECT RAISE(FAIL, 'forced typed Docker session delete failure'); END;",
        )
        .unwrap();
    drop(connection);
    let mut new_password = old_password.clone();
    new_password.hash = "new-password-hash".to_string();
    new_password.updated_at = crate::time_utils::iso_after_seconds(1);
    assert!(
        store
            .replace_docker_admin_password_and_clear_security_state(&new_password)
            .await
            .is_err()
    );
    assert_eq!(
        store.docker_admin_password().await.unwrap().unwrap().hash,
        old_password.hash
    );
    assert!(
        store
            .docker_admin_session(&first_session.id)
            .await
            .unwrap()
            .is_some()
    );
    assert!(store.reset_docker_admin_password_state().await.is_err());
    assert!(store.docker_admin_password().await.unwrap().is_some());

    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
    connection
        .execute("DROP TRIGGER fail_typed_docker_session_delete", [])
        .unwrap();
    drop(connection);
    let summary = store
        .reset_docker_admin_password_state()
        .await
        .expect("atomically reset Docker admin state");
    assert!(summary.password_cleared);
    assert_eq!(summary.sessions_cleared, 2);
    assert_eq!(summary.login_failures_cleared, 1);
    assert!(store.docker_admin_password().await.unwrap().is_none());
    assert_eq!(store.typed_docker_admin.counts().await.unwrap(), (0, 0));
}

#[tokio::test]
async fn binding_keep_ttl_rejects_missing_keys_and_preserves_persistent_keys() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    store
        .set_json_value("fn_knock:session:binding-owner", &json!({ "live": true }))
        .await
        .expect("seed live owner");

    let binding = json!({
        "ownerSessionId": "binding-owner",
        "currentIp": "192.0.2.10"
    });
    assert!(
        !store
            .save_auth_mobility_binding_keep_ttl(
                "proxy-session",
                "missing-binding",
                &binding,
                "binding-owner",
            )
            .await
            .expect("missing binding is rejected")
    );
    assert!(
        store
            .get_auth_mobility_binding("proxy-session", "missing-binding")
            .await
            .unwrap()
            .is_none()
    );

    let subject_hash = auth_mobility_subject_hash("proxy-session", "persistent-binding");
    let binding_key = auth_mobility_binding_key("proxy-session", &subject_hash);
    store
        .set_json_value(&binding_key, &binding)
        .await
        .expect("seed persistent binding");
    let next = json!({
        "ownerSessionId": "binding-owner",
        "currentIp": "192.0.2.11"
    });
    assert!(
        store
            .save_auth_mobility_binding_keep_ttl(
                "proxy-session",
                "persistent-binding",
                &next,
                "binding-owner",
            )
            .await
            .expect("persistent binding update")
    );
    let mut conn = store.conn();
    let ttl: i64 = redis::cmd("PTTL")
        .arg(&binding_key)
        .query_async(&mut conn)
        .await
        .expect("persistent PTTL");
    assert_eq!(ttl, -1);
    assert_eq!(
        store
            .get_auth_mobility_binding("proxy-session", "persistent-binding")
            .await
            .unwrap(),
        Some(next)
    );
}

#[tokio::test]
async fn mobility_whitelist_snapshot_matches_atomic_destroy_and_ignores_foreign_bindings() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    let session_id = "mobility-snapshot-session";
    let foreign_session_id = "mobility-snapshot-foreign";
    store
        .set_json_value(
            &crate::auth_session_keys::session_key(session_id),
            &json!({ "live": true }),
        )
        .await
        .expect("seed live session");
    store
        .set_json_value(
            &crate::auth_session_keys::session_key(foreign_session_id),
            &json!({ "live": true }),
        )
        .await
        .expect("seed foreign live session");

    let proxy_hash = auth_mobility_subject_hash("proxy-session", session_id);
    assert!(
        store
            .initialize_auth_mobility_login_session(
                session_id,
                &proxy_hash,
                &json!({
                    "ownerSessionId": session_id,
                    "whitelistRecordId": "whitelist:proxy"
                }),
                &json!({ "type": "login" }),
                &json!({ "count": 1 }),
                "whitelist:proxy",
                3_600,
            )
            .await
            .expect("initialize proxy mobility")
    );
    assert!(
        store
            .save_auth_mobility_owned_binding(
                "fnos-token",
                "owned-subject",
                &json!({
                    "ownerSessionId": session_id,
                    "whitelistRecordId": "whitelist:owned"
                }),
                session_id,
                3_600,
                Some(3_600),
            )
            .await
            .expect("save owned binding")
    );
    assert!(
        store
            .save_auth_mobility_active_ip_detail(
                session_id,
                "192.0.2.40",
                40,
                &json!({ "whitelistRecordId": "whitelist:active" }),
                3_600,
            )
            .await
            .expect("save active IP")
    );
    assert!(
        store
            .add_auth_mobility_pending_whitelist(
                session_id,
                "whitelist:pending",
                "fn_knock:test:pending-owner-record",
                3_600,
            )
            .await
            .expect("save pending whitelist")
    );
    assert!(
        store
            .add_auth_mobility_pending_whitelist(
                session_id,
                " whitelist:opaque ",
                " fn_knock:test:opaque-owner-record ",
                3_600,
            )
            .await
            .expect("save opaque pending whitelist")
    );
    store
        .set_json_value(
            " fn_knock:test:opaque-owner-record ",
            &json!({ "owned": true }),
        )
        .await
        .expect("seed opaque owner record");
    assert!(
        store
            .set_json_value_nx_ex(
                &crate::auth_mobility_keys::session_mutation_lock_key(session_id),
                &json!({ "lockId": "typed-shadow-lock", "sessionId": session_id }),
                120,
            )
            .await
            .expect("seed mobility mutation lock")
    );

    let foreign_subject = "foreign-subject";
    assert!(
        store
            .save_auth_mobility_owned_binding(
                "fnos-token",
                foreign_subject,
                &json!({
                    "ownerSessionId": foreign_session_id,
                    "whitelistRecordId": "whitelist:foreign"
                }),
                foreign_session_id,
                3_600,
                Some(3_600),
            )
            .await
            .expect("save foreign binding")
    );
    let foreign_hash = auth_mobility_subject_hash("fnos-token", foreign_subject);
    let foreign_binding_key = auth_mobility_binding_key("fnos-token", &foreign_hash);
    let mut conn = store.conn();
    conn.sadd(
        auth_mobility_session_index_key(session_id),
        &foreign_binding_key,
    )
    .await
    .expect("inject stale foreign index member");

    let expected = vec![
        " whitelist:opaque ".to_string(),
        "whitelist:active".to_string(),
        "whitelist:owned".to_string(),
        "whitelist:pending".to_string(),
        "whitelist:proxy".to_string(),
    ];
    let typed = store
        .typed_mobility
        .load_session(session_id)
        .await
        .expect("load typed mobility aggregate")
        .expect("typed mobility aggregate exists");
    assert!(typed.session.is_some());
    assert!(typed.timeline.is_some());
    assert!(typed.summary.is_some());
    assert_eq!(typed.binding_index.len(), 3);
    assert_eq!(typed.bindings.len(), 3);
    assert_eq!(typed.active_ips.len(), 1);
    assert_eq!(typed.pending_whitelist.len(), 2);
    assert_eq!(typed.whitelist_owners.len(), 1);
    assert!(typed.mutation_lock.is_some());
    assert_eq!(store.typed_mobility.counts().await.unwrap(), (2, 0));
    assert_eq!(
        store
            .list_auth_mobility_session_whitelist_ids(session_id)
            .await
            .expect("collect atomic mobility snapshot"),
        expected
    );
    assert_eq!(
        store
            .destroy_auth_mobility_session(session_id)
            .await
            .expect("destroy the same aggregate"),
        expected
    );
    assert!(
        store
            .get_session(session_id)
            .await
            .expect("load destroyed session authority")
            .is_none()
    );
    let destroyed_typed = store
        .typed_mobility
        .load_session(session_id)
        .await
        .expect("load typed aggregate immediately after destroy");
    assert!(destroyed_typed.is_none());
    assert_eq!(
        store
            .get_auth_mobility_binding("fnos-token", foreign_subject)
            .await
            .expect("load foreign binding"),
        Some(json!({
            "ownerSessionId": foreign_session_id,
            "whitelistRecordId": "whitelist:foreign"
        }))
    );
    assert!(
        store
            .get_json_value(" fn_knock:test:opaque-owner-record ")
            .await
            .expect("load opaque owner record")
            .is_none()
    );
    assert!(
        store
            .list_auth_mobility_session_whitelist_ids(session_id)
            .await
            .expect("collect destroyed mobility snapshot")
            .is_empty()
    );
}

#[tokio::test]
async fn typed_mobility_failure_rolls_back_the_authoritative_session_write() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let existing_session = new_login_session(
        "typed-mobility-eval-failure",
        "Typed mobility EVAL failure",
        "192.0.2.89",
        "test",
        3_600,
    );
    store
        .add_session("typed-mobility-eval-failure", &existing_session, 3_600)
        .await
        .expect("seed existing session");
    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "CREATE TRIGGER fail_typed_mobility_insert
             BEFORE INSERT ON mobility_session_aggregates
             BEGIN
               SELECT RAISE(ABORT, 'injected typed mobility failure');
             END;",
        )
        .unwrap();
    drop(connection);

    let session = new_login_session(
        "typed-mobility-failure",
        "Typed mobility failure",
        "192.0.2.90",
        "test",
        3_600,
    );
    let error = store
        .add_session("typed-mobility-failure", &session, 3_600)
        .await
        .expect_err("typed failure must reject the entire session write");
    assert!(
        error
            .to_string()
            .contains("injected typed mobility failure")
    );
    assert!(
        store
            .get_session("typed-mobility-failure")
            .await
            .expect("read rolled back session")
            .is_none()
    );
    assert!(
        store
            .typed_mobility
            .load_session("typed-mobility-failure")
            .await
            .expect("read rolled back typed aggregate")
            .is_none()
    );
    let eval_error = store
        .add_auth_mobility_pending_whitelist(
            "typed-mobility-eval-failure",
            "whitelist:must-rollback",
            "fn_knock:test:must-rollback-owner",
            3_600,
        )
        .await
        .expect_err("typed failure must reject the entire EVAL mutation");
    assert!(
        eval_error
            .to_string()
            .contains("injected typed mobility failure")
    );
    assert!(
        store
            .list_auth_mobility_session_whitelist_ids("typed-mobility-eval-failure")
            .await
            .expect("read rolled back EVAL aggregate")
            .is_empty()
    );
}

#[tokio::test]
async fn corrupt_typed_mobility_shadow_returns_legacy_snapshot_and_repairs() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let session_id = "typed-mobility-repair";
    store
        .set_json_value(
            &crate::auth_session_keys::session_key(session_id),
            &json!({ "live": true }),
        )
        .await
        .expect("seed session");
    assert!(
        store
            .add_auth_mobility_pending_whitelist(
                session_id,
                "whitelist:repair",
                "fn_knock:test:repair-owner",
                3_600,
            )
            .await
            .expect("seed pending whitelist")
    );

    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE mobility_session_aggregates SET aggregate_json = 'not-json' WHERE session_id = ?1",
            [session_id],
        )
        .unwrap();
    drop(connection);

    assert_eq!(
        store
            .list_auth_mobility_session_whitelist_ids(session_id)
            .await
            .expect("legacy snapshot survives corrupt typed shadow"),
        vec!["whitelist:repair".to_string()]
    );
    let repaired = store
        .typed_mobility
        .load_session(session_id)
        .await
        .expect("load repaired typed shadow")
        .expect("repaired typed shadow exists");
    assert_eq!(repaired.pending_whitelist.len(), 1);
    assert_eq!(repaired.pending_whitelist[0].record_id, "whitelist:repair");
    let status = store.typed_mobility_shadow_status();
    assert!(!status.healthy);
    assert_eq!(status.mismatch_count, 1);
    assert_eq!(status.phase, "dual_write_shadow");
}

#[tokio::test]
async fn auth_session_reads_repair_shadow_but_never_authorize_typed_only_state() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let session_id = "auth-session-shadow-authority";
    let session = new_login_session(session_id, "Legacy authority", "192.0.2.91", "test", 3_600);
    store
        .add_session(session_id, &session, 3_600)
        .await
        .expect("seed authoritative session");

    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE mobility_session_aggregates SET aggregate_json = 'not-json' WHERE session_id = ?1",
            [session_id],
        )
        .unwrap();
    drop(connection);

    let legacy_read = store
        .get_session(session_id)
        .await
        .expect("read legacy session despite corrupt shadow")
        .expect("legacy session remains authoritative");
    assert_eq!(
        serde_json::to_value(&legacy_read).unwrap(),
        serde_json::to_value(&session).unwrap()
    );
    let repaired = store
        .typed_mobility
        .load_session(session_id)
        .await
        .expect("load repaired aggregate")
        .expect("repaired aggregate exists");
    assert_eq!(
        repaired.session.expect("typed session component").value,
        serde_json::to_value(&session).unwrap()
    );

    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
    connection
        .execute(
            "DELETE FROM kv_keys WHERE key = ?1",
            [crate::auth_session_keys::session_key(session_id)],
        )
        .unwrap();
    drop(connection);

    assert!(
        store
            .get_session(session_id)
            .await
            .expect("typed-only state must not authorize")
            .is_none()
    );
    assert!(
        store
            .typed_mobility
            .load_session(session_id)
            .await
            .expect("load aggregate after typed-only repair")
            .is_none()
    );
    let status = store.typed_mobility_shadow_status();
    assert!(!status.healthy);
    assert_eq!(status.mismatch_count, 2);
}

#[tokio::test]
async fn auth_session_and_mobility_destroy_roll_back_as_one_transaction() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let session_id = "atomic-auth-session-destroy";
    let session = new_login_session(session_id, "Atomic destroy", "192.0.2.92", "test", 3_600);
    store
        .add_session(session_id, &session, 3_600)
        .await
        .expect("seed authoritative session");
    assert!(
        store
            .add_auth_mobility_pending_whitelist(
                session_id,
                "whitelist:atomic-destroy",
                "fn_knock:test:atomic-destroy-owner",
                3_600,
            )
            .await
            .expect("seed mobility state")
    );

    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "CREATE TRIGGER fail_typed_mobility_delete
             BEFORE DELETE ON mobility_session_aggregates
             BEGIN
               SELECT RAISE(ABORT, 'injected typed mobility delete failure');
             END;",
        )
        .unwrap();
    drop(connection);

    let error = store
        .destroy_auth_mobility_session(session_id)
        .await
        .expect_err("typed delete failure must reject the complete teardown");
    assert!(
        error
            .to_string()
            .contains("injected typed mobility delete failure"),
        "unexpected injected failure: {error:?}"
    );
    let rolled_back_session = store
        .get_session(session_id)
        .await
        .expect("authoritative session must roll back")
        .expect("authoritative session still exists");
    assert_eq!(
        serde_json::to_value(&rolled_back_session).unwrap(),
        serde_json::to_value(&session).unwrap()
    );
    assert_eq!(
        store
            .list_auth_mobility_session_whitelist_ids(session_id)
            .await
            .expect("mobility state must roll back"),
        vec!["whitelist:atomic-destroy".to_string()]
    );

    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch("DROP TRIGGER fail_typed_mobility_delete;")
        .unwrap();
    drop(connection);
    assert_eq!(
        store
            .destroy_auth_mobility_session(session_id)
            .await
            .expect("retry atomic teardown"),
        vec!["whitelist:atomic-destroy".to_string()]
    );
    assert!(store.get_session(session_id).await.unwrap().is_none());
}

#[tokio::test]
async fn login_backoff_dual_write_uses_legacy_authority_and_repairs_shadow() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let ip = "192.0.2.140";

    let first = store
        .register_login_backoff_failure(ip)
        .await
        .expect("register login failure");
    assert_eq!(first.attempts, 1);
    let typed = store
        .typed_login_backoff
        .load(ip)
        .await
        .expect("load typed login backoff")
        .expect("typed login backoff exists");
    assert_eq!(
        serde_json::from_str::<Value>(&typed.state_json).unwrap()["attempts"],
        json!(1)
    );
    assert!(typed.expires_at_ms > crate::time_utils::now_ms());

    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE login_backoff_attempts SET state_json = ?2 WHERE ip = ?1",
            tokio_rusqlite::rusqlite::params![
                ip,
                json!({
                    "ip": ip,
                    "attempts": 999,
                    "lastAttempt": 0,
                    "blockedUntil": 9_999_999_999_999_i64
                })
                .to_string()
            ],
        )
        .unwrap();
    drop(connection);

    let status = store
        .get_login_backoff_status(ip)
        .await
        .expect("legacy status survives typed mismatch");
    assert_eq!(status.attempts, 1);
    let repaired = store
        .typed_login_backoff
        .load(ip)
        .await
        .expect("load repaired typed login backoff")
        .expect("repaired typed login backoff exists");
    assert_eq!(
        serde_json::from_str::<Value>(&repaired.state_json).unwrap()["attempts"],
        json!(1)
    );
    let shadow = store.typed_login_backoff_shadow_status();
    assert!(!shadow.healthy);
    assert_eq!(shadow.mismatch_count, 1);

    store
        .reset_login_backoff(ip)
        .await
        .expect("reset login backoff");
    assert_eq!(store.typed_login_backoff.count().await.unwrap(), 0);
}

#[tokio::test]
async fn login_backoff_concurrent_failures_remain_atomic_in_both_stores() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    let ip = "192.0.2.141";
    let mut tasks = Vec::new();
    for _ in 0..16 {
        let store = store.clone();
        tasks.push(tokio::spawn(async move {
            store.register_login_backoff_failure(ip).await
        }));
    }
    let mut attempts = Vec::new();
    for task in tasks {
        attempts.push(
            task.await
                .expect("join concurrent failure")
                .expect("register concurrent failure")
                .attempts,
        );
    }
    attempts.sort_unstable();
    assert_eq!(attempts, (1..=16).collect::<Vec<_>>());

    let status = store
        .get_login_backoff_status(ip)
        .await
        .expect("load final login backoff");
    assert_eq!(status.attempts, 16);
    let typed = store
        .typed_login_backoff
        .load(ip)
        .await
        .expect("load typed final login backoff")
        .expect("typed final login backoff exists");
    assert_eq!(
        serde_json::from_str::<Value>(&typed.state_json).unwrap()["attempts"],
        json!(16)
    );
    assert!(store.typed_login_backoff_shadow_status().healthy);
}

#[tokio::test]
async fn malformed_legacy_login_backoff_never_counts_as_healthy_typed_evidence() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    let ip = "192.0.2.145";
    store
        .set_string_value_with_optional_ttl(&login_backoff_key(ip), "not-json", Some(3_600))
        .await
        .expect("seed malformed legacy backoff");

    let status = store
        .get_login_backoff_status(ip)
        .await
        .expect("legacy-compatible malformed status");
    assert_eq!(status.attempts, 0);
    assert!(!status.blocked);
    assert!(store.typed_login_backoff.load(ip).await.unwrap().is_none());
    let shadow = store.typed_login_backoff_shadow_status();
    assert!(!shadow.healthy);
    assert_eq!(shadow.mismatch_count, 1);
}

#[tokio::test]
async fn login_backoff_typed_failure_rolls_back_legacy_and_lazy_expiry_syncs() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let rollback_ip = "192.0.2.142";
    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "CREATE TRIGGER fail_typed_login_backoff_insert
             BEFORE INSERT ON login_backoff_attempts
             BEGIN
               SELECT RAISE(FAIL, 'forced typed login-backoff failure');
             END;",
        )
        .unwrap();
    drop(connection);
    assert!(
        store
            .register_login_backoff_failure(rollback_ip)
            .await
            .is_err()
    );
    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
    let legacy_count = connection
        .query_row(
            "SELECT COUNT(*) FROM kv_keys WHERE key = ?1",
            [login_backoff_key(rollback_ip)],
            |row| row.get::<_, i64>(0),
        )
        .unwrap();
    assert_eq!(legacy_count, 0);
    connection
        .execute("DROP TRIGGER fail_typed_login_backoff_insert", [])
        .unwrap();
    drop(connection);

    let expiry_ip = "192.0.2.143";
    store
        .register_login_backoff_failure(expiry_ip)
        .await
        .expect("seed expiring login backoff");
    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE kv_keys SET expires_at_ms = 0 WHERE key = ?1",
            [login_backoff_key(expiry_ip)],
        )
        .unwrap();
    drop(connection);
    let expired = store
        .get_login_backoff_status(expiry_ip)
        .await
        .expect("read expired login backoff");
    assert_eq!(expired.attempts, 0);
    assert!(
        store
            .typed_login_backoff
            .load(expiry_ip)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn login_backoff_shadow_rebuilds_after_backup_restore_and_clear() {
    let source_dir = tempfile::tempdir().expect("create source temp dir");
    let source = Store::connect(source_dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open source store");
    let ip = "192.0.2.144";
    source
        .register_login_backoff_failure(ip)
        .await
        .expect("seed source login backoff");
    let entries = source
        .export_backup_entries_by_prefix_limited("fn_knock:login_backoff:", 1_000_000, |_| true)
        .await
        .expect("export login-backoff backup");

    let target_dir = tempfile::tempdir().expect("create target temp dir");
    let target = Store::connect(target_dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open target store");
    target
        .replace_backup_entries_by_prefix("fn_knock:", &entries, 200)
        .await
        .expect("restore login-backoff backup");
    assert!(target.typed_login_backoff.load(ip).await.unwrap().is_some());
    assert_eq!(
        target.get_login_backoff_status(ip).await.unwrap().attempts,
        1
    );

    target.clear_all_keys().await.expect("clear restored store");
    assert_eq!(target.typed_login_backoff.count().await.unwrap(), 0);
}

fn subdomain_rate_limit_key(scope: &str, fill: char) -> String {
    format!(
        "{}{scope}:{}",
        crate::storage::typed_subdomain_rate_limit::RATE_LIMIT_PREFIX,
        fill.to_string().repeat(64)
    )
}

#[tokio::test]
async fn subdomain_rate_limit_is_atomic_in_legacy_and_typed_stores() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    let key = subdomain_rate_limit_key("client", 'a');
    let mut tasks = Vec::new();
    for _ in 0..16 {
        let store = store.clone();
        let key = key.clone();
        tasks.push(tokio::spawn(async move {
            store.increment_counter_with_ttl(&key, 60).await
        }));
    }
    let mut counts = Vec::new();
    for task in tasks {
        counts.push(
            task.await
                .expect("join counter increment")
                .expect("increment counter"),
        );
    }
    counts.sort_unstable();
    assert_eq!(counts, (1..=16).collect::<Vec<_>>());

    assert_eq!(
        store.get_string_value(&key).await.unwrap().as_deref(),
        Some("16")
    );
    let typed = store
        .typed_subdomain_rate_limit
        .load(&key)
        .await
        .unwrap()
        .expect("typed rate-limit counter");
    assert_eq!(typed.scope, "client");
    assert_eq!(typed.counter_value, 16);
    assert!(typed.expires_at_ms > crate::time_utils::now_ms());
    assert!(store.typed_subdomain_rate_limit_shadow_status().healthy);
}

#[tokio::test]
async fn subdomain_rate_limit_uses_legacy_authority_and_reports_repair() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let key = subdomain_rate_limit_key("host", 'b');
    assert_eq!(store.increment_counter_with_ttl(&key, 60).await.unwrap(), 1);

    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE subdomain_rule_rate_limit_counters SET counter_value = 999",
            [],
        )
        .unwrap();
    drop(connection);

    assert_eq!(store.increment_counter_with_ttl(&key, 60).await.unwrap(), 2);
    assert_eq!(
        store.get_string_value(&key).await.unwrap().as_deref(),
        Some("2")
    );
    assert_eq!(
        store
            .typed_subdomain_rate_limit
            .load(&key)
            .await
            .unwrap()
            .unwrap()
            .counter_value,
        2
    );
    let status = store.typed_subdomain_rate_limit_shadow_status();
    assert!(!status.healthy);
    assert_eq!(status.mismatch_count, 1);
}

#[tokio::test]
async fn subdomain_rate_limit_typed_failure_rolls_back_and_malformed_values_fail_closed() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let rollback_key = subdomain_rate_limit_key("client", 'c');
    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "CREATE TRIGGER fail_typed_subdomain_rate_limit_insert
             BEFORE INSERT ON subdomain_rule_rate_limit_counters
             BEGIN
               SELECT RAISE(FAIL, 'forced typed subdomain rate-limit failure');
             END;",
        )
        .unwrap();
    drop(connection);
    assert!(
        store
            .increment_counter_with_ttl(&rollback_key, 60)
            .await
            .is_err()
    );
    assert!(
        store
            .get_string_value(&rollback_key)
            .await
            .unwrap()
            .is_none()
    );

    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
    connection
        .execute("DROP TRIGGER fail_typed_subdomain_rate_limit_insert", [])
        .unwrap();
    drop(connection);
    let malformed_key = subdomain_rate_limit_key("host", 'd');
    store
        .set_string_value_with_optional_ttl(&malformed_key, "not-an-integer", Some(60))
        .await
        .expect("seed malformed compatibility counter");
    assert!(
        store
            .increment_counter_with_ttl(&malformed_key, 60)
            .await
            .is_err()
    );
    assert_eq!(
        store
            .get_string_value(&malformed_key)
            .await
            .unwrap()
            .as_deref(),
        Some("not-an-integer")
    );
    assert!(
        store
            .typed_subdomain_rate_limit
            .load(&malformed_key)
            .await
            .unwrap()
            .is_none()
    );
    assert!(!store.typed_subdomain_rate_limit_shadow_status().healthy);
}

#[tokio::test]
async fn subdomain_rate_limit_shadow_rebuilds_after_expiry_backup_and_clear() {
    let source_dir = tempfile::tempdir().expect("create source temp dir");
    let source_path = source_dir.path().join("fn-knock.sqlite3");
    let source = Store::connect(&source_path)
        .await
        .expect("open source store");
    let expired_key = subdomain_rate_limit_key("client", 'e');
    source
        .increment_counter_with_ttl(&expired_key, 60)
        .await
        .expect("seed expiring counter");
    let connection = tokio_rusqlite::rusqlite::Connection::open(&source_path).unwrap();
    connection
        .execute(
            "UPDATE kv_keys SET expires_at_ms = 0 WHERE key = ?1",
            [&expired_key],
        )
        .unwrap();
    drop(connection);
    assert!(
        !source
            .typed_subdomain_rate_limit
            .verify_and_repair(&expired_key)
            .await
            .unwrap()
    );
    assert!(
        source
            .typed_subdomain_rate_limit
            .load(&expired_key)
            .await
            .unwrap()
            .is_none()
    );

    let backup_key = subdomain_rate_limit_key("host", 'f');
    source
        .increment_counter_with_ttl(&backup_key, 60)
        .await
        .expect("seed backup counter");
    let entries = source
        .export_backup_entries_by_prefix_limited(
            crate::storage::typed_subdomain_rate_limit::RATE_LIMIT_PREFIX,
            1_000_000,
            |_| true,
        )
        .await
        .expect("export rate-limit backup");
    let target_dir = tempfile::tempdir().expect("create target temp dir");
    let target = Store::connect(target_dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open target store");
    target
        .replace_backup_entries_by_prefix("fn_knock:", &entries, 200)
        .await
        .expect("restore rate-limit backup");
    assert_eq!(
        target
            .typed_subdomain_rate_limit
            .load(&backup_key)
            .await
            .unwrap()
            .unwrap()
            .counter_value,
        1
    );
    target.clear_all_keys().await.expect("clear restored store");
    assert_eq!(target.typed_subdomain_rate_limit.count().await.unwrap(), 0);
}

fn wol_cooldown_key(target_id: &str) -> String {
    format!(
        "{}{target_id}",
        crate::storage::typed_wol_cooldown::COOLDOWN_PREFIX
    )
}

#[tokio::test]
async fn wol_cooldown_allows_one_concurrent_winner_in_both_stores() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    let key = wol_cooldown_key("concurrent-target");
    let mut tasks = Vec::new();
    for _ in 0..16 {
        let store = store.clone();
        let key = key.clone();
        tasks.push(tokio::spawn(async move {
            store.set_key_if_not_exists_with_ttl(&key, "1", 3).await
        }));
    }
    let mut winners = 0;
    for task in tasks {
        if task.await.unwrap().unwrap() {
            winners += 1;
        }
    }
    assert_eq!(winners, 1);
    assert_eq!(
        store.get_string_value(&key).await.unwrap().as_deref(),
        Some("1")
    );
    let typed = store
        .typed_wol_cooldown
        .load("concurrent-target")
        .await
        .unwrap()
        .expect("typed WOL cooldown");
    assert!(typed.expires_at_ms > crate::time_utils::now_ms());
}

#[tokio::test]
async fn wol_cooldown_uses_legacy_authority_repairs_and_rolls_back_typed_failure() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let repair_key = wol_cooldown_key("repair-target");
    assert!(
        store
            .set_key_if_not_exists_with_ttl(&repair_key, "1", 60)
            .await
            .unwrap()
    );
    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE wol_wake_cooldowns SET expires_at_ms = 1 WHERE target_id = 'repair-target'",
            [],
        )
        .unwrap();
    drop(connection);
    assert!(
        !store
            .set_key_if_not_exists_with_ttl(&repair_key, "1", 60)
            .await
            .unwrap()
    );
    assert!(
        store
            .typed_wol_cooldown
            .load("repair-target")
            .await
            .unwrap()
            .unwrap()
            .expires_at_ms
            > crate::time_utils::now_ms()
    );
    let status = store.typed_wol_cooldown_shadow_status();
    assert!(!status.healthy);
    assert_eq!(status.mismatch_count, 1);

    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "CREATE TRIGGER fail_typed_wol_cooldown_insert
             BEFORE INSERT ON wol_wake_cooldowns
             BEGIN
               SELECT RAISE(FAIL, 'forced typed WOL cooldown failure');
             END;",
        )
        .unwrap();
    drop(connection);
    let rollback_key = wol_cooldown_key("rollback-target");
    assert!(
        store
            .set_key_if_not_exists_with_ttl(&rollback_key, "1", 60)
            .await
            .is_err()
    );
    assert!(
        store
            .get_string_value(&rollback_key)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .typed_wol_cooldown
            .load("rollback-target")
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn wol_cooldown_expiry_backup_and_clear_keep_typed_shadow_exact() {
    let source_dir = tempfile::tempdir().expect("create source temp dir");
    let source_path = source_dir.path().join("fn-knock.sqlite3");
    let source = Store::connect(&source_path)
        .await
        .expect("open source store");
    let key = wol_cooldown_key("backup-target");
    source
        .set_key_if_not_exists_with_ttl(&key, "1", 60)
        .await
        .expect("seed WOL cooldown");
    let entries = source
        .export_backup_entries_by_prefix_limited(
            crate::storage::typed_wol_cooldown::COOLDOWN_PREFIX,
            1_000_000,
            |_| true,
        )
        .await
        .expect("export WOL cooldown");
    let target_dir = tempfile::tempdir().expect("create target temp dir");
    let target = Store::connect(target_dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open target store");
    target
        .replace_backup_entries_by_prefix("fn_knock:", &entries, 200)
        .await
        .expect("restore WOL cooldown");
    assert!(
        target
            .typed_wol_cooldown
            .load("backup-target")
            .await
            .unwrap()
            .is_some()
    );

    let connection = tokio_rusqlite::rusqlite::Connection::open(&source_path).unwrap();
    connection
        .execute(
            "UPDATE kv_keys SET expires_at_ms = 0 WHERE key = ?1",
            [&key],
        )
        .unwrap();
    drop(connection);
    assert!(
        !source
            .typed_wol_cooldown
            .verify_and_repair("backup-target")
            .await
            .unwrap()
    );
    assert!(
        source
            .typed_wol_cooldown
            .load("backup-target")
            .await
            .unwrap()
            .is_none()
    );
    target
        .clear_all_keys()
        .await
        .expect("clear target keyspace");
    assert_eq!(target.typed_wol_cooldown.count().await.unwrap(), 0);
}

#[tokio::test]
async fn hmac_nonce_allows_one_concurrent_winner_and_stores_only_a_typed_digest() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    let nonce = "concurrent-sensitive-nonce";
    let mut tasks = Vec::new();
    for _ in 0..16 {
        let store = store.clone();
        tasks.push(tokio::spawn(async move {
            store.set_nonce_if_not_exists(nonce, 60).await
        }));
    }
    let mut winners = 0;
    for task in tasks {
        if task.await.unwrap().unwrap() {
            winners += 1;
        }
    }
    assert_eq!(winners, 1);
    let typed = store
        .typed_hmac_nonce
        .load(nonce)
        .await
        .unwrap()
        .expect("typed HMAC nonce");
    assert_eq!(typed.nonce_digest.len(), 64);
    assert_ne!(typed.nonce_digest, nonce);
    assert!(typed.expires_at_ms > crate::time_utils::now_ms());
}

#[tokio::test]
async fn hmac_nonce_repairs_legacy_authority_and_typed_failure_rolls_back() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let nonce = "repair-sensitive-nonce";
    assert!(store.set_nonce_if_not_exists(nonce, 60).await.unwrap());

    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
    connection
        .execute("UPDATE hmac_replay_nonces SET expires_at_ms = 1", [])
        .unwrap();
    drop(connection);
    assert!(!store.set_nonce_if_not_exists(nonce, 60).await.unwrap());
    assert!(
        store
            .typed_hmac_nonce
            .load(nonce)
            .await
            .unwrap()
            .unwrap()
            .expires_at_ms
            > crate::time_utils::now_ms()
    );
    let status = store.typed_hmac_nonce_shadow_status();
    assert!(!status.healthy);
    assert_eq!(status.mismatch_count, 1);

    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "CREATE TRIGGER fail_typed_hmac_nonce_insert
             BEFORE INSERT ON hmac_replay_nonces
             BEGIN
               SELECT RAISE(FAIL, 'forced typed HMAC nonce failure');
             END;",
        )
        .unwrap();
    drop(connection);
    let rollback_nonce = "rollback-sensitive-nonce";
    assert!(
        store
            .set_nonce_if_not_exists(rollback_nonce, 60)
            .await
            .is_err()
    );
    assert!(
        store
            .get_string_value(&format!(
                "{}{}",
                crate::storage::typed_hmac_nonce::NONCE_PREFIX,
                rollback_nonce
            ))
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .typed_hmac_nonce
            .load(rollback_nonce)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn hmac_nonce_expiry_backup_and_clear_keep_typed_shadow_exact() {
    let source_dir = tempfile::tempdir().expect("create source temp dir");
    let source_path = source_dir.path().join("fn-knock.sqlite3");
    let source = Store::connect(&source_path)
        .await
        .expect("open source store");
    let nonce = "backup-sensitive-nonce";
    source
        .set_nonce_if_not_exists(nonce, 60)
        .await
        .expect("seed HMAC nonce");
    let entries = source
        .export_backup_entries_by_prefix_limited(
            crate::storage::typed_hmac_nonce::NONCE_PREFIX,
            1_000_000,
            |_| true,
        )
        .await
        .expect("export HMAC nonce");
    let target_dir = tempfile::tempdir().expect("create target temp dir");
    let target = Store::connect(target_dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open target store");
    target
        .replace_backup_entries_by_prefix("fn_knock:", &entries, 200)
        .await
        .expect("restore HMAC nonce");
    assert!(target.typed_hmac_nonce.load(nonce).await.unwrap().is_some());

    let key = format!(
        "{}{}",
        crate::storage::typed_hmac_nonce::NONCE_PREFIX,
        nonce
    );
    let connection = tokio_rusqlite::rusqlite::Connection::open(&source_path).unwrap();
    connection
        .execute(
            "UPDATE kv_keys SET expires_at_ms = 0 WHERE key = ?1",
            [&key],
        )
        .unwrap();
    drop(connection);
    assert!(
        !source
            .typed_hmac_nonce
            .verify_and_repair(nonce)
            .await
            .unwrap()
    );
    assert!(source.typed_hmac_nonce.load(nonce).await.unwrap().is_none());
    target
        .clear_all_keys()
        .await
        .expect("clear target keyspace");
    assert_eq!(target.typed_hmac_nonce.count().await.unwrap(), 0);
}

fn fnos_validation_document() -> Value {
    json!({
        "version": 2,
        "valid": true,
        "validationState": "valid",
        "shareId": "abc123abc123abc123",
        "backendId": "backend-digest",
        "cleanPath": "/s/abc123abc123abc123",
        "token": "share-token",
        "checkedAt": "2026-08-11T00:00:00Z"
    })
}

fn fnos_session_document() -> Value {
    json!({
        "version": 2,
        "shareId": "abc123abc123abc123",
        "backendId": "backend-digest",
        "cleanPath": "/s/abc123abc123abc123",
        "token": "share-token",
        "issuedAt": "2026-08-11T00:00:00Z",
        "lastSeenAt": "2026-08-11T00:00:01Z"
    })
}

#[tokio::test]
async fn fnos_share_aggregate_tracks_documents_and_one_lock_winner() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    let validation_key = format!(
        "{}backend:share",
        crate::storage::typed_fnos_share::VALIDATION_PREFIX
    );
    let session_key = format!(
        "{}session-id",
        crate::storage::typed_fnos_share::SESSION_PREFIX
    );
    let lock_key = format!(
        "{}backend:share",
        crate::storage::typed_fnos_share::LOCK_PREFIX
    );
    store
        .set_json_value_ex(&validation_key, &fnos_validation_document(), 60)
        .await
        .unwrap();
    store
        .set_json_value_ex(&session_key, &fnos_session_document(), 60)
        .await
        .unwrap();

    let mut tasks = Vec::new();
    for index in 0..16 {
        let store = store.clone();
        let key = lock_key.clone();
        tasks.push(tokio::spawn(async move {
            let token = format!("secret-lock-token-{index}");
            let won = store
                .set_key_if_not_exists_with_ttl(&key, &token, 60)
                .await?;
            Ok::<_, crate::storage::StorageError>((won, token))
        }));
    }
    let mut winner = None;
    for task in tasks {
        let (won, token) = task.await.unwrap().unwrap();
        if won {
            assert!(winner.replace(token).is_none());
        }
    }
    let winner = winner.expect("one lock winner");
    assert_eq!(store.typed_fnos_share.count().await.unwrap(), 3);
    assert!(
        store
            .typed_fnos_share
            .load_key(&validation_key)
            .await
            .unwrap()
            .unwrap()
            .payload_json
            .is_some()
    );
    let lock = store
        .typed_fnos_share
        .load_key(&lock_key)
        .await
        .unwrap()
        .unwrap();
    assert!(lock.payload_json.is_none());
    assert_eq!(lock.guard_digest.as_deref().map(str::len), Some(64));
    assert_ne!(lock.guard_digest.as_deref(), Some(winner.as_str()));
}

#[tokio::test]
async fn fnos_share_repairs_legacy_authority_and_typed_failure_rolls_back() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let validation_key = format!(
        "{}backend:repair",
        crate::storage::typed_fnos_share::VALIDATION_PREFIX
    );
    store
        .set_json_value_ex(&validation_key, &fnos_validation_document(), 60)
        .await
        .unwrap();
    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE fnos_share_runtime_capabilities SET payload_json = '{}'",
            [],
        )
        .unwrap();
    drop(connection);
    assert_eq!(
        store
            .get_json_value(&validation_key)
            .await
            .unwrap()
            .unwrap()["shareId"],
        json!("abc123abc123abc123")
    );
    let status = store.typed_fnos_share_shadow_status();
    assert!(!status.healthy);
    assert_eq!(status.mismatch_count, 1);

    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "CREATE TRIGGER fail_typed_fnos_share_insert
             BEFORE INSERT ON fnos_share_runtime_capabilities
             BEGIN
               SELECT RAISE(FAIL, 'forced typed fnOS share failure');
             END;",
        )
        .unwrap();
    drop(connection);
    let rollback_key = format!(
        "{}rollback-session",
        crate::storage::typed_fnos_share::SESSION_PREFIX
    );
    assert!(
        store
            .set_json_value_ex(&rollback_key, &fnos_session_document(), 60)
            .await
            .is_err()
    );
    assert!(store.get_json_value(&rollback_key).await.unwrap().is_none());
    assert!(
        store
            .typed_fnos_share
            .load_key(&rollback_key)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn fnos_share_expiry_restore_and_clear_keep_typed_shadow_exact() {
    let source_dir = tempfile::tempdir().expect("create source temp dir");
    let source_path = source_dir.path().join("fn-knock.sqlite3");
    let source = Store::connect(&source_path)
        .await
        .expect("open source store");
    let session_key = format!(
        "{}backup-session",
        crate::storage::typed_fnos_share::SESSION_PREFIX
    );
    source
        .set_json_value_ex(&session_key, &fnos_session_document(), 60)
        .await
        .unwrap();
    let entries = source
        .export_backup_entries_by_prefix_limited(
            crate::storage::typed_fnos_share::SESSION_PREFIX,
            1_000_000,
            |_| true,
        )
        .await
        .unwrap();
    let target_dir = tempfile::tempdir().expect("create target temp dir");
    let target = Store::connect(target_dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open target store");
    target
        .replace_backup_entries_by_prefix("fn_knock:", &entries, 200)
        .await
        .unwrap();
    assert!(
        target
            .typed_fnos_share
            .load_key(&session_key)
            .await
            .unwrap()
            .is_some()
    );

    let connection = tokio_rusqlite::rusqlite::Connection::open(&source_path).unwrap();
    connection
        .execute(
            "UPDATE kv_keys SET expires_at_ms = 0 WHERE key = ?1",
            [&session_key],
        )
        .unwrap();
    drop(connection);
    assert!(
        !source
            .typed_fnos_share
            .verify_and_repair_key(&session_key)
            .await
            .unwrap()
    );
    assert!(
        source
            .typed_fnos_share
            .load_key(&session_key)
            .await
            .unwrap()
            .is_none()
    );
    target.clear_all_keys().await.unwrap();
    assert_eq!(target.typed_fnos_share.count().await.unwrap(), 0);
}

fn subdomain_grant_keys(token: &str, host: &str) -> (String, String) {
    (
        format!(
            "{}{}",
            crate::storage::typed_subdomain_grant::GRANT_PREFIX,
            crate::crypto_utils::sha256_hex_str(token)
        ),
        format!(
            "{}{}",
            crate::storage::typed_subdomain_grant::ACTIVE_INDEX_PREFIX,
            crate::crypto_utils::sha256_hex_str(host)
        ),
    )
}

fn subdomain_grant_document(host: &str, last_access_at: i64) -> String {
    serde_json::to_string(&json!({
        "host": host,
        "policy_version": "policy-v1",
        "group_id": "group-v1",
        "issued_at": 1_700_000_000,
        "last_access_at": last_access_at,
        "hard_expires_at": 1_800_000_000
    }))
    .unwrap()
}

#[tokio::test]
async fn subdomain_grant_dual_writes_record_and_active_index_atomically() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    let host = "app.example.com";
    let (grant_key, active_key) = subdomain_grant_keys("grant-token", host);
    assert!(
        store
            .set_expiring_string_with_zset_limit(
                &grant_key,
                &subdomain_grant_document(host, 1_700_000_010),
                60,
                &active_key,
                1_700_000_010,
                1_700_000_070,
                10,
            )
            .await
            .unwrap()
    );
    let grant = store
        .typed_subdomain_grant
        .load_grant(&grant_key)
        .await
        .unwrap()
        .expect("typed subdomain grant");
    assert_eq!(grant.host, host);
    assert_eq!(grant.last_access_at, 1_700_000_010);
    assert!(grant.expires_at_ms > crate::time_utils::now_ms());
    let active = store
        .typed_subdomain_grant
        .active_entries(&active_key)
        .await
        .unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].expires_at_score, 1_700_000_070);
    assert_eq!(store.typed_subdomain_grant.counts().await.unwrap(), (1, 1));

    store
        .delete_string_and_zrem(&grant_key, &active_key, &grant_key)
        .await
        .unwrap();
    assert_eq!(store.typed_subdomain_grant.counts().await.unwrap(), (0, 0));
}

#[tokio::test]
async fn subdomain_grant_repairs_whole_aggregate_and_typed_failure_rolls_back() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let host = "repair.example.com";
    let (grant_key, active_key) = subdomain_grant_keys("repair-token", host);
    store
        .set_expiring_string_with_zset_limit(
            &grant_key,
            &subdomain_grant_document(host, 1_700_000_020),
            60,
            &active_key,
            1_700_000_020,
            1_700_000_080,
            10,
        )
        .await
        .unwrap();
    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
    connection
        .execute("DELETE FROM subdomain_rule_grant_active_entries", [])
        .unwrap();
    drop(connection);
    assert!(store.get_string_value(&grant_key).await.unwrap().is_some());
    assert_eq!(
        store
            .typed_subdomain_grant
            .active_entries(&active_key)
            .await
            .unwrap()
            .len(),
        1
    );
    let status = store.typed_subdomain_grant_shadow_status();
    assert!(!status.healthy);
    assert_eq!(status.mismatch_count, 1);

    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "CREATE TRIGGER fail_typed_subdomain_grant_insert
             BEFORE INSERT ON subdomain_rule_grants
             BEGIN
               SELECT RAISE(FAIL, 'forced typed subdomain grant failure');
             END;",
        )
        .unwrap();
    drop(connection);
    let (rollback_grant, rollback_active) =
        subdomain_grant_keys("rollback-token", "rollback.example.com");
    assert!(
        store
            .set_expiring_string_with_zset_limit(
                &rollback_grant,
                &subdomain_grant_document("rollback.example.com", 1_700_000_030),
                60,
                &rollback_active,
                1_700_000_030,
                1_700_000_090,
                10,
            )
            .await
            .is_err()
    );
    assert!(
        store
            .get_string_value(&rollback_grant)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .typed_subdomain_grant
            .load_grant(&rollback_grant)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn subdomain_grant_expiry_restore_and_clear_keep_aggregate_exact() {
    let source_dir = tempfile::tempdir().expect("create source temp dir");
    let source_path = source_dir.path().join("fn-knock.sqlite3");
    let source = Store::connect(&source_path)
        .await
        .expect("open source store");
    let host = "backup.example.com";
    let (grant_key, active_key) = subdomain_grant_keys("backup-token", host);
    source
        .set_expiring_string_with_zset_limit(
            &grant_key,
            &subdomain_grant_document(host, 1_700_000_040),
            60,
            &active_key,
            1_700_000_040,
            1_700_000_100,
            10,
        )
        .await
        .unwrap();
    let entries = source
        .export_backup_entries_by_prefix_limited("fn_knock:auth:subdomain_rule_", 1_000_000, |_| {
            true
        })
        .await
        .unwrap();
    let target_dir = tempfile::tempdir().expect("create target temp dir");
    let target = Store::connect(target_dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open target store");
    target
        .replace_backup_entries_by_prefix("fn_knock:", &entries, 200)
        .await
        .unwrap();
    assert_eq!(target.typed_subdomain_grant.counts().await.unwrap(), (1, 1));

    let connection = tokio_rusqlite::rusqlite::Connection::open(&source_path).unwrap();
    connection
        .execute(
            "UPDATE kv_keys SET expires_at_ms = 0 WHERE key = ?1",
            [&grant_key],
        )
        .unwrap();
    drop(connection);
    assert!(source.get_string_value(&grant_key).await.unwrap().is_none());
    assert_eq!(source.typed_subdomain_grant.counts().await.unwrap(), (0, 0));
    target.clear_all_keys().await.unwrap();
    assert_eq!(target.typed_subdomain_grant.counts().await.unwrap(), (0, 0));
}

fn whitelist_owner_keys(label: &str) -> (String, String) {
    let mapping = format!(
        "{}{}",
        crate::storage::typed_whitelist_runtime::OWNER_PREFIX,
        crate::crypto_utils::sha256_hex_str(label)
    );
    let lock = format!("{mapping}:lock");
    (mapping, lock)
}

#[tokio::test]
async fn whitelist_owner_runtime_tracks_mapping_and_one_lock_winner() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    let (mapping_key, lock_key) = whitelist_owner_keys("owner-one");
    store
        .set_string_value_with_optional_ttl(&mapping_key, "whitelist:record-one", Some(120))
        .await
        .unwrap();

    let mut tasks = Vec::new();
    for index in 0..16 {
        let store = store.clone();
        let key = lock_key.clone();
        tasks.push(tokio::spawn(async move {
            let lock_id = format!("private-lock-{index}");
            let won = store
                .set_json_value_nx_ex(
                    &key,
                    &json!({ "lockId": lock_id, "createdAt": "2026-08-11T00:00:00Z" }),
                    60,
                )
                .await?;
            Ok::<_, crate::storage::StorageError>((won, lock_id))
        }));
    }
    let mut winner = None;
    for task in tasks {
        let (won, lock_id) = task.await.unwrap().unwrap();
        if won {
            assert!(winner.replace(lock_id).is_none());
        }
    }
    let winner = winner.expect("one whitelist owner lock winner");
    assert_eq!(
        store.typed_whitelist_runtime.counts().await.unwrap(),
        (1, 1)
    );
    let mapping = store
        .typed_whitelist_runtime
        .load_key(&mapping_key)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        mapping,
        crate::storage::typed_whitelist_runtime::TypedWhitelistOwnerRuntime::Mapping {
            record_id,
            ..
        } if record_id == "whitelist:record-one"
    ));
    let lock = store
        .typed_whitelist_runtime
        .load_key(&lock_key)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        lock,
        crate::storage::typed_whitelist_runtime::TypedWhitelistOwnerRuntime::Lock {
            lock_digest,
            ..
        } if lock_digest.len() == 64 && lock_digest != winner
    ));
}

#[tokio::test]
async fn whitelist_owner_runtime_repairs_and_owned_lock_operations_stay_exact() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let (mapping_key, lock_key) = whitelist_owner_keys("owner-repair");
    store
        .set_string_value_with_optional_ttl(&mapping_key, "whitelist:record-repair", None)
        .await
        .unwrap();
    assert!(
        store
            .set_json_value_nx_ex(
                &lock_key,
                &json!({ "lockId": "owned-lock", "createdAt": "2026-08-11T00:00:00Z" }),
                60,
            )
            .await
            .unwrap()
    );
    assert!(
        store
            .set_json_lock_if_owned_ex(
                &lock_key,
                "owned-lock",
                &json!({ "lockId": "owned-lock", "createdAt": "2026-08-11T00:00:01Z" }),
                120,
            )
            .await
            .unwrap()
    );

    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE whitelist_auto_owner_mappings SET whitelist_record_id = 'typed-only-wrong'",
            [],
        )
        .unwrap();
    drop(connection);
    assert_eq!(
        store
            .get_string_value(&mapping_key)
            .await
            .unwrap()
            .as_deref(),
        Some("whitelist:record-repair")
    );
    let status = store.typed_whitelist_runtime_shadow_status();
    assert!(!status.healthy);
    assert_eq!(status.mismatch_count, 1);
    assert!(
        store
            .delete_lock_if_owned(&lock_key, "owned-lock")
            .await
            .unwrap()
    );
    assert_eq!(
        store.typed_whitelist_runtime.counts().await.unwrap(),
        (1, 0)
    );
}

#[tokio::test]
async fn whitelist_owner_runtime_typed_failure_rolls_back_restore_and_clear() {
    let source_dir = tempfile::tempdir().expect("create source temp dir");
    let source_path = source_dir.path().join("fn-knock.sqlite3");
    let source = Store::connect(&source_path)
        .await
        .expect("open source store");
    let (mapping_key, _) = whitelist_owner_keys("owner-backup");
    source
        .set_string_value_with_optional_ttl(&mapping_key, "whitelist:record-backup", Some(120))
        .await
        .unwrap();
    let entries = source
        .export_backup_entries_by_prefix_limited(
            crate::storage::typed_whitelist_runtime::OWNER_PREFIX,
            1_000_000,
            |key| !key.ends_with(":lock"),
        )
        .await
        .unwrap();
    let target_dir = tempfile::tempdir().expect("create target temp dir");
    let target = Store::connect(target_dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open target store");
    target
        .replace_backup_entries_by_prefix("fn_knock:", &entries, 200)
        .await
        .unwrap();
    assert_eq!(
        target.typed_whitelist_runtime.counts().await.unwrap(),
        (1, 0)
    );

    let connection = tokio_rusqlite::rusqlite::Connection::open(&source_path).unwrap();
    connection
        .execute_batch(
            "CREATE TRIGGER fail_typed_whitelist_owner_insert
             BEFORE INSERT ON whitelist_auto_owner_mappings
             BEGIN
               SELECT RAISE(FAIL, 'forced typed whitelist owner failure');
             END;",
        )
        .unwrap();
    drop(connection);
    let (rollback_key, _) = whitelist_owner_keys("owner-rollback");
    assert!(
        source
            .set_string_value_with_optional_ttl(&rollback_key, "whitelist:rollback", None)
            .await
            .is_err()
    );
    assert!(
        source
            .get_string_value(&rollback_key)
            .await
            .unwrap()
            .is_none()
    );
    target.clear_all_keys().await.unwrap();
    assert_eq!(
        target.typed_whitelist_runtime.counts().await.unwrap(),
        (0, 0)
    );
}

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
        .typed_notification_runtime
        .load_lease("concurrent")
        .await
        .unwrap()
        .expect("typed notification lease");
    assert_eq!(typed.token, winner);
    assert!(typed.expires_at_ms > crate::time_utils::now_ms());

    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
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
            .typed_notification_runtime
            .load_window(&key)
            .await
            .unwrap()
            .len(),
        16
    );

    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
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

    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
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
    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
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
    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
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
    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
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
    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
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
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
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
            .typed_notification_runtime
            .load_ready_queue()
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn notification_delivery_queue_recovers_non_terminal_history_after_crash() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
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
            .typed_notification_runtime
            .load_lease("restore")
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(
        target
            .typed_notification_runtime
            .load_window(&notification_window_key("restore-rule", "global"))
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(
        target
            .typed_notification_runtime
            .load_cooldown(&notification_cooldown_key("restore-rule", "global"))
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(
        target
            .typed_notification_runtime
            .load_ready_queue()
            .await
            .unwrap()
            .len(),
        1
    );

    target.clear_all_keys().await.unwrap();
    let connection = tokio_rusqlite::rusqlite::Connection::open(&target_path).unwrap();
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

    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
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
            .typed_notification_runtime
            .load_ready_queue()
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn passkey_runtime_capabilities_use_legacy_authority_and_repair_shadow() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let challenge = "passkey-runtime-authority-challenge";
    let challenge_key = format!(
        "{}{}",
        crate::storage::typed_passkey_runtime::CHALLENGE_PREFIX,
        challenge
    );
    store
        .set_passkey_challenge(challenge, "auth", 600)
        .await
        .expect("seed challenge");
    store
        .set_passkey_state(challenge, &json!({ "ceremony": "auth" }), 600)
        .await
        .expect("seed state");
    let bind_token = store
        .create_passkey_bind_token("totp-passkey-runtime", 600)
        .await
        .expect("seed bind token");
    let bind_key = format!(
        "{}{}",
        crate::storage::typed_passkey_runtime::BIND_PREFIX,
        bind_token
    );
    assert_eq!(store.typed_passkey_runtime.count().await.unwrap(), 3);
    let typed_challenge = store
        .typed_passkey_runtime
        .load_key(&challenge_key)
        .await
        .unwrap()
        .expect("typed challenge");
    assert_eq!(typed_challenge.kind, "challenge");
    assert_eq!(typed_challenge.value, "auth");
    assert_eq!(
        typed_challenge.expires_at_ms,
        sqlite_key_expiry_at_ms(&path, &challenge_key)
            .await
            .expect("legacy challenge expiry")
    );
    assert_ne!(typed_challenge.digest, challenge);
    assert!(!typed_challenge.digest.contains(challenge));

    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE passkey_runtime_capabilities
             SET challenge_type = 'register'
             WHERE capability_kind = 'challenge' AND capability_digest = ?1",
            [typed_challenge.digest.as_str()],
        )
        .unwrap();
    drop(connection);
    assert!(
        !store
            .consume_passkey_challenge(challenge, "register")
            .await
            .expect("legacy type remains authoritative")
    );
    assert_eq!(
        store
            .typed_passkey_runtime
            .load_key(&challenge_key)
            .await
            .unwrap()
            .unwrap()
            .value,
        "auth"
    );
    assert!(
        store
            .consume_passkey_challenge(challenge, "auth")
            .await
            .expect("consume authoritative challenge")
    );
    assert!(
        store
            .typed_passkey_runtime
            .load_key(&challenge_key)
            .await
            .unwrap()
            .is_none()
    );

    let state_key = format!(
        "{}{}",
        crate::storage::typed_passkey_runtime::STATE_PREFIX,
        challenge
    );
    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
    connection
        .execute("DELETE FROM kv_keys WHERE key = ?1", [state_key.as_str()])
        .unwrap();
    drop(connection);
    assert!(
        store
            .consume_passkey_state(challenge)
            .await
            .expect("typed-only state cannot complete a ceremony")
            .is_none()
    );
    assert!(
        store
            .typed_passkey_runtime
            .load_key(&state_key)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        store
            .consume_passkey_bind_token(&bind_token)
            .await
            .expect("consume bind token")
            .as_deref(),
        Some("totp-passkey-runtime")
    );
    assert!(
        store
            .typed_passkey_runtime
            .load_key(&bind_key)
            .await
            .unwrap()
            .is_none()
    );
    let status = store.typed_passkey_runtime_shadow_status();
    assert!(status.healthy);
    assert_eq!(status.mismatch_count, 2);
}

#[tokio::test]
async fn passkey_runtime_typed_failures_roll_back_create_and_consume() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "CREATE TRIGGER fail_passkey_runtime_insert
             BEFORE INSERT ON passkey_runtime_capabilities
             BEGIN SELECT RAISE(ABORT, 'injected passkey runtime insert failure'); END;",
        )
        .unwrap();
    drop(connection);
    let challenge = "passkey-runtime-create-rollback";
    let challenge_key = format!(
        "{}{}",
        crate::storage::typed_passkey_runtime::CHALLENGE_PREFIX,
        challenge
    );
    let error = store
        .set_passkey_challenge(challenge, "auth", 600)
        .await
        .expect_err("typed insert failure must roll back challenge creation");
    assert!(
        error
            .to_string()
            .contains("injected passkey runtime insert failure")
    );
    assert!(
        store
            .get_string_value(&challenge_key)
            .await
            .unwrap()
            .is_none()
    );

    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch("DROP TRIGGER fail_passkey_runtime_insert;")
        .unwrap();
    drop(connection);
    let bind_token = store
        .create_passkey_bind_token("totp-rollback", 600)
        .await
        .expect("seed bind token");
    let bind_key = format!(
        "{}{}",
        crate::storage::typed_passkey_runtime::BIND_PREFIX,
        bind_token
    );
    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "CREATE TRIGGER fail_passkey_runtime_delete
             BEFORE DELETE ON passkey_runtime_capabilities
             BEGIN SELECT RAISE(ABORT, 'injected passkey runtime delete failure'); END;",
        )
        .unwrap();
    drop(connection);
    let error = store
        .consume_passkey_bind_token(&bind_token)
        .await
        .expect_err("typed delete failure must roll back one-time consumption");
    assert!(
        error
            .to_string()
            .contains("injected passkey runtime delete failure")
    );
    assert_eq!(
        store.get_string_value(&bind_key).await.unwrap().as_deref(),
        Some("totp-rollback")
    );
}

#[tokio::test]
async fn passkey_runtime_backup_restore_and_clear_rebuild_shadow() {
    let source_dir = tempfile::tempdir().expect("create source temp dir");
    let source = Store::connect(source_dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open source store");
    source
        .set_passkey_challenge("backup-challenge", "register", 600)
        .await
        .unwrap();
    source
        .set_passkey_state("backup-challenge", &json!({ "backup": true }), 600)
        .await
        .unwrap();
    let entries = source
        .export_backup_entries_by_prefix_limited("fn_knock:passkey:", 1_000_000, |_| true)
        .await
        .expect("export passkey runtime capabilities");
    assert_eq!(entries.len(), 2);

    let target_dir = tempfile::tempdir().expect("create target temp dir");
    let target = Store::connect(target_dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open target store");
    target
        .replace_backup_entries_by_prefix("fn_knock:", &entries, 200)
        .await
        .expect("restore passkey runtime capabilities");
    assert_eq!(target.typed_passkey_runtime.count().await.unwrap(), 2);
    assert!(
        target
            .consume_passkey_challenge("backup-challenge", "register")
            .await
            .unwrap()
    );
    assert_eq!(
        target
            .consume_passkey_state("backup-challenge")
            .await
            .unwrap(),
        Some(json!({ "backup": true }))
    );
    target.clear_all_keys().await.expect("clear target store");
    assert_eq!(target.typed_passkey_runtime.count().await.unwrap(), 0);
}

#[tokio::test]
async fn identity_runtime_aggregate_tracks_indexes_ttl_and_repairs_from_legacy() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let provider_key = "fn_knock:oidc:providers:data:provider-shadow";
    let provider_index = "fn_knock:oidc:providers:index";
    let binding_key = "fn_knock:oidc:bindings:data:binding-shadow";
    let subject_key = "fn_knock:oidc:bindings:subject:subject-shadow";
    let binding_index = "fn_knock:oidc:bindings:index";
    let state_key = "fn_knock:oidc:state:state-shadow";
    store
        .set_json_value(provider_key, &json!({ "id": "provider-shadow" }))
        .await
        .unwrap();
    store
        .zadd_string_member(provider_index, "provider-shadow", 10)
        .await
        .unwrap();
    store
        .set_json_value(
            binding_key,
            &json!({
                "id": "binding-shadow",
                "provider_id": "provider-shadow",
                "totp_id": "totp-shadow",
                "subject_key": "subject-shadow"
            }),
        )
        .await
        .unwrap();
    store
        .set_string_value(subject_key, "binding-shadow")
        .await
        .unwrap();
    store
        .zadd_string_member(binding_index, "binding-shadow", 20)
        .await
        .unwrap();
    store
        .set_json_value_ex(state_key, &json!({ "flow": "shadow" }), 600)
        .await
        .unwrap();

    let aggregate = store
        .typed_identity_runtime
        .load_protocol("oidc")
        .await
        .unwrap()
        .expect("OIDC aggregate");
    assert_eq!(aggregate.providers.len(), 1);
    assert_eq!(aggregate.provider_index.len(), 1);
    assert_eq!(aggregate.bindings.len(), 1);
    assert_eq!(aggregate.binding_index.len(), 1);
    assert_eq!(aggregate.subjects.len(), 1);
    assert_eq!(aggregate.capabilities.len(), 1);
    assert_eq!(
        aggregate.capabilities[0].expires_at_ms,
        sqlite_key_expiry_at_ms(&path, state_key)
            .await
            .expect("legacy OIDC state expiry")
    );

    let corrupt = json!({ "protocol": "oidc" });
    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE identity_runtime_aggregates SET aggregate_json = ?1 WHERE protocol = 'oidc'",
            [serde_json::to_string(&corrupt).unwrap()],
        )
        .unwrap();
    drop(connection);
    store
        .verify_identity_runtime_shadow("oidc")
        .await
        .expect("repair OIDC aggregate");
    assert_eq!(
        store
            .typed_identity_runtime
            .load_protocol("oidc")
            .await
            .unwrap()
            .unwrap(),
        aggregate
    );
    assert_eq!(
        store.typed_identity_runtime_shadow_status().mismatch_count,
        1
    );

    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
    connection
        .execute("DELETE FROM kv_keys WHERE key = ?1", [state_key])
        .unwrap();
    drop(connection);
    assert!(store.get_json_value(state_key).await.unwrap().is_none());
    store
        .verify_identity_runtime_shadow("oidc")
        .await
        .expect("typed-only capability must be removed");
    assert!(
        store
            .typed_identity_runtime
            .load_protocol("oidc")
            .await
            .unwrap()
            .unwrap()
            .capabilities
            .is_empty()
    );
    store
        .verify_identity_runtime_shadow("oidc")
        .await
        .expect("matching comparison recovers health");
    let status = store.typed_identity_runtime_shadow_status();
    assert!(status.healthy);
    assert_eq!(status.mismatch_count, 2);
}

#[tokio::test]
async fn identity_runtime_typed_failures_roll_back_create_and_consume() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let state_key = "fn_knock:oidc:state:rollback-shadow";
    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "CREATE TRIGGER fail_identity_runtime_update
             BEFORE UPDATE ON identity_runtime_aggregates
             WHEN NEW.protocol = 'oidc'
             BEGIN SELECT RAISE(ABORT, 'injected identity runtime failure'); END;",
        )
        .unwrap();
    drop(connection);
    let error = store
        .set_json_value_ex(state_key, &json!({ "flow": "rollback" }), 600)
        .await
        .expect_err("typed failure must roll back OIDC state creation");
    assert!(
        error
            .to_string()
            .contains("injected identity runtime failure")
    );
    assert!(store.get_json_value(state_key).await.unwrap().is_none());

    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch("DROP TRIGGER fail_identity_runtime_update")
        .unwrap();
    drop(connection);
    store
        .set_json_value_ex(state_key, &json!({ "flow": "rollback" }), 600)
        .await
        .unwrap();
    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "CREATE TRIGGER fail_identity_runtime_delete_sync
             BEFORE UPDATE ON identity_runtime_aggregates
             WHEN NEW.protocol = 'oidc'
             BEGIN SELECT RAISE(ABORT, 'injected identity consume failure'); END;",
        )
        .unwrap();
    drop(connection);
    let error = store
        .consume_json_value(state_key)
        .await
        .expect_err("typed failure must roll back one-time state consumption");
    assert!(
        error
            .to_string()
            .contains("injected identity consume failure")
    );
    assert_eq!(
        store.get_json_value(state_key).await.unwrap(),
        Some(json!({ "flow": "rollback" }))
    );
}

#[tokio::test]
async fn identity_runtime_backup_restore_and_clear_rebuild_shadow() {
    let source_dir = tempfile::tempdir().expect("create source temp dir");
    let source = Store::connect(source_dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open source store");
    source
        .set_json_value_ex(
            "fn_knock:ldap:invite:backup-shadow",
            &json!({ "provider_id": "ldap-provider", "totp_id": "totp" }),
            600,
        )
        .await
        .unwrap();
    let entries = source
        .export_backup_entries_by_prefix_limited("fn_knock:ldap:", 1_000_000, |_| true)
        .await
        .expect("export LDAP identity runtime");
    assert_eq!(entries.len(), 1);

    let target_dir = tempfile::tempdir().expect("create target temp dir");
    let target = Store::connect(target_dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open target store");
    target
        .replace_backup_entries_by_prefix("fn_knock:", &entries, 200)
        .await
        .expect("restore LDAP identity runtime");
    assert_eq!(
        target
            .typed_identity_runtime
            .load_protocol("ldap")
            .await
            .unwrap()
            .unwrap()
            .capabilities
            .len(),
        1
    );
    target.clear_all_keys().await.expect("clear target store");
    let aggregate = target
        .typed_identity_runtime
        .load_protocol("ldap")
        .await
        .unwrap()
        .unwrap();
    assert!(aggregate.providers.is_empty());
    assert!(aggregate.bindings.is_empty());
    assert!(aggregate.subjects.is_empty());
    assert!(aggregate.capabilities.is_empty());
}

#[tokio::test]
async fn identity_runtime_concurrent_capabilities_and_legacy_restart_preserve_shadow() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = std::sync::Arc::new(Store::connect(&path).await.expect("open store"));
    let mut creates = tokio::task::JoinSet::new();
    for index in 0..16 {
        let store = store.clone();
        creates.spawn(async move {
            let key = format!("fn_knock:oidc:state:concurrent-{index}");
            store
                .set_json_value_ex(&key, &json!({ "index": index }), 600)
                .await
                .map(|_| key)
        });
    }
    let mut keys = Vec::new();
    while let Some(result) = creates.join_next().await {
        keys.push(result.unwrap().unwrap());
    }
    assert_eq!(
        store
            .typed_identity_runtime
            .load_protocol("oidc")
            .await
            .unwrap()
            .unwrap()
            .capabilities
            .len(),
        16
    );

    let mut consumes = tokio::task::JoinSet::new();
    for key in keys {
        let store = store.clone();
        consumes.spawn(async move { store.consume_json_value(&key).await });
    }
    let mut consumed = 0;
    while let Some(result) = consumes.join_next().await {
        if result.unwrap().unwrap().is_some() {
            consumed += 1;
        }
    }
    assert_eq!(consumed, 16);
    assert!(
        store
            .typed_identity_runtime
            .load_protocol("oidc")
            .await
            .unwrap()
            .unwrap()
            .capabilities
            .is_empty()
    );

    drop(store);
    let legacy_key = "fn_knock:ldap:invite:legacy-restart";
    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .unwrap();
    connection
        .execute(
            "INSERT INTO kv_keys(key, kind, expires_at_ms) VALUES (?1, 'string', ?2)",
            tokio_rusqlite::rusqlite::params![legacy_key, crate::time_utils::now_ms() + 600_000],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO kv_strings(key, value) VALUES (?1, ?2)",
            tokio_rusqlite::rusqlite::params![
                legacy_key,
                serde_json::to_string(
                    &json!({ "provider_id": "legacy-provider", "totp_id": "legacy-totp" })
                )
                .unwrap()
            ],
        )
        .unwrap();
    drop(connection);

    let reopened = Store::connect(&path)
        .await
        .expect("reopen after legacy write");
    let ldap = reopened
        .typed_identity_runtime
        .load_protocol("ldap")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(ldap.capabilities.len(), 1);
    assert_eq!(ldap.capabilities[0].digest, "legacy-restart");
    assert_eq!(
        reopened.get_json_value(legacy_key).await.unwrap(),
        Some(json!({ "provider_id": "legacy-provider", "totp_id": "legacy-totp" }))
    );
}

#[tokio::test]
async fn oidc_invite_consumption_and_subject_binding_commit_atomically() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let invite_key = "fn_knock:oidc:invite:atomic-claim";
    let subject_key = "fn_knock:oidc:bindings:subject:atomic-subject";
    let binding_key = "fn_knock:oidc:bindings:data:atomic-binding";
    let index_key = "fn_knock:oidc:bindings:index";
    let binding = json!({
        "id": "atomic-binding",
        "provider_id": "provider-a",
        "totp_id": "totp-a",
        "subject_key": "atomic-subject",
        "updated_at": crate::time_utils::now_iso(),
    });
    store
        .set_json_value_ex(
            invite_key,
            &json!({ "provider_id": "provider-a", "totp_id": "totp-a" }),
            600,
        )
        .await
        .unwrap();
    assert!(
        store
            .claim_oidc_binding_and_consume_invite(OidcBindingClaim {
                invite_key,
                subject_key,
                binding_key,
                bindings_index_key: index_key,
                binding_id: "atomic-binding",
                binding: &binding,
                provider_id: "provider-a",
                totp_id: "totp-a",
                score: 42,
            })
            .await
            .expect("claim OIDC binding")
    );
    assert!(store.get_json_value(invite_key).await.unwrap().is_none());
    assert_eq!(
        store
            .get_string_value(subject_key)
            .await
            .unwrap()
            .as_deref(),
        Some("atomic-binding")
    );
    assert_eq!(
        store.get_json_value(binding_key).await.unwrap(),
        Some(binding)
    );
    assert_eq!(
        store.zrevrange_strings(index_key).await.unwrap(),
        vec!["atomic-binding"]
    );

    let rollback_invite_key = "fn_knock:oidc:invite:rollback-claim";
    let rollback_binding_key = "fn_knock:oidc:bindings:data:rollback-binding";
    store
        .set_json_value_ex(
            rollback_invite_key,
            &json!({ "provider_id": "provider-a", "totp_id": "totp-a" }),
            600,
        )
        .await
        .unwrap();
    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "CREATE TRIGGER fail_oidc_binding_insert
             BEFORE INSERT ON kv_strings
             WHEN NEW.key = 'fn_knock:oidc:bindings:data:rollback-binding'
             BEGIN SELECT RAISE(ABORT, 'injected OIDC binding failure'); END;",
        )
        .unwrap();
    drop(connection);
    let rollback_binding = json!({
        "id": "rollback-binding",
        "provider_id": "provider-a",
        "totp_id": "totp-a",
        "subject_key": "rollback-subject",
        "updated_at": crate::time_utils::now_iso(),
    });
    let error = store
        .claim_oidc_binding_and_consume_invite(OidcBindingClaim {
            invite_key: rollback_invite_key,
            subject_key: "fn_knock:oidc:bindings:subject:rollback-subject",
            binding_key: rollback_binding_key,
            bindings_index_key: index_key,
            binding_id: "rollback-binding",
            binding: &rollback_binding,
            provider_id: "provider-a",
            totp_id: "totp-a",
            score: 43,
        })
        .await
        .expect_err("binding failure must preserve the invitation");
    assert!(error.to_string().contains("injected OIDC binding failure"));
    assert!(
        store
            .get_json_value(rollback_invite_key)
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        store
            .get_json_value(rollback_binding_key)
            .await
            .unwrap()
            .is_none()
    );
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
        .typed_mobility
        .load_session(session_id)
        .await
        .expect("load restored aggregate")
        .expect("restored aggregate exists");
    assert!(restored.session.is_some());
    assert_eq!(restored.pending_whitelist.len(), 1);

    target.clear_all_keys().await.expect("clear restored store");
    assert_eq!(target.typed_mobility.counts().await.unwrap(), (0, 0));
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
    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
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

    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
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
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    for session_id in ["typed-mobility-changed", "typed-mobility-unchanged"] {
        let session = new_login_session(session_id, session_id, "192.0.2.94", "test", 3_600);
        store
            .add_session(session_id, &session, 3_600)
            .await
            .expect("seed typed mobility session");
    }
    let changed_before = store
        .typed_mobility
        .aggregate_updated_at_ms("typed-mobility-changed")
        .await
        .unwrap()
        .unwrap();
    let unchanged_before = store
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
        .typed_mobility
        .aggregate_updated_at_ms("typed-mobility-changed")
        .await
        .unwrap()
        .unwrap();
    let unchanged_after = store
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
    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
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
    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
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
        .typed_mobility
        .load_session(first_session)
        .await
        .unwrap()
        .unwrap();
    let second_typed = store
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
    assert_eq!(store.typed_mobility.counts().await.unwrap(), (2, 1));
    assert!(
        store
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
    assert_eq!(store.typed_mobility.counts().await.unwrap(), (2, 0));

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
        .typed_mobility
        .load_session(first_session)
        .await
        .unwrap()
        .unwrap();
    let second_typed = store
        .typed_mobility
        .load_session(second_session)
        .await
        .unwrap()
        .unwrap();
    assert!(first_typed.whitelist_owners.is_empty());
    assert_eq!(second_typed.whitelist_owners.len(), 1);

    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
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
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
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

    assert_eq!(store.typed_mobility.counts().await.unwrap(), (8, 0));
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

async fn sqlite_key_expiry_at_ms(path: &Path, key: &str) -> Option<i64> {
    let connection = tokio_rusqlite::Connection::open(path)
        .await
        .expect("open expiry observer");
    let key = key.to_string();
    connection
        .call(move |connection| {
            connection
                .query_row(
                    "SELECT expires_at_ms FROM kv_keys WHERE key = ?1",
                    [&key],
                    |row| row.get::<_, Option<i64>>(0),
                )
                .optional()
        })
        .await
        .expect("query expiry")
        .flatten()
}

#[test]
fn default_config_top_level_keys_match_node_default_config() {
    let config = default_config();
    let keys = config
        .as_object()
        .expect("default config is object")
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let expected = [
        "run_type",
        "reverse_proxy_submode",
        "auto_manage_firewall",
        "firewall_additional_ports",
        "whitelist_ips",
        "proxy_mappings",
        "host_mappings",
        "host_mapping_groups",
        "host_mapping_grouped_view",
        "stream_mappings",
        "subdomain_mode",
        "ssl",
        "default_route",
        "default_tunnel",
        "fnos_share_bypass",
        "fnos_port_icon_hijack",
        "fnos_connect_waf",
        "fnos_network_tuning",
        "gateway_logging",
        "waf",
        "reverse_proxy_throttle",
        "gateway_visibility",
        "visibility_policies",
        "gateway_proxy_headers",
        "gateway_host_response",
        "gateway_crawler_blocker",
        "gateway_portal",
        "gateway_unmatched_route",
        "appearance",
        "dashboard_display",
        "auto_https",
        "smart_connect",
        "scan_discovery",
        "auth_credential_settings",
        "event_system",
        "terminal_feature",
        "wol_feature",
        "ssh_security",
        "locale",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();

    assert_eq!(keys, expected);
}

#[test]
fn default_config_includes_node_runtime_feature_defaults() {
    let config = default_config();

    assert_eq!(
        config.pointer("/event_system/rules/cpu_alert/enabled"),
        Some(&json!(true))
    );
    assert_eq!(
        config.pointer("/event_system/rules/cpu_alert/threshold_percent"),
        Some(&json!(80))
    );
    assert_eq!(
        config.pointer("/event_system/rules/memory_alert/sample_interval_seconds"),
        Some(&json!(5))
    );
    assert_eq!(
        config.pointer("/event_system/rules/gateway_visibility_block/enabled"),
        Some(&json!(true))
    );
    assert_eq!(
        config.pointer("/terminal_feature/idle_timeout_seconds"),
        Some(&json!(86400))
    );
    assert_eq!(
        config.pointer("/gateway_portal/display_style"),
        Some(&json!("title"))
    );
    assert_eq!(
        config.pointer("/gateway_portal/version"),
        Some(&json!("v1"))
    );
    assert_eq!(
        config.pointer("/gateway_portal/show_wol"),
        Some(&json!(true))
    );
    assert_eq!(config.pointer("/wol_feature/enabled"), Some(&json!(false)));
    assert_eq!(
        config.pointer("/gateway_unmatched_route/behavior"),
        Some(&json!("error_page"))
    );
    assert_eq!(
        config.pointer("/gateway_unmatched_route/upstream_error_detail"),
        Some(&json!("less"))
    );
    assert_eq!(
        config.pointer("/dashboard_display/date_time_display_mode"),
        Some(&json!("human_friendly"))
    );
    assert_eq!(
        config.pointer("/dashboard_display/show_console_app_list"),
        Some(&json!(false))
    );
    assert_eq!(
        config.pointer("/waf/system_rules_auto_update_enabled"),
        Some(&json!(true))
    );
}

#[test]
fn normalizes_totp_access_scopes_like_node() {
    assert_eq!(
        normalize_totp_access_scopes(json!([
            " docker_admin_panel ",
            "other",
            "docker_admin_panel"
        ])),
        json!(["docker_admin_panel"])
    );
    assert_eq!(normalize_totp_access_scopes(json!("nope")), json!([]));
}

#[test]
fn normalizes_totp_credentials_like_node_store() {
    let credentials = normalize_totp_credentials_value(&json!([
        {
            "id": " one ",
            "secret": " SECRET ",
            "comment": "  Comment  ",
            "createdAt": "",
            "access_scopes": [" docker_admin_panel "],
            "subdomain_access": {
                "mode": "custom",
                "hosts": ["Example.com."],
                "streams": [
                    { "protocol": "TCP", "listen_port": 2222 },
                    { "protocol": "tcp", "listen_port": 2222 },
                    { "protocol": "udp", "listen_port": 53 }
                ]
            }
        },
        { "id": "", "secret": "NOPE" }
    ]));
    assert_eq!(credentials.len(), 1);
    assert_eq!(credentials[0].id, "one");
    assert_eq!(credentials[0].secret, "SECRET");
    assert_eq!(credentials[0].comment, "Comment");
    assert!(crate::time_utils::parse_iso_ms(&credentials[0].created_at).is_some());
    assert_eq!(credentials[0].access_scopes, json!(["docker_admin_panel"]));
    assert_eq!(
        credentials[0].subdomain_access,
        json!({
            "mode": "custom",
            "hosts": ["example.com"],
            "streams": [
                { "protocol": "udp", "listen_port": 53 },
                { "protocol": "tcp", "listen_port": 2222 }
            ]
        })
    );
}

#[test]
fn normalizes_totp_subdomain_access_like_node() {
    assert_eq!(
        normalize_totp_subdomain_access(json!({
            "mode": "custom",
            "hosts": [
                "HTTPS://Example.COM:8443/path?q=1",
                "example.com.",
                "/__select__",
                "*.bad.test",
                "bad host"
            ],
            "streams": [
                { "protocol": "TCP", "listen_port": 2222 },
                { "protocol": "tcp", "listen_port": 2222 },
                { "protocol": "udp", "listen_port": 53 },
                { "protocol": "icmp", "listen_port": 7 },
                { "protocol": "tcp", "listen_port": 0 },
                { "protocol": "udp", "listen_port": 65536 }
            ]
        })),
        json!({
            "mode": "custom",
            "hosts": ["__builtin_select__", "example.com"],
            "streams": [
                { "protocol": "udp", "listen_port": 53 },
                { "protocol": "tcp", "listen_port": 2222 }
            ]
        })
    );
    assert_eq!(
        normalize_totp_subdomain_access(json!({ "mode": "all", "hosts": ["example.com"] })),
        json!({ "mode": "all", "hosts": [], "streams": [] })
    );
}

#[test]
fn cname_whitelist_concrete_targets_normalize_dedupe_and_sort_ips() {
    let record = WhitelistRecord {
        id: "whitelist:1".to_string(),
        ip: "example.com".to_string(),
        target_type: "cname".to_string(),
        expire_at: None,
        source: "manual".to_string(),
        created_at: 1,
        status: "active".to_string(),
        comment: None,
        ip_location: None,
        resolved_targets: Some(vec![
            " 192.0.2.1 ".to_string(),
            "not-an-ip".to_string(),
            "2001:DB8::1".to_string(),
            "192.0.2.1".to_string(),
        ]),
        check_interval_minutes: None,
        last_checked_at: None,
        last_resolved_at: None,
        resolve_status: None,
        resolve_message: None,
    };
    let targets = record
        .concrete_targets()
        .into_iter()
        .map(|target| target.target)
        .collect::<Vec<_>>();
    assert_eq!(targets, vec!["192.0.2.1", "2001:DB8::1"]);
}

#[tokio::test]
async fn stale_whitelist_replace_cannot_recreate_a_deleted_record() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    let record = WhitelistRecord {
        id: "whitelist:stale-cname-refresh".to_string(),
        ip: "example.com".to_string(),
        target_type: "cname".to_string(),
        expire_at: None,
        source: "manual".to_string(),
        created_at: 1,
        status: "active".to_string(),
        comment: None,
        ip_location: None,
        resolved_targets: Some(vec!["192.0.2.1".to_string()]),
        check_interval_minutes: Some(5),
        last_checked_at: None,
        last_resolved_at: None,
        resolve_status: Some("resolved".to_string()),
        resolve_message: None,
    };
    store
        .insert_whitelist_record(&record)
        .await
        .expect("insert whitelist record");

    let mut stale_refresh = record.clone();
    stale_refresh.resolved_targets = Some(vec!["192.0.2.2".to_string()]);
    stale_refresh.last_checked_at = Some(2);
    store
        .delete_whitelist_record(&record.id)
        .await
        .expect("delete whitelist record")
        .expect("deleted record");

    let error = store
        .replace_whitelist_record(&record, &stale_refresh)
        .await
        .expect_err("stale refresh must fail");
    assert!(error.to_string().contains("changed concurrently"));
    assert!(
        store
            .get_whitelist_record(&record.id)
            .await
            .expect("read deleted record")
            .is_none()
    );
    assert!(
        store
            .list_whitelist_records()
            .await
            .expect("list whitelist records")
            .is_empty()
    );
    assert!(
        store
            .typed_whitelist
            .load_one("record", &record.id)
            .await
            .expect("read typed whitelist record")
            .is_none()
    );
}

#[tokio::test]
async fn whitelist_record_writes_keep_typed_shadow_in_the_same_transaction() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    let record = WhitelistRecord {
        id: "whitelist:typed-shadow".to_string(),
        ip: "192.0.2.10".to_string(),
        target_type: "ip".to_string(),
        expire_at: Some(9_999_999_999),
        source: "manual".to_string(),
        created_at: 123,
        status: "active".to_string(),
        comment: None,
        ip_location: None,
        resolved_targets: None,
        check_interval_minutes: None,
        last_checked_at: None,
        last_resolved_at: None,
        resolve_status: None,
        resolve_message: None,
    };

    store
        .insert_whitelist_record(&record)
        .await
        .expect("insert dual-written whitelist record");
    assert_eq!(store.typed_whitelist.count().await.unwrap(), 1);
    let typed = store
        .typed_whitelist
        .load_one("record", &record.id)
        .await
        .unwrap()
        .expect("typed whitelist record");
    assert_eq!(typed.document_json, serde_json::to_string(&record).unwrap());
    assert_eq!(typed.sort_score, record.created_at);
    assert_eq!(typed.expires_at, record.expire_at);
    assert_eq!(typed.status, "active");

    let updated = store
        .update_whitelist_comment(&record.id, "updated".to_string())
        .await
        .expect("update comment")
        .expect("updated record");
    let typed = store
        .typed_whitelist
        .load_one("record", &record.id)
        .await
        .unwrap()
        .expect("updated typed record");
    assert_eq!(
        typed.document_json,
        serde_json::to_string(&updated).unwrap()
    );

    store
        .delete_whitelist_record(&record.id)
        .await
        .expect("delete record")
        .expect("deleted record");
    assert!(
        store
            .typed_whitelist
            .load_one("record", &record.id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .get_whitelist_record(&record.id)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn typed_whitelist_record_mismatch_falls_back_and_repairs_primary() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    let record = WhitelistRecord {
        id: "whitelist:typed-primary-repair".to_string(),
        ip: "192.0.2.20".to_string(),
        target_type: "ip".to_string(),
        expire_at: None,
        source: "manual".to_string(),
        created_at: 321,
        status: "active".to_string(),
        comment: Some("legacy authoritative".to_string()),
        ip_location: None,
        resolved_targets: None,
        check_interval_minutes: None,
        last_checked_at: None,
        last_resolved_at: None,
        resolve_status: None,
        resolve_message: None,
    };
    store
        .insert_whitelist_record(&record)
        .await
        .expect("insert whitelist record");

    let mut divergent = record.clone();
    divergent.comment = Some("typed divergence".to_string());
    let divergent_json = serde_json::to_string(&divergent).unwrap();
    let record_id = record.id.clone();
    store
        .manager
        .call(move |conn| {
            conn.execute(
                "UPDATE whitelist_documents SET document_json = ?1 WHERE kind = 'record' AND id = ?2",
                tokio_rusqlite::rusqlite::params![divergent_json, record_id],
            )?;
            Ok(())
        })
        .await
        .expect("inject typed divergence");

    let listed = store
        .list_whitelist_records()
        .await
        .expect("fallback to legacy whitelist list");
    assert_eq!(listed, vec![record.clone()]);
    assert_eq!(store.typed_whitelist_shadow_mismatch_count(), 1);
    let repaired = store
        .typed_whitelist
        .load_one("record", &record.id)
        .await
        .unwrap()
        .expect("repaired typed record");
    assert_eq!(
        repaired.document_json,
        serde_json::to_string(&record).unwrap()
    );

    let listed_again = store
        .list_whitelist_records()
        .await
        .expect("read repaired typed primary");
    assert_eq!(listed_again, vec![record]);
    assert_eq!(store.typed_whitelist_shadow_mismatch_count(), 1);
}

#[tokio::test]
async fn corrupt_typed_whitelist_record_falls_back_without_expanding_authorization() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    let record = WhitelistRecord {
        id: "whitelist:typed-primary-corrupt".to_string(),
        ip: "198.51.100.20".to_string(),
        target_type: "ip".to_string(),
        expire_at: None,
        source: "manual".to_string(),
        created_at: 654,
        status: "active".to_string(),
        comment: None,
        ip_location: None,
        resolved_targets: None,
        check_interval_minutes: None,
        last_checked_at: None,
        last_resolved_at: None,
        resolve_status: None,
        resolve_message: None,
    };
    store
        .insert_whitelist_record(&record)
        .await
        .expect("insert whitelist record");
    let record_id = record.id.clone();
    store
        .manager
        .call(move |conn| {
            conn.execute(
                "UPDATE whitelist_documents SET document_json = '{\"unexpected\":true}' WHERE kind = 'record' AND id = ?1",
                [record_id],
            )?;
            Ok(())
        })
        .await
        .expect("corrupt typed document");

    assert_eq!(
        store
            .get_whitelist_record(&record.id)
            .await
            .expect("fallback to legacy record"),
        Some(record.clone())
    );
    assert_eq!(store.typed_whitelist_shadow_mismatch_count(), 1);
    assert_eq!(
        store
            .get_whitelist_record(&record.id)
            .await
            .expect("read repaired record"),
        Some(record)
    );
    assert_eq!(store.typed_whitelist_shadow_mismatch_count(), 1);
}

#[tokio::test]
async fn typed_only_whitelist_record_is_never_used_for_authorization() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    let typed_only = WhitelistRecord {
        id: "whitelist:typed-only".to_string(),
        ip: "203.0.113.99".to_string(),
        target_type: "ip".to_string(),
        expire_at: None,
        source: "manual".to_string(),
        created_at: 777,
        status: "active".to_string(),
        comment: None,
        ip_location: None,
        resolved_targets: None,
        check_interval_minutes: None,
        last_checked_at: None,
        last_resolved_at: None,
        resolve_status: None,
        resolve_message: None,
    };
    let typed_json = serde_json::to_string(&typed_only).unwrap();
    let typed_id = typed_only.id.clone();
    store
        .manager
        .call(move |conn| {
            conn.execute(
                "INSERT INTO whitelist_documents(kind, id, document_json, sort_score, expires_at, status, updated_at_ms)
                 VALUES ('record', ?1, ?2, 777, NULL, 'active', 777)",
                tokio_rusqlite::rusqlite::params![typed_id, typed_json],
            )?;
            Ok(())
        })
        .await
        .expect("inject typed-only authorization record");

    assert!(
        store
            .list_whitelist_records()
            .await
            .expect("compare typed-only record against legacy keyspace")
            .is_empty()
    );
    assert_eq!(store.typed_whitelist_shadow_mismatch_count(), 1);
    assert!(
        store
            .typed_whitelist
            .load_one("record", &typed_only.id)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn typed_whitelist_primary_keeps_pending_records_out_of_authorization_lists() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    let pending = WhitelistRecord {
        id: "whitelist:typed-pending".to_string(),
        ip: "203.0.113.20".to_string(),
        target_type: "ip".to_string(),
        expire_at: Some(9_999_999_999),
        source: "auto".to_string(),
        created_at: 987,
        status: "pending".to_string(),
        comment: None,
        ip_location: None,
        resolved_targets: None,
        check_interval_minutes: None,
        last_checked_at: None,
        last_resolved_at: None,
        resolve_status: None,
        resolve_message: None,
    };
    store
        .insert_whitelist_record(&pending)
        .await
        .expect("insert pending whitelist record");

    assert!(
        store
            .list_whitelist_records()
            .await
            .expect("list authorization records")
            .is_empty()
    );
    assert_eq!(
        store
            .get_whitelist_record(&pending.id)
            .await
            .expect("load pending record for promotion"),
        Some(pending)
    );
    assert_eq!(store.typed_whitelist_shadow_mismatch_count(), 0);
}

#[tokio::test]
async fn typed_whitelist_region_mismatch_falls_back_and_repairs_primary() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    let region = WhitelistRegionGroupRecord {
        id: "whitelist-region:typed-primary-repair".to_string(),
        regions: vec![WhitelistRegionInput {
            province: "广东".to_string(),
            query_city: None,
            operator: None,
        }],
        cidrs: vec!["192.0.2.0/24".to_string()],
        policy_id: String::new(),
        policy: None,
        source_cidr_count: 1,
        range_count: 1,
        expire_at: None,
        source: "manual".to_string(),
        created_at: 741,
        updated_at: 741,
        status: "active".to_string(),
        comment: Some("legacy authoritative".to_string()),
    };
    store
        .insert_whitelist_region_group(&region)
        .await
        .expect("insert whitelist region");

    let mut divergent = region.clone();
    divergent.comment = Some("typed divergence".to_string());
    let divergent_json = serde_json::to_string(&divergent).unwrap();
    let region_id = region.id.clone();
    store
        .manager
        .call(move |conn| {
            conn.execute(
                "UPDATE whitelist_documents SET document_json = ?1 WHERE kind = 'region' AND id = ?2",
                tokio_rusqlite::rusqlite::params![divergent_json, region_id],
            )?;
            Ok(())
        })
        .await
        .expect("inject typed region divergence");

    assert_eq!(
        store
            .list_whitelist_region_groups()
            .await
            .expect("fallback to legacy region list"),
        vec![region.clone()]
    );
    assert_eq!(store.typed_whitelist_shadow_mismatch_count(), 1);
    let repaired = store
        .typed_whitelist
        .load_one("region", &region.id)
        .await
        .unwrap()
        .expect("repaired typed region");
    assert_eq!(
        repaired.document_json,
        serde_json::to_string(&region).unwrap()
    );
}

#[tokio::test]
async fn typed_whitelist_failure_rolls_back_compatibility_indexes() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    store
        .manager
        .call(|conn| {
            conn.execute("DROP TABLE whitelist_documents", [])?;
            Ok(())
        })
        .await
        .expect("remove typed table");
    let record = WhitelistRecord {
        id: "whitelist:typed-rollback".to_string(),
        ip: "192.0.2.11".to_string(),
        target_type: "ip".to_string(),
        expire_at: None,
        source: "manual".to_string(),
        created_at: 456,
        status: "active".to_string(),
        comment: None,
        ip_location: None,
        resolved_targets: None,
        check_interval_minutes: None,
        last_checked_at: None,
        last_resolved_at: None,
        resolve_status: None,
        resolve_message: None,
    };

    store
        .insert_whitelist_record(&record)
        .await
        .expect_err("typed write failure must abort compatibility write");
    let mut conn = store.conn();
    let stored: Option<String> = conn.hget(WHITELIST_RECORDS, &record.id).await.unwrap();
    assert!(stored.is_none());
    let ordered: Vec<String> = conn.zrevrange(WHITELIST_RECORD_ORDER, 0, -1).await.unwrap();
    assert!(!ordered.contains(&record.id));
    let indexed: Vec<String> = conn
        .smembers(whitelist_ip_records_key(&record.ip))
        .await
        .unwrap();
    assert!(!indexed.contains(&record.id));
}

#[tokio::test]
async fn typed_whitelist_rebuilds_after_legacy_backup_restore_and_reopen() {
    let source_dir = tempfile::tempdir().expect("create source temp dir");
    let source_path = source_dir.path().join("fn-knock.sqlite3");
    let source = Store::connect(&source_path)
        .await
        .expect("open source store");
    let record = WhitelistRecord {
        id: "whitelist:restored-shadow".to_string(),
        ip: "192.0.2.12".to_string(),
        target_type: "ip".to_string(),
        expire_at: None,
        source: "manual".to_string(),
        created_at: 789,
        status: "active".to_string(),
        comment: Some("restored".to_string()),
        ip_location: None,
        resolved_targets: None,
        check_interval_minutes: None,
        last_checked_at: None,
        last_resolved_at: None,
        resolve_status: None,
        resolve_message: None,
    };
    source
        .insert_whitelist_record(&record)
        .await
        .expect("seed source whitelist");
    let entries = source
        .export_backup_entries_by_prefix_limited("fn_knock:whitelist:", 1_000_000, |_| true)
        .await
        .expect("export whitelist entries");
    assert!(!entries.is_empty());

    let target_dir = tempfile::tempdir().expect("create target temp dir");
    let target_path = target_dir.path().join("fn-knock.sqlite3");
    let target = Store::connect(&target_path)
        .await
        .expect("open target store");
    target
        .replace_backup_entries_by_prefix("fn_knock:", &entries, 200)
        .await
        .expect("restore whitelist entries");
    let typed = target
        .typed_whitelist
        .load_one("record", &record.id)
        .await
        .unwrap()
        .expect("restored typed whitelist");
    assert_eq!(typed.document_json, serde_json::to_string(&record).unwrap());
    drop(target);

    // Reopening represents a newer binary reconciling writes made by a 2.x
    // binary that only knew the compatibility keyspace.
    let mut legacy = record.clone();
    legacy.comment = Some("legacy rewrite".to_string());
    let mut connection = redis::ConnectionManager::open(&target_path)
        .await
        .expect("open compatibility connection");
    let _: () = redis::cmd("HSET")
        .arg(WHITELIST_RECORDS)
        .arg(&record.id)
        .arg(serde_json::to_string(&legacy).unwrap())
        .query_async(&mut connection)
        .await
        .expect("simulate legacy whitelist rewrite");
    drop(connection);

    let reopened = Store::connect(&target_path)
        .await
        .expect("reopen upgraded store");
    let typed = reopened
        .typed_whitelist
        .load_one("record", &record.id)
        .await
        .unwrap()
        .expect("reconciled typed whitelist");
    assert_eq!(typed.document_json, serde_json::to_string(&legacy).unwrap());
}

#[test]
fn stale_whitelist_cleanup_targets_match_node_indexes() {
    let mut record = WhitelistRecord {
        id: "whitelist:1".to_string(),
        ip: "example.com".to_string(),
        target_type: "cname".to_string(),
        expire_at: None,
        source: "manual".to_string(),
        created_at: 1,
        status: "expired".to_string(),
        comment: None,
        ip_location: None,
        resolved_targets: Some(vec![
            "192.0.2.1".to_string(),
            "bad".to_string(),
            "192.0.2.1".to_string(),
            "2001:DB8::1".to_string(),
        ]),
        check_interval_minutes: None,
        last_checked_at: None,
        last_resolved_at: None,
        resolve_status: None,
        resolve_message: None,
    };
    assert_eq!(
        whitelist_stale_ip_index_targets(&record),
        vec!["192.0.2.1".to_string(), "2001:DB8::1".to_string()]
    );

    record.target_type = "cidr".to_string();
    record.ip = "192.0.2.0/24".to_string();
    record.resolved_targets = None;
    assert!(whitelist_stale_ip_index_targets(&record).is_empty());
}

#[test]
fn deserializes_whitelist_records_like_node_store() {
    let record = deserialize_whitelist_record(
        r#"{
                "id": " whitelist:legacy ",
                "ip": "Example.COM.",
                "expireAt": "123abc",
                "createdAt": "456.9",
                "resolvedTargets": [" 192.0.2.1 ", "bad", "2001:DB8::1", "192.0.2.1"],
                "checkIntervalMinutes": "10m",
                "lastCheckedAt": "",
                "resolveStatus": "nope",
                "resolveMessage": " resolved "
            }"#,
    )
    .unwrap();
    assert_eq!(record.id, "whitelist:legacy");
    assert_eq!(record.ip, "example.com");
    assert_eq!(record.target_type, "cname");
    assert_eq!(record.expire_at, Some(123));
    assert_eq!(record.created_at, 456);
    assert_eq!(record.source, "manual");
    assert_eq!(record.status, "active");
    assert_eq!(
        record.resolved_targets,
        Some(vec!["192.0.2.1".to_string(), "2001:DB8::1".to_string()])
    );
    assert_eq!(record.check_interval_minutes, Some(10));
    assert_eq!(record.last_checked_at, None);
    assert_eq!(record.resolve_status.as_deref(), Some("pending"));
    assert_eq!(record.resolve_message.as_deref(), Some("resolved"));
}

#[test]
fn deserializes_whitelist_region_groups_like_node_store() {
    let group = deserialize_whitelist_region_group(
        r#"{
                "id": " whitelist-region:legacy ",
                "regions": [
                    { "province": 440000, "query_city": true },
                    { "province": "广东", "query_city": "" },
                    { "province": "广东", "query_city": "深圳", "operator": "移动" },
                    { "province": "广东", "query_city": "深圳", "operator": "电信" },
                    { "province": " ", "query_city": "ignored" },
                    null
                ],
                "cidrs": [" 192.0.2.0/24 ", 123, null],
                "expireAt": "0x10",
                "createdAt": true,
                "updatedAt": "456.9",
                "status": "nope",
                "source": "auto",
                "comment": null
            }"#,
    )
    .unwrap();
    assert_eq!(group.id, "whitelist-region:legacy");
    assert_eq!(
        group.regions,
        vec![
            WhitelistRegionInput {
                province: "440000".to_string(),
                query_city: Some("true".to_string()),
                operator: None,
            },
            WhitelistRegionInput {
                province: "广东".to_string(),
                query_city: None,
                operator: None,
            },
            WhitelistRegionInput {
                province: "广东".to_string(),
                query_city: Some("深圳".to_string()),
                operator: Some(crate::cidr::CidrOperator::Mobile),
            },
            WhitelistRegionInput {
                province: "广东".to_string(),
                query_city: Some("深圳".to_string()),
                operator: Some(crate::cidr::CidrOperator::Telecom),
            }
        ]
    );
    assert_eq!(
        group.cidrs,
        vec!["192.0.2.0/24".to_string(), "123".to_string()]
    );
    assert_eq!(group.expire_at, Some(16));
    assert_eq!(group.created_at, 1);
    assert_eq!(group.updated_at, 456);
    assert_eq!(group.status, "active");
    assert_eq!(group.source, "manual");
    assert_eq!(group.comment.as_deref(), Some(""));
}

#[tokio::test]
async fn whitelist_region_migration_compiles_active_records_and_compacts_tombstones() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    let mut conn = store.conn();
    let active_id = "whitelist-region:active";
    let deleted_id = "whitelist-region:deleted";
    let active = json!({
        "id": active_id,
        "regions": [{ "province": "广东", "query_city": null }],
        "cidrs": ["192.0.2.0/25", "192.0.2.128/25", "2001:db8::/32"],
        "expireAt": null,
        "source": "manual",
        "createdAt": 20,
        "updatedAt": 20,
        "status": "active"
    });
    let deleted = json!({
        "id": deleted_id,
        "regions": [{ "province": "浙江", "query_city": null }],
        "cidrs": ["198.51.100.0/24", "2001:db8:1::/48"],
        "expireAt": null,
        "source": "manual",
        "createdAt": 10,
        "updatedAt": 10,
        "status": "deleted"
    });
    let _: () = redis::cmd("HSET")
        .arg(WHITELIST_REGION_GROUP_RECORDS)
        .arg(active_id)
        .arg(active.to_string())
        .arg(deleted_id)
        .arg(deleted.to_string())
        .query_async(&mut conn)
        .await
        .expect("seed region groups");
    drop(conn);

    let migrated = store
        .migrate_whitelist_region_groups_to_ipsets()
        .await
        .expect("migrate region groups");
    assert_eq!(migrated.len(), 1);
    assert_eq!(migrated[0].id, active_id);
    assert!(migrated[0].cidrs.is_empty());
    assert!(migrated[0].policy_id.starts_with("ipset-v2:"));
    assert_eq!(migrated[0].range_count, 2);
    assert!(
        migrated[0]
            .policy()
            .is_some_and(|policy| policy.contains("192.0.2.255".parse().unwrap()))
    );

    let deleted_raw = store
        .hgetall_string_map(WHITELIST_REGION_GROUP_RECORDS)
        .await
        .expect("read region groups")
        .remove(deleted_id)
        .expect("read compact tombstone");
    let deleted = deserialize_whitelist_region_group(&deleted_raw).unwrap();
    assert!(deleted.cidrs.is_empty());
    assert!(deleted.policy.is_none());
    assert!(deleted.policy_id.is_empty());
    assert_eq!(deleted.source_cidr_count, 0);
    assert_eq!(deleted.range_count, 0);

    let typed_active = store
        .typed_whitelist
        .load_one("region", active_id)
        .await
        .expect("read typed active region")
        .expect("typed active region");
    let typed_deleted = store
        .typed_whitelist
        .load_one("region", deleted_id)
        .await
        .expect("read typed deleted region")
        .expect("typed deleted region");
    assert_eq!(typed_active.status, "active");
    assert_eq!(typed_deleted.status, "deleted");
    assert_eq!(typed_deleted.document_json, deleted_raw);
    assert_eq!(store.typed_whitelist.count().await.unwrap(), 2);

    assert_eq!(
        store
            .list_whitelist_region_groups()
            .await
            .expect("list active groups")
            .len(),
        1
    );
}

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
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
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
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
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
    assert_eq!(store.typed_events.count().await.unwrap(), 1);

    drop(store);
    let reopened = Store::connect(&path).await.expect("reopen store");
    assert_eq!(reopened.typed_events.count().await.unwrap(), 1);

    reopened
        .delete_system_events(&["typed-event".to_string()])
        .await
        .expect("delete event from both stores");
    assert_eq!(reopened.typed_events.count().await.unwrap(), 0);
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
    assert_eq!(reopened.typed_events.count().await.unwrap(), 0);
}

#[tokio::test]
async fn typed_system_event_failure_rolls_back_legacy_write() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let store = Store::connect(&path).await.expect("open store");
    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
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
    assert_eq!(store.typed_events.count().await.unwrap(), 0);
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
    assert_eq!(store.typed_event_dedupe.count().await.unwrap(), 0);
}

#[tokio::test]
async fn concurrent_system_event_dedupe_claim_and_event_write_are_one_transaction() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
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
    assert_eq!(store.typed_events.count().await.unwrap(), 1);
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
    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
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
    assert_eq!(store.typed_event_dedupe.count().await.unwrap(), 0);
    assert_eq!(store.typed_events.count().await.unwrap(), 0);
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
    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
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
        .typed_event_dedupe
        .load("shadow-repair")
        .await
        .unwrap()
        .unwrap();
    assert!(typed.expires_at_ms > crate::time_utils::now_ms());
    let status = store.typed_event_dedupe_shadow_status();
    assert!(!status.healthy);
    assert_eq!(status.mismatch_count, 1);
    assert_eq!(store.typed_events.count().await.unwrap(), 1);
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
            .typed_event_dedupe
            .load("backup-dedupe")
            .await
            .unwrap()
            .is_some()
    );

    let connection = tokio_rusqlite::rusqlite::Connection::open(&source_path).unwrap();
    connection
        .execute(
            "UPDATE kv_keys SET expires_at_ms = 0 WHERE key = ?1",
            [format!("{EVENTS_DEDUPE_PREFIX}backup-dedupe")],
        )
        .unwrap();
    drop(connection);
    assert!(
        !source
            .typed_event_dedupe
            .verify_and_repair("backup-dedupe")
            .await
            .unwrap()
    );
    assert!(
        source
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
    assert_eq!(target.typed_event_dedupe.count().await.unwrap(), 0);
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
    assert_eq!(target.typed_events.count().await.unwrap(), 1);
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
    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
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
async fn concurrent_system_event_writes_preserve_typed_and_legacy_history() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
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
    assert_eq!(store.typed_events.count().await.unwrap(), WRITERS as i64);
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
            .typed_notifications
            .count_kind("provider")
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        store.typed_notifications.count_kind("rule").await.unwrap(),
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
            .typed_notifications
            .count_kind("provider")
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        reopened
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
            .typed_notifications
            .count_kind("provider")
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        reopened
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
    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
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
    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
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
            .typed_notifications
            .count_history("trigger")
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        store
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
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
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
            .typed_notifications
            .count_history("delivery")
            .await
            .unwrap(),
        WRITERS as i64
    );
}

#[tokio::test]
async fn typed_notification_history_nx_preserves_existing_record_and_repairs_index() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
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
    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
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
    let connection = tokio_rusqlite::rusqlite::Connection::open(&path).unwrap();
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
            .typed_notifications
            .load_history_one("delivery", "delivery-shadow")
            .await
            .expect("typed delivery repaired"),
        Some(delivery)
    );
}

#[tokio::test]
async fn deleting_typed_delivery_history_also_removes_ready_queue_member() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
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

#[tokio::test]
async fn sorted_set_record_cap_removes_oldest_members() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    for (member, score) in [("old", 1), ("middle", 2), ("new", 3)] {
        store
            .zadd_string_member("fn_knock:test:bounded-history", member, score)
            .await
            .unwrap();
    }
    let removed = store
        .trim_oldest_zset_members("fn_knock:test:bounded-history", 2)
        .await
        .unwrap();
    assert_eq!(removed, vec!["old".to_string()]);
    assert_eq!(
        store
            .zrevrange_strings("fn_knock:test:bounded-history")
            .await
            .unwrap(),
        vec!["new".to_string(), "middle".to_string()]
    );
}

#[tokio::test]
async fn expired_key_gc_physically_removes_unread_keys() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    let key = "fn_knock:test:expired-gc";
    store
        .set_string_value_with_optional_ttl(key, "stale", Some(60))
        .await
        .unwrap();
    let connection = tokio_rusqlite::rusqlite::Connection::open(&store.path).unwrap();
    connection
        .execute("UPDATE kv_keys SET expires_at_ms = 0 WHERE key = ?1", [key])
        .unwrap();
    drop(connection);

    assert_eq!(store.purge_expired_keys().await.unwrap(), 1);
    assert_eq!(store.manager.key_count_by_prefix(key).await.unwrap(), 0);
}

#[test]
fn parses_traffic_members_and_ignores_invalid_values() {
    assert_eq!(
        parse_traffic_points(&[
            "10:5".to_string(),
            "bad".to_string(),
            "11:nope".to_string(),
            "12:0".to_string()
        ]),
        vec![
            TrafficDeltaPoint { ts: 10, delta: 5.0 },
            TrafficDeltaPoint { ts: 12, delta: 0.0 }
        ]
    );
}

#[tokio::test]
async fn traffic_history_reads_do_not_wait_for_primary_storage_executor() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    let manager = store.manager.clone();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let blocker = tokio::spawn(async move {
        manager
            .call(move |_conn| -> crate::storage::StorageResult<()> {
                let _ = started_tx.send(());
                release_rx
                    .recv_timeout(std::time::Duration::from_secs(5))
                    .map_err(|error| {
                        crate::storage::storage_error(format!("release blocker: {error}"))
                    })?;
                Ok(())
            })
            .await
            .expect("primary executor blocker");
    });
    started_rx.await.expect("primary executor started");

    let points = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        store.list_traffic_points("global", "in", 0, 10, None, None),
    )
    .await;
    release_tx.send(()).expect("release primary executor");
    let points = points
        .expect("analytics read should use its isolated executor")
        .expect("traffic history read");
    assert!(points.is_empty());
    blocker.await.expect("primary executor task");
}

#[test]
fn traffic_cleanup_maps_metric_keys_to_last_total_keys() {
    assert_eq!(
        super::traffic::traffic_last_total_key_for_metric_key(
            "fn_knock:traffic:global:host:example.com:in"
        )
        .as_deref(),
        Some("fn_knock:traffic:last:global:host:example.com:in")
    );
    assert_eq!(
        super::traffic::traffic_last_total_key_for_metric_key("fn_knock:traffic:global:out")
            .as_deref(),
        Some("fn_knock:traffic:last:global:out")
    );
    assert_eq!(
        super::traffic::traffic_last_total_key_for_metric_key("fn_knock:errors:global:5xx")
            .as_deref(),
        Some("fn_knock:errors:last:global:5xx")
    );
    assert_eq!(
        super::traffic::traffic_last_total_key_for_metric_key("fn_knock:traffic:global:bad"),
        None
    );
}

#[test]
fn counter_delta_handles_first_sample_and_resets() {
    assert_eq!(compute_counter_delta(100.0, None), 100.0);
    assert_eq!(compute_counter_delta(120.0, Some(100.0)), 20.0);
    assert_eq!(compute_counter_delta(12.0, Some(100.0)), 12.0);
    assert_eq!(compute_counter_delta(-1.0, Some(100.0)), 0.0);
}

#[test]
fn waf_log_dates_include_neighboring_utc_days() {
    let dates = waf_log_dates_for_range(1_704_067_200_000, 1_704_153_600_000);
    assert!(dates.contains(&"2023-12-31".to_string()));
    assert!(dates.contains(&"2024-01-01".to_string()));
    assert!(dates.contains(&"2024-01-02".to_string()));
    assert!(dates.contains(&"2024-01-03".to_string()));
}

#[tokio::test]
async fn waf_event_persistence_is_atomic_and_idempotent_for_lease_retries() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    let event = json!({
        "trace_id": "waf_retry",
        "time": "2026-08-15T15:28:20Z",
        "action": "deny",
        "status": 403,
        "rule_ids": [921150]
    });
    let events = vec![event.clone()];

    let (first, duplicate) = tokio::join!(
        store.persist_waf_events(&events, 7),
        store.persist_waf_events(&events, 7)
    );
    first.expect("persist leased event");
    duplicate.expect("persist duplicate lease delivery");

    assert_eq!(
        store.get_waf_log_event("waf_retry").await.unwrap(),
        Some(event)
    );
    let score = crate::time_utils::parse_iso_ms("2026-08-15T15:28:20Z").unwrap();
    let date = crate::time_utils::local_date_from_ms(score);
    assert_eq!(store.waf_log_date_total(&date).await.unwrap(), 1);
    let stats = store
        .conn()
        .hgetall(&waf_log_stats_key(&date))
        .await
        .unwrap();
    assert_eq!(stats.get("events").map(String::as_str), Some("1"));
    assert_eq!(stats.get("action:deny").map(String::as_str), Some("1"));
}
