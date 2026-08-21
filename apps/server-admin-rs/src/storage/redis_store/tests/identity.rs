use super::*;

#[tokio::test]
async fn every_application_eval_operation_runs_on_sqlite() {
    let (_dir, store) = open_test_store().await;

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
    let (_dir, store) = open_test_store().await;
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

    let connection = open_fixture_connection(&path);
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

    let connection = open_fixture_connection(&path);
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
    let (_dir, store) = open_test_store().await;

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
    let (_dir, store) = open_test_store().await;
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
        .typed
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
    assert_eq!(
        store.typed.typed_docker_admin.counts().await.unwrap(),
        (1, 1)
    );

    let connection = open_fixture_connection(&path);
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
                .typed
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
    assert_eq!(
        store.typed.typed_docker_admin.counts().await.unwrap(),
        (0, 0)
    );
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
    let connection = open_fixture_connection(&path);
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
    let connection = open_fixture_connection(&path);
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
    let connection = open_fixture_connection(&path);
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
    assert_eq!(
        store.typed.typed_docker_admin.counts().await.unwrap(),
        (0, 0)
    );
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
    assert_eq!(
        target.typed.typed_docker_admin.counts().await.unwrap(),
        (1, 1)
    );
    target.clear_all_keys().await.expect("clear restored store");
    assert_eq!(
        target.typed.typed_docker_admin.counts().await.unwrap(),
        (0, 0)
    );
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

    let connection = open_fixture_connection(&path);
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

    let connection = open_fixture_connection(&path);
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
    assert_eq!(
        store.typed.typed_docker_admin.counts().await.unwrap(),
        (0, 0)
    );
}
