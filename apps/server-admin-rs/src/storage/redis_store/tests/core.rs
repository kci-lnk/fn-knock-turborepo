use super::*;
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
    let (_dir, store) = open_test_store().await;
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
        .typed
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
        .typed
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
        .typed
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
        .typed
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
    let (_dir, store) = open_test_store().await;
    let stale = store.get_config().await.expect("load stale candidate");
    let stale_revision = store
        .typed
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

    let updates = store.subscribe_config_snapshot();
    store.publish_config_snapshot(stale, stale_revision);
    assert_eq!(store.config_snapshot().as_ref(), &fresh);
    assert!(!updates.has_changed().unwrap());
}

#[tokio::test]
async fn equal_typed_revision_cannot_replace_an_existing_snapshot() {
    let (_dir, store) = open_test_store().await;
    let expected = store.get_config().await.unwrap();
    let revision = store
        .typed
        .typed_config
        .load()
        .await
        .unwrap()
        .unwrap()
        .revision;
    let mut stale = expected.clone();
    stale["locale"] = json!({ "default_locale": "stale" });
    let updates = store.subscribe_config_snapshot();
    store.publish_config_snapshot(stale, revision);
    assert_eq!(store.config_snapshot().as_ref(), &expected);
    assert!(!updates.has_changed().unwrap());
}

#[tokio::test]
async fn config_repair_detects_revision_reuse_by_an_older_store() {
    let (_dir, older_store) = open_test_store().await;
    let newer_store = Store::connect(&older_store.path).await.unwrap();
    let mut expected = newer_store
        .set_config_top_level_value("locale", json!({ "default_locale": "ja-JP" }))
        .await
        .unwrap();
    let previous_revision = newer_store
        .typed
        .typed_config
        .load()
        .await
        .unwrap()
        .unwrap()
        .revision;
    strip_internal_config_metadata(&mut expected);
    expected["locale"] = json!({ "default_locale": "en" });
    let replacement_json = expected.to_string();
    newer_store
        .manager
        .call(move |conn| {
            conn.execute(
                "UPDATE kv_strings SET value = ?2 WHERE key = ?1",
                [CONFIG_KEY, replacement_json.as_str()],
            )?;
            conn.execute("DELETE FROM config_documents WHERE singleton = 1", [])?;
            Ok(())
        })
        .await
        .unwrap();

    older_store.get_config().await.unwrap();
    assert_eq!(
        older_store
            .typed
            .typed_config
            .load()
            .await
            .unwrap()
            .unwrap()
            .revision,
        previous_revision,
        "the older Store recreates the row at an already-published revision"
    );
    assert_eq!(
        newer_store.locale().await.unwrap()["default_locale"],
        "ja-JP"
    );
    newer_store.get_config().await.unwrap();
    assert_eq!(newer_store.locale().await.unwrap(), expected["locale"]);
    assert!(
        newer_store
            .typed
            .typed_config
            .load()
            .await
            .unwrap()
            .unwrap()
            .revision
            > previous_revision
    );
}

#[tokio::test]
async fn typed_config_mismatch_falls_back_to_legacy_and_repairs_the_typed_primary() {
    let (_dir, store) = open_test_store().await;
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
        .typed
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
    let (_dir, store) = open_test_store().await;
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
        .typed
        .typed_config
        .load()
        .await
        .expect("load repaired typed config")
        .expect("typed config is repaired from legacy fallback");
    assert_eq!(repaired.document, expected);
}

