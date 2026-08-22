use super::*;

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
    let (_dir, store) = open_test_store().await;
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
    assert_eq!(store.typed.typed_fnos_share.count().await.unwrap(), 3);
    assert!(
        store
            .typed
            .typed_fnos_share
            .load_key(&validation_key)
            .await
            .unwrap()
            .unwrap()
            .payload_json
            .is_some()
    );
    let lock = store
        .typed
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
    let connection = open_fixture_connection(&path);
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

    let connection = install_failure_trigger(
        &path,
        "CREATE TRIGGER fail_typed_fnos_share_insert
         BEFORE INSERT ON fnos_share_runtime_capabilities
         BEGIN
           SELECT RAISE(FAIL, 'forced typed fnOS share failure');
         END;",
    );
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
            .typed
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
            .typed
            .typed_fnos_share
            .load_key(&session_key)
            .await
            .unwrap()
            .is_some()
    );

    let connection = open_fixture_connection(&source_path);
    connection
        .execute(
            "UPDATE kv_keys SET expires_at_ms = 0 WHERE key = ?1",
            [&session_key],
        )
        .unwrap();
    drop(connection);
    assert!(
        !source
            .typed
            .typed_fnos_share
            .verify_and_repair_key(&session_key)
            .await
            .unwrap()
    );
    assert!(
        source
            .typed
            .typed_fnos_share
            .load_key(&session_key)
            .await
            .unwrap()
            .is_none()
    );
    target.clear_all_keys().await.unwrap();
    assert_eq!(target.typed.typed_fnos_share.count().await.unwrap(), 0);
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
    let (_dir, store) = open_test_store().await;
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
        .typed
        .typed_subdomain_grant
        .load_grant(&grant_key)
        .await
        .unwrap()
        .expect("typed subdomain grant");
    assert_eq!(grant.host, host);
    assert_eq!(grant.last_access_at, 1_700_000_010);
    assert!(grant.expires_at_ms > crate::time_utils::now_ms());
    let active = store
        .typed
        .typed_subdomain_grant
        .active_entries(&active_key)
        .await
        .unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].expires_at_score, 1_700_000_070);
    assert_eq!(
        store.typed.typed_subdomain_grant.counts().await.unwrap(),
        (1, 1)
    );

    store
        .delete_string_and_zrem(&grant_key, &active_key, &grant_key)
        .await
        .unwrap();
    assert_eq!(
        store.typed.typed_subdomain_grant.counts().await.unwrap(),
        (0, 0)
    );
}

#[tokio::test]
async fn matched_subdomain_grant_read_bypasses_the_primary_executor() {
    let (_dir, store) = open_test_store().await;
    let host = "reader.example.com";
    let document = subdomain_grant_document(host, 1_700_000_015);
    let (grant_key, active_key) = subdomain_grant_keys("reader-token", host);
    store
        .set_expiring_string_with_zset_limit(
            &grant_key,
            &document,
            60,
            &active_key,
            1_700_000_015,
            1_700_000_075,
            10,
        )
        .await
        .expect("seed grant");

    let manager = store.manager.clone();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let blocker = manager.call(move |_conn| {
        let _ = started_tx.send(());
        release_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .map_err(|error| {
                crate::storage::storage_error(format!("release grant blocker: {error}"))
            })?;
        Ok(())
    });
    let read = async {
        started_rx.await.expect("primary executor started");
        let loaded = tokio::time::timeout(
            std::time::Duration::from_millis(250),
            store.get_string_value_auth(&grant_key),
        )
        .await
        .expect("matched grant read must bypass primary storage")
        .expect("read grant");
        assert_eq!(loaded.as_deref(), Some(document.as_str()));
        release_tx.send(()).expect("release primary executor");
    };
    let (blocker_result, ()) = tokio::join!(blocker, read);
    blocker_result.expect("primary blocker result");
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
    let connection = open_fixture_connection(&path);
    connection
        .execute("DELETE FROM subdomain_rule_grant_active_entries", [])
        .unwrap();
    drop(connection);
    assert!(store.get_string_value(&grant_key).await.unwrap().is_some());
    assert_eq!(
        store
            .typed
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

    let connection = open_fixture_connection(&path);
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
            .typed
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
    assert_eq!(
        target.typed.typed_subdomain_grant.counts().await.unwrap(),
        (1, 1)
    );

    let connection = open_fixture_connection(&source_path);
    connection
        .execute(
            "UPDATE kv_keys SET expires_at_ms = 0 WHERE key = ?1",
            [&grant_key],
        )
        .unwrap();
    drop(connection);
    assert!(source.get_string_value(&grant_key).await.unwrap().is_none());
    assert_eq!(
        source.typed.typed_subdomain_grant.counts().await.unwrap(),
        (0, 0)
    );
    target.clear_all_keys().await.unwrap();
    assert_eq!(
        target.typed.typed_subdomain_grant.counts().await.unwrap(),
        (0, 0)
    );
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
    let (_dir, store) = open_test_store().await;
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
        store.typed.typed_whitelist_runtime.counts().await.unwrap(),
        (1, 1)
    );
    let mapping = store
        .typed
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
        .typed
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

    let connection = open_fixture_connection(&path);
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
        store.typed.typed_whitelist_runtime.counts().await.unwrap(),
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
        target.typed.typed_whitelist_runtime.counts().await.unwrap(),
        (1, 0)
    );

    let connection = open_fixture_connection(&source_path);
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
        target.typed.typed_whitelist_runtime.counts().await.unwrap(),
        (0, 0)
    );
}
