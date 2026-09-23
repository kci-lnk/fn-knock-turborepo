use super::*;
use serde_json::json;

async fn test_state() -> (tempfile::TempDir, AppState) {
    let directory = tempfile::tempdir().unwrap();
    let mut settings = {
        let _environment = crate::test_support::EnvGuard::new(&[]);
        crate::settings::Settings::from_env()
    };
    settings.data_dir = directory.path().join("data");
    settings.gateway_config_dir = directory.path().join("gateway");
    settings.sqlite_path = directory.path().join("fn-knock.sqlite3");
    settings.legacy_redis_url = String::new();
    settings.internal_rpc_token = "auth-request-context-test".into();
    let state = AppState::new(settings).await.unwrap();
    (directory, state)
}

#[tokio::test]
async fn scope_reuses_first_valid_credential_and_next_request_observes_revocation() {
    let (_directory, state) = test_state().await;
    state
        .storage
        .store
        .set_json_value(
            "fn_knock:totps",
            &json!([
                { "id": "a", "secret": "" },
                { "id": " a ", "secret": "first" },
                { "id": "a", "secret": "second" }
            ]),
        )
        .await
        .unwrap();
    state
        .storage
        .store
        .set_json_value(
            "fn_knock:auth:accounts:v1",
            &json!([
                { "id": "a", "username": "" },
                { "id": " a ", "username": " first " },
                { "id": "a", "username": "second" }
            ]),
        )
        .await
        .unwrap();
    scope(&state, async {
        assert_eq!(totp(&state, "a").await.unwrap().unwrap().secret, "first");
        assert_eq!(
            account(&state, "a").await.unwrap().unwrap().username,
            "first"
        );
        state
            .storage
            .store
            .set_json_value("fn_knock:totps", &json!([]))
            .await
            .unwrap();
        state
            .storage
            .store
            .set_json_value("fn_knock:auth:accounts:v1", &json!([]))
            .await
            .unwrap();
        // Nested stages share one immutable request input, never a global TTL cache.
        scope(&state, async {
            assert!(totp(&state, "a").await.unwrap().is_some());
            assert!(account(&state, "a").await.unwrap().is_some());
        })
        .await;
    })
    .await;
    scope(&state, async {
        assert!(totp(&state, "a").await.unwrap().is_none());
        assert!(account(&state, "a").await.unwrap().is_none());
    })
    .await;
}

#[tokio::test]
async fn config_is_consistent_within_scope_and_refreshed_between_requests() {
    let (_directory, state) = test_state().await;
    let mut next_config = state.storage.store.get_config().await.unwrap();
    next_config["marker"] = json!(1);
    state.storage.store.save_config(&next_config).await.unwrap();
    scope(&state, async {
        assert_eq!(config(&state)["marker"], 1);
        let mut next_config = state.storage.store.get_config().await.unwrap();
        next_config["marker"] = json!(2);
        state.storage.store.save_config(&next_config).await.unwrap();
        assert_eq!(config(&state)["marker"], 1);
    })
    .await;
    assert_eq!(config(&state)["marker"], 2);
}

#[tokio::test]
async fn credential_id_reads_match_legacy_lists_at_multiple_sizes() {
    let (_directory, state) = test_state().await;
    for size in [1, 100, 1000] {
        let totp_values = (0..size)
            .map(|index| {
                json!({
                    "id": format!("t-{index:04}"), "secret": format!("secret-{index}"),
                    "createdAt": "2026-01-01T00:00:00Z"
                })
            })
            .collect::<Vec<_>>();
        let account_values = (0..size)
            .map(|index| {
                json!({
                    "id": format!("a-{index:04}"), "username": format!("user-{index}"),
                    "createdAt": "2026-01-01T00:00:00Z", "updatedAt": "2026-01-01T00:00:00Z"
                })
            })
            .collect::<Vec<_>>();
        state
            .storage
            .store
            .set_json_value("fn_knock:totps", &json!(totp_values))
            .await
            .unwrap();
        state
            .storage
            .store
            .set_json_value("fn_knock:auth:accounts:v1", &json!(account_values))
            .await
            .unwrap();
        let expected_totps = state.storage.store.get_totps().await.unwrap();
        let expected_accounts = state.storage.store.get_auth_accounts().await.unwrap();
        scope(&state, async {
            let requested = expected_totps
                .iter()
                .map(|item| item.id.clone())
                .collect::<HashSet<_>>();
            assert_eq!(
                matching_totp_ids(&state, &requested).await.unwrap(),
                requested
            );
            for index in [0, size / 2, size - 1] {
                assert_eq!(
                    serde_json::to_value(totp(&state, &format!("t-{index:04}")).await.unwrap())
                        .unwrap(),
                    serde_json::to_value(Some(&expected_totps[index])).unwrap()
                );
                assert_eq!(
                    serde_json::to_value(account(&state, &format!("a-{index:04}")).await.unwrap())
                        .unwrap(),
                    serde_json::to_value(Some(&expected_accounts[index])).unwrap()
                );
            }
            assert!(totp(&state, "missing").await.unwrap().is_none());
            assert!(account(&state, "missing").await.unwrap().is_none());
        })
        .await;
    }
}

