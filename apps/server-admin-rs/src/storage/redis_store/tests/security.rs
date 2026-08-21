use super::*;

fn subdomain_rate_limit_key(scope: &str, fill: char) -> String {
    format!(
        "{}{scope}:{}",
        crate::storage::typed_subdomain_rate_limit::RATE_LIMIT_PREFIX,
        fill.to_string().repeat(64)
    )
}

#[tokio::test]
async fn subdomain_rate_limit_is_atomic_in_legacy_and_typed_stores() {
    let (_dir, store) = open_test_store().await;
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
        .typed
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

    let connection = open_fixture_connection(&path);
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
            .typed
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
    let connection = open_fixture_connection(&path);
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

    let connection = open_fixture_connection(&path);
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
            .typed
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
    let connection = open_fixture_connection(&source_path);
    connection
        .execute(
            "UPDATE kv_keys SET expires_at_ms = 0 WHERE key = ?1",
            [&expired_key],
        )
        .unwrap();
    drop(connection);
    assert!(
        !source
            .typed
            .typed_subdomain_rate_limit
            .verify_and_repair(&expired_key)
            .await
            .unwrap()
    );
    assert!(
        source
            .typed
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
            .typed
            .typed_subdomain_rate_limit
            .load(&backup_key)
            .await
            .unwrap()
            .unwrap()
            .counter_value,
        1
    );
    target.clear_all_keys().await.expect("clear restored store");
    assert_eq!(
        target
            .typed
            .typed_subdomain_rate_limit
            .count()
            .await
            .unwrap(),
        0
    );
}

fn wol_cooldown_key(target_id: &str) -> String {
    format!(
        "{}{target_id}",
        crate::storage::typed_wol_cooldown::COOLDOWN_PREFIX
    )
}

#[tokio::test]
async fn wol_cooldown_allows_one_concurrent_winner_in_both_stores() {
    let (_dir, store) = open_test_store().await;
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
        .typed
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
    let connection = open_fixture_connection(&path);
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
            .typed
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

    let connection = open_fixture_connection(&path);
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
            .typed
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
            .typed
            .typed_wol_cooldown
            .load("backup-target")
            .await
            .unwrap()
            .is_some()
    );

    let connection = open_fixture_connection(&source_path);
    connection
        .execute(
            "UPDATE kv_keys SET expires_at_ms = 0 WHERE key = ?1",
            [&key],
        )
        .unwrap();
    drop(connection);
    assert!(
        !source
            .typed
            .typed_wol_cooldown
            .verify_and_repair("backup-target")
            .await
            .unwrap()
    );
    assert!(
        source
            .typed
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
    assert_eq!(target.typed.typed_wol_cooldown.count().await.unwrap(), 0);
}

#[tokio::test]
async fn hmac_nonce_allows_one_concurrent_winner_and_stores_only_a_typed_digest() {
    let (_dir, store) = open_test_store().await;
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
        .typed
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

    let connection = open_fixture_connection(&path);
    connection
        .execute("UPDATE hmac_replay_nonces SET expires_at_ms = 1", [])
        .unwrap();
    drop(connection);
    assert!(!store.set_nonce_if_not_exists(nonce, 60).await.unwrap());
    assert!(
        store
            .typed
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

    let connection = open_fixture_connection(&path);
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
            .typed
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
    assert!(
        target
            .typed
            .typed_hmac_nonce
            .load(nonce)
            .await
            .unwrap()
            .is_some()
    );

    let key = format!(
        "{}{}",
        crate::storage::typed_hmac_nonce::NONCE_PREFIX,
        nonce
    );
    let connection = open_fixture_connection(&source_path);
    connection
        .execute(
            "UPDATE kv_keys SET expires_at_ms = 0 WHERE key = ?1",
            [&key],
        )
        .unwrap();
    drop(connection);
    assert!(
        !source
            .typed
            .typed_hmac_nonce
            .verify_and_repair(nonce)
            .await
            .unwrap()
    );
    assert!(
        source
            .typed
            .typed_hmac_nonce
            .load(nonce)
            .await
            .unwrap()
            .is_none()
    );
    target
        .clear_all_keys()
        .await
        .expect("clear target keyspace");
    assert_eq!(target.typed.typed_hmac_nonce.count().await.unwrap(), 0);
}