#[tokio::test]
async fn missing_typed_document_after_bootstrap_is_observable_and_repaired() {
    let (_dir, store) = open_test_store().await;
    store
        .set_config_top_level_value("locale", json!({ "default_locale": "en" }))
        .await
        .expect("seed presentation snapshot");
    let mut expected = store
        .set_config_top_level_value("typed_document_recovery", json!(true))
        .await
        .expect("seed config");
    strip_internal_config_metadata(&mut expected);
    let previous_revision = store
        .typed
        .typed_config
        .load()
        .await
        .unwrap()
        .unwrap()
        .revision;
    expected["locale"] = json!({ "default_locale": "ja-JP" });
    let replacement_json = expected.to_string();

    store
        .manager
        .call(move |conn| {
            conn.execute(
                "UPDATE kv_strings SET value = ?2 WHERE key = ?1",
                [CONFIG_KEY, replacement_json.as_str()],
            )?;
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
        .typed
        .typed_config
        .load()
        .await
        .expect("load repaired typed document")
        .expect("typed document is restored");
    assert_eq!(repaired.document, expected);
    assert!(repaired.revision > previous_revision);
    assert_eq!(store.locale().await.unwrap(), expected["locale"]);
    store
        .set_config_top_level_value("locale", json!({ "default_locale": "ko-KR" }))
        .await
        .expect("update after typed repair");
    assert_eq!(store.locale().await.unwrap()["default_locale"], "ko-KR");
}

#[tokio::test]
async fn config_repair_by_an_older_store_preserves_newer_snapshot_progress() {
    let (_dir, older_store) = open_test_store().await;
    let newer_store = Store::connect(&older_store.path).await.unwrap();
    for revision in 0..5 {
        newer_store
            .set_config_top_level_value("recovery_revision", json!(revision))
            .await
            .unwrap();
    }
    let previous_revision = newer_store
        .typed
        .typed_config
        .load()
        .await
        .unwrap()
        .unwrap()
        .revision;
    let mut expected = newer_store.get_config().await.unwrap();
    strip_internal_config_metadata(&mut expected);
    expected["locale"] = json!({ "default_locale": "ja-JP" });
    let replacement_json = expected.to_string();
    newer_store
        .manager
        .call(move |conn| {
            conn.execute(
                "UPDATE kv_strings SET value = ?2 WHERE key = ?1",
                [CONFIG_KEY, replacement_json.as_str()],
            )?;
            conn.execute("DELETE FROM config_documents WHERE singleton = 1", [])?;
            Ok(())
        })
        .await
        .unwrap();

    older_store.get_config().await.unwrap();
    let first_repair = older_store
        .typed
        .typed_config
        .load()
        .await
        .unwrap()
        .unwrap();
    assert!(first_repair.revision < previous_revision);
    assert_eq!(first_repair.document, expected);

    let mut loaded = newer_store.get_config().await.unwrap();
    strip_internal_config_metadata(&mut loaded);
    assert_eq!(loaded, expected);
    assert_eq!(newer_store.locale().await.unwrap(), expected["locale"]);
    let repaired = newer_store
        .typed
        .typed_config
        .load()
        .await
        .unwrap()
        .unwrap();
    assert!(repaired.revision > previous_revision);
    newer_store
        .set_config_top_level_value("locale", json!({ "default_locale": "ko-KR" }))
        .await
        .unwrap();
    older_store.get_config().await.unwrap();
    assert_eq!(
        older_store.locale().await.unwrap()["default_locale"],
        "ko-KR"
    );
}

#[tokio::test]
async fn typed_config_failure_rolls_back_the_legacy_config_transaction() {
    let (_dir, store) = open_test_store().await;
    let legacy_before = store.get_string_value(CONFIG_KEY).await.unwrap();
    let generation_before = store
        .get_string_value(HOST_MAPPINGS_GENERATION_KEY)
        .await
        .unwrap();
    let typed_before = store
        .typed
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
        .typed
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
    let (_dir, store) = open_test_store().await;
    let initial_revision = store
        .typed
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
        .typed
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
        .typed
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
        .typed
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
    let (_dir, store) = open_test_store().await;
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
    let (_dir, store) = open_test_store().await;
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
        .typed
        .typed_config
        .load()
        .await
        .expect("load typed config after backup restore")
        .expect("typed config exists after backup restore");
    assert_eq!(typed.document, restored_config);
    assert_eq!(typed.host_mappings_generation, 2);
}