#[tokio::test]
async fn public_captcha_reads_keep_legacy_precedence_and_observe_updates() {
    let (_directory, state) = test_state().await;
    state
        .storage
        .store
        .set_json_value(
            "fn_knock:config:captcha",
            &json!({"provider":"turnstile", "turnstile":{"site_key":"legacy"}}),
        )
        .await
        .unwrap();
    for raw in [
        "broken-json",
        "null",
        r#"{"provider":"pow","pow":{"base_max_number":1000}}"#,
    ] {
        state
            .storage
            .store
            .set_string_value("fn_knock:captcha:settings", raw)
            .await
            .unwrap();
        let expected = crate::config::runtime::load_captcha_settings(&state)
            .await
            .unwrap();
        let actual = crate::config::runtime::load_captcha_settings_for_authorization(&state)
            .await
            .unwrap();
        assert_eq!(actual, expected);
    }
}

#[tokio::test]
async fn unqueried_ids_and_missing_results_use_the_original_snapshot() {
    let (_directory, state) = test_state().await;
    state
        .storage
        .store
        .set_json_value(
            "fn_knock:totps",
            &json!([
                {"id":"a","secret":"first"}, {"id":"b","secret":"second"}
            ]),
        )
        .await
        .unwrap();
    state
        .storage
        .store
        .set_json_value(
            "fn_knock:auth:accounts:v1",
            &json!([
                {"id":"a","username":"first"}, {"id":"b","username":"second"}
            ]),
        )
        .await
        .unwrap();
    scope(&state, async {
        let first_totp = totp(&state, "a").await.unwrap().unwrap();
        let first_account = account(&state, "a").await.unwrap().unwrap();
        assert!(totp(&state, "missing").await.unwrap().is_none());
        assert!(account(&state, "missing").await.unwrap().is_none());
        state.storage.store.set_json_value("fn_knock:totps", &json!([
            {"id":"b","secret":"changed"}, {"id":"missing","secret":"new"}
        ])).await.unwrap();
        state.storage.store.set_json_value("fn_knock:auth:accounts:v1", &json!([
            {"id":"b","username":"changed"}, {"id":"missing","username":"new"}
        ])).await.unwrap();
        let second_totp = totp(&state, "b").await.unwrap().unwrap();
        let second_account = account(&state, "b").await.unwrap().unwrap();
        assert_eq!(second_totp.secret, "second");
        assert_eq!(second_account.username, "second");
        assert!(totp(&state, "missing").await.unwrap().is_none());
        assert!(account(&state, "missing").await.unwrap().is_none());
        assert_eq!(first_totp.created_at, second_totp.created_at);
        assert_eq!(first_account.created_at, second_account.created_at);
        assert_eq!(first_account.updated_at, second_account.updated_at);
        let context = CURRENT.with(Arc::clone);
        assert_eq!(first_totp.created_at, context.totps.get().unwrap().normalized_at);
        assert_eq!(first_account.created_at, context.accounts.get().unwrap().normalized_at);
        assert!(matches!(&*context.totps.get().unwrap().cache.lock().unwrap(), CredentialCache::Indexed(by_id) if by_id.len() == 2));
        assert!(matches!(&*context.accounts.get().unwrap().cache.lock().unwrap(), CredentialCache::Indexed(by_id) if by_id.len() == 2));
        assert_eq!(matching_totp_ids(&state, &HashSet::from(["a".into(), "b".into(), "missing".into()])).await.unwrap(), HashSet::from(["a".into(), "b".into()]));
    }).await;
    assert_eq!(totp(&state, "b").await.unwrap().unwrap().secret, "changed");
    assert_eq!(
        account(&state, "b").await.unwrap().unwrap().username,
        "changed"
    );
}

