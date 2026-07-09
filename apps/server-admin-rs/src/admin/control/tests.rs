use super::*;

#[test]
fn normalizes_auth_credential_settings_defaults() {
    let value = normalize_auth_credential_settings(json!({ "session_ttl_seconds": 60 }), None);
    assert_eq!(value["session_ttl_seconds"], 60);
    assert_eq!(value["post_login_ip_grant_mode"], "follow_session");
    assert!(value["post_login_ip_grant_ttl_seconds"].is_null());
    assert_eq!(value["passkey_bind_prompt_enabled"], true);
}

#[test]
fn normalizes_auth_credential_settings_like_node_clamps_and_nulls() {
    let value = normalize_auth_credential_settings(
        json!({
            "session_ttl_seconds": "59.9",
            "remember_me_ttl_seconds": "61.7",
            "post_login_ip_grant_mode": "follow_session",
            "post_login_ip_grant_ttl_seconds": "7200",
            "session_ip_mobility_window_seconds": 90_000
        }),
        None,
    );
    assert_eq!(value["session_ttl_seconds"], 60);
    assert_eq!(value["remember_me_ttl_seconds"], 61);
    assert!(value["post_login_ip_grant_ttl_seconds"].is_null());
    assert_eq!(value["session_ip_mobility_window_seconds"], 86_400);

    let custom = normalize_auth_credential_settings(
        json!({
            "session_ttl_seconds": 120,
            "remember_me_ttl_seconds": 60,
            "post_login_ip_grant_mode": "custom",
            "post_login_ip_grant_ttl_seconds": "10"
        }),
        None,
    );
    assert_eq!(custom["remember_me_ttl_seconds"], 120);
    assert_eq!(custom["post_login_ip_grant_ttl_seconds"], 60);
}

#[test]
fn normalizes_auth_credential_settings_legacy_auto_add_flag_like_node() {
    let value = normalize_auth_credential_settings(json!({}), Some(false));
    assert_eq!(value["post_login_ip_grant_mode"], "disabled");

    let explicit = normalize_auth_credential_settings(
        json!({ "post_login_ip_grant_mode": "follow_session" }),
        Some(false),
    );
    assert_eq!(explicit["post_login_ip_grant_mode"], "follow_session");
}

#[test]
fn import_plan_skips_duplicate_totp_credentials() {
    let existing = vec![TotpCredential {
        id: "a".to_string(),
        secret: "AAAA".to_string(),
        comment: String::new(),
        created_at: time_utils::now_iso(),
        access_scopes: Value::Array(Vec::new()),
        subdomain_access: json!({ "mode": "all", "hosts": [] }),
    }];
    let payload = json!({
        "kind": TOTP_TRANSFER_KIND,
        "version": TOTP_TRANSFER_VERSION,
        "credentials": [
            { "id": "a", "secret": "BBBB" },
            { "id": "b", "secret": "AAAA" },
            { "id": "b", "secret": "CCCC" },
            { "id": "c", "secret": "CCCC" }
        ]
    });
    let (credentials, summary) = build_totp_import_plan(&existing, &payload).unwrap();
    assert_eq!(credentials.len(), 1);
    assert_eq!(credentials[0].id, "c");
    assert_eq!(summary["skipped_existing_id"], 1);
    assert_eq!(summary["skipped_existing_secret"], 1);
}

#[test]
fn totp_bind_comment_matches_node_truthy_fallback() {
    assert_eq!(node_totp_bind_comment(None), "New Token");
    assert_eq!(node_totp_bind_comment(Some(String::new())), "New Token");
    assert_eq!(
        node_totp_bind_comment(Some("   ".to_string())),
        "   ".to_string()
    );
}

