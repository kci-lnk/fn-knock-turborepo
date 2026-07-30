use super::*;
use crate::{cidr::CidrOperator, store::WhitelistRegionInput};

fn test_whitelist_record(id: &str, source: &str) -> WhitelistRecord {
    WhitelistRecord {
        id: id.to_string(),
        ip: "203.0.113.8".to_string(),
        target_type: "ip".to_string(),
        expire_at: None,
        source: source.to_string(),
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
    }
}

#[test]
fn normalizes_whitelist_targets() {
    assert_eq!(
        normalize_target("192.168.1.2:443", "manual", None).unwrap(),
        ("192.168.1.2".to_string(), "ip".to_string())
    );
    assert_eq!(
        normalize_target("192.168.1.10/24", "manual", None).unwrap(),
        ("192.168.1.0/24".to_string(), "cidr".to_string())
    );
    assert_eq!(
        normalize_target("Example.COM.", "manual", None).unwrap(),
        ("example.com".to_string(), "cname".to_string())
    );
    assert!(normalize_target("example.com", "auto", None).is_err());
}

#[test]
fn selects_auto_whitelist_records_for_direct_mode_cleanup() {
    let records = vec![
        test_whitelist_record("manual-1", "manual"),
        test_whitelist_record("auto-1", "auto"),
        test_whitelist_record("auto-2", "auto"),
    ];

    assert_eq!(
        whitelist_record_ids_by_source(&records, "auto"),
        vec!["auto-1".to_string(), "auto-2".to_string()]
    );
    assert_eq!(
        whitelist_record_ids_by_source(&records, "manual"),
        vec!["manual-1".to_string()]
    );
}

#[test]
fn localizes_whitelist_errors_and_response_comments() {
    let zh = Translator::new("zh-CN");
    assert_eq!(whitelist_text(&zh, "listFailed"), "读取白名单列表失败");
    assert_eq!(
        localize_whitelist_error(&zh, "Invalid whitelist target format"),
        "IP、CIDR 或域名格式不正确"
    );
    assert_eq!(
        localize_whitelist_error(&zh, "Invalid whitelist CIDR"),
        "CIDR 格式不正确"
    );
    assert_eq!(
        whitelist_manager_text_params(&zh, "resolvedIpCount", &[("count", "2".to_string())],),
        "已解析 2 个 IP"
    );

    let mut record = test_whitelist_record("whitelist:test", "auto");
    record.comment = Some("Automatically authorized after sign-in".to_string());
    assert_eq!(
        whitelist_record_for_response(record, &zh)
            .comment
            .as_deref(),
        Some("登录后自动授权")
    );
}

#[test]
fn builds_gateway_trusted_source_map_with_private_ips() {
    let mut source_map = BTreeMap::new();
    add_ip_source(&mut source_map, "192.168.1.2", "session:a".to_string());
    add_ip_source(&mut source_map, "8.8.8.8", "session:b".to_string());
    add_ip_source(&mut source_map, "::1", "session:loopback".to_string());
    add_ip_source(&mut source_map, "not-an-ip", "session:c".to_string());
    assert_eq!(
        source_map
            .get("192.168.1.2")
            .unwrap()
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec!["session:a".to_string()]
    );
    assert_eq!(
        source_map
            .get("8.8.8.8")
            .unwrap()
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec!["session:b".to_string()]
    );
    assert_eq!(
        source_map
            .get("127.0.0.1")
            .unwrap()
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec!["session:loopback".to_string()]
    );
    assert!(!source_map.contains_key("not-an-ip"));
}

