use super::*;

fn defaults() -> ScannerEnvDefaults {
    ScannerEnvDefaults {
        enabled: false,
        window_minutes: 5,
        threshold: 5,
        blacklist_ttl_seconds: 90 * 24 * 3600,
    }
}

#[test]
fn scanner_settings_preserve_node_defaults_and_effective_cidrs() {
    let raw = json!({
        "enabled": true,
        "windowMinutes": 2,
        "threshold": 3,
        "blacklistTtlSeconds": 120,
        "commonLocationExemptEnabled": true,
        "cidrExemptions": [" 10.0.0.0/8 ", "10.0.0.0/8", "bad"],
        "cidrExemptionRegions": [{
            "province": "广东",
            "city": null,
            "label": "广东全省",
            "value": "__province_all__",
            "query_city": null,
            "is_province_wide": true,
            "is_municipality": false
        }],
        "cidrExemptionRegionCidrs": ["1.1.1.0/24"]
    });

    let settings = scanner_settings_from_raw(Some(&raw), defaults());

    assert!(settings.enabled);
    assert_eq!(settings.window_seconds, SCANNER_BASE_WINDOW_SECONDS);
    assert_eq!(settings.cidr_exemptions, vec!["10.0.0.0/8"]);
    assert!(settings.cidr_exemption_region_cidrs.is_empty());
    assert!(settings.cidr_exemption_cidrs.is_empty());
}

#[test]
fn scanner_compaction_removes_legacy_arrays_and_deduplicates_equal_policies() {
    let raw = json!({
        "enabled": true,
        "cidrExemptions": [],
        "cidrExemptionRegions": [{
            "province": "浙江",
            "city": null,
            "label": "浙江全省",
            "value": "__province_all__",
            "query_city": null,
            "is_province_wide": true,
            "is_municipality": false
        }],
        "cidrExemptionRegionCidrs": ["192.0.2.0/25", "192.0.2.128/25"],
        "cidrExemptionCidrs": ["192.0.2.0/24"]
    });
    let (stored, policy) = settings::compact_scanner_settings(&raw).unwrap();
    assert!(stored.get("cidrExemptionRegionCidrs").is_none());
    assert!(stored.get("cidrExemptionCidrs").is_none());
    assert!(stored["cidrExemptionRegionPolicy"].is_object());
    assert!(stored.get("cidrExemptionPolicy").is_none());
    assert_eq!(stored["cidrExemptionPolicyId"], json!(policy.id));
    assert_eq!(stored["cidrExemptionRangeCount"], json!(1));
}

#[test]
fn validates_cidr_exemptions_without_canonicalizing_values() {
    assert_eq!(
        validate_scanner_cidr_exemptions(vec![
            " 2001:DB8::/32 ".to_string(),
            "2001:db8::/32".to_string(),
            "192.168.0.0/16".to_string(),
        ])
        .unwrap(),
        vec!["2001:DB8::/32", "192.168.0.0/16"]
    );
    assert!(validate_scanner_cidr_exemptions(vec!["10.0.0.0/33".to_string()]).is_err());
}

