use super::*;

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
fn builds_reverse_proxy_source_map_without_private_ips() {
    let mut source_map = BTreeMap::new();
    add_ip_source(&mut source_map, "192.168.1.2", "session:a".to_string());
    add_ip_source(&mut source_map, "8.8.8.8", "session:b".to_string());
    assert!(!source_map.contains_key("192.168.1.2"));
    assert_eq!(
        source_map
            .get("8.8.8.8")
            .unwrap()
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec!["session:b".to_string()]
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