#[tokio::test]
async fn gateway_trusted_runtime_compiles_sessions_and_all_whitelist_sources_when_throttle_disabled()
 {
    let (_directory, state) = gateway_trusted_runtime_test_state("sources", false).await;
    state
        .store
        .add_session(
            "local-session",
            &gateway_trusted_test_session("192.168.1.2"),
            3600,
        )
        .await
        .expect("store local session");
    state
        .store
        .add_session(
            "public-session",
            &gateway_trusted_test_session("203.0.113.10"),
            3600,
        )
        .await
        .expect("store public session");

    for record in [
        WhitelistRecord {
            id: "whitelist:manual-ip".to_string(),
            ip: "198.51.100.20".to_string(),
            source: "manual".to_string(),
            ..test_whitelist_record("whitelist:manual-ip", "manual")
        },
        WhitelistRecord {
            id: "whitelist:auto-ip".to_string(),
            ip: "100.64.0.8".to_string(),
            source: "auto".to_string(),
            ..test_whitelist_record("whitelist:auto-ip", "auto")
        },
        WhitelistRecord {
            id: "whitelist:cname".to_string(),
            ip: "trusted.example.test".to_string(),
            target_type: "cname".to_string(),
            resolved_targets: Some(vec!["2001:db8::8".to_string()]),
            ..test_whitelist_record("whitelist:cname", "manual")
        },
        WhitelistRecord {
            id: "whitelist:cidr".to_string(),
            ip: "172.16.0.0/12".to_string(),
            target_type: "cidr".to_string(),
            ..test_whitelist_record("whitelist:cidr", "manual")
        },
    ] {
        state
            .store
            .insert_whitelist_record(&record)
            .await
            .expect("store whitelist record");
    }
    state
        .store
        .insert_whitelist_region_group(&WhitelistRegionGroupRecord {
            id: "whitelist-region:test".to_string(),
            regions: vec![],
            cidrs: vec!["100.64.0.0/10".to_string()],
            policy_id: String::new(),
            policy: None,
            source_cidr_count: 0,
            range_count: 0,
            expire_at: None,
            source: "manual".to_string(),
            created_at: now_seconds(),
            updated_at: now_seconds(),
            status: "active".to_string(),
            comment: None,
        })
        .await
        .expect("store whitelist region group");

    let compiled = compile_reverse_proxy_trusted_ips(&state)
        .await
        .expect("compile trusted runtime");
    let ips = compiled_gateway_trusted_ips(&compiled);
    for expected in [
        "192.168.1.2",
        "203.0.113.10",
        "198.51.100.20",
        "100.64.0.8",
        "2001:db8::8",
    ] {
        assert!(
            ips.contains(expected),
            "compiled IPs missing {expected}: {ips:?}"
        );
    }
    assert!(compiled.get("cidrs").is_none());
    assert!(
        compiled
            .get("policy_id")
            .and_then(Value::as_str)
            .is_some_and(|id| id.starts_with("ipset-v2:"))
    );
    assert!(compiled.get("policy").is_some_and(Value::is_object));
    assert!(
        compiled.get("enabled") == Some(&Value::Bool(false)),
        "shared runtime must preserve the independent throttle switch"
    );

    sync_reverse_proxy_trusted_ips(&state).await;
    let stored = state
        .store
        .get_json_value("fn_knock:gateway:trusted-client-ips:runtime")
        .await
        .expect("read stored trusted runtime")
        .expect("stored trusted runtime");
    assert_eq!(
        compiled_gateway_trusted_ips(&stored),
        compiled_gateway_trusted_ips(&compiled)
    );

    state
        .store
        .delete_session("local-session")
        .await
        .expect("delete local session");
    let after_delete = compile_reverse_proxy_trusted_ips(&state)
        .await
        .expect("compile after session delete");
    assert!(!compiled_gateway_trusted_ips(&after_delete).contains("192.168.1.2"));
}

#[tokio::test]
async fn gateway_trusted_runtime_keeps_primary_and_current_mobility_window_ips() {
    let (_directory, state) = gateway_trusted_runtime_test_state("mobility", true).await;
    let session_id = "mobile-session";
    state
        .store
        .add_session(
            session_id,
            &gateway_trusted_test_session("203.0.113.30"),
            3600,
        )
        .await
        .expect("store mobile session");

    let now = now_seconds();
    for (ip, score) in [
        ("203.0.113.30", now - 1300),
        ("203.0.113.31", now - 30),
        ("203.0.113.32", now - 1300),
    ] {
        assert!(
            state
                .store
                .save_auth_mobility_active_ip_detail(
                    session_id,
                    ip,
                    score,
                    &json!({
                        "ip": ip,
                        "firstSeenAt": score,
                        "lastSeenAt": score
                    }),
                    3600,
                )
                .await
                .expect("store mobility IP"),
            "live session must own mobility IP {ip}"
        );
    }

    let compiled = compile_reverse_proxy_trusted_ips(&state)
        .await
        .expect("compile mobility runtime");
    let ips = compiled_gateway_trusted_ips(&compiled);
    assert!(ips.contains("203.0.113.30"));
    assert!(ips.contains("203.0.113.31"));
    assert!(!ips.contains("203.0.113.32"));

    state
        .store
        .delete_session(session_id)
        .await
        .expect("revoke mobile session");
    let revoked = compile_reverse_proxy_trusted_ips(&state)
        .await
        .expect("compile after revoke");
    let revoked_ips = compiled_gateway_trusted_ips(&revoked);
    assert!(!revoked_ips.contains("203.0.113.30"));
    assert!(!revoked_ips.contains("203.0.113.31"));
}

