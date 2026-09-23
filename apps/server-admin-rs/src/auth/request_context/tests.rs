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
    state
        .storage
        .store
        .save_config(&json!({"marker": 1}))
        .await
        .unwrap();
    scope(&state, async {
        assert_eq!(config(&state)["marker"], 1);
        state
            .storage
            .store
            .save_config(&json!({"marker": 2}))
            .await
            .unwrap();
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
            assert_eq!(
                serde_json::to_value(totps(&state).await.unwrap()).unwrap(),
                serde_json::to_value(&expected_totps).unwrap()
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