#[test]
fn import_plan_normalizes_totp_metadata_like_node() {
    let payload = json!({
        "kind": TOTP_TRANSFER_KIND,
        "version": TOTP_TRANSFER_VERSION,
        "credentials": [
            {
                "id": " imported ",
                "secret": " SECRET ",
                "comment": " Comment ",
                "createdAt": "not-a-date",
                "access_scopes": [" docker_admin_panel ", "other", "docker_admin_panel"],
                "subdomain_access": {
                    "mode": "custom",
                    "hosts": ["https://Example.com:8443/path", "/__select__", "bad host"]
                }
            }
        ]
    });

    let (credentials, summary) = build_totp_import_plan(&[], &payload).unwrap();
    assert_eq!(summary["imported"], 1);
    assert_eq!(credentials.len(), 1);
    let credential = &credentials[0];
    assert_eq!(credential.id, "imported");
    assert_eq!(credential.secret, "SECRET");
    assert_eq!(credential.comment, "Comment");
    assert!(time_utils::parse_iso_ms(&credential.created_at).is_some());
    assert_eq!(credential.access_scopes, json!(["docker_admin_panel"]));
    assert_eq!(
        credential.subdomain_access,
        json!({
            "mode": "custom",
            "hosts": ["__builtin_select__", "example.com"]
        })
    );
}

#[test]
fn export_payload_normalizes_totp_metadata_like_node() {
    let payload = build_totp_export_payload(
        &[TotpCredential {
            id: " id ".to_string(),
            secret: " SECRET ".to_string(),
            comment: " comment ".to_string(),
            created_at: "not-a-date".to_string(),
            access_scopes: json!(["docker_admin_panel", "unknown", "docker_admin_panel"]),
            subdomain_access: json!({
                "mode": "custom",
                "hosts": [
                    " HTTPS://Example.COM:443/path ",
                    "*bad.example",
                    "__builtin_select__"
                ]
            }),
        }],
        "2026-01-02T03:04:05.000Z",
    );
    assert_eq!(payload["kind"], TOTP_TRANSFER_KIND);
    assert_eq!(payload["version"], TOTP_TRANSFER_VERSION);
    assert_eq!(payload["login_mode"], "totp");
    assert_eq!(
        payload["credentials"][0],
        json!({
            "id": "id",
            "secret": "SECRET",
            "comment": "comment",
            "createdAt": "2026-01-02T03:04:05.000Z",
            "access_scopes": ["docker_admin_panel"],
            "subdomain_access": {
                "mode": "custom",
                "hosts": ["__builtin_select__", "example.com"]
            }
        })
    );
}

#[test]
fn export_payload_includes_password_mode_credentials() {
    let hash = "ab".repeat(64);
    let payload = build_password_export_payload(
        &[AuthAccount {
            id: " account-1 ".to_string(),
            username: "alice".to_string(),
            display_name: "Alice".to_string(),
            source_totp_id: "totp-1".to_string(),
            created_at: "not-a-date".to_string(),
            updated_at: "2026-01-02T03:04:05.000Z".to_string(),
            access_scopes: json!(["docker_admin_panel", "unknown"]),
            subdomain_access: json!({ "mode": "all", "hosts": ["ignored.example"] }),
        }],
        &[AuthPasswordCredential {
            account_id: "account-1".to_string(),
            algorithm: "scrypt".to_string(),
            salt: "00112233445566778899aabbccddeeff".to_string(),
            hash,
            n: 16_384,
            r: 8,
            p: 1,
            key_length: 64,
            created_at: "bad-date".to_string(),
            updated_at: "2026-01-03T03:04:05.000Z".to_string(),
        }],
        &[TotpCredential {
            id: "totp-1".to_string(),
            secret: " SECRET ".to_string(),
            comment: " Alice ".to_string(),
            created_at: "bad-date".to_string(),
            access_scopes: json!(["docker_admin_panel"]),
            subdomain_access: json!({ "mode": "all", "hosts": [] }),
        }],
        "2026-01-02T03:04:05.000Z",
    );

    assert_eq!(payload["kind"], PASSWORD_TRANSFER_KIND);
    assert_eq!(payload["version"], PASSWORD_TRANSFER_VERSION);
    assert_eq!(payload["login_mode"], "password");
    assert_eq!(payload["accounts"][0]["id"], "account-1");
    assert_eq!(payload["accounts"][0]["username"], "alice");
    assert_eq!(payload["accounts"][0]["sourceTotpId"], "totp-1");
    assert_eq!(
        payload["accounts"][0]["access_scopes"],
        json!(["docker_admin_panel"])
    );
    assert_eq!(payload["password_credentials"][0]["accountId"], "account-1");
    assert_eq!(payload["password_credentials"][0]["algorithm"], "scrypt");
    assert_eq!(payload["totp_credentials"][0]["id"], "totp-1");
    assert_eq!(payload["totp_credentials"][0]["secret"], "SECRET");
}

