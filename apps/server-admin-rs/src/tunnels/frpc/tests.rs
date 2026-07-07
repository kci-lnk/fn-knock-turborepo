use super::*;

#[test]
fn parses_frpc_summary_from_camel_case_toml() {
    let summary = build_summary(
        r#"
serverAddr = "frp.example.com"
serverPort = 7001

[[proxies]]
localPort = 7999
remotePort = 443
"#,
    );
    assert_eq!(summary.server_addr, "frp.example.com");
    assert_eq!(summary.server_port, "7001");
    assert_eq!(summary.local_port, "7999");
    assert_eq!(summary.remote_port, "443");
}

#[test]
fn parses_frpc_summary_like_node_toml_regex() {
    let summary = build_summary(
        r#"
serverAddr = frp.example.com
serverPort = 7001 # comment

[[proxies]]
localPort = "7999"
remotePort = 443 # comment
"#,
    );
    assert_eq!(summary.server_addr, "");
    assert_eq!(summary.server_port, "7000");
    assert_eq!(summary.local_port, "7999");
    assert_eq!(summary.remote_port, "");
}

#[test]
fn detected_frpc_runtime_clears_stale_exit_state() {
    let runtime = FrpcInstanceRuntime {
        desired_running: true,
        pid: Some(42),
        started_at: Some("2026-01-01T00:00:00Z".to_string()),
        stopped_at: Some("2026-01-01T00:01:00Z".to_string()),
        last_exit_code: Some(1),
        last_message: Some("frpc exited with code 1".to_string()),
    };

    let next = merge_detected_frpc_runtime(runtime.clone(), 42);
    assert_eq!(next.pid, Some(42));
    assert_eq!(next.started_at, runtime.started_at);
    assert_eq!(next.stopped_at, None);
    assert_eq!(next.last_exit_code, None);
    assert_eq!(
        next.last_message.as_deref(),
        Some("frpc process detected pid=42")
    );
    assert!(should_persist_detected_runtime(&runtime, &next));
}

#[test]
fn matches_frpc_process_config_args() {
    assert!(is_frpc_process_args_for_config(
        &[
            "/opt/frp/frpc".to_string(),
            "-c".to_string(),
            "/tmp/frpc.toml".to_string()
        ],
        "/tmp/frpc.toml"
    ));
    assert!(is_frpc_process_args_for_config(
        &["frpc".to_string(), "--config=/tmp/frpc.toml".to_string()],
        "/tmp/frpc.toml"
    ));
    assert!(!is_frpc_process_args_for_config(
        &[
            "frps".to_string(),
            "-c".to_string(),
            "/tmp/frpc.toml".to_string()
        ],
        "/tmp/frpc.toml"
    ));
}

#[test]
fn sanitizes_instance_ids_like_node() {
    assert_eq!(sanitize_instance_id("abc-123").as_deref(), Some("abc-123"));
    assert!(sanitize_instance_id("../bad").is_none());
    assert!(sanitize_instance_id("").is_none());
}

#[test]
fn default_instance_names_match_node_default_locale() {
    assert_eq!(default_frpc_primary_name(), "主 FRP");
    assert_eq!(default_frpc_instance_name(), "FRP 实例");
}

#[test]
fn log_limit_parser_matches_node_parse_int_prefixes() {
    assert_eq!(parse_limit(None), 200);
    assert_eq!(parse_limit(Some("")), 200);
    assert_eq!(parse_limit(Some("10x")), 10);
    assert_eq!(parse_limit(Some("0x10")), 1);
    assert_eq!(parse_limit(Some("-5")), 1);
    assert_eq!(parse_limit(Some("5000")), 1000);
    assert_eq!(parse_limit(Some("abc")), 200);
}

#[test]
fn localizes_frpc_errors_and_runtime_messages() {
    let translator = Translator::new("zh-CN");
    assert_eq!(
        localize_frpc_error(&translator, "FRPC instance not found: abc"),
        "FRP 实例不存在：abc"
    );
    assert_eq!(
        localize_frpc_error(&translator, "FRPC instance limit exceeded (20)"),
        "额外 FRP 实例最多支持 20 个"
    );
    assert_eq!(
        localize_frpc_error(&translator, "Primary FRPC instance cannot be deleted"),
        "主 FRP 实例不允许删除"
    );
    assert_eq!(
        localize_frpc_error(&translator, "frpc config verify failed with code 2"),
        "frpc verify 校验失败，退出码 2"
    );
    assert_eq!(
        localize_frpc_error(&translator, "Failed to read frpc pid"),
        "读取 frpc PID 失败"
    );

    let localized = localize_frpc_response_value(
        json!({
            "item": { "lastMessage": "frpc started pid=1234" },
            "status": { "lastMessage": "frpc exited with code 1" },
            "legacy": { "last_message": "frpc already stopped" }
        }),
        &translator,
    );
    assert_eq!(localized["item"]["lastMessage"], "frpc 已启动 pid=1234");
    assert_eq!(
        localized["status"]["lastMessage"],
        "frpc 进程已退出（退出码 1）"
    );
    assert_eq!(localized["legacy"]["last_message"], "frpc 已停止");
}