#[tokio::test]
async fn gateway_trusted_runtime_excludes_expired_sessions_still_present_in_storage() {
    let (_directory, state) = gateway_trusted_runtime_test_state("expired", false).await;
    let mut expired_session = gateway_trusted_test_session("203.0.113.40");
    expired_session.expires_at = Some(time_utils::iso_after_seconds(-60));
    state
        .store
        .add_session("expired-session", &expired_session, 3600)
        .await
        .expect("store expired session with a deliberately longer storage TTL");

    let compiled = compile_reverse_proxy_trusted_ips(&state)
        .await
        .expect("compile trusted runtime");

    assert!(
        !compiled_gateway_trusted_ips(&compiled).contains("203.0.113.40"),
        "an expired authoritative session must not remain gateway-trusted"
    );
}

#[tokio::test]
async fn gateway_trusted_sync_waits_for_the_state_serialization_lock() {
    let (_directory, state) = gateway_trusted_runtime_test_state("serialized", false).await;
    state
        .store
        .add_session(
            "serialized-session",
            &gateway_trusted_test_session("203.0.113.41"),
            3600,
        )
        .await
        .expect("store session");

    let guard = state.whitelist_runtime_sync_lock.lock().await;
    let sync_state = state.clone();
    let mut sync_task =
        tokio::spawn(async move { sync_reverse_proxy_trusted_ips(&sync_state).await });

    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(50), &mut sync_task)
            .await
            .is_err(),
        "trusted runtime sync must wait for the state-scoped serialization lock"
    );

    state
        .store
        .delete_session("serialized-session")
        .await
        .expect("revoke session while the sync is queued");
    drop(guard);
    sync_task.await.expect("queued trusted runtime sync");

    let stored = state
        .store
        .get_json_value("fn_knock:gateway:trusted-client-ips:runtime")
        .await
        .expect("read stored trusted runtime")
        .expect("stored trusted runtime");
    assert!(
        !compiled_gateway_trusted_ips(&stored).contains("203.0.113.41"),
        "the queued sync must compile after revocation instead of publishing an old snapshot"
    );
}

#[test]
fn reverse_proxy_trusted_ips_rewrites_session_auto_whitelist_target_without_mobility() {
    let mut session_linked = BTreeMap::new();
    session_linked.insert("whitelist:login".to_string(), "203.0.113.42".to_string());
    let target = WhitelistConcreteTarget {
        record_id: "whitelist:login".to_string(),
        record_target: "198.51.100.10".to_string(),
        record_target_type: "ip".to_string(),
        source: "auto".to_string(),
        target: "198.51.100.10".to_string(),
        target_type: "ip".to_string(),
    };

    assert_eq!(
        reverse_proxy_compiled_whitelist_target(&target, &session_linked, false),
        "203.0.113.42"
    );
    assert_eq!(
        reverse_proxy_compiled_whitelist_target(&target, &session_linked, true),
        "198.51.100.10"
    );
}

#[test]
fn normalizes_cname_check_interval_bounds() {
    assert_eq!(normalize_cname_check_interval(None), 5);
    assert_eq!(normalize_cname_check_interval(Some(0)), 1);
    assert_eq!(normalize_cname_check_interval(Some(10_000)), 1440);
}

#[test]
fn normalizes_whitelist_region_inputs() {
    let regions = normalize_whitelist_region_inputs(&[
        json!({ "province": "广东", "query_city": "深圳" }),
        json!({ "province": "广东", "query_city": "深圳" }),
        json!({ "province": "广东", "query_city": "深圳", "operator": "移动" }),
        json!({ "province": "广东", "query_city": "" }),
        json!({ "province": 440000, "query_city": true }),
        json!({ "province": ["广东", null, "深圳"], "query_city": false }),
        json!({ "province": " " }),
        json!("ignored"),
    ])
    .unwrap();

    assert_eq!(regions.len(), 5);
    assert_eq!(regions[0].province, "广东");
    assert_eq!(regions[0].query_city.as_deref(), Some("深圳"));
    assert_eq!(regions[1].operator, Some(CidrOperator::Mobile));
    assert_eq!(regions[2].province, "广东");
    assert_eq!(regions[2].query_city, None);
    assert_eq!(regions[3].province, "440000");
    assert_eq!(regions[3].query_city.as_deref(), Some("true"));
    assert_eq!(regions[4].province, "广东,,深圳");
    assert_eq!(regions[4].query_city.as_deref(), Some("false"));
}

#[test]
fn whitelist_regions_reject_non_string_operator() {
    assert!(
        normalize_whitelist_region_inputs(&[
            json!({ "province": "广东", "query_city": "深圳", "operator": false }),
        ])
        .is_err()
    );
}