#[test]
fn password_import_plan_merges_accounts_passwords_and_linked_totps() {
    let existing_accounts = vec![AuthAccount {
        id: "existing-account".to_string(),
        username: "taken".to_string(),
        display_name: "taken".to_string(),
        source_totp_id: "existing-totp".to_string(),
        created_at: "2026-01-01T00:00:00.000Z".to_string(),
        updated_at: "2026-01-01T00:00:00.000Z".to_string(),
        access_scopes: Value::Array(Vec::new()),
        subdomain_access: json!({ "mode": "all", "hosts": [] }),
    }];
    let existing_totps = vec![TotpCredential {
        id: "existing-totp".to_string(),
        secret: "EXISTING".to_string(),
        comment: "taken".to_string(),
        created_at: "2026-01-01T00:00:00.000Z".to_string(),
        access_scopes: Value::Array(Vec::new()),
        subdomain_access: json!({ "mode": "all", "hosts": [] }),
    }];
    let hash = "ab".repeat(64);
    let payload = json!({
        "kind": PASSWORD_TRANSFER_KIND,
        "version": PASSWORD_TRANSFER_VERSION,
        "accounts": [
            { "id": "existing-account", "username": "other" },
            { "id": "new-username-conflict", "username": "TAKEN" },
            {
                "id": "new-account",
                "username": "Alice",
                "sourceTotpId": "new-totp",
                "createdAt": "bad-date",
                "access_scopes": ["docker_admin_panel", "nope"],
                "subdomain_access": {
                    "mode": "custom",
                    "hosts": ["https://Example.com:8443/path"]
                }
            },
            { "id": "invalid-account", "username": "x" }
        ],
        "password_credentials": [
            {
                "accountId": "new-account",
                "algorithm": "scrypt",
                "salt": "00112233445566778899aabbccddeeff",
                "hash": hash,
                "n": 16384,
                "r": 8,
                "p": 1,
                "key_length": 64,
                "created_at": "bad-date"
            },
            {
                "accountId": "missing-account",
                "algorithm": "scrypt",
                "salt": "00112233445566778899aabbccddeeff",
                "hash": "ab".repeat(64),
                "n": 16384,
                "r": 8,
                "p": 1,
                "key_length": 64
            }
        ],
        "totp_credentials": [
            { "id": "new-totp", "secret": " NEWSECRET ", "comment": " Alice " }
        ]
    });

    let plan = build_credential_import_plan(
        &existing_totps,
        &existing_accounts,
        &HashSet::from(["existing-account".to_string()]),
        &payload,
    )
    .unwrap();
    let CredentialImportPlan::Password(plan) = plan else {
        panic!("expected password import plan");
    };

    assert_eq!(plan.accounts.len(), 1);
    assert_eq!(plan.accounts[0].id, "new-account");
    assert_eq!(plan.accounts[0].username, "alice");
    assert_eq!(plan.accounts[0].source_totp_id, "new-totp");
    assert_eq!(
        plan.accounts[0].access_scopes,
        json!(["docker_admin_panel"])
    );
    assert_eq!(
        plan.accounts[0].subdomain_access,
        json!({ "mode": "custom", "hosts": ["example.com"] })
    );
    assert_eq!(plan.password_credentials.len(), 1);
    assert_eq!(plan.password_credentials[0].account_id, "new-account");
    assert_eq!(plan.totp_credentials.len(), 1);
    assert_eq!(plan.totp_credentials[0].id, "new-totp");
    assert_eq!(plan.totp_credentials[0].secret, "NEWSECRET");
    assert_eq!(plan.summary["imported"], 1);
    assert_eq!(plan.summary["skipped_existing_id"], 1);
    assert_eq!(plan.summary["skipped_existing_username"], 1);
    assert_eq!(plan.summary["invalid"], 1);
    assert_eq!(plan.summary["password_imported"], 1);
    assert_eq!(plan.summary["password_skipped_missing_account"], 1);
    assert_eq!(plan.summary["totp_imported"], 1);
}

