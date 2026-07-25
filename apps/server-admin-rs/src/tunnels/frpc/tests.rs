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
fn process_alive_rejects_pid_values_outside_pid_t_range() {
    assert!(!is_process_alive(u32::MAX));
}

#[test]
fn legacy_runtime_is_promoted_to_a_supervisor_snapshot() {
    let runtime = normalize_runtime(json!({
        "desiredRunning": true,
        "pid": 42,
        "startedAt": "2026-01-01T00:00:00Z",
        "lastExitCode": 1,
        "lastMessage": "legacy message"
    }));
    assert!(runtime.supervisor.desired_running);
    assert!(runtime.supervisor.running);
    assert_eq!(runtime.supervisor.pid, Some(42));
    assert_eq!(runtime.supervisor.state, SupervisorPhase::Running);
}

#[test]
fn default_tunnel_state_matches_node_absent_key_shape() {
    let state = Value::Object(default_tunnel_state());
    assert_eq!(state["frp_enabled"], false);
    assert_eq!(state["cloudflared_enabled"], false);
    assert_eq!(state["last_tunnel"], "frp");
    assert_eq!(state["updated_at"], "1970-01-01T00:00:00.000Z");
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

#[test]
fn extracts_only_configured_frpc_secrets_for_log_redaction() {
    assert_eq!(
        extract_frpc_secrets(
            r#"
[auth]
method = "token"
token = "top-secret"
oidcClientSecret = 'oidc-secret'
serverAddr = "example.com"
[webServer]
password = "web-secret" # known credential
[proxies.plugin]
credentialFile = "/not/a/secret/value"
"#
        ),
        vec![
            "top-secret",
            "oidc-secret",
            "web-secret",
            "/not/a/secret/value"
        ]
    );
}

#[test]
fn verify_output_truncation_preserves_utf8_boundaries() {
    let output = "一".repeat(4_001);
    let normalized = normalize_verify_output(&output);
    assert_eq!(normalized.chars().count(), 4_003);
    assert!(normalized.ends_with("..."));
}