#[test]
fn parses_blacklist_delete_body_shapes_like_node() {
    assert_eq!(
        parse_blacklist_delete_ips(br#"["1.1.1.1", 2, " 2.2.2.2 "]"#).unwrap(),
        vec!["1.1.1.1", " 2.2.2.2 "]
    );
    assert_eq!(
        parse_blacklist_delete_ips(br#"{"ips":["3.3.3.3","3.3.3.3"]}"#).unwrap(),
        vec!["3.3.3.3", "3.3.3.3"]
    );
    assert_eq!(
        parse_blacklist_delete_ips(br#"["   "]"#).unwrap(),
        vec!["   "]
    );
    assert!(parse_blacklist_delete_ips(br#""not-json""#).is_err());
}

#[test]
fn scanner_query_int_parser_matches_node_parse_int_edges() {
    assert_eq!(parse_i64(None, 20), 20);
    assert_eq!(parse_i64(Some(""), 20), 20);
    assert_eq!(parse_i64(Some("   "), 20), 20);
    assert_eq!(parse_i64(Some("2x"), 20), 2);
    assert_eq!(parse_i64(Some("  +3.9"), 20), 3);
    assert_eq!(parse_i64(Some("-1"), 20), -1);
}

#[test]
fn subsonic_rest_endpoint_parser_matches_node_route_regex() {
    assert_eq!(normalize_subsonic_rest_endpoint("/rest/ping.view"), "ping");
    assert_eq!(
        normalize_subsonic_rest_endpoint("/rest/getLicense.json"),
        "getlicense"
    );
    assert_eq!(normalize_subsonic_rest_endpoint("/rest/ping.xml"), "ping");
    assert_eq!(normalize_subsonic_rest_endpoint("/rest/ping"), "ping");
    assert_eq!(normalize_subsonic_rest_endpoint("/rest/ping.bad"), "");
    assert_eq!(normalize_subsonic_rest_endpoint("/rest/foo/bar"), "");
    assert_eq!(
        normalize_subsonic_rest_endpoint("/rest/ping.view/extra"),
        ""
    );
}

#[test]
fn scanner_local_address_detection_matches_node_regex_edges() {
    assert!(is_scanner_local_address("10.999.999.999"));
    assert!(is_scanner_local_address("10.0.0.1:7999"));
    assert!(!is_scanner_local_address("10.0.0.1:bad"));
    assert!(is_scanner_local_address("127.999.999.999"));
    assert!(is_scanner_local_address("192.168.999.999"));
    assert!(is_scanner_local_address("172.16.999.999"));
    assert!(is_scanner_local_address("172.31.999.999"));
    assert!(!is_scanner_local_address("172.32.0.1"));
    assert!(!is_scanner_local_address("172.016.0.1"));
    assert!(is_scanner_local_address("::ffff:10.999.999.999"));
    assert!(!is_scanner_local_address("::ffff:10.0.0.1:bad"));
}

#[test]
fn scanner_host_normalization_preserves_node_fallback_port_rules() {
    assert_eq!(normalize_scanner_host("Example.COM:7999"), "example.com");
    assert_eq!(normalize_scanner_host("foo:bar"), "foo:bar");
    assert_eq!(normalize_scanner_host("foo:123abc"), "foo:123abc");
    assert_eq!(normalize_scanner_host("http://foo.example/path"), "http");
    assert_eq!(normalize_scanner_host("2001:db8::1"), "2001:db8:");
}

#[test]
fn dedupes_region_inputs_by_province_and_query_city() {
    let regions = dedupe_scanner_cidr_exemption_region_inputs(vec![
        ScannerCidrExemptionRegionBody {
            province: " 广东 ".to_string(),
            query_city: None,
            operator: None,
        },
        ScannerCidrExemptionRegionBody {
            province: "广东".to_string(),
            query_city: Some("".to_string()),
            operator: None,
        },
        ScannerCidrExemptionRegionBody {
            province: "广东".to_string(),
            query_city: Some("深圳".to_string()),
            operator: None,
        },
        ScannerCidrExemptionRegionBody {
            province: "广东".to_string(),
            query_city: Some("深圳".to_string()),
            operator: Some(json!("移动")),
        },
    ])
    .unwrap();

    assert_eq!(regions.len(), 3);
    assert_eq!(regions[0].province, "广东");
    assert_eq!(regions[0].query_city, None);
    assert_eq!(regions[1].query_city.as_deref(), Some("深圳"));
    assert_eq!(regions[2].operator, Some(CidrOperator::Mobile));
}

#[test]
fn scanner_regions_reject_non_string_operator() {
    let result =
        dedupe_scanner_cidr_exemption_region_inputs(vec![ScannerCidrExemptionRegionBody {
            province: "浙江".to_string(),
            query_city: Some("杭州".to_string()),
            operator: Some(json!({ "unexpected": true })),
        }]);
    assert!(result.is_err());
}

#[test]
fn builds_public_cidr_lookup_payload_with_camel_case_fields() {
    let payload = crate::cidr::lookup_payload_from_data(
        &CidrRegionQuery::new("广东", Some("深圳"), Some(CidrOperator::Mobile)),
        &json!({
            "province": "广东",
            "city": "深圳",
            "cidr_groups": {
                "4": ["1.1.1.0/24"],
                "6": ["2001:db8::/32"]
            },
            "counts": {
                "4": 10,
                "6": 1
            }
        }),
        None,
    );

    assert_eq!(payload["selection"]["queryCity"], "深圳");
    assert_eq!(payload["selection"]["operator"], "移动");
    assert_eq!(payload["selection"]["label"], "深圳 · 移动");
    assert_eq!(payload["selection"]["isProvinceWide"], false);
    assert_eq!(payload["cidrGroups"]["ipv4"][0], "1.1.1.0/24");
    assert_eq!(payload["counts"]["ipv4"], 10);
    assert_eq!(payload["totalCount"], 11);
}

#[test]
fn cidr_cities_total_fallback_excludes_province_wide_option_like_node() {
    assert_eq!(crate::cidr::cities_total(&json!({}), 2), 2);
    assert_eq!(crate::cidr::cities_total(&json!({ "total": "7.9" }), 2), 7);
}

#[test]
fn public_cidr_payload_localizes_province_wide_label_like_node() {
    let en = Translator::new("en-US");
    let payload = crate::cidr::lookup_payload_from_data(
        &CidrRegionQuery::new("Guangdong", None::<String>, None),
        &json!({
            "province": "Guangdong",
            "cidr_groups": {
                "4": [],
                "6": []
            }
        }),
        Some(&en),
    );

    assert_eq!(
        crate::cidr::province_wide_label(Some(&en), "Guangdong"),
        "All Guangdong"
    );
    assert_eq!(payload["selection"]["label"], "All Guangdong");
    assert_eq!(payload["selection"]["value"], "__province_all__");
}

#[test]
fn public_cidr_payload_preserves_upstream_cidr_arrays_like_node() {
    let payload = crate::cidr::lookup_payload_from_data(
        &CidrRegionQuery::new("广东", Some("深圳"), None),
        &json!({
            "province": "广东",
            "city": "深圳",
            "cidr_groups": {
                "4": ["1.1.1.0/24", 123, null],
                "6": []
            }
        }),
        None,
    );

    assert_eq!(payload["cidrGroups"]["ipv4"][0], "1.1.1.0/24");
    assert_eq!(payload["cidrGroups"]["ipv4"][1], 123);
    assert_eq!(payload["cidrGroups"]["ipv4"][2], Value::Null);
    assert_eq!(payload["counts"]["ipv4"], 3);
    assert_eq!(payload["totalCount"], 3);
}

#[test]
fn resolves_cidr_api_base_url_like_node_helper() {
    assert_eq!(
        crate::cidr::normalize_cidr_api_base_url("https://example.test").unwrap(),
        "https://example.test/api/v1"
    );
    assert_eq!(
        crate::cidr::normalize_cidr_api_base_url("https://example.test/custom/").unwrap(),
        "https://example.test/custom"
    );
}

#[test]
fn localizes_scanner_and_cidr_route_errors() {
    let translator = Translator::new("zh-CN");
    assert_eq!(
        localize_scanner_error(&translator, "Invalid request body"),
        "请求体不正确"
    );
    assert_eq!(
        localize_scanner_error(&translator, "At least one IP is required"),
        "请至少提供一个 IP"
    );
    assert_eq!(
        localize_scanner_error(&translator, "Invalid CIDR exemptions: 10.0.0.0/33"),
        "CIDR 豁免格式不正确：10.0.0.0/33"
    );
    assert_eq!(
        localize_cidr_error(&translator, "CIDR upstream request failed: HTTP 502"),
        "CIDR 上游请求失败 (502)"
    );
    assert_eq!(
        localize_cidr_error(&translator, "CIDR upstream response missing data"),
        "CIDR 上游返回异常"
    );
    assert_eq!(
        localize_cidr_error(&translator, "CIDR operator filtering is unsupported"),
        "当前 CIDR 服务不支持运营商筛选，请升级 CIDR 容器至 0.1.3 或更高版本"
    );
}
