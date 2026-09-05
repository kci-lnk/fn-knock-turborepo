use super::*;

fn account_and_password(id: &str, username: &str) -> (AuthAccount, AuthPasswordCredential) {
    let now = crate::time_utils::now_iso();
    (
        AuthAccount {
            id: id.to_string(),
            username: username.to_string(),
            display_name: username.to_string(),
            source_totp_id: String::new(),
            created_at: now.clone(),
            updated_at: now.clone(),
            access_scopes: json!([]),
            subdomain_access: json!({ "mode": "all", "hosts": [] }),
        },
        AuthPasswordCredential {
            account_id: id.to_string(),
            algorithm: "scrypt".to_string(),
            salt: "00".repeat(16),
            hash: format!("password-for-{id}"),
            n: 16_384,
            r: 8,
            p: 1,
            key_length: 64,
            created_at: now.clone(),
            updated_at: now,
        },
    )
}

#[tokio::test]
async fn auth_account_creation_cas_accepts_legacy_missing_timestamps() {
    let (_dir, store) = open_test_store().await;
    store
        .set_json_value(
            "fn_knock:auth:accounts:v1",
            &json!([{ "id": "legacy-id", "username": "legacy", "createdAt": "  " }]),
        )
        .await
        .unwrap();
    let expected = store.get_auth_accounts().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    let (created, password) = account_and_password("created-id", "created");
    let mut replacement = expected.clone();
    replacement.push(created);
    assert!(
        store
            .compare_and_set_auth_accounts_with_password(&expected, &replacement, &password)
            .await
            .unwrap()
    );
    let stored = store.get_auth_accounts().await.unwrap();
    assert_eq!(stored.len(), 2);
    assert_eq!(stored[0].created_at, expected[0].created_at);
    assert_eq!(stored[0].updated_at, expected[0].updated_at);
    assert!(
        store
            .get_auth_password_credential("created-id")
            .await
            .unwrap()
            .is_some()
    );
}

#[tokio::test]
async fn auth_account_creation_cas_rejects_changed_fields_and_persisted_timestamps() {
    let (_dir, store) = open_test_store().await;
    let (existing, _) = account_and_password("existing-id", "existing");
    let (created, password) = account_and_password("created-id", "created");
    for field in ["username", "createdAt", "updatedAt"] {
        store
            .set_auth_accounts(std::slice::from_ref(&existing))
            .await
            .unwrap();
        let expected = store.get_auth_accounts().await.unwrap();
        let mut changed = serde_json::to_value(&expected).unwrap();
        changed[0][field] = json!("concurrently-changed");
        store
            .set_json_value("fn_knock:auth:accounts:v1", &changed)
            .await
            .unwrap();
        let mut replacement = expected.clone();
        replacement.push(created.clone());
        assert!(
            !store
                .compare_and_set_auth_accounts_with_password(&expected, &replacement, &password)
                .await
                .unwrap(),
            "accepted a concurrent change to {field}"
        );
        assert_eq!(
            serde_json::to_value(store.get_auth_accounts().await.unwrap()).unwrap(),
            changed
        );
        assert!(
            store
                .get_auth_password_credential("created-id")
                .await
                .unwrap()
                .is_none()
        );
    }
}

#[tokio::test]
async fn auth_account_creation_cas_preserves_concurrent_distinct_accounts() {
    let (_dir, first) = open_test_store().await;
    let second = Store::connect(&first.path).await.unwrap();
    let (alice, alice_password) = account_and_password("alice-id", "alice");
    let (bob, bob_password) = account_and_password("bob-id", "bob");
    let (alice_result, bob_result) = tokio::join!(
        first.compare_and_set_auth_accounts_with_password(
            &[],
            std::slice::from_ref(&alice),
            &alice_password,
        ),
        second.compare_and_set_auth_accounts_with_password(
            &[],
            std::slice::from_ref(&bob),
            &bob_password,
        ),
    );
    let alice_won = alice_result.unwrap();
    assert_ne!(alice_won, bob_result.unwrap());
    let (pending, pending_password) = if alice_won {
        (&bob, &bob_password)
    } else {
        (&alice, &alice_password)
    };
    assert!(
        first
            .get_auth_password_credential(&pending.id)
            .await
            .unwrap()
            .is_none()
    );
    let current = first.get_auth_accounts().await.unwrap();
    let mut replacement = current.clone();
    replacement.push(pending.clone());
    assert!(
        first
            .compare_and_set_auth_accounts_with_password(&current, &replacement, pending_password,)
            .await
            .unwrap()
    );
    let accounts = first.get_auth_accounts().await.unwrap();
    assert_eq!(accounts.len(), 2);
    for expected in [&alice, &bob] {
        assert!(accounts.iter().any(|account| account.id == expected.id));
        assert!(
            first
                .get_auth_password_credential(&expected.id)
                .await
                .unwrap()
                .is_some()
        );
    }
}

