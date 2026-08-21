use super::*;

#[tokio::test]
async fn binding_keep_ttl_rejects_missing_keys_and_preserves_persistent_keys() {
    let (_dir, store) = open_test_store().await;
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
    let (_dir, store) = open_test_store().await;
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
        .typed
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
    assert_eq!(store.typed.typed_mobility.counts().await.unwrap(), (2, 0));
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
        .typed
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
    let connection = open_fixture_connection(&path);
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
            .typed
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

    let connection = open_fixture_connection(&path);
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
        .typed
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

    let connection = open_fixture_connection(&path);
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
        .typed
        .typed_mobility
        .load_session(session_id)
        .await
        .expect("load repaired aggregate")
        .expect("repaired aggregate exists");
    assert_eq!(
        repaired.session.expect("typed session component").value,
        serde_json::to_value(&session).unwrap()
    );

    let connection = open_fixture_connection(&path);
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
            .typed
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

    let connection = open_fixture_connection(&path);
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

    let connection = open_fixture_connection(&path);
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
        .typed
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

    let connection = open_fixture_connection(&path);
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
        .typed
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
    assert_eq!(store.typed.typed_login_backoff.count().await.unwrap(), 0);
}

#[tokio::test]
async fn login_backoff_concurrent_failures_remain_atomic_in_both_stores() {
    let (_dir, store) = open_test_store().await;
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
        .typed
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
    let (_dir, store) = open_test_store().await;
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
    assert!(
        store
            .typed
            .typed_login_backoff
            .load(ip)
            .await
            .unwrap()
            .is_none()
    );
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
    let connection = open_fixture_connection(&path);
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
    let connection = open_fixture_connection(&path);
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
    let connection = open_fixture_connection(&path);
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
            .typed
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
    assert!(
        target
            .typed
            .typed_login_backoff
            .load(ip)
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(
        target.get_login_backoff_status(ip).await.unwrap().attempts,
        1
    );

    target.clear_all_keys().await.expect("clear restored store");
    assert_eq!(target.typed.typed_login_backoff.count().await.unwrap(), 0);
}
