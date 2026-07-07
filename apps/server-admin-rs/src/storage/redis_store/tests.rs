use super::*;

#[test]
fn sorts_backup_strings_like_node_locale_compare() {
    let mut values = [
        "fn_knock:a",
        "fn_knock:Z",
        "fn_knock:A",
        "fn_knock:z",
        "fn_knock:_",
        "fn_knock:-",
        "fn_knock:2",
        "fn_knock:10",
        "fn_knock:á",
        "fn_knock:ä",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<Vec<_>>();

    values.sort_by(|left, right| node_locale_compare_ordering(left, right));

    assert_eq!(
        values,
        vec![
            "fn_knock:_",
            "fn_knock:-",
            "fn_knock:10",
            "fn_knock:2",
            "fn_knock:a",
            "fn_knock:A",
            "fn_knock:á",
            "fn_knock:ä",
            "fn_knock:z",
            "fn_knock:Z",
        ]
    );
    assert_eq!(node_locale_compare_ordering("a", "Z"), Ordering::Less);
    assert_eq!(node_locale_compare_ordering("😀", "0"), Ordering::Less);
    assert_eq!(node_locale_compare_ordering("中", "z"), Ordering::Greater);
}

#[test]
fn default_config_top_level_keys_match_node_default_config() {
    let config = default_config();
    let keys = config
        .as_object()
        .expect("default config is object")
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let expected = [
        "run_type",
        "reverse_proxy_submode",
        "auto_manage_firewall",
        "whitelist_ips",
        "proxy_mappings",
        "host_mappings",
        "stream_mappings",
        "subdomain_mode",
        "ssl",
        "default_route",
        "default_tunnel",
        "fnos_share_bypass",
        "fnos_port_icon_hijack",
        "fnos_network_tuning",
        "gateway_logging",
        "waf",
        "reverse_proxy_throttle",
        "gateway_visibility",
        "gateway_proxy_headers",
        "gateway_host_response",
        "gateway_crawler_blocker",
        "gateway_portal",
        "appearance",
        "dashboard_display",
        "auto_https",
        "smart_connect",
        "scan_discovery",
        "auth_credential_settings",
        "event_system",
        "terminal_feature",
        "ssh_security",
        "locale",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();

    assert_eq!(keys, expected);
}

#[test]
fn default_config_includes_node_runtime_feature_defaults() {
    let config = default_config();

    assert_eq!(
        config.pointer("/event_system/rules/cpu_alert/enabled"),
        Some(&json!(true))
    );
    assert_eq!(
        config.pointer("/event_system/rules/cpu_alert/threshold_percent"),
        Some(&json!(80))
    );
    assert_eq!(
        config.pointer("/event_system/rules/memory_alert/sample_interval_seconds"),
        Some(&json!(5))
    );
    assert_eq!(
        config.pointer("/terminal_feature/idle_timeout_seconds"),
        Some(&json!(86400))
    );
    assert_eq!(
        config.pointer("/gateway_portal/display_style"),
        Some(&json!("title"))
    );
    assert_eq!(
        config.pointer("/waf/system_rules_auto_update_enabled"),
        Some(&json!(true))
    );
}

#[test]
fn normalizes_totp_access_scopes_like_node() {
    assert_eq!(
        normalize_totp_access_scopes(json!([
            " docker_admin_panel ",
            "other",
            "docker_admin_panel"
        ])),
        json!(["docker_admin_panel"])
    );
    assert_eq!(normalize_totp_access_scopes(json!("nope")), json!([]));
}

#[test]
fn normalizes_totp_credentials_like_node_store() {
    let credentials = normalize_totp_credentials_value(&json!([
        {
            "id": " one ",
            "secret": " SECRET ",
            "comment": "  Comment  ",
            "createdAt": "",
            "access_scopes": [" docker_admin_panel "],
            "subdomain_access": { "mode": "custom", "hosts": ["Example.com."] }
        },
        { "id": "", "secret": "NOPE" }
    ]));
    assert_eq!(credentials.len(), 1);
    assert_eq!(credentials[0].id, "one");
    assert_eq!(credentials[0].secret, "SECRET");
    assert_eq!(credentials[0].comment, "Comment");
    assert!(crate::time_utils::parse_iso_ms(&credentials[0].created_at).is_some());
    assert_eq!(credentials[0].access_scopes, json!(["docker_admin_panel"]));
    assert_eq!(
        credentials[0].subdomain_access,
        json!({ "mode": "custom", "hosts": ["example.com"] })
    );
}

#[test]
fn normalizes_totp_subdomain_access_like_node() {
    assert_eq!(
        normalize_totp_subdomain_access(json!({
            "mode": "custom",
            "hosts": [
                "HTTPS://Example.COM:8443/path?q=1",
                "example.com.",
                "/__select__",
                "*.bad.test",
                "bad host"
            ]
        })),
        json!({
            "mode": "custom",
            "hosts": ["__builtin_select__", "example.com"]
        })
    );
    assert_eq!(
        normalize_totp_subdomain_access(json!({ "mode": "all", "hosts": ["example.com"] })),
        json!({ "mode": "all", "hosts": [] })
    );
}

#[test]
fn cname_whitelist_concrete_targets_normalize_dedupe_and_sort_ips() {
    let record = WhitelistRecord {
        id: "whitelist:1".to_string(),
        ip: "example.com".to_string(),
        target_type: "cname".to_string(),
        expire_at: None,
        source: "manual".to_string(),
        created_at: 1,
        status: "active".to_string(),
        comment: None,
        ip_location: None,
        resolved_targets: Some(vec![
            " 192.0.2.1 ".to_string(),
            "not-an-ip".to_string(),
            "2001:DB8::1".to_string(),
            "192.0.2.1".to_string(),
        ]),
        check_interval_minutes: None,
        last_checked_at: None,
        last_resolved_at: None,
        resolve_status: None,
        resolve_message: None,
    };
    let targets = record
        .concrete_targets()
        .into_iter()
        .map(|target| target.target)
        .collect::<Vec<_>>();
    assert_eq!(targets, vec!["192.0.2.1", "2001:DB8::1"]);
}

#[test]
fn stale_whitelist_cleanup_targets_match_node_indexes() {
    let mut record = WhitelistRecord {
        id: "whitelist:1".to_string(),
        ip: "example.com".to_string(),
        target_type: "cname".to_string(),
        expire_at: None,
        source: "manual".to_string(),
        created_at: 1,
        status: "expired".to_string(),
        comment: None,
        ip_location: None,
        resolved_targets: Some(vec![
            "192.0.2.1".to_string(),
            "bad".to_string(),
            "192.0.2.1".to_string(),
            "2001:DB8::1".to_string(),
        ]),
        check_interval_minutes: None,
        last_checked_at: None,
        last_resolved_at: None,
        resolve_status: None,
        resolve_message: None,
    };
    assert_eq!(
        whitelist_stale_ip_index_targets(&record),
        vec!["192.0.2.1".to_string(), "2001:DB8::1".to_string()]
    );

    record.target_type = "cidr".to_string();
    record.ip = "192.0.2.0/24".to_string();
    record.resolved_targets = None;
    assert!(whitelist_stale_ip_index_targets(&record).is_empty());
}

#[test]
fn deserializes_whitelist_records_like_node_store() {
    let record = deserialize_whitelist_record(
        r#"{
                "id": " whitelist:legacy ",
                "ip": "Example.COM.",
                "expireAt": "123abc",
                "createdAt": "456.9",
                "resolvedTargets": [" 192.0.2.1 ", "bad", "2001:DB8::1", "192.0.2.1"],
                "checkIntervalMinutes": "10m",
                "lastCheckedAt": "",
                "resolveStatus": "nope",
                "resolveMessage": " resolved "
            }"#,
    )
    .unwrap();
    assert_eq!(record.id, "whitelist:legacy");
    assert_eq!(record.ip, "example.com");
    assert_eq!(record.target_type, "cname");
    assert_eq!(record.expire_at, Some(123));
    assert_eq!(record.created_at, 456);
    assert_eq!(record.source, "manual");
    assert_eq!(record.status, "active");
    assert_eq!(
        record.resolved_targets,
        Some(vec!["192.0.2.1".to_string(), "2001:DB8::1".to_string()])
    );
    assert_eq!(record.check_interval_minutes, Some(10));
    assert_eq!(record.last_checked_at, None);
    assert_eq!(record.resolve_status.as_deref(), Some("pending"));
    assert_eq!(record.resolve_message.as_deref(), Some("resolved"));
}

#[test]
fn deserializes_whitelist_region_groups_like_node_store() {
    let group = deserialize_whitelist_region_group(
        r#"{
                "id": " whitelist-region:legacy ",
                "regions": [
                    { "province": 440000, "query_city": true },
                    { "province": "广东", "query_city": "" },
                    { "province": " ", "query_city": "ignored" },
                    null
                ],
                "cidrs": [" 192.0.2.0/24 ", 123, null],
                "expireAt": "0x10",
                "createdAt": true,
                "updatedAt": "456.9",
                "status": "nope",
                "source": "auto",
                "comment": null
            }"#,
    )
    .unwrap();
    assert_eq!(group.id, "whitelist-region:legacy");
    assert_eq!(
        group.regions,
        vec![
            WhitelistRegionInput {
                province: "440000".to_string(),
                query_city: Some("true".to_string())
            },
            WhitelistRegionInput {
                province: "广东".to_string(),
                query_city: None
            }
        ]
    );
    assert_eq!(
        group.cidrs,
        vec!["192.0.2.0/24".to_string(), "123".to_string()]
    );
    assert_eq!(group.expire_at, Some(16));
    assert_eq!(group.created_at, 1);
    assert_eq!(group.updated_at, 456);
    assert_eq!(group.status, "active");
    assert_eq!(group.source, "manual");
    assert_eq!(group.comment.as_deref(), Some(""));
}

