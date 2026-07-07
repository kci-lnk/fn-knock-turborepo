use super::*;
use serde_json::json;

#[test]
fn sanitizes_initialization_rules_like_node() {
    let event = sanitize_event(json!({
        "trace_id": "t1",
        "action": "log",
        "rules": [
            { "id": 901, "file": "/x/REQUEST-901-INITIALIZATION.conf" },
            { "id": 1001, "file": "/x/rule.conf" }
        ],
        "rule_ids": [901, 1001],
        "interruption": { "rule_id": 901 }
    }))
    .unwrap();

    assert_eq!(event["rules"].as_array().unwrap().len(), 1);
    assert_eq!(event["rule_ids"], json!([1001]));
    assert!(event.get("interruption").is_none());
}

#[test]
fn drops_events_without_rule_or_blocking_signal() {
    assert!(
        sanitize_event(json!({
            "trace_id": "t1",
            "action": "log",
            "rules": []
        }))
        .is_none()
    );
    assert!(
        sanitize_event(json!({
            "trace_id": "t1",
            "action": "block"
        }))
        .is_some()
    );
}

#[test]
fn filters_waf_events_by_query() {
    let event = json!({
        "trace_id": "abc",
        "host": "example.com",
        "client_ip": "1.1.1.1",
        "route_type": "host",
        "mode": "blocking",
        "rule_ids": [1001],
        "path": "/login"
    });
    assert!(event_matches(
        &event,
        &WafLogQuery {
            date: None,
            trace_id: None,
            search: Some("login".to_string()),
            host: Some("EXAMPLE.com".to_string()),
            client_ip: Some("1.1.1.1".to_string()),
            rule_id: Some("1001".to_string()),
            route_type: Some("host".to_string()),
            mode: Some("blocking".to_string()),
            cursor: None,
            limit: None,
        }
    ));
}

#[test]
fn waf_query_number_parsers_match_node_parse_int_edges() {
    assert_eq!(normalize_limit(Some("10x")), 10);
    assert_eq!(normalize_limit(Some("  +3.9")), 3);
    assert_eq!(normalize_limit(Some("-1")), 50);
    assert_eq!(normalize_limit(Some("300")), 200);
    assert_eq!(normalize_limit(Some("0x10")), 50);

    assert_eq!(normalize_cursor(Some("12x")), 12);
    assert_eq!(normalize_cursor(Some("  +3.9")), 3);
    assert_eq!(normalize_cursor(Some("-1")), 0);
    assert_eq!(normalize_cursor(Some("0x10")), 0);
}

#[test]
fn waf_event_filters_match_node_unicode_and_rule_id_prefixes() {
    let event = json!({
        "trace_id": "abc",
        "host": "Ä.example",
        "client_ip": "1.1.1.1",
        "route_type": "host",
        "mode": "blocking",
        "rule_ids": [1001],
        "path": "/Älice"
    });

    assert!(event_matches(
        &event,
        &WafLogQuery {
            date: None,
            trace_id: None,
            search: Some("älice".to_string()),
            host: Some("ä.example".to_string()),
            client_ip: None,
            rule_id: Some("1001x".to_string()),
            route_type: None,
            mode: None,
            cursor: None,
            limit: None,
        }
    ));

    assert!(!event_matches(
        &event,
        &WafLogQuery {
            date: None,
            trace_id: None,
            search: None,
            host: None,
            client_ip: None,
            rule_id: Some("nope".to_string()),
            route_type: None,
            mode: None,
            cursor: None,
            limit: None,
        }
    ));
}

#[test]
fn normalizes_waf_rule_filenames_like_node() {
    assert_eq!(
        safe_rule_filename("../custom rule.conf").unwrap(),
        "custom-rule.conf"
    );
    assert!(safe_rule_filename("../secret.txt").is_err());
    assert!(safe_rule_filename("..").is_err());
}

#[test]
fn localizes_waf_route_and_service_errors() {
    let translator = Translator::new("zh-CN");

    assert_eq!(
        waf_text(&translator, "detailsLoadFailed"),
        "读取 WAF 详情失败"
    );
    assert_eq!(
        localize_waf_error(&translator, "WAF manifest is empty"),
        "系统规则清单为空"
    );
    assert_eq!(
        localize_waf_error(&translator, "Duplicate WAF bundle file: REQUEST.conf"),
        "系统规则包内存在重复文件: REQUEST.conf"
    );
    assert_eq!(
        localize_waf_error(&translator, "WAF rule file is too large: custom.conf"),
        "custom.conf 超过 1MB"
    );
    assert_eq!(
        localize_waf_error(&translator, "Invalid WAF rule source"),
        "规则来源不正确"
    );
    assert_eq!(
        localize_waf_error(&translator, "invalid date, expected YYYY-MM-DD"),
        "日期格式不正确，应为 YYYY-MM-DD"
    );
}

#[test]
fn blocks_filesystem_directives_in_uploaded_rules() {
    assert!(contains_blocked_directive("  Include /tmp/*.conf"));
    assert!(contains_blocked_directive("SecAuditLog /tmp/audit.log"));
    assert!(!contains_blocked_directive(
        "SecRule ARGS attack \"id:1001\""
    ));
}

#[test]
fn defaults_high_noise_system_rules_to_disabled() {
    assert!(is_system_rule_enabled_by_default(
        "REQUEST-901-INITIALIZATION.conf"
    ));
    assert!(!is_system_rule_enabled_by_default(
        "REQUEST-942-APPLICATION-ATTACK-SQLI.conf"
    ));
    assert!(is_system_rule_enabled_by_default(
        "REQUEST-949-BLOCKING-EVALUATION.conf"
    ));
}

#[test]
fn validates_waf_bundle_paths() {
    assert_eq!(
        safe_bundle_entry_path("REQUEST-920-PROTOCOL-ENFORCEMENT.conf").unwrap(),
        "REQUEST-920-PROTOCOL-ENFORCEMENT.conf"
    );
    assert!(safe_bundle_entry_path("../evil.conf").is_err());
    assert!(safe_bundle_entry_path("/absolute.conf").is_err());
    assert!(safe_bundle_entry_path("nested//rule.conf").is_err());
}
