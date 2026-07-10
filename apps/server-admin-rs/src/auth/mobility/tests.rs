use super::*;

#[test]
fn auth_credential_settings_runtime_normalizes_like_node() {
    let settings = AuthCredentialSettings::from_config(&json!({
        "subdomain_mode": { "auto_add_whitelist_on_login": false },
        "auth_credential_settings": {
            "session_ttl_seconds": "59.8",
            "remember_me_ttl_seconds": "10",
            "post_login_ip_grant_ttl_seconds": "10",
            "session_ip_mobility_window_seconds": "90000"
        }
    }));
    assert_eq!(settings.session_ttl_seconds, 60);
    assert_eq!(settings.remember_me_ttl_seconds, 60);
    assert_eq!(settings.post_login_ip_grant_mode, "disabled");
    assert_eq!(
        settings.post_login_ip_grant_ttl_seconds,
        DEFAULT_POST_LOGIN_IP_GRANT_TTL_SECONDS
    );
    assert_eq!(settings.session_ip_mobility_window_seconds, 86_400);

    let custom = AuthCredentialSettings::from_raw(&json!({
        "session_ttl_seconds": 120,
        "remember_me_ttl_seconds": 240,
        "post_login_ip_grant_mode": "custom",
        "post_login_ip_grant_ttl_seconds": 3.5,
        "session_ip_mobility_window_seconds": "30"
    }));
    assert_eq!(custom.post_login_ip_grant_mode, "custom");
    assert_eq!(custom.post_login_ip_grant_ttl_seconds, 60);
    assert_eq!(custom.session_ip_mobility_window_seconds, 60);
}

#[test]
fn mobility_binding_builder_preserves_and_clears_node_fields() {
    let original = json!({
        "createdAt": "2026-01-01T00:00:00.000Z",
        "ownerSessionId": "old-session",
        "whitelistRecordId": "old-whitelist",
        "custom": true
    });

    let owned = build_or_update_mobility_binding(
        Some(original),
        "fnos-token",
        "secret-token",
        "203.0.113.10",
        Some(1_800_000_000),
        Some("session-1"),
        Some("whitelist-1".to_string()),
    );

    assert_eq!(owned.get("version").and_then(Value::as_i64), Some(1));
    assert_eq!(
        owned.get("subjectType").and_then(Value::as_str),
        Some("fnos-token")
    );
    let expected_hash = auth_mobility_subject_hash("fnos-token", "secret-token");
    assert_eq!(
        owned.get("subjectHash").and_then(Value::as_str),
        Some(expected_hash.as_str())
    );
    assert_eq!(
        owned.get("currentIp").and_then(Value::as_str),
        Some("203.0.113.10")
    );
    assert_eq!(
        owned.get("expireAt").and_then(Value::as_i64),
        Some(1_800_000_000)
    );
    assert_eq!(
        owned.get("ownerSessionId").and_then(Value::as_str),
        Some("session-1")
    );
    assert_eq!(
        owned.get("whitelistRecordId").and_then(Value::as_str),
        Some("whitelist-1")
    );
    assert_eq!(
        owned.get("createdAt").and_then(Value::as_str),
        Some("2026-01-01T00:00:00.000Z")
    );
    assert_eq!(owned.get("custom").and_then(Value::as_bool), Some(true));
    assert!(owned.get("lastSeenAt").and_then(Value::as_str).is_some());

    let cleared = build_or_update_mobility_binding(
        Some(owned),
        "fnos-token",
        "secret-token",
        "203.0.113.11",
        None,
        None,
        None,
    );
    assert!(cleared.get("ownerSessionId").is_none());
    assert!(cleared.get("whitelistRecordId").is_none());
    assert!(cleared.get("expireAt").is_some_and(Value::is_null));
    assert_eq!(
        cleared.get("createdAt").and_then(Value::as_str),
        Some("2026-01-01T00:00:00.000Z")
    );

    let mut orphaned = cleared;
    clear_binding_owner_session(&mut orphaned);
    set_binding_last_seen(&mut orphaned);
    assert!(orphaned.get("ownerSessionId").is_none());
    assert!(orphaned.get("lastSeenAt").and_then(Value::as_str).is_some());
}

