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