#[test]
fn classifies_dns_no_data_errors_like_node() {
    assert!(is_node_no_data_lookup_error(&std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "resolver returned no record"
    )));
    assert!(is_node_no_data_lookup_error(&std::io::Error::other(
        "query failed: ENOTFOUND"
    )));
    assert!(is_node_no_data_lookup_error(&std::io::Error::other(
        "nodename nor servname provided, or not known"
    )));
    assert!(!is_node_no_data_lookup_error(&std::io::Error::other(
        "temporary failure in name resolution"
    )));
}

#[test]
fn summarizes_whitelist_region_group_without_cidrs() {
    let group = WhitelistRegionGroupRecord {
        id: "whitelist-region:test".to_string(),
        regions: vec![WhitelistRegionInput {
            province: "广东".to_string(),
            query_city: Some("深圳".to_string()),
            operator: None,
        }],
        cidrs: vec!["1.1.1.0/24".to_string(), "2001:db8::/32".to_string()],
        policy_id: String::new(),
        policy: None,
        source_cidr_count: 0,
        range_count: 0,
        expire_at: None,
        source: "manual".to_string(),
        created_at: 10,
        updated_at: 11,
        status: "active".to_string(),
        comment: Some("office".to_string()),
    };

    let summary = group.summary();
    assert_eq!(summary.cidr_count, 2);
    let targets = group.concrete_targets();
    assert_eq!(targets.len(), 2);
    assert_eq!(targets[0].record_id, "whitelist-region:test");
    assert_eq!(targets[0].target_type, "cidr");
}

fn compiled_gateway_trusted_ips(runtime: &Value) -> BTreeSet<&str> {
    runtime
        .get("items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("ip").and_then(Value::as_str))
        .collect()
}

fn gateway_trusted_test_session(ip: &str) -> LoginSession {
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
        stream_access_expires_at: None,
        comment: None,
        ip: ip.to_string(),
        user_agent: "test".to_string(),
        login_time: time_utils::now_iso(),
        expires_at: Some(time_utils::iso_after_seconds(3600)),
        ip_location: None,
    }
}

async fn gateway_trusted_runtime_test_state(
    name: &str,
    mobility_enabled: bool,
) -> (tempfile::TempDir, AppState) {
    let directory = tempfile::tempdir().expect("temporary trusted runtime database");
    let mut settings = {
        let _environment = crate::test_support::EnvGuard::new(&[]);
        crate::settings::Settings::from_env()
    };
    settings.data_dir = directory.path().join("data");
    settings.gateway_config_dir = directory.path().join("gateway");
    settings.sqlite_path = directory.path().join("fn-knock.sqlite3");
    settings.legacy_redis_url = String::new();
    settings.go_backend_grpc_addr = "127.0.0.1:1".to_string();
    settings.internal_rpc_token = format!("gateway-trusted-{name}-test");
    let state = AppState::new(settings)
        .await
        .expect("trusted runtime test state");
    state
        .store
        .save_config(&json!({
            "reverse_proxy_throttle": {
                "enabled": false
            },
            "auth_credential_settings": {
                "session_ip_mobility_enabled": mobility_enabled,
                "session_ip_mobility_window_seconds": 1200,
                "session_ip_mobility_max_ips": 4
            }
        }))
        .await
        .expect("trusted runtime test config");
    (directory, state)
}

#[test]
fn gateway_trusted_runtime_confirmation_rejects_stale_or_partial_echoes() {
    let policy = json!({
        "id": "ipset-v2:test",
        "format_version": 2,
        "ipv4_ranges": "eNpjYGBgAAAABAAB",
        "ipv6_ranges": ""
    });
    let requested = json!({
        "ips": ["203.0.113.8", "::1"],
        "policy_id": "ipset-v2:test",
        "policy": policy.clone(),
        "updated_at": "2026-07-31T01:00:00Z"
    });
    let applied = json!({
        "success": true,
        "data": {
            "ips": ["127.0.0.1", "203.0.113.8"],
            "policy_id": "ipset-v2:test",
            "policy": policy,
            "updated_at": "2026-07-31T01:00:00Z"
        }
    });
    ensure_gateway_ip_runtime_applied(
        "gateway trusted client IP",
        &requested,
        &applied,
        false,
        true,
    )
    .unwrap();

    let mut stale = applied.clone();
    stale["data"]["updated_at"] = json!("2026-07-31T00:59:59Z");
    assert!(
        ensure_gateway_ip_runtime_applied(
            "gateway trusted client IP",
            &requested,
            &stale,
            false,
            true,
        )
        .unwrap_err()
        .to_string()
        .contains("did not apply")
    );

    let mut missing_ip = applied;
    missing_ip["data"]["ips"] = json!([]);
    assert!(
        ensure_gateway_ip_runtime_applied(
            "gateway trusted client IP",
            &requested,
            &missing_ip,
            false,
            true,
        )
        .is_err()
    );
}