#[test]
fn password_import_plan_rejects_unsupported_hash_parameters() {
    let payload = json!({
        "kind": PASSWORD_TRANSFER_KIND,
        "version": PASSWORD_TRANSFER_VERSION,
        "accounts": [
            { "id": "new-account", "username": "alice" }
        ],
        "password_credentials": [
            {
                "accountId": "new-account",
                "algorithm": "scrypt",
                "salt": "00112233445566778899aabbccddeeff",
                "hash": "ab".repeat(64),
                "n": 1048576,
                "r": 8,
                "p": 1,
                "key_length": 64
            },
            {
                "accountId": "new-account-2",
                "algorithm": "scrypt",
                "salt": "00112233445566778899aabbccddeeff",
                "hash": "abcdef",
                "n": 16384,
                "r": 8,
                "p": 1,
                "key_length": 64
            }
        ]
    });

    let plan = build_credential_import_plan(&[], &[], &HashSet::new(), &payload).unwrap();
    let CredentialImportPlan::Password(plan) = plan else {
        panic!("expected password import plan");
    };

    assert_eq!(plan.accounts.len(), 1);
    assert!(plan.password_credentials.is_empty());
    assert_eq!(plan.summary["password_invalid"], 2);
}

#[test]
fn builds_mobility_login_event_from_session() {
    let event = build_mobility_login_event(&json!({
        "ip": "203.0.113.8",
        "ipLocation": "Test City",
        "loginTime": "2026-07-05T01:02:03Z"
    }))
    .unwrap();
    assert_eq!(event["kind"], "login");
    assert_eq!(event["source"], "login");
    assert_eq!(event["toIp"], "203.0.113.8");
    assert_eq!(event["toIpLocation"], "Test City");
    assert_eq!(event["happenedAt"], "2026-07-05T01:02:03Z");
}

#[test]
fn applies_cached_mobility_event_locations_like_node() {
    let mut events = vec![json!({
        "kind": "drift",
        "toIp": " 203.0.113.8 ",
        "fromIp": "2001:db8::1",
        "toIpLocation": "old"
    })];
    let locations = BTreeMap::from([
        ("203.0.113.8".to_string(), "Tokyo".to_string()),
        ("2001:db8::1".to_string(), "Seoul".to_string()),
    ]);

    apply_mobility_event_ip_locations(&mut events, &locations);

    assert_eq!(events[0]["toIpLocation"], "Tokyo");
    assert_eq!(events[0]["fromIpLocation"], "Seoul");
}

#[test]
fn builds_mobility_summary_from_drift_events() {
    let summary = build_mobility_summary(&[
        json!({ "kind": "login", "happenedAt": "2026-07-05T01:00:00Z" }),
        json!({ "kind": "drift", "source": "proxy-session", "happenedAt": "2026-07-05T01:10:00Z" }),
        json!({ "kind": "drift", "source": "session-refresh", "happenedAt": "2026-07-05T01:20:00Z" }),
    ]);
    assert_eq!(summary["hasHistory"], true);
    assert_eq!(summary["driftCount"], 2);
    assert_eq!(summary["lastDriftAt"], "2026-07-05T01:20:00Z");
    assert_eq!(summary["lastDriftSource"], "session-refresh");
}

