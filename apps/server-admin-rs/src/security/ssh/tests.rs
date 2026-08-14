use super::config::parse_allowed_region;
use super::*;

#[test]
fn parses_ssh_login_messages() {
    let success = parse_ssh_message(
        "sshd[1]: Accepted publickey for root from 1.2.3.4 port 456 ssh2",
        "2026-01-01T00:00:00Z",
        "auth.log",
    )
    .unwrap();
    assert_eq!(success["outcome"], json!("success"));
    assert_eq!(success["ip"], json!("1.2.3.4"));
    assert_eq!(success["port"], json!(456));

    let failure = parse_ssh_message(
        "sshd[1]: Failed password for invalid user admin from 5.6.7.8 port 22 ssh2",
        "2026-01-01T00:00:00Z",
        "auth.log",
    )
    .unwrap();
    assert_eq!(failure["outcome"], json!("failure"));
    assert_eq!(failure["invalid_user"], json!(true));
}

#[test]
fn normalizes_ssh_config_defaults() {
    let config = normalize_config(None);
    assert_eq!(config["window_minutes"], json!(10));
    assert_eq!(config["block_duration_unit"], json!("day"));
}

#[test]
fn supports_month_ssh_block_durations() {
    let config = normalize_config(Some(json!({
        "block_duration_value": 2,
        "block_duration_unit": "month"
    })));
    assert_eq!(config["block_duration_unit"], json!("month"));
    assert_eq!(ssh_block_duration_seconds(&config), 60 * 24 * 3600);
}

#[test]
fn disabled_ssh_runtime_keeps_compiled_allow_policy_for_offline_reenable() {
    let runtime = config::build_runtime_from_config(
        &json!({
            "enabled": false,
            "custom_cidrs": ["192.0.2.0/24", "2001:db8::/32"]
        }),
        compile_ip_set(std::iter::empty::<&str>()).unwrap(),
    )
    .unwrap();
    assert_eq!(runtime["enabled"], json!(false));
    assert!(runtime.get("allowed_cidrs").is_none());
    assert!(
        runtime["policy_id"]
            .as_str()
            .is_some_and(|id| id.starts_with("ipset-v2:"))
    );
    assert!(runtime["policy"].is_object());
    assert_eq!(runtime["range_count"], json!(2));
    let policy = config::policy_from_runtime(&runtime).unwrap();
    assert!(policy.contains("192.0.2.1".parse().unwrap()));
    assert!(policy.contains("2001:db8::1".parse().unwrap()));
}

#[test]
fn localizes_ssh_security_route_success_messages() {
    let zh = Translator::new("zh-CN");
    assert_eq!(
        ssh_security_route_text_params(
            &zh,
            "syncFirewallSuccess",
            &[
                ("allowedCidrs", "2".to_string()),
                ("ports", "22, 2222".to_string()),
                ("synced", "3".to_string())
            ],
        ),
        "已同步 2 条允许 CIDR 与 3 个 SSH 封锁 IP 到 22, 2222 端口"
    );
    assert_eq!(
        ssh_security_route_text(&zh, "clearFirewallSuccess"),
        "已清空 SSH 专用防火墙规则"
    );
}

#[test]
fn active_block_requires_applied_and_future_expiry() {
    let record = json!({
        "ip": "1.2.3.4",
        "blocked_at": "2026-01-01T00:00:00Z",
        "expires_at": "2999-01-01T00:00:00Z",
        "applied": true,
        "ports": ["22", "2222x", 0, 22]
    });
    let normalized = normalize_block_record(record).unwrap();
    assert!(is_active_block(&normalized, time_utils::now_ms()));
    assert_eq!(normalized["ports"], json!([22, 2222]));
}

#[test]
fn ssh_query_and_delete_parsers_match_node_edges() {
    assert_eq!(parse_positive(None, 1, 100), 1);
    assert_eq!(parse_positive(Some("2x"), 1, 100), 2);
    assert_eq!(parse_positive(Some("  +3.9"), 1, 100), 3);
    assert_eq!(parse_positive(Some("-1"), 1, 100), 1);
    assert_eq!(parse_positive(Some("999"), 1, 100), 100);

    assert_eq!(delete_ip_value_to_string(&Value::Null), "");
    assert_eq!(delete_ip_value_to_string(&json!(123)), "123");
    assert_eq!(
        delete_ip_value_to_string(&json!({"ip":"1.2.3.4"})),
        "[object Object]"
    );
    assert_eq!(
        delete_ip_value_to_string(&json!(["1.2.3.4", null, true])),
        "1.2.3.4,,true"
    );
}

#[test]
fn coalesces_success_login_logs_like_node_window() {
    let first = json!({
        "id": "a",
        "happened_at": "2026-01-01T00:00:00Z",
        "outcome": "success",
        "username": "root",
        "ip": "1.2.3.4",
        "source": "auth.log",
        "auth_method": "publickey",
        "port": 22,
        "raw": "first"
    });
    let second = json!({
        "id": "b",
        "happened_at": "2026-01-01T00:00:20Z",
        "outcome": "success",
        "username": "root",
        "ip": "1.2.3.4",
        "source": "auth.log",
        "auth_method": "publickey",
        "port": "2222",
        "raw": "second"
    });
    let failure = json!({
        "id": "c",
        "happened_at": "2026-01-01T00:00:21Z",
        "outcome": "failure",
        "username": "root",
        "ip": "1.2.3.4",
        "source": "auth.log",
        "raw": "failure"
    });

    let entries = coalesce_success_login_logs(vec![first, second, failure]);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0]["repeat_count"], json!(2));
    assert_eq!(entries[0]["related_ports"], json!([22, 2222]));
    assert_eq!(entries[0]["raw"], json!("first\nsecond"));
    assert_eq!(entries[1]["outcome"], json!("failure"));
}

#[test]
fn localizes_ssh_security_route_and_validation_text() {
    let zh = Translator::new("zh-CN");
    assert_eq!(
        ssh_security_route_text(&zh, "listBlocksFailed"),
        "获取 SSH 封锁列表失败"
    );
    let error = validate_cidrs(Some(&json!(["bad-cidr"])), &zh).unwrap_err();
    match error {
        SshError::BadRequest(message) => {
            assert_eq!(message, "自定义 CIDR 格式不正确：bad-cidr");
        }
        _ => panic!("expected bad request"),
    }
}

#[test]
fn ssh_regions_preserve_operator_and_reject_non_string_values() {
    let zh = Translator::new("zh-CN");
    let query = parse_allowed_region(
        &json!({ "province": "浙江", "query_city": "杭州", "operator": "联通" }),
        &zh,
    )
    .unwrap()
    .unwrap();
    assert_eq!(query.operator, Some(CidrOperator::Unicom));

    let error = parse_allowed_region(
        &json!({ "province": "浙江", "query_city": "杭州", "operator": [] }),
        &zh,
    )
    .unwrap_err();
    match error {
        SshError::BadRequest(message) => assert_eq!(message, "运营商仅支持电信、联通或移动"),
        _ => panic!("expected bad request"),
    }
}