#[test]
fn mobility_touch_debounce_requires_same_live_owner_and_ip() {
    let binding = json!({
        "ownerSessionId": "session-1",
        "currentIp": "203.0.113.10",
        "lastSeenAt": "2027-01-15T08:00:00Z"
    });
    let last_seen = parse_iso_unix(binding.get("lastSeenAt").and_then(Value::as_str))
        .expect("valid test timestamp");

    assert!(mobility_touch_is_fresh(last_seen, last_seen + 29, 30));
    assert!(!mobility_touch_is_fresh(last_seen, last_seen + 30, 30));
    assert!(!mobility_touch_is_fresh(last_seen, last_seen - 1, 30));
    assert!(mobility_binding_touch_is_fresh(
        &binding,
        "203.0.113.10",
        "session-1",
        last_seen + 29,
    ));
    assert!(!mobility_binding_touch_is_fresh(
        &binding,
        "203.0.113.11",
        "session-1",
        last_seen + 29,
    ));
    assert!(!mobility_binding_touch_is_fresh(
        &binding,
        "203.0.113.10",
        "revoked-owner",
        last_seen + 29,
    ));
}

#[test]
fn active_ip_parser_exposes_persisted_last_seen_for_debounce() {
    let parsed = parse_active_ip_detail(json!({
        "ip": "203.0.113.10",
        "firstSeenAt": 100,
        "lastSeenAt": 120,
        "whitelistRecordId": "whitelist-1"
    }))
    .expect("active IP detail");

    assert_eq!(parsed.first_seen_at, 100);
    assert_eq!(parsed.last_seen_at, 120);
    assert_eq!(parsed.whitelist_record_id.as_deref(), Some("whitelist-1"));
}

#[test]
fn session_ip_match_uses_normalized_addresses_and_rejects_empty_values() {
    let mut session = test_browser_session("[2001:db8::10]");
    assert!(session_ip_matches(&session, "2001:db8::10"));
    assert!(!session_ip_matches(&session, "2001:db8::11"));

    session.ip.clear();
    assert!(!session_ip_matches(&session, ""));
}

#[tokio::test]
async fn repeated_same_session_ip_does_not_commit_storage_writes() {
    let directory = tempfile::tempdir().expect("temporary auth database");
    let sqlite_path = directory.path().join("fn-knock.sqlite3");
    let mut settings = {
        let _environment = crate::test_support::EnvGuard::new(&[]);
        crate::settings::Settings::from_env()
    };
    settings.data_dir = directory.path().join("data");
    settings.gateway_config_dir = directory.path().join("gateway");
    settings.sqlite_path = sqlite_path.clone();
    settings.legacy_redis_url = String::new();
    settings.internal_rpc_token = "auth-hotpath-test".to_string();
    let state = AppState::new(settings).await.expect("auth test state");
    state
        .store
        .save_config(&json!({
            "auth_credential_settings": {
                "session_ip_mobility_enabled": true,
                "session_ip_mobility_window_seconds": 1200
            }
        }))
        .await
        .expect("mobility config");

    let mut session = test_browser_session("203.0.113.10");
    session.ip_location = Some("test-location".to_string());
    session.expires_at = Some(time_utils::iso_after_seconds(3600));
    state
        .store
        .add_session("session-1", &session, 3600)
        .await
        .expect("session");

    let updated = sync_browser_session_ip_with_session(
        &state,
        "session-1",
        &session,
        "203.0.113.10",
        "browser-session",
    )
    .await
    .expect("initial active-IP touch")
    .expect("session remains live");
    let observer = tokio_rusqlite::Connection::open(&sqlite_path)
        .await
        .expect("observer connection");
    let before = sqlite_data_version(&observer).await;

    for _ in 0..32 {
        let result = sync_browser_session_ip_with_session(
            &state,
            "session-1",
            &updated,
            "203.0.113.10",
            "browser-session",
        )
        .await
        .expect("coalesced active-IP touch");
        assert!(result.is_some());
    }

    assert_eq!(sqlite_data_version(&observer).await, before);

    let mut control_update = Map::new();
    control_update.insert(
        "userAgent".to_string(),
        Value::String("observer-control-write".to_string()),
    );
    state
        .store
        .update_session_value("session-1", control_update)
        .await
        .expect("control storage write");
    assert_ne!(sqlite_data_version(&observer).await, before);
}