#[test]
fn builds_session_attachment_from_binding_like_node() {
    let attachment = session_attachment_from_binding(
        &json!({
            "subjectType": "fnos-token",
            "subjectHash": "hash-1",
            "currentIp": "203.0.113.8",
            "createdAt": "2026-07-05T01:00:00Z",
            "lastSeenAt": "2026-07-05T01:20:00Z",
            "expireAt": 1783213200,
            "ownerSessionId": "session-1"
        }),
        "session-1",
        "fnos-token",
    )
    .unwrap();

    assert_eq!(attachment["subjectHash"], "hash-1");
    assert_eq!(attachment["currentIp"], "203.0.113.8");
    assert_eq!(attachment["createdAt"], "2026-07-05T01:00:00Z");
    assert_eq!(attachment["lastSeenAt"], "2026-07-05T01:20:00Z");
    assert_eq!(attachment["expiresAt"], "2026-07-05T01:00:00Z");
}

#[test]
fn rejects_stale_session_attachment_bindings_like_node() {
    assert!(
        session_attachment_from_binding(
            &json!({
                "subjectType": "fnos-token",
                "ownerSessionId": "other-session"
            }),
            "session-1",
            "fnos-token",
        )
        .is_none()
    );
    assert!(
        session_attachment_from_binding(
            &json!({
                "subjectType": "trim-media-token",
                "ownerSessionId": "session-1"
            }),
            "session-1",
            "fnos-token",
        )
        .is_none()
    );
}

#[test]
fn normalizes_auto_ip_grant_comment_like_node() {
    let translator = Translator::new("zh-CN");
    assert_eq!(
        normalize_auto_ip_grant_comment_value(
            Some(" Automatically authorized after sign-in "),
            &translator,
        ),
        "登录后自动授权"
    );
    assert_eq!(
        normalize_auto_ip_grant_comment_value(Some(" custom note "), &translator),
        "custom note"
    );
    assert_eq!(
        normalize_auto_ip_grant_comment_value(Some("   "), &translator),
        ""
    );
}

#[test]
fn custom_post_login_grant_revoke_condition_matches_node() {
    let mut session = LoginSession {
        totp_id: "totp".to_string(),
        method: "TOTP".to_string(),
        credential_id: "cred".to_string(),
        credential_name: "Credential".to_string(),
        linked_totp_name: None,
        access_scopes: None,
        subdomain_access: None,
        grant_type: Some("login_ip_grant".to_string()),
        post_login_ip_grant_mode: Some("custom".to_string()),
        post_login_ip_grant_record_id: None,
        comment: None,
        ip: "203.0.113.8".to_string(),
        user_agent: "test".to_string(),
        login_time: "2026-07-05T01:00:00Z".to_string(),
        expires_at: None,
        ip_location: None,
    };
    assert!(should_revoke_custom_post_login_ip_grant(
        &session,
        &json!({"auth_credential_settings": {"post_login_ip_grant_mode": "follow_session"}})
    ));

    session.grant_type = Some("session".to_string());
    session.post_login_ip_grant_mode = Some("follow_session".to_string());
    session.comment = Some("登录后自动授权".to_string());
    assert!(should_revoke_custom_post_login_ip_grant(
        &session,
        &json!({"auth_credential_settings": {"post_login_ip_grant_mode": "custom"}})
    ));
    assert!(!should_revoke_custom_post_login_ip_grant(
        &session,
        &json!({"auth_credential_settings": {"post_login_ip_grant_mode": "follow_session"}})
    ));
}

#[test]
fn localizes_admin_control_route_text() {
    let translator = Translator::new("zh-CN");
    assert_eq!(
        admin_control_text(&translator, "authCredentialSettings.loadFailed"),
        "加载认证凭据配置失败"
    );
    assert_eq!(
        admin_control_text(&translator, "totp.notFound"),
        "TOTP 凭据不存在"
    );
    assert_eq!(
        admin_control_text(&translator, "passkeys.notFound"),
        "Passkey 不存在"
    );
    assert_eq!(
        admin_control_text(&translator, "sessions.notFound"),
        "会话不存在"
    );
    let error = totp_import_error_with_max(StatusCode::BAD_REQUEST, "countExceeded", 200);
    assert_eq!(
        totp_import_error_message(&translator, &error),
        "单次最多导入 200 个 TOTP 凭证"
    );
}
