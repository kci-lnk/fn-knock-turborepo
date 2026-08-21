use super::*;

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
    assert_eq!(store.typed.typed_passkey_runtime.count().await.unwrap(), 3);
    let typed_challenge = store
        .typed
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

    let connection = open_fixture_connection(&path);
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
            .typed
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
            .typed
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
    let connection = open_fixture_connection(&path);
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
            .typed
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
            .typed
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
    let connection = open_fixture_connection(&path);
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

    let connection = open_fixture_connection(&path);
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
    let connection = open_fixture_connection(&path);
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
    assert_eq!(target.typed.typed_passkey_runtime.count().await.unwrap(), 2);
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
    assert_eq!(target.typed.typed_passkey_runtime.count().await.unwrap(), 0);
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
        .typed
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
    let connection = open_fixture_connection(&path);
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
            .typed
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

    let connection = open_fixture_connection(&path);
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
            .typed
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
    let connection = open_fixture_connection(&path);
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

    let connection = open_fixture_connection(&path);
    connection
        .execute_batch("DROP TRIGGER fail_identity_runtime_update")
        .unwrap();
    drop(connection);
    store
        .set_json_value_ex(state_key, &json!({ "flow": "rollback" }), 600)
        .await
        .unwrap();
    let connection = open_fixture_connection(&path);
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
            .typed
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
        .typed
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
            .typed
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
            .typed
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
    let connection = open_fixture_connection(&path);
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
        .typed
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
    let connection = open_fixture_connection(&path);
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