#[tokio::test]
async fn revoked_borrowed_session_cannot_recreate_active_ip_or_whitelist() {
    let directory = tempfile::tempdir().expect("temporary auth database");
    let mut settings = {
        let _environment = crate::test_support::EnvGuard::new(&[]);
        crate::settings::Settings::from_env()
    };
    settings.data_dir = directory.path().join("data");
    settings.gateway_config_dir = directory.path().join("gateway");
    settings.sqlite_path = directory.path().join("fn-knock.sqlite3");
    settings.legacy_redis_url = String::new();
    settings.internal_rpc_token = "auth-revocation-race-test".to_string();
    let state = AppState::new(settings).await.expect("auth test state");
    state
        .store
        .save_config(&json!({
            "auth_credential_settings": {
                "session_ip_mobility_enabled": true,
                "session_ip_mobility_window_seconds": 1200,
                "post_login_ip_grant_mode": "follow_session"
            }
        }))
        .await
        .expect("mobility config");

    let mut stale_session = test_browser_session("203.0.113.10");
    stale_session.grant_type = Some("login_ip_grant".to_string());
    stale_session.post_login_ip_grant_mode = Some("follow_session".to_string());
    stale_session.comment = Some("Automatically authorized after sign-in".to_string());
    state
        .store
        .add_session("revoked-session", &stale_session, 3600)
        .await
        .expect("session");

    // Models logout after preflight captured stale_session but before activity
    // synchronization reaches its write path.
    destroy_session(&state, "revoked-session")
        .await
        .expect("revoke session");
    let result = sync_browser_session_ip_with_session(
        &state,
        "revoked-session",
        &stale_session,
        "203.0.113.10",
        "browser-session",
    )
    .await
    .expect("revoked sync is handled");

    assert!(result.is_none());
    assert!(
        state
            .store
            .get_session("revoked-session")
            .await
            .expect("session lookup")
            .is_none()
    );
    assert!(
        state
            .store
            .get_auth_mobility_active_ip_detail("revoked-session", "203.0.113.10")
            .await
            .expect("active IP lookup")
            .is_none()
    );
    assert!(
        state
            .store
            .list_whitelist_records()
            .await
            .expect("whitelist lookup")
            .is_empty()
    );

    state
        .store
        .add_session("revoked-update-session", &stale_session, 3600)
        .await
        .expect("second session");
    destroy_session(&state, "revoked-update-session")
        .await
        .expect("revoke second session");
    let update_result = sync_browser_session_ip_with_session(
        &state,
        "revoked-update-session",
        &stale_session,
        "203.0.113.11",
        "browser-session",
    )
    .await
    .expect("revoked update is handled");
    assert!(update_result.is_none());
    assert!(
        state
            .store
            .get_auth_mobility_active_ip_detail("revoked-update-session", "203.0.113.11")
            .await
            .expect("second active IP lookup")
            .is_none()
    );
}

