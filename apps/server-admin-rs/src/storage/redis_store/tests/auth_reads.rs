use super::*;
use std::time::Duration;

#[tokio::test]
async fn authorization_metadata_does_not_queue_behind_primary() {
    let (_directory, store) = open_test_store().await;
    let binding_key = auth_mobility_binding_key(
        "proxy-session",
        &auth_mobility_subject_hash("proxy-session", "s"),
    );
    store
        .set_json_value(&binding_key, &json!({"ownerSessionId":"s"}))
        .await
        .unwrap();
    store
        .set_json_value("fn_knock:totps", &json!([{"id":"t","secret":"secret"}]))
        .await
        .unwrap();
    store
        .set_json_value(
            "fn_knock:auth:accounts:v1",
            &json!([{"id":"a","username":"name"}]),
        )
        .await
        .unwrap();
    let mut conn = store.conn();
    conn.hset(
        auth_mobility_active_ip_details_key("s"),
        "192.0.2.1",
        r#"{"ip":"192.0.2.1"}"#,
    )
    .await
    .unwrap();
    store
        .set_json_value("fn_knock:passkeys", &json!([{"id":"p","totpId":"t"}]))
        .await
        .unwrap();
    store
        .set_json_value("fn_knock:captcha:settings", &json!({"provider":"pow"}))
        .await
        .unwrap();
    let (release, blocker) = block_primary_executor(&store).await;
    let result = tokio::time::timeout(Duration::from_millis(500), async {
        assert!(
            store
                .get_auth_mobility_binding_for_authorization("proxy-session", "s")
                .await
                .unwrap()
                .is_some()
        );
        assert!(
            store
                .get_auth_mobility_active_ip_detail_for_authorization("s", "192.0.2.1")
                .await
                .unwrap()
                .is_some()
        );
        assert_eq!(store.get_totps_for_authorization().await.unwrap().len(), 1);
        assert_eq!(
            store
                .get_auth_accounts_for_authorization()
                .await
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            store.get_passkeys_for_authorization().await.unwrap().len(),
            1
        );
        assert!(
            store
                .get_auth_login_mode_for_authorization()
                .await
                .unwrap()
                .allows_totp_family()
        );
        assert_eq!(
            store
                .get_auth_presentation_values(&["fn_knock:captcha:settings"])
                .await
                .unwrap(),
            vec![Some(json!({"provider":"pow"}))]
        );
    })
    .await;
    release.send(()).unwrap();
    blocker.await.unwrap();
    result.expect("auth metadata must not need the primary executor");
}

