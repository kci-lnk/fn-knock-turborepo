use super::*;

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
        "firewall_additional_ports",
        "whitelist_ips",
        "proxy_mappings",
        "host_mappings",
        "host_mapping_groups",
        "host_mapping_grouped_view",
        "stream_mappings",
        "subdomain_mode",
        "ssl",
        "default_route",
        "default_tunnel",
        "fnos_share_bypass",
        "fnos_port_icon_hijack",
        "fnos_connect_waf",
        "fnos_network_tuning",
        "gateway_logging",
        "waf",
        "reverse_proxy_throttle",
        "gateway_visibility",
        "visibility_policies",
        "gateway_proxy_headers",
        "gateway_host_response",
        "gateway_crawler_blocker",
        "gateway_portal",
        "gateway_unmatched_route",
        "appearance",
        "dashboard_display",
        "auto_https",
        "smart_connect",
        "scan_discovery",
        "auth_credential_settings",
        "event_system",
        "terminal_feature",
        "wol_feature",
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
        config.pointer("/event_system/rules/gateway_visibility_block/enabled"),
        Some(&json!(true))
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
        config.pointer("/gateway_portal/version"),
        Some(&json!("v1"))
    );
    assert_eq!(
        config.pointer("/gateway_portal/show_wol"),
        Some(&json!(true))
    );
    assert_eq!(config.pointer("/wol_feature/enabled"), Some(&json!(false)));
    assert_eq!(
        config.pointer("/gateway_unmatched_route/behavior"),
        Some(&json!("error_page"))
    );
    assert_eq!(
        config.pointer("/gateway_unmatched_route/upstream_error_detail"),
        Some(&json!("less"))
    );
    assert_eq!(
        config.pointer("/dashboard_display/date_time_display_mode"),
        Some(&json!("human_friendly"))
    );
    assert_eq!(
        config.pointer("/dashboard_display/show_console_app_list"),
        Some(&json!(false))
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
            "subdomain_access": {
                "mode": "custom",
                "hosts": ["Example.com."],
                "streams": [
                    { "protocol": "TCP", "listen_port": 2222 },
                    { "protocol": "tcp", "listen_port": 2222 },
                    { "protocol": "udp", "listen_port": 53 }
                ]
            }
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
        json!({
            "mode": "custom",
            "hosts": ["example.com"],
            "streams": [
                { "protocol": "udp", "listen_port": 53 },
                { "protocol": "tcp", "listen_port": 2222 }
            ]
        })
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
            ],
            "streams": [
                { "protocol": "TCP", "listen_port": 2222 },
                { "protocol": "tcp", "listen_port": 2222 },
                { "protocol": "udp", "listen_port": 53 },
                { "protocol": "icmp", "listen_port": 7 },
                { "protocol": "tcp", "listen_port": 0 },
                { "protocol": "udp", "listen_port": 65536 }
            ]
        })),
        json!({
            "mode": "custom",
            "hosts": ["__builtin_select__", "example.com"],
            "streams": [
                { "protocol": "udp", "listen_port": 53 },
                { "protocol": "tcp", "listen_port": 2222 }
            ]
        })
    );
    assert_eq!(
        normalize_totp_subdomain_access(json!({ "mode": "all", "hosts": ["example.com"] })),
        json!({ "mode": "all", "hosts": [], "streams": [] })
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

#[tokio::test]
async fn stale_whitelist_replace_cannot_recreate_a_deleted_record() {
    let (_dir, store) = open_test_store().await;
    let record = WhitelistRecord {
        id: "whitelist:stale-cname-refresh".to_string(),
        ip: "example.com".to_string(),
        target_type: "cname".to_string(),
        expire_at: None,
        source: "manual".to_string(),
        created_at: 1,
        status: "active".to_string(),
        comment: None,
        ip_location: None,
        resolved_targets: Some(vec!["192.0.2.1".to_string()]),
        check_interval_minutes: Some(5),
        last_checked_at: None,
        last_resolved_at: None,
        resolve_status: Some("resolved".to_string()),
        resolve_message: None,
    };
    store
        .insert_whitelist_record(&record)
        .await
        .expect("insert whitelist record");

    let mut stale_refresh = record.clone();
    stale_refresh.resolved_targets = Some(vec!["192.0.2.2".to_string()]);
    stale_refresh.last_checked_at = Some(2);
    store
        .delete_whitelist_record(&record.id)
        .await
        .expect("delete whitelist record")
        .expect("deleted record");

    let error = store
        .replace_whitelist_record(&record, &stale_refresh)
        .await
        .expect_err("stale refresh must fail");
    assert!(error.to_string().contains("changed concurrently"));
    assert!(
        store
            .get_whitelist_record(&record.id)
            .await
            .expect("read deleted record")
            .is_none()
    );
    assert!(
        store
            .list_whitelist_records()
            .await
            .expect("list whitelist records")
            .is_empty()
    );
    assert!(
        store
            .typed
            .typed_whitelist
            .load_one("record", &record.id)
            .await
            .expect("read typed whitelist record")
            .is_none()
    );
}

#[tokio::test]
async fn whitelist_record_writes_keep_typed_shadow_in_the_same_transaction() {
    let (_dir, store) = open_test_store().await;
    let record = WhitelistRecord {
        id: "whitelist:typed-shadow".to_string(),
        ip: "192.0.2.10".to_string(),
        target_type: "ip".to_string(),
        expire_at: Some(9_999_999_999),
        source: "manual".to_string(),
        created_at: 123,
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

    store
        .insert_whitelist_record(&record)
        .await
        .expect("insert dual-written whitelist record");
    assert_eq!(store.typed.typed_whitelist.count().await.unwrap(), 1);
    let typed = store
        .typed
        .typed_whitelist
        .load_one("record", &record.id)
        .await
        .unwrap()
        .expect("typed whitelist record");
    assert_eq!(typed.document_json, serde_json::to_string(&record).unwrap());
    assert_eq!(typed.sort_score, record.created_at);
    assert_eq!(typed.expires_at, record.expire_at);
    assert_eq!(typed.status, "active");

    let updated = store
        .update_whitelist_comment(&record.id, "updated".to_string())
        .await
        .expect("update comment")
        .expect("updated record");
    let typed = store
        .typed
        .typed_whitelist
        .load_one("record", &record.id)
        .await
        .unwrap()
        .expect("updated typed record");
    assert_eq!(
        typed.document_json,
        serde_json::to_string(&updated).unwrap()
    );

    store
        .delete_whitelist_record(&record.id)
        .await
        .expect("delete record")
        .expect("deleted record");
    assert!(
        store
            .typed
            .typed_whitelist
            .load_one("record", &record.id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .get_whitelist_record(&record.id)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn typed_whitelist_record_mismatch_falls_back_and_repairs_primary() {
    let (_dir, store) = open_test_store().await;
    let record = WhitelistRecord {
        id: "whitelist:typed-primary-repair".to_string(),
        ip: "192.0.2.20".to_string(),
        target_type: "ip".to_string(),
        expire_at: None,
        source: "manual".to_string(),
        created_at: 321,
        status: "active".to_string(),
        comment: Some("legacy authoritative".to_string()),
        ip_location: None,
        resolved_targets: None,
        check_interval_minutes: None,
        last_checked_at: None,
        last_resolved_at: None,
        resolve_status: None,
        resolve_message: None,
    };
    store
        .insert_whitelist_record(&record)
        .await
        .expect("insert whitelist record");

    let mut divergent = record.clone();
    divergent.comment = Some("typed divergence".to_string());
    let divergent_json = serde_json::to_string(&divergent).unwrap();
    let record_id = record.id.clone();
    store
        .manager
        .call(move |conn| {
            conn.execute(
                "UPDATE whitelist_documents SET document_json = ?1 WHERE kind = 'record' AND id = ?2",
                tokio_rusqlite::rusqlite::params![divergent_json, record_id],
            )?;
            Ok(())
        })
        .await
        .expect("inject typed divergence");

    let listed = store
        .list_whitelist_records()
        .await
        .expect("fallback to legacy whitelist list");
    assert_eq!(listed, vec![record.clone()]);
    assert_eq!(store.typed_whitelist_shadow_mismatch_count(), 1);
    let repaired = store
        .typed
        .typed_whitelist
        .load_one("record", &record.id)
        .await
        .unwrap()
        .expect("repaired typed record");
    assert_eq!(
        repaired.document_json,
        serde_json::to_string(&record).unwrap()
    );

    let listed_again = store
        .list_whitelist_records()
        .await
        .expect("read repaired typed primary");
    assert_eq!(listed_again, vec![record]);
    assert_eq!(store.typed_whitelist_shadow_mismatch_count(), 1);
}

#[tokio::test]
async fn corrupt_typed_whitelist_record_falls_back_without_expanding_authorization() {
    let (_dir, store) = open_test_store().await;
    let record = WhitelistRecord {
        id: "whitelist:typed-primary-corrupt".to_string(),
        ip: "198.51.100.20".to_string(),
        target_type: "ip".to_string(),
        expire_at: None,
        source: "manual".to_string(),
        created_at: 654,
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
    store
        .insert_whitelist_record(&record)
        .await
        .expect("insert whitelist record");
    let record_id = record.id.clone();
    store
        .manager
        .call(move |conn| {
            conn.execute(
                "UPDATE whitelist_documents SET document_json = '{\"unexpected\":true}' WHERE kind = 'record' AND id = ?1",
                [record_id],
            )?;
            Ok(())
        })
        .await
        .expect("corrupt typed document");

    assert_eq!(
        store
            .get_whitelist_record(&record.id)
            .await
            .expect("fallback to legacy record"),
        Some(record.clone())
    );
    assert_eq!(store.typed_whitelist_shadow_mismatch_count(), 1);
    assert_eq!(
        store
            .get_whitelist_record(&record.id)
            .await
            .expect("read repaired record"),
        Some(record)
    );
    assert_eq!(store.typed_whitelist_shadow_mismatch_count(), 1);
}

#[tokio::test]
async fn typed_only_whitelist_record_is_never_used_for_authorization() {
    let (_dir, store) = open_test_store().await;
    let typed_only = WhitelistRecord {
        id: "whitelist:typed-only".to_string(),
        ip: "203.0.113.99".to_string(),
        target_type: "ip".to_string(),
        expire_at: None,
        source: "manual".to_string(),
        created_at: 777,
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
    let typed_json = serde_json::to_string(&typed_only).unwrap();
    let typed_id = typed_only.id.clone();
    store
        .manager
        .call(move |conn| {
            conn.execute(
                "INSERT INTO whitelist_documents(kind, id, document_json, sort_score, expires_at, status, updated_at_ms)
                 VALUES ('record', ?1, ?2, 777, NULL, 'active', 777)",
                tokio_rusqlite::rusqlite::params![typed_id, typed_json],
            )?;
            Ok(())
        })
        .await
        .expect("inject typed-only authorization record");

    assert!(
        store
            .list_whitelist_records()
            .await
            .expect("compare typed-only record against legacy keyspace")
            .is_empty()
    );
    assert_eq!(store.typed_whitelist_shadow_mismatch_count(), 1);
    assert!(
        store
            .typed
            .typed_whitelist
            .load_one("record", &typed_only.id)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn typed_whitelist_primary_keeps_pending_records_out_of_authorization_lists() {
    let (_dir, store) = open_test_store().await;
    let pending = WhitelistRecord {
        id: "whitelist:typed-pending".to_string(),
        ip: "203.0.113.20".to_string(),
        target_type: "ip".to_string(),
        expire_at: Some(9_999_999_999),
        source: "auto".to_string(),
        created_at: 987,
        status: "pending".to_string(),
        comment: None,
        ip_location: None,
        resolved_targets: None,
        check_interval_minutes: None,
        last_checked_at: None,
        last_resolved_at: None,
        resolve_status: None,
        resolve_message: None,
    };
    store
        .insert_whitelist_record(&pending)
        .await
        .expect("insert pending whitelist record");

    assert!(
        store
            .list_whitelist_records()
            .await
            .expect("list authorization records")
            .is_empty()
    );
    assert_eq!(
        store
            .get_whitelist_record(&pending.id)
            .await
            .expect("load pending record for promotion"),
        Some(pending)
    );
    assert_eq!(store.typed_whitelist_shadow_mismatch_count(), 0);
}

#[tokio::test]
async fn typed_whitelist_region_mismatch_falls_back_and_repairs_primary() {
    let (_dir, store) = open_test_store().await;
    let region = WhitelistRegionGroupRecord {
        id: "whitelist-region:typed-primary-repair".to_string(),
        regions: vec![WhitelistRegionInput {
            province: "广东".to_string(),
            query_city: None,
            operator: None,
        }],
        cidrs: vec!["192.0.2.0/24".to_string()],
        policy_id: String::new(),
        policy: None,
        source_cidr_count: 1,
        range_count: 1,
        expire_at: None,
        source: "manual".to_string(),
        created_at: 741,
        updated_at: 741,
        status: "active".to_string(),
        comment: Some("legacy authoritative".to_string()),
    };
    store
        .insert_whitelist_region_group(&region)
        .await
        .expect("insert whitelist region");

    let mut divergent = region.clone();
    divergent.comment = Some("typed divergence".to_string());
    let divergent_json = serde_json::to_string(&divergent).unwrap();
    let region_id = region.id.clone();
    store
        .manager
        .call(move |conn| {
            conn.execute(
                "UPDATE whitelist_documents SET document_json = ?1 WHERE kind = 'region' AND id = ?2",
                tokio_rusqlite::rusqlite::params![divergent_json, region_id],
            )?;
            Ok(())
        })
        .await
        .expect("inject typed region divergence");

    assert_eq!(
        store
            .list_whitelist_region_groups()
            .await
            .expect("fallback to legacy region list"),
        vec![region.clone()]
    );
    assert_eq!(store.typed_whitelist_shadow_mismatch_count(), 1);
    let repaired = store
        .typed
        .typed_whitelist
        .load_one("region", &region.id)
        .await
        .unwrap()
        .expect("repaired typed region");
    assert_eq!(
        repaired.document_json,
        serde_json::to_string(&region).unwrap()
    );
}

#[tokio::test]
async fn typed_whitelist_failure_rolls_back_compatibility_indexes() {
    let (_dir, store) = open_test_store().await;
    store
        .manager
        .call(|conn| {
            conn.execute("DROP TABLE whitelist_documents", [])?;
            Ok(())
        })
        .await
        .expect("remove typed table");
    let record = WhitelistRecord {
        id: "whitelist:typed-rollback".to_string(),
        ip: "192.0.2.11".to_string(),
        target_type: "ip".to_string(),
        expire_at: None,
        source: "manual".to_string(),
        created_at: 456,
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

    store
        .insert_whitelist_record(&record)
        .await
        .expect_err("typed write failure must abort compatibility write");
    let mut conn = store.conn();
    let stored: Option<String> = conn.hget(WHITELIST_RECORDS, &record.id).await.unwrap();
    assert!(stored.is_none());
    let ordered: Vec<String> = conn.zrevrange(WHITELIST_RECORD_ORDER, 0, -1).await.unwrap();
    assert!(!ordered.contains(&record.id));
    let indexed: Vec<String> = conn
        .smembers(whitelist_ip_records_key(&record.ip))
        .await
        .unwrap();
    assert!(!indexed.contains(&record.id));
}

#[tokio::test]
async fn typed_whitelist_rebuilds_after_legacy_backup_restore_and_reopen() {
    let source_dir = tempfile::tempdir().expect("create source temp dir");
    let source_path = source_dir.path().join("fn-knock.sqlite3");
    let source = Store::connect(&source_path)
        .await
        .expect("open source store");
    let record = WhitelistRecord {
        id: "whitelist:restored-shadow".to_string(),
        ip: "192.0.2.12".to_string(),
        target_type: "ip".to_string(),
        expire_at: None,
        source: "manual".to_string(),
        created_at: 789,
        status: "active".to_string(),
        comment: Some("restored".to_string()),
        ip_location: None,
        resolved_targets: None,
        check_interval_minutes: None,
        last_checked_at: None,
        last_resolved_at: None,
        resolve_status: None,
        resolve_message: None,
    };
    source
        .insert_whitelist_record(&record)
        .await
        .expect("seed source whitelist");
    let entries = source
        .export_backup_entries_by_prefix_limited("fn_knock:whitelist:", 1_000_000, |_| true)
        .await
        .expect("export whitelist entries");
    assert!(!entries.is_empty());

    let target_dir = tempfile::tempdir().expect("create target temp dir");
    let target_path = target_dir.path().join("fn-knock.sqlite3");
    let target = Store::connect(&target_path)
        .await
        .expect("open target store");
    target
        .replace_backup_entries_by_prefix("fn_knock:", &entries, 200)
        .await
        .expect("restore whitelist entries");
    let typed = target
        .typed
        .typed_whitelist
        .load_one("record", &record.id)
        .await
        .unwrap()
        .expect("restored typed whitelist");
    assert_eq!(typed.document_json, serde_json::to_string(&record).unwrap());
    drop(target);

    // Reopening represents a newer binary reconciling writes made by a 2.x
    // binary that only knew the compatibility keyspace.
    let mut legacy = record.clone();
    legacy.comment = Some("legacy rewrite".to_string());
    let mut connection = redis::ConnectionManager::open(&target_path)
        .await
        .expect("open compatibility connection");
    let _: () = redis::cmd("HSET")
        .arg(WHITELIST_RECORDS)
        .arg(&record.id)
        .arg(serde_json::to_string(&legacy).unwrap())
        .query_async(&mut connection)
        .await
        .expect("simulate legacy whitelist rewrite");
    drop(connection);

    let reopened = Store::connect(&target_path)
        .await
        .expect("reopen upgraded store");
    let typed = reopened
        .typed
        .typed_whitelist
        .load_one("record", &record.id)
        .await
        .unwrap()
        .expect("reconciled typed whitelist");
    assert_eq!(typed.document_json, serde_json::to_string(&legacy).unwrap());
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
                    { "province": "广东", "query_city": "深圳", "operator": "移动" },
                    { "province": "广东", "query_city": "深圳", "operator": "电信" },
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
                query_city: Some("true".to_string()),
                operator: None,
            },
            WhitelistRegionInput {
                province: "广东".to_string(),
                query_city: None,
                operator: None,
            },
            WhitelistRegionInput {
                province: "广东".to_string(),
                query_city: Some("深圳".to_string()),
                operator: Some(crate::cidr::CidrOperator::Mobile),
            },
            WhitelistRegionInput {
                province: "广东".to_string(),
                query_city: Some("深圳".to_string()),
                operator: Some(crate::cidr::CidrOperator::Telecom),
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

#[tokio::test]
async fn whitelist_region_migration_compiles_active_records_and_compacts_tombstones() {
    let (_dir, store) = open_test_store().await;
    let mut conn = store.conn();
    let active_id = "whitelist-region:active";
    let deleted_id = "whitelist-region:deleted";
    let active = json!({
        "id": active_id,
        "regions": [{ "province": "广东", "query_city": null }],
        "cidrs": ["192.0.2.0/25", "192.0.2.128/25", "2001:db8::/32"],
        "expireAt": null,
        "source": "manual",
        "createdAt": 20,
        "updatedAt": 20,
        "status": "active"
    });
    let deleted = json!({
        "id": deleted_id,
        "regions": [{ "province": "浙江", "query_city": null }],
        "cidrs": ["198.51.100.0/24", "2001:db8:1::/48"],
        "expireAt": null,
        "source": "manual",
        "createdAt": 10,
        "updatedAt": 10,
        "status": "deleted"
    });
    let _: () = redis::cmd("HSET")
        .arg(WHITELIST_REGION_GROUP_RECORDS)
        .arg(active_id)
        .arg(active.to_string())
        .arg(deleted_id)
        .arg(deleted.to_string())
        .query_async(&mut conn)
        .await
        .expect("seed region groups");
    drop(conn);

    let migrated = store
        .migrate_whitelist_region_groups_to_ipsets()
        .await
        .expect("migrate region groups");
    assert_eq!(migrated.len(), 1);
    assert_eq!(migrated[0].id, active_id);
    assert!(migrated[0].cidrs.is_empty());
    assert!(migrated[0].policy_id.starts_with("ipset-v2:"));
    assert_eq!(migrated[0].range_count, 2);
    assert!(
        migrated[0]
            .policy()
            .is_some_and(|policy| policy.contains("192.0.2.255".parse().unwrap()))
    );

    let deleted_raw = store
        .hgetall_string_map(WHITELIST_REGION_GROUP_RECORDS)
        .await
        .expect("read region groups")
        .remove(deleted_id)
        .expect("read compact tombstone");
    let deleted = deserialize_whitelist_region_group(&deleted_raw).unwrap();
    assert!(deleted.cidrs.is_empty());
    assert!(deleted.policy.is_none());
    assert!(deleted.policy_id.is_empty());
    assert_eq!(deleted.source_cidr_count, 0);
    assert_eq!(deleted.range_count, 0);

    let typed_active = store
        .typed
        .typed_whitelist
        .load_one("region", active_id)
        .await
        .expect("read typed active region")
        .expect("typed active region");
    let typed_deleted = store
        .typed
        .typed_whitelist
        .load_one("region", deleted_id)
        .await
        .expect("read typed deleted region")
        .expect("typed deleted region");
    assert_eq!(typed_active.status, "active");
    assert_eq!(typed_deleted.status, "deleted");
    assert_eq!(typed_deleted.document_json, deleted_raw);
    assert_eq!(store.typed.typed_whitelist.count().await.unwrap(), 2);

    assert_eq!(
        store
            .list_whitelist_region_groups()
            .await
            .expect("list active groups")
            .len(),
        1
    );
}