#[tokio::test]
async fn concurrent_first_fragments_create_one_follow_session_whitelist() {
    let (_directory, state) = mobility_test_state("first-fragment-race").await;
    let session_id = "first-fragment-session";
    let client_ip = "203.0.113.40";
    let mut session = test_browser_session(client_ip);
    session.grant_type = Some("login_ip_grant".to_string());
    session.post_login_ip_grant_mode = Some("follow_session".to_string());
    session.comment = Some("Automatically authorized after sign-in".to_string());
    state
        .store
        .add_session(session_id, &session, 3600)
        .await
        .expect("session");

    const FRAGMENTS: usize = 12;
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(FRAGMENTS + 1));
    let mut tasks = Vec::new();
    for _ in 0..FRAGMENTS {
        let task_state = state.clone();
        let task_barrier = std::sync::Arc::clone(&barrier);
        tasks.push(tokio::spawn(async move {
            task_barrier.wait().await;
            sync_browser_session_ip(
                &task_state,
                session_id,
                client_ip,
                "concurrent-first-fragment",
            )
            .await
        }));
    }
    barrier.wait().await;
    for task in tasks {
        assert!(
            task.await
                .expect("fragment task")
                .expect("fragment sync")
                .is_some()
        );
    }

    let records = state
        .store
        .list_whitelist_records()
        .await
        .expect("whitelist records");
    assert_eq!(records.len(), 1, "parallel first use leaked an orphan");
    let detail = state
        .store
        .get_auth_mobility_active_ip_detail(session_id, client_ip)
        .await
        .expect("active detail")
        .expect("active detail exists");
    assert_eq!(
        detail.get("whitelistRecordId").and_then(Value::as_str),
        Some(records[0].id.as_str())
    );
}