#[tokio::test]
async fn authorization_credentials_preserve_corruption_and_legacy_migration() {
    let (_directory, store) = open_test_store().await;
    store
        .set_string_value("fn_knock:totps", "broken-json")
        .await
        .unwrap();
    store
        .set_string_value("fn_knock:auth:accounts:v1", "broken-json")
        .await
        .unwrap();
    assert!(
        store
            .get_totps_for_authorization()
            .await
            .unwrap()
            .is_empty()
    );
    assert!(store.get_auth_accounts_for_authorization().await.is_err());
    store.delete_key("fn_knock:totps").await.unwrap();
    store
        .set_string_value("fn_knock:totp_secret", "legacy-secret")
        .await
        .unwrap();
    store
        .set_json_value("fn_knock:passkeys", &json!([{"id":"p"}]))
        .await
        .unwrap();
    let totps = store.get_totps_for_authorization().await.unwrap();
    assert_eq!(totps[0].id, "legacy-totp-id");
    assert_eq!(totps[0].secret, "legacy-secret");
    assert_eq!(
        store.get_passkeys().await.unwrap()[0]["totpId"],
        "legacy-totp-id"
    );
    assert!(
        store
            .get_string_value("fn_knock:totp_secret")
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn authorization_string_and_hash_ttls_are_checked_after_reader_admission() {
    let (directory, store) = open_test_store().await;
    let binding_key = auth_mobility_binding_key(
        "proxy-session",
        &auth_mobility_subject_hash("proxy-session", "s"),
    );
    let hash_key = auth_mobility_active_ip_details_key("s");
    store
        .set_json_value(&binding_key, &json!({"ownerSessionId":"s"}))
        .await
        .unwrap();
    let mut conn = store.conn();
    conn.hset(&hash_key, "192.0.2.1", r#"{"ip":"192.0.2.1"}"#)
        .await
        .unwrap();
    let manager = store.manager.clone();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let blocker = tokio::spawn(async move {
        manager
            .call_auth_read(move |_| {
                let _ = started_tx.send(());
                release_rx
                    .recv_timeout(Duration::from_secs(5))
                    .map_err(|error| crate::storage::storage_error(error.to_string()))?;
                Ok(())
            })
            .await
            .unwrap();
    });
    started_rx.await.unwrap();
    let reader_store = store.clone();
    let mut reader = tokio::spawn(async move {
        tokio::join!(
            reader_store.get_auth_mobility_binding_for_authorization("proxy-session", "s"),
            reader_store.get_auth_mobility_active_ip_detail_for_authorization("s", "192.0.2.1"),
        )
    });
    assert!(
        tokio::time::timeout(Duration::from_millis(20), &mut reader)
            .await
            .is_err()
    );
    let fixture = open_fixture_connection(directory.path().join("fn-knock.sqlite3"));
    fixture
        .execute(
            "UPDATE kv_keys SET expires_at_ms = ?1 WHERE key IN (?2, ?3)",
            tokio_rusqlite::rusqlite::params![crate::time_utils::now_ms(), binding_key, hash_key],
        )
        .unwrap();
    release_tx.send(()).unwrap();
    blocker.await.unwrap();
    let (binding, detail) = reader.await.unwrap();
    assert!(binding.unwrap().is_none());
    assert!(detail.unwrap().is_none());
    // Read-only filtering leaves cleanup to maintenance, including expired keys.
    assert_eq!(
        fixture
            .query_row(
                "SELECT COUNT(*) FROM kv_keys WHERE key IN (?1, ?2)",
                [&binding_key, &hash_key],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        2
    );
}

#[tokio::test]
async fn batched_session_ip_snapshot_preserves_recent_detail_semantics_and_order() {
    let (directory, store) = open_test_store().await;
    let mut first = new_login_session("t", "TOTP", "192.0.2.1", "test", 3600);
    first.login_time = "2026-01-01T00:00:00Z".into();
    let mut second = first.clone();
    second.login_time = "2026-01-02T00:00:00Z".into();
    store.add_session("first", &first, 3600).await.unwrap();
    store.add_session("second", &second, 3600).await.unwrap();
    let now = crate::time_utils::now_ms().div_euclid(1000);
    store
        .save_auth_mobility_active_ip_detail(
            "first",
            "legacy-member",
            now,
            &json!({"ip":"2001:db8::1"}),
            3600,
        )
        .await
        .unwrap();
    store
        .save_auth_mobility_active_ip_detail(
            "first",
            "stale",
            now - 61,
            &json!({"ip":"192.0.2.2"}),
            3600,
        )
        .await
        .unwrap();
    store
        .save_auth_mobility_active_ip_detail(
            "second",
            "detail-expired",
            now,
            &json!({"ip":"192.0.2.3"}),
            3600,
        )
        .await
        .unwrap();
    let fixture = open_fixture_connection(directory.path().join("fn-knock.sqlite3"));
    fixture
        .execute(
            "UPDATE kv_keys SET expires_at_ms = 1 WHERE key = ?1",
            [auth_mobility_active_ip_details_key("second")],
        )
        .unwrap();
    let (release, blocker) = block_primary_executor(&store).await;
    let snapshot = tokio::time::timeout(
        Duration::from_millis(500),
        store.list_auth_session_ip_candidates(Some(60)),
    )
    .await;
    release.send(()).unwrap();
    blocker.await.unwrap();
    let snapshot = snapshot.unwrap().unwrap();
    assert_eq!(
        snapshot
            .iter()
            .map(|row| row.0.as_str())
            .collect::<Vec<_>>(),
        vec!["second", "first"]
    );
    assert!(snapshot[0].2.is_empty());
    assert_eq!(snapshot[1].2, vec![json!({"ip":"2001:db8::1"})]);
    assert_eq!(
        store
            .get_auth_mobility_recent_ip_details_for_authorization("first", 60)
            .await
            .unwrap(),
        snapshot[1].2
    );
}

#[tokio::test]
async fn batched_ip_candidates_match_legacy_reads_at_multiple_session_counts() {
    for size in [1, 100, 1000] {
        let (_directory, store) = open_test_store().await;
        let now = crate::time_utils::now_ms().div_euclid(1000);
        store
            .manager
            .call(move |conn| {
                let tx = conn.transaction()?;
                for index in 0..size {
                    let id = format!("s-{index:04}");
                    let mut session = new_login_session("t", "TOTP", "192.0.2.1", "test", 3600);
                    session.login_time =
                        format!("2026-01-01T00:{:02}:{:02}Z", index / 60, index % 60);
                    crate::storage::redis_compat::execute_command_in_transaction(
                        &tx,
                        "SET",
                        vec![
                            format!("fn_knock:session:{id}"),
                            serde_json::to_string(&session)?,
                        ],
                    )?;
                    for (member, score, detail) in [
                        (
                            "legacy-member",
                            now,
                            json!({"ip":format!("2001:db8::{:x}", index + 1)}),
                        ),
                        ("stale", now - 120, json!({"ip":"192.0.2.2"})),
                    ] {
                        crate::storage::redis_compat::execute_command_in_transaction(
                            &tx,
                            "ZADD",
                            vec![
                                auth_mobility_active_ip_zset_key(&id),
                                score.to_string(),
                                member.into(),
                            ],
                        )?;
                        crate::storage::redis_compat::execute_command_in_transaction(
                            &tx,
                            "HSET",
                            vec![
                                auth_mobility_active_ip_details_key(&id),
                                member.into(),
                                detail.to_string(),
                            ],
                        )?;
                    }
                }
                tx.commit()?;
                Ok(())
            })
            .await
            .unwrap();
        let candidates = store
            .list_auth_session_ip_candidates(Some(60))
            .await
            .unwrap();
        let legacy = store.list_login_sessions().await.unwrap();
        assert_eq!(candidates.len(), size);
        for ((id, session, details), (legacy_id, legacy_session)) in candidates.iter().zip(legacy) {
            assert_eq!(*id, legacy_id);
            assert_eq!(
                serde_json::to_value(session).unwrap(),
                serde_json::to_value(legacy_session).unwrap()
            );
            assert_eq!(
                *details,
                store
                    .list_auth_mobility_recent_active_ip_details(id, now - 59)
                    .await
                    .unwrap()
            );
        }
    }
}
