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
    let store = Store::connect(dir.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
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
            .update_ldap_binding_if_owned(LdapBindingUpdate {
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

    store
        .delete_keys(&[subject_key.to_string(), binding_key.to_string()])
        .await
        .unwrap();
    assert!(
        !store
            .update_ldap_binding_if_owned(LdapBindingUpdate {
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
    assert_eq!(traffic_scope_segment("global", None), "global");
    assert_eq!(traffic_scope_segment("", None), "");
    assert_eq!(traffic_scope_segment(" user ", None), " user ");
    assert_eq!(
        traffic_scope_segment("global", Some("example.com")),
        "global:host:example.com"
    );
    assert_eq!(
        traffic_scope_segment(" user ", Some("example.com")),
        " user :host:example.com"
    );
    assert_eq!(
        traffic_scope_segment("u", Some("[2001:db8::1]")),
        "u:host:%5B2001%3Adb8%3A%3A1%5D"
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