#[tokio::test]
async fn concurrent_logout_is_a_barrier_for_active_whitelist_and_bindings() {
    let (_directory, state) = mobility_test_state("logout-race").await;
    let session_id = "logout-race-session";
    let initial_ip = "203.0.113.50";
    let mut session = test_browser_session(initial_ip);
    session.grant_type = Some("login_ip_grant".to_string());
    session.post_login_ip_grant_mode = Some("follow_session".to_string());
    session.comment = Some("Automatically authorized after sign-in".to_string());
    state
        .store
        .add_session(session_id, &session, 3600)
        .await
        .expect("session");
    sync_browser_session_ip(&state, session_id, initial_ip, "seed")
        .await
        .expect("seed sync")
        .expect("live seed session");
    let binding = build_or_update_mobility_binding(
        None,
        "fnos-token",
        "logout-race-token",
        initial_ip,
        parse_iso_unix(session.expires_at.as_deref()),
        Some(session_id),
        None,
    );
    assert!(
        state
            .store
            .save_auth_mobility_owned_binding(
                "fnos-token",
                "logout-race-token",
                &binding,
                session_id,
                3600,
                Some(3600),
            )
            .await
            .expect("seed binding")
    );

    const WRITERS: usize = 8;
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(WRITERS + 2));
    let mut writers = Vec::new();
    for index in 0..WRITERS {
        let task_state = state.clone();
        let task_barrier = std::sync::Arc::clone(&barrier);
        writers.push(tokio::spawn(async move {
            let ip = format!("203.0.113.{}", 60 + index);
            task_barrier.wait().await;
            sync_browser_session_ip(&task_state, session_id, &ip, "logout-race").await
        }));
    }
    let destroy_state = state.clone();
    let destroy_barrier = std::sync::Arc::clone(&barrier);
    let destroyer = tokio::spawn(async move {
        destroy_barrier.wait().await;
        destroy_session(&destroy_state, session_id).await
    });
    barrier.wait().await;
    for writer in writers {
        writer.await.expect("writer task").expect("writer result");
    }
    destroyer
        .await
        .expect("destroy task")
        .expect("destroy result");

    assert!(state.store.get_session(session_id).await.unwrap().is_none());
    assert!(
        state
            .store
            .list_auth_mobility_active_ip_details(session_id)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        state
            .store
            .list_whitelist_records()
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        state
            .store
            .get_auth_mobility_binding("fnos-token", "logout-race-token")
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        state
            .store
            .list_auth_mobility_session_binding_keys(session_id)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn logout_collects_whitelist_left_pending_before_publication() {
    let (_directory, state) = mobility_test_state("pending-whitelist-cleanup").await;
    let session_id = "pending-whitelist-session";
    let client_ip = "203.0.113.90";
    let session = test_browser_session(client_ip);
    state
        .store
        .add_session(session_id, &session, 3600)
        .await
        .expect("session");

    let owner_key = format!("auth-mobility:active-ip:{session_id}:{client_ip}");
    let owner_record_key = crate::whitelist::whitelist_auto_owner_record_key(&owner_key);
    let record_id = "whitelist:pending-crash";
    assert!(
        state
            .store
            .add_auth_mobility_pending_whitelist(session_id, record_id, &owner_record_key, 3600,)
            .await
            .expect("pending reverse index")
    );
    let deferred = crate::whitelist::ensure_pending_session_auto_whitelist(
        &state,
        &owner_key,
        client_ip,
        Some(now_seconds() + 3600),
        Some("pending".to_string()),
        None,
        record_id,
    )
    .await
    .expect("pending record");
    assert_eq!(deferred.record.status, "pending");
    assert_eq!(
        state
            .store
            .get_whitelist_record(record_id)
            .await
            .unwrap()
            .expect("stored pending record")
            .status,
        "pending"
    );

    destroy_session(&state, session_id)
        .await
        .expect("destroy pending session");
    assert!(
        state
            .store
            .get_whitelist_record(record_id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        state
            .store
            .get_string_value(&owner_record_key)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn custom_login_whitelist_is_session_indexed_and_revoked() {
    let (_directory, state) = mobility_test_state("custom-login-index").await;
    let mut config = state.store.get_config().await.expect("current config");
    config.as_object_mut().expect("config object").insert(
        "auth_credential_settings".to_string(),
        json!({
            "session_ip_mobility_enabled": false,
            "session_ttl_seconds": 3600,
            "remember_me_ttl_seconds": 3600,
            "post_login_ip_grant_mode": "custom",
            "post_login_ip_grant_ttl_seconds": 600
        }),
    );
    state
        .store
        .save_config(&config)
        .await
        .expect("custom config");
    let created = create_login_session(
        &state,
        &config,
        CreateLoginSessionInput {
            auth_method: "TOTP".to_string(),
            auth_provider_name: None,
            credential_id: "totp-1".to_string(),
            credential_name: "TOTP".to_string(),
            totp_id: "totp-1".to_string(),
            linked_totp_name: None,
            totp_credential: None,
            client_ip: "203.0.113.91".to_string(),
            user_agent: "login-test".to_string(),
            remember_me: false,
        },
    )
    .await
    .expect("custom login");
    let record_id = created
        .whitelist_record_id
        .as_deref()
        .expect("custom whitelist ID");
    assert_eq!(
        state
            .store
            .get_whitelist_record(record_id)
            .await
            .unwrap()
            .expect("custom whitelist")
            .status,
        "active"
    );

    destroy_session(&state, &created.session_id)
        .await
        .expect("destroy custom login");
    assert!(
        state
            .store
            .get_whitelist_record(record_id)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn disabled_mobility_same_ip_keeps_session_migration_grant_without_writes() {
    let directory = tempfile::tempdir().expect("temporary auth database");
    let sqlite_path = directory.path().join("fn-knock.sqlite3");
    let mut settings = {
        let _environment = crate::test_support::EnvGuard::new(&[]);
        crate::settings::Settings::from_env()
    };
    settings.data_dir = directory.path().join("data");
    settings.gateway_config_dir = directory.path().join("gateway");
    settings.sqlite_path = sqlite_path.clone();
    settings.legacy_redis_url = String::new();
    settings.internal_rpc_token = "auth-restore-compat-test".to_string();
    let state = AppState::new(settings).await.expect("auth test state");
    state
        .store
        .save_config(&json!({
            "auth_credential_settings": {
                "session_ip_mobility_enabled": false
            }
        }))
        .await
        .expect("mobility config");

    let session_id = "same-ip-session";
    let client_ip = "203.0.113.10";
    let session = test_browser_session(client_ip);
    state
        .store
        .add_session(session_id, &session, 3600)
        .await
        .expect("session");
    let whitelist_record = crate::store::WhitelistRecord {
        id: "same-ip-whitelist".to_string(),
        ip: client_ip.to_string(),
        target_type: "ip".to_string(),
        expire_at: Some(now_seconds() + 3600),
        source: "auto".to_string(),
        created_at: now_seconds(),
        status: "active".to_string(),
        comment: None,
        ip_location: None,
        resolved_targets: None,
        check_interval_minutes: None,
        last_checked_at: None,
        last_resolved_at: None,
        resolve_status: None,
        resolve_message: None,
    };
    state
        .store
        .insert_whitelist_record(&whitelist_record)
        .await
        .expect("whitelist");
    let binding = build_or_update_mobility_binding(
        None,
        "proxy-session",
        session_id,
        client_ip,
        parse_iso_unix(session.expires_at.as_deref()),
        Some(session_id),
        Some(whitelist_record.id.clone()),
    );
    state
        .store
        .save_auth_mobility_binding_with_ttl("proxy-session", session_id, &binding, 3600)
        .await
        .expect("binding");

    let observer = tokio_rusqlite::Connection::open(&sqlite_path)
        .await
        .expect("observer connection");
    let before = sqlite_data_version(&observer).await;
    let restored = try_restore_access(
        &state,
        client_ip,
        AuthMobilityRestoreIdentity {
            session_id: Some(session_id),
            ..Default::default()
        },
    )
    .await
    .expect("same-IP restore");

    assert!(restored.success);
    assert_eq!(restored.grant_type, Some("session_migration"));
    assert_eq!(sqlite_data_version(&observer).await, before);
}

fn test_browser_session(ip: &str) -> LoginSession {
    LoginSession {
        totp_id: "totp-1".to_string(),
        method: "TOTP".to_string(),
        credential_id: "totp-1".to_string(),
        credential_name: "TOTP".to_string(),
        linked_totp_name: None,
        access_scopes: None,
        subdomain_access: None,
        grant_type: Some("browser_session".to_string()),
        post_login_ip_grant_mode: None,
        post_login_ip_grant_record_id: None,
        comment: None,
        ip: ip.to_string(),
        user_agent: "video-player".to_string(),
        login_time: time_utils::now_iso(),
        expires_at: Some(time_utils::iso_after_seconds(3600)),
        ip_location: None,
    }
}

async fn mobility_test_state(name: &str) -> (tempfile::TempDir, AppState) {
    let directory = tempfile::tempdir().expect("temporary auth database");
    let mut settings = {
        let _environment = crate::test_support::EnvGuard::new(&[]);
        crate::settings::Settings::from_env()
    };
    settings.data_dir = directory.path().join("data");
    settings.gateway_config_dir = directory.path().join("gateway");
    settings.sqlite_path = directory.path().join("fn-knock.sqlite3");
    settings.legacy_redis_url = String::new();
    settings.internal_rpc_token = format!("auth-{name}-test");
    let state = AppState::new(settings).await.expect("auth test state");
    state
        .store
        .save_config(&json!({
            "auth_credential_settings": {
                "session_ip_mobility_enabled": true,
                "session_ip_mobility_window_seconds": 1200,
                "post_login_ip_grant_mode": "follow_session"
            }
        }))
        .await
        .expect("mobility config");
    (directory, state)
}

async fn sqlite_data_version(connection: &tokio_rusqlite::Connection) -> i64 {
    connection
        .call(|connection| {
            connection.query_row("PRAGMA data_version", [], |row| row.get::<_, i64>(0))
        })
        .await
        .expect("sqlite data_version")
}