#[tokio::test]
async fn membership_projection_migrates_legacy_totp_without_retaining_credentials() {
    let (_directory, state) = test_state().await;
    state
        .storage
        .store
        .set_string_value("fn_knock:totp_secret", "legacy-secret")
        .await
        .unwrap();
    state
        .storage
        .store
        .set_json_value("fn_knock:passkeys", &json!([{"id":"p"}]))
        .await
        .unwrap();
    scope(&state, async {
        assert!(
            matching_totp_ids(&state, &HashSet::new())
                .await
                .unwrap()
                .is_empty()
        );
        let context = CURRENT.with(Arc::clone);
        assert!(matches!(
            &*context.totps.get().unwrap().cache.lock().unwrap(),
            CredentialCache::Selective { first: None, .. }
        ));
        assert_eq!(
            state.storage.store.get_passkeys().await.unwrap()[0]["totpId"],
            "legacy-totp-id"
        );
        assert!(
            state
                .storage
                .store
                .get_string_value("fn_knock:totp_secret")
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(
            totp(&state, "legacy-totp-id")
                .await
                .unwrap()
                .unwrap()
                .secret,
            "legacy-secret"
        );
    })
    .await;
}

#[tokio::test]
async fn selective_credential_reads_match_legacy_normalization_and_json_validation() {
    let (_directory, state) = test_state().await;
    let deep_valid = format!("{}0{}", "[".repeat(96), "]".repeat(96));
    let too_deep = format!("{}0{}", "[".repeat(140), "]".repeat(140));
    let first = r#"{"id":"a","secret":"first","username":"first","createdAt":"kept","updatedAt":"updated"}"#;
    let cases = vec![
        format!("[{first}]"),
        format!(r#"[{first},{{"unknown":{deep_valid}}}]"#),
        format!(r#"[{first},{{"unknown":{too_deep}}}]"#),
        format!(r#"[{first},{{"unknown":1e9999}}]"#),
        format!(r#"[{first},{{"unknown":"\ud800"}}]"#),
        format!("[{first},"),
        format!("[{first}] trailing"),
        format!(r#"[{{"id":"a","secret":"","username":""}},{first}]"#),
        format!(r#"[{{"id":false,"secret":{{}},"username":99}},{first}]"#),
        r#"[{"secret":"no-id","username":"no-id"},{"id":" a ","secret":42,"username":" user ","createdAt":null},{"id":"b","secret":"b","username":"b","createdAt":"given"}]"#.into(),
        r#"[{"id":"a","id":"other","secret":"duplicate-object-key","username":"other"}]"#.into(),
        r#"[{"id":"a","secret":" secret ","comment":7,"username":" user ","displayName":" User ","sourceTotpId":" source ","createdAt":"kept","access_scopes":["docker_admin_panel","docker_admin_panel",{},7],"subdomain_access":{"mode":"all"}}]"#.into(),
        "{}".into(), "null".into(), "  ".into(), "broken-json".into(),
    ];
    for raw in cases {
        state
            .storage
            .store
            .set_string_value("fn_knock:totps", &raw)
            .await
            .unwrap();
        state
            .storage
            .store
            .set_string_value("fn_knock:auth:accounts:v1", &raw)
            .await
            .unwrap();
        let expected_totps = state.storage.store.get_totps().await.unwrap();
        let expected_accounts = state.storage.store.get_auth_accounts().await;
        scope(&state, async {
            // Check projection directly from raw bytes before any ID lookup
            // promotes the cache, including malformed suffixes after a match.
            let requested = HashSet::from(["a".into(), "b".into(), "false".into(), "other".into()]);
            let expected_ids = expected_totps
                .iter()
                .filter(|item| requested.contains(&item.id))
                .map(|item| item.id.clone())
                .collect::<HashSet<_>>();
            assert_eq!(
                matching_totp_ids(&state, &requested).await.unwrap(),
                expected_ids,
                "raw membership mismatch: {raw}"
            );
            for id in ["a", "b", "missing", "false", "other", "a"] {
                let actual = totp(&state, id).await.unwrap();
                let expected = expected_totps.iter().find(|item| item.id == id);
                assert_eq!(
                    actual.as_ref().map(|item| (
                        &item.id,
                        &item.secret,
                        &item.comment,
                        &item.access_scopes,
                        &item.subdomain_access
                    )),
                    expected.map(|item| (
                        &item.id,
                        &item.secret,
                        &item.comment,
                        &item.access_scopes,
                        &item.subdomain_access
                    )),
                    "TOTP mismatch for {id}: {raw}"
                );
                if let (Some(actual), Some(expected)) = (&actual, expected)
                    && ["kept", "null", "given"].contains(&expected.created_at.as_str())
                {
                    assert_eq!(actual.created_at, expected.created_at);
                }
                let actual = account(&state, id).await;
                match &expected_accounts {
                    Err(_) => assert!(actual.is_err(), "account JSON should fail: {raw}"),
                    Ok(accounts) => {
                        let actual = actual.unwrap();
                        let expected = accounts.iter().find(|item| item.id == id);
                        assert_eq!(
                            actual.as_ref().map(|item| (
                                &item.id,
                                &item.username,
                                &item.display_name,
                                &item.source_totp_id,
                                &item.access_scopes,
                                &item.subdomain_access
                            )),
                            expected.map(|item| (
                                &item.id,
                                &item.username,
                                &item.display_name,
                                &item.source_totp_id,
                                &item.access_scopes,
                                &item.subdomain_access
                            )),
                            "account mismatch for {id}: {raw}"
                        );
                        if let (Some(actual), Some(expected)) = (actual, expected)
                            && !expected.created_at.contains('T')
                        {
                            assert_eq!(actual.created_at, expected.created_at);
                            assert_eq!(actual.updated_at, expected.updated_at);
                        }
                    }
                }
            }
            let requested = HashSet::from(["a".into(), "b".into(), "false".into(), "other".into()]);
            assert_eq!(
                matching_totp_ids(&state, &requested).await.unwrap(),
                expected_totps
                    .iter()
                    .filter(|item| requested.contains(&item.id))
                    .map(|item| item.id.clone())
                    .collect::<HashSet<_>>(),
                "membership mismatch: {raw}"
            );
        })
        .await;
    }
}

#[tokio::test]
async fn one_id_and_membership_reads_do_not_retain_the_full_normalized_collection() {
    let (_directory, state) = test_state().await;
    let values = (0..1000).map(|index| json!({"id":format!("t-{index}"),"secret":"secret","unknown":{"nested":[1,2,3]}})).collect::<Vec<_>>();
    state
        .storage
        .store
        .set_json_value("fn_knock:totps", &json!(values))
        .await
        .unwrap();
    scope(&state, async {
        assert_eq!(totp(&state, "t-0").await.unwrap().unwrap().id, "t-0");
        assert_eq!(matching_totp_ids(&state, &HashSet::from(["t-999".into()])).await.unwrap(), HashSet::from(["t-999".into()]));
        let context = CURRENT.with(Arc::clone);
        let snapshot = context.totps.get().unwrap();
        assert!(matches!(&*snapshot.cache.lock().unwrap(), CredentialCache::Selective { raw: Some(raw), first: Some((id, Some(_))) } if id == "t-0" && raw.contains("t-999")));
    }).await;
}

#[test]
fn multiple_owner_lookup_scans_at_most_twice_and_releases_raw_snapshot() {
    use std::cell::Cell;
    let values = (0..1000)
        .map(|index| json!({"id":format!("t-{index}"),"secret":"secret"}))
        .collect::<Vec<_>>();
    let snapshot = CredentialSnapshot::new(Some(serde_json::to_string(&values).unwrap()));
    let calls = Cell::new(0);
    for index in 0..1000 {
        let id = format!("t-{index}");
        let actual = snapshot
            .get(&id, true, |value, selected, time| {
                calls.set(calls.get() + 1);
                Store::authorization_totp_from_value_at(value, selected, time)
            })
            .unwrap()
            .unwrap();
        assert_eq!(actual.id, id);
        assert_eq!(actual.created_at, snapshot.normalized_at);
    }
    assert_eq!(calls.get(), 1001);
    assert!(
        snapshot
            .get("missing", true, Store::authorization_totp_from_value_at)
            .unwrap()
            .is_none()
    );
    assert!(
        matches!(&*snapshot.cache.lock().unwrap(), CredentialCache::Indexed(by_id) if by_id.len() == 1000)
    );
}

#[test]
fn skipped_json_suffix_has_the_same_validation_as_serde_value() {
    let mut cases = vec![
        r#"[{"id":"a"},{"unknown":1e9999}]"#.to_string(),
        r#"[{"id":"a"},{"unknown":"\ud800"}]"#.to_string(),
        r#"[{"id":"a"},{"unknown":-9223372036854775809}]"#.to_string(),
        r#"[{"id":"a"},{"unknown":"\ud83d\ude00"}]"#.to_string(),
        r#"[{"id":"a"},{"unknown":{"nested":[null,true,false,1.2,"hello"]}}]"#.to_string(),
    ];
    for depth in 120..=132 {
        cases.push(format!(
            "[{{}},{}0{}]",
            "[".repeat(depth),
            "]".repeat(depth)
        ));
        cases.push(format!(
            "[{{}},{}0{}]",
            r#"{"nested":"#.repeat(depth),
            "}".repeat(depth)
        ));
    }
    for raw in cases {
        assert_eq!(
            json_scan::array(&raw, |_| false).is_ok(),
            serde_json::from_str::<Value>(&raw).is_ok(),
            "{raw}"
        );
    }
}