#[test]
fn reads_login_backoff_status_like_node_store() {
    let status = login_backoff_status_from_raw(
        "203.0.113.10",
        Some(r#"{"ip":"ignored","attempts":-2,"blockedUntil":1100}"#),
        1000,
    );
    assert_eq!(status.ip, "203.0.113.10");
    assert_eq!(status.attempts, -2);
    assert!(status.blocked);
    assert_eq!(status.retry_after, Some(1));
    assert_eq!(status.blocked_until, Some(1100));

    let expired = login_backoff_status_from_raw(
        "203.0.113.10",
        Some(r#"{"ip":"ignored","attempts":3,"blockedUntil":999}"#),
        1000,
    );
    assert_eq!(expired.attempts, 3);
    assert!(!expired.blocked);
    assert_eq!(expired.retry_after, None);
}

#[test]
fn docker_admin_session_record_accepts_legacy_missing_ttl() {
    let record: DockerAdminSessionRecord = serde_json::from_str(
        r#"{
                "id": "session-1",
                "created_at": "2026-01-01T00:00:00.000Z",
                "updated_at": "2026-01-01T00:00:00.000Z",
                "expires_at": "2026-01-01T12:00:00.000Z",
                "ip": "203.0.113.10",
                "user_agent": "ua"
            }"#,
    )
    .expect("legacy docker admin session");

    assert_eq!(record.ttl_seconds, 0);
}

#[test]
fn traffic_scope_matches_node_uri_encoding() {
    assert_eq!(traffic_scope_segment("global", None), "global");
    assert_eq!(traffic_scope_segment("", None), "");
    assert_eq!(traffic_scope_segment(" user ", None), " user ");
    assert_eq!(
        traffic_scope_segment("global", Some("example.com")),
        "global:host:example.com"
    );
    assert_eq!(
        traffic_scope_segment(" user ", Some("example.com")),
        " user :host:example.com"
    );
    assert_eq!(
        traffic_scope_segment("u", Some("[2001:db8::1]")),
        "u:host:%5B2001%3Adb8%3A%3A1%5D"
    );
}

#[test]
fn system_event_search_uses_unicode_lowercase_like_node() {
    let event = json!({
        "id": "evt_unicode",
        "type": "FN_EVENT_AUTH_LOGIN_SUCCESS",
        "source": "SERVER_ADMIN",
        "level": "INFO",
        "happened_at": "2026-07-07T00:00:00.000Z",
        "payload": {
            "credential_name": "Älice"
        }
    });

    assert!(system_event_matches_filters(
        &event, "älice", None, None, None
    ));
}

#[test]
fn parses_traffic_members_and_ignores_invalid_values() {
    assert_eq!(
        parse_traffic_points(&[
            "10:5".to_string(),
            "bad".to_string(),
            "11:nope".to_string(),
            "12:0".to_string()
        ]),
        vec![
            TrafficDeltaPoint { ts: 10, delta: 5.0 },
            TrafficDeltaPoint { ts: 12, delta: 0.0 }
        ]
    );
}

#[test]
fn counter_delta_handles_first_sample_and_resets() {
    assert_eq!(compute_counter_delta(100.0, None), 100.0);
    assert_eq!(compute_counter_delta(120.0, Some(100.0)), 20.0);
    assert_eq!(compute_counter_delta(12.0, Some(100.0)), 12.0);
    assert_eq!(compute_counter_delta(-1.0, Some(100.0)), 0.0);
}

#[test]
fn waf_log_dates_include_neighboring_utc_days() {
    let dates = waf_log_dates_for_range(1_704_067_200_000, 1_704_153_600_000);
    assert!(dates.contains(&"2023-12-31".to_string()));
    assert!(dates.contains(&"2024-01-01".to_string()));
    assert!(dates.contains(&"2024-01-02".to_string()));
    assert!(dates.contains(&"2024-01-03".to_string()));
}