#[tokio::test]
async fn auth_account_creation_cas_allows_only_one_concurrent_username() {
    let (_dir, first) = open_test_store().await;
    let second = Store::connect(&first.path).await.unwrap();
    let (alice, alice_password) = account_and_password("first-id", "alice");
    let (other, other_password) = account_and_password("second-id", "ALICE");
    let (first_result, second_result) = tokio::join!(
        first.compare_and_set_auth_accounts_with_password(
            &[],
            std::slice::from_ref(&alice),
            &alice_password,
        ),
        second.compare_and_set_auth_accounts_with_password(
            &[],
            std::slice::from_ref(&other),
            &other_password,
        ),
    );
    let first_won = first_result.unwrap();
    assert_ne!(first_won, second_result.unwrap());
    let accounts = first.get_auth_accounts().await.unwrap();
    assert_eq!(accounts.len(), 1);
    assert!(accounts[0].username.eq_ignore_ascii_case("alice"));
    let loser_id = if first_won { &other.id } else { &alice.id };
    assert!(
        first
            .get_auth_password_credential(loser_id)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn auth_account_creation_cas_rolls_back_projection_on_credential_failure() {
    let (_dir, store) = open_test_store().await;
    let (existing, existing_password) = account_and_password("existing-id", "existing");
    store
        .set_auth_accounts(std::slice::from_ref(&existing))
        .await
        .unwrap();
    store
        .set_auth_password_credential(&existing_password)
        .await
        .unwrap();
    let expected = store.get_auth_accounts().await.unwrap();
    let (created, password) = account_and_password("failed-id", "failed");
    let connection = open_fixture_connection(&store.path);
    connection
        .execute_batch(
            "CREATE TRIGGER fail_created_account_password
         BEFORE INSERT ON kv_strings
         WHEN NEW.key = 'fn_knock:auth:password_credentials:v1:failed-id'
         BEGIN SELECT RAISE(FAIL, 'forced password write failure'); END;",
        )
        .unwrap();
    drop(connection);
    // This also checks that obsolete projection credentials are not removed
    // when the newly-created credential cannot be persisted.
    assert!(
        store
            .compare_and_set_auth_accounts_with_password(
                &expected,
                std::slice::from_ref(&created),
                &password,
            )
            .await
            .is_err()
    );
    assert_eq!(
        serde_json::to_value(store.get_auth_accounts().await.unwrap()).unwrap(),
        serde_json::to_value(&expected).unwrap(),
    );
    assert!(
        store
            .get_auth_password_credential(&created.id)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        store
            .get_auth_password_credential(&existing.id)
            .await
            .unwrap()
            .unwrap()
            .hash,
        existing_password.hash,
    );
    let connection = open_fixture_connection(&store.path);
    connection
        .execute("DROP TRIGGER fail_created_account_password", [])
        .unwrap();
    drop(connection);
    assert!(
        store
            .compare_and_set_auth_accounts_with_password(
                &expected,
                std::slice::from_ref(&created),
                &password,
            )
            .await
            .unwrap()
    );
    assert!(
        store
            .get_auth_password_credential(&existing.id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .get_auth_password_credential(&created.id)
            .await
            .unwrap()
            .is_some()
    );
}

#[tokio::test]
async fn auth_account_mutation_cas_rolls_back_all_records_on_password_failure() {
    let (_dir, store) = open_test_store().await;
    let (account, password) = account_and_password("atomic-id", "original");
    store.set_auth_accounts(&[account]).await.unwrap();
    let original = store
        .get_auth_account_mutation_snapshot("atomic-id", None)
        .await
        .unwrap();
    let mut replacement = original.clone();
    replacement.accounts[0].username = "changed".to_string();
    replacement.password = Some(password);
    replacement.totps.push(TotpCredential {
        id: "new-totp".to_string(),
        secret: "SECRET".to_string(),
        comment: "changed".to_string(),
        created_at: crate::time_utils::now_iso(),
        access_scopes: json!([]),
        subdomain_access: json!({"mode":"all", "hosts":[]}),
    });
    let connection = open_fixture_connection(&store.path);
    connection
        .execute_batch(
            "CREATE TRIGGER fail_mutation_password BEFORE INSERT ON kv_strings
        WHEN NEW.key = 'fn_knock:auth:password_credentials:v1:atomic-id'
        BEGIN SELECT RAISE(FAIL, 'forced password failure'); END;",
        )
        .unwrap();
    drop(connection);
    assert!(
        store
            .compare_and_set_auth_account_mutation("atomic-id", &original, &replacement)
            .await
            .is_err()
    );
    let current = store
        .get_auth_account_mutation_snapshot("atomic-id", None)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(&current.accounts).unwrap(),
        serde_json::to_value(&original.accounts).unwrap()
    );
    assert!(current.totps.is_empty());
    assert!(current.password.is_none());
}

#[tokio::test]
async fn auth_account_mutation_cas_rejects_changed_password_totp_and_account() {
    let (_dir, store) = open_test_store().await;
    let other = Store::connect(&store.path).await.unwrap();
    let (account, password) = account_and_password("atomic-id", "original");
    let totp = TotpCredential {
        id: "source".to_string(),
        secret: "SECRET".to_string(),
        comment: "original".to_string(),
        created_at: crate::time_utils::now_iso(),
        access_scopes: json!([]),
        subdomain_access: json!({"mode":"all", "hosts":[]}),
    };
    for changed in ["account", "password", "totp"] {
        store
            .set_auth_accounts(std::slice::from_ref(&account))
            .await
            .unwrap();
        store.set_auth_password_credential(&password).await.unwrap();
        store.set_totps(std::slice::from_ref(&totp)).await.unwrap();
        let original = store
            .get_auth_account_mutation_snapshot(&account.id, None)
            .await
            .unwrap();
        let mut replacement = original.clone();
        replacement.password.as_mut().unwrap().hash = "requested-password".to_string();
        match changed {
            "account" => {
                other.set_auth_accounts(&[]).await.unwrap();
            }
            "password" => {
                let mut concurrent = password.clone();
                concurrent.hash = "concurrent-password".to_string();
                other
                    .set_auth_password_credential(&concurrent)
                    .await
                    .unwrap();
            }
            "totp" => {
                other.set_totps(&[]).await.unwrap();
            }
            _ => unreachable!(),
        }
        assert!(
            !store
                .compare_and_set_auth_account_mutation(&account.id, &original, &replacement)
                .await
                .unwrap(),
            "accepted changed {changed}"
        );
        let current = store
            .get_auth_password_credential(&account.id)
            .await
            .unwrap()
            .unwrap();
        assert_ne!(current.hash, "requested-password");
    }
}

#[tokio::test]
async fn auth_account_mutation_rollback_cas_preserves_concurrent_create_and_delete() {
    let (_dir, store) = open_test_store().await;
    let other = Store::connect(&store.path).await.unwrap();
    let (account, password) = account_and_password("atomic-id", "original");
    for changed in ["create", "delete", "password"] {
        store
            .set_auth_accounts(std::slice::from_ref(&account))
            .await
            .unwrap();
        store.set_auth_password_credential(&password).await.unwrap();
        let original = store
            .get_auth_account_mutation_snapshot(&account.id, None)
            .await
            .unwrap();
        let mut applied = original.clone();
        applied.accounts[0].username = "applied-name".to_string();
        applied.password.as_mut().unwrap().hash = "applied-password".to_string();
        assert!(
            store
                .compare_and_set_auth_account_mutation(&account.id, &original, &applied)
                .await
                .unwrap()
        );
        match changed {
            "create" => {
                let (created, credential) = account_and_password("created-id", "created");
                let mut next = applied.accounts.clone();
                next.push(created);
                assert!(
                    other
                        .compare_and_set_auth_accounts_with_password(
                            &applied.accounts,
                            &next,
                            &credential
                        )
                        .await
                        .unwrap()
                );
            }
            "delete" => {
                other.set_auth_accounts(&[]).await.unwrap();
            }
            "password" => {
                let mut credential = password.clone();
                credential.hash = "newer-password".to_string();
                other
                    .set_auth_password_credential(&credential)
                    .await
                    .unwrap();
            }
            _ => unreachable!(),
        }
        assert!(
            !store
                .compare_and_set_auth_account_mutation(&account.id, &applied, &original)
                .await
                .unwrap()
        );
        let after = store
            .get_auth_account_mutation_snapshot(&account.id, None)
            .await
            .unwrap();
        match changed {
            "create" => assert_eq!(after.accounts.len(), 2),
            "delete" => assert!(after.accounts.is_empty()),
            "password" => assert_eq!(after.password.unwrap().hash, "newer-password"),
            _ => unreachable!(),
        }
    }
}

#[tokio::test]
async fn auth_account_mutation_snapshot_preserves_legacy_timestamp_defaults() {
    let (_dir, store) = open_test_store().await;
    store
        .set_json_value(
            "fn_knock:auth:accounts:v1",
            &json!([{"id":"legacy-id", "username":"legacy"}]),
        )
        .await
        .unwrap();
    store
        .set_json_value(
            "fn_knock:totps",
            &json!([{"id":"legacy-totp", "secret":"SECRET"}]),
        )
        .await
        .unwrap();
    let original = store
        .get_auth_account_mutation_snapshot("legacy-id", None)
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    let current = store
        .get_auth_account_mutation_snapshot("legacy-id", Some(&original))
        .await
        .unwrap();
    assert_eq!(
        current.accounts[0].created_at,
        original.accounts[0].created_at
    );
    assert_eq!(current.totps[0].created_at, original.totps[0].created_at);
    assert!(
        store
            .compare_and_set_auth_account_mutation("legacy-id", &original, &current)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn auth_account_mutation_rollback_cas_restores_its_exact_write() {
    let (_dir, store) = open_test_store().await;
    let (account, password) = account_and_password("atomic-id", "original");
    store.set_auth_accounts(&[account]).await.unwrap();
    let original = store
        .get_auth_account_mutation_snapshot("atomic-id", None)
        .await
        .unwrap();
    let mut applied = original.clone();
    applied.accounts[0].username = "applied-name".to_string();
    applied.password = Some(password);
    applied.totps.push(TotpCredential {
        id: "new-source".to_string(),
        secret: "SECRET".to_string(),
        comment: "applied-name".to_string(),
        created_at: crate::time_utils::now_iso(),
        access_scopes: json!([]),
        subdomain_access: json!({"mode":"all", "hosts":[]}),
    });
    assert!(
        store
            .compare_and_set_auth_account_mutation("atomic-id", &original, &applied)
            .await
            .unwrap()
    );
    assert!(
        store
            .compare_and_set_auth_account_mutation("atomic-id", &applied, &original)
            .await
            .unwrap()
    );
    let current = store
        .get_auth_account_mutation_snapshot("atomic-id", None)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(current.accounts).unwrap(),
        serde_json::to_value(&original.accounts).unwrap()
    );
    assert!(current.password.is_none());
    assert!(current.totps.is_empty());

    // Canonicalizing the expected value must not mask a subsequent real
    // permission change. Keep using the original, noncanonical applied value.
    assert!(
        store
            .compare_and_set_auth_account_mutation("atomic-id", &original, &applied)
            .await
            .unwrap()
    );
    let mut concurrent_totps = applied.totps.clone();
    concurrent_totps[0].subdomain_access = json!({
        "mode": "custom", "hosts": ["concurrent.example.com"],
        "streams": [{"protocol": "tcp", "listen_port": 443}],
    });
    store.set_totps(&concurrent_totps).await.unwrap();
    assert!(
        !store
            .compare_and_set_auth_account_mutation("atomic-id", &applied, &original)
            .await
            .unwrap()
    );
    assert_eq!(
        store.get_totps().await.unwrap()[0].subdomain_access,
        concurrent_totps[0].subdomain_access
    );
}
