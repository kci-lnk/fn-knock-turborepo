use super::*;

#[test]
fn build_session_name_matches_node_prefix_and_length() {
    assert_eq!(
        build_session_name("12345678-90ab-cdef-1111-222233334444"),
        "fnk_1234567890abcdef"
    );
}

#[test]
fn default_session_title_skips_used_indexes() {
    let translator = Translator::new("zh-CN");
    let sessions = vec![
        TerminalSessionRecord {
            title: "会话-1".to_string(),
            ..Default::default()
        },
        TerminalSessionRecord {
            title: "Terminal Session 3".to_string(),
            ..Default::default()
        },
    ];
    assert_eq!(
        build_default_session_title(&sessions, &translator),
        "会话-2"
    );
}

#[test]
fn normalize_session_default_title_and_shape_match_node() {
    let session = normalize_session(TerminalSessionRecord {
        cwd: "/".to_string(),
        ..Default::default()
    });
    assert_eq!(session.title, terminal_default_text("defaultTitle", &[]));

    let value = serde_json::to_value(&session).expect("serialize terminal session");
    assert_eq!(
        value.get("last_frame_revision").and_then(Value::as_str),
        Some("")
    );
}

#[test]
fn pane_snapshot_output_normalizes_crlf_like_node_regex() {
    assert_eq!(normalize_pane_snapshot_output("a\nb\n"), "a\r\nb");
    assert_eq!(normalize_pane_snapshot_output("a\r\nb\r\n"), "a\r\nb");
    assert_eq!(normalize_pane_snapshot_output("a\rb"), "a\rb");
    assert_eq!(normalize_pane_snapshot_output("  \r\n\t"), "");
}

#[test]
fn normalizes_terminal_feature_like_node() {
    let value = serde_json::json!({
        "enabled": true,
        "default_cwd": "",
        "max_sessions": 99,
        "idle_timeout_seconds": 1,
        "allow_mobile_toolbar": false,
        "dangerously_run_as_current_user": false
    });
    assert_eq!(normalize_terminal_feature(Some(&value)).max_sessions, 12);
    assert_eq!(
        normalize_terminal_feature(Some(&value)).idle_timeout_seconds,
        60
    );
    assert_eq!(normalize_terminal_feature(Some(&value)).default_cwd, "~");
    assert!(!normalize_terminal_feature(Some(&value)).allow_mobile_toolbar);

    let value = serde_json::json!({
        "max_sessions": "2x",
        "idle_timeout_seconds": "90.8"
    });
    assert_eq!(normalize_terminal_feature(Some(&value)).max_sessions, 2);
    assert_eq!(
        normalize_terminal_feature(Some(&value)).idle_timeout_seconds,
        90
    );

    let value = serde_json::json!({
        "max_sessions": ["4.9"]
    });
    assert_eq!(normalize_terminal_feature(Some(&value)).max_sessions, 4);
}

#[test]
fn normalizes_terminal_runtime_default_cwd_to_home_marker() {
    let value = serde_json::json!({
        "default_cwd": "/usr/local/etc/fn-knock/"
    });
    assert_eq!(normalize_terminal_feature(Some(&value)).default_cwd, "~");
}

#[test]
fn auto_shell_candidates_prefer_zsh_like_node() {
    assert_eq!(
        auto_shell_candidates_from_env("/bin/bash"),
        vec![
            "zsh",
            "/bin/zsh",
            "/usr/bin/zsh",
            "/bin/bash",
            "bash",
            "/usr/bin/bash",
            "sh",
            "/bin/sh",
            "/usr/bin/sh",
        ]
    );
    assert_eq!(
        auto_shell_candidates_from_env("/opt/homebrew/bin/zsh"),
        vec![
            "/opt/homebrew/bin/zsh",
            "zsh",
            "/bin/zsh",
            "/usr/bin/zsh",
            "bash",
            "/bin/bash",
            "/usr/bin/bash",
            "sh",
            "/bin/sh",
            "/usr/bin/sh",
        ]
    );
    assert_eq!(
        auto_shell_candidates_from_env("/bin/zsh"),
        vec![
            "/bin/zsh",
            "zsh",
            "/usr/bin/zsh",
            "bash",
            "/bin/bash",
            "/usr/bin/bash",
            "sh",
            "/bin/sh",
            "/usr/bin/sh",
        ]
    );
}

#[test]
fn zsh_session_command_uses_login_interactive_shell_like_node() {
    assert_eq!(
        build_session_shell_command("/bin/zsh"),
        "exec '/bin/zsh' -il"
    );
    assert_eq!(build_session_shell_command("/bin/bash"), "exec '/bin/bash'");
}

#[test]
fn terminal_dimensions_match_node_number_rules() {
    assert_eq!(normalize_terminal_dimension(None, 120, 40, 400), 120);
    assert_eq!(normalize_terminal_dimension(Some(0.0), 120, 40, 400), 120);
    assert_eq!(normalize_terminal_dimension(Some(80.9), 120, 40, 400), 80);
    assert_eq!(normalize_terminal_dimension(Some(-1.2), 120, 40, 400), 40);
    assert_eq!(normalize_terminal_dimension(Some(999.0), 120, 40, 400), 400);
}

#[test]
fn terminal_poll_timeout_matches_node_default_and_clamp_rules() {
    assert_eq!(
        normalize_terminal_poll_timeout_ms(None),
        DEFAULT_POLL_TIMEOUT_MS
    );
    assert_eq!(
        normalize_terminal_poll_timeout_ms(Some(0.0)),
        DEFAULT_POLL_TIMEOUT_MS
    );
    assert_eq!(normalize_terminal_poll_timeout_ms(Some(500.0)), 1_000);
    assert_eq!(normalize_terminal_poll_timeout_ms(Some(1500.8)), 1_500);
    assert_eq!(normalize_terminal_poll_timeout_ms(Some(30_000.0)), 20_000);
}

#[test]
fn output_cursor_parser_matches_node_parse_int_edges() {
    assert_eq!(parse_output_cursor_like_node(None), 0);
    assert_eq!(parse_output_cursor_like_node(Some("")), 0);
    assert_eq!(parse_output_cursor_like_node(Some("   ")), 0);
    assert_eq!(parse_output_cursor_like_node(Some("2x")), 2);
    assert_eq!(parse_output_cursor_like_node(Some("  +3.9")), 3);
    assert_eq!(parse_output_cursor_like_node(Some("-1")), 0);
}

#[test]
fn home_dir_resolution_matches_node_homedir_fallback() {
    assert_eq!(
        resolve_home_dir(
            Some(" /home/fn "),
            Some(PathBuf::from("/root")),
            Some(Path::new("/srv/fn-knock")),
        ),
        PathBuf::from("/home/fn")
    );
    assert_eq!(
        resolve_home_dir(None, Some(PathBuf::from("/root")), None),
        PathBuf::from("/root")
    );
    assert_eq!(
        resolve_home_dir(Some(""), Some(PathBuf::from("/root")), None),
        PathBuf::from("/root")
    );
    assert_eq!(resolve_home_dir(None, None, None), PathBuf::from("/"));
}

#[test]
fn home_dir_prefers_account_home_when_env_home_is_runtime_directory() {
    assert_eq!(
        resolve_home_dir(
            Some("/usr/local/etc/fn-knock"),
            Some(PathBuf::from("/root")),
            Some(Path::new("/usr/local/etc/fn-knock")),
        ),
        PathBuf::from("/root")
    );
}

#[test]
fn localizes_terminal_error_from_default_locale() {
    let translator = Translator::new("en");
    let raw = terminal_default_text("sessionLimitReached", &[("count", "3".to_string())]);
    assert_eq!(
        localize_terminal_error(&translator, &raw),
        "Terminal session limit reached (3)"
    );

    let raw = format!(
        "{}: broken pipe",
        terminal_default_text("inputSendFailed", &[])
    );
    assert_eq!(
        localize_terminal_error(&translator, &raw),
        "Failed to send terminal input: broken pipe"
    );
}

#[test]
fn tmux_install_state_defaults_use_server_locale_messages() {
    let state = default_tmux_install_state();
    assert_eq!(state.message, "未检测到 tmux，请先安装 tmux 环境");

    let ready = terminal_default_text(
        "tmuxInstallCompleteWithVersion",
        &[("version", "tmux 3.4".to_string())],
    );
    assert_eq!(ready, "tmux 安装完成：tmux 3.4");
}

#[test]
fn tmux_error_state_message_is_not_double_wrapped() {
    let translator = Translator::new("en");
    let mut state = TerminalTmuxInstallState {
        status: "error".to_string(),
        progress: 0,
        message: format!("{}: broken", terminal_default_text("aptUpdateFailed", &[])),
        executable_path: String::new(),
        detection_source: None,
        version: String::new(),
    };
    localize_tmux_install_state(&mut state, &translator);
    assert_eq!(state.message, "apt-get update failed: broken");

    let blocked_reason = translator.t_params(
        "server.terminal.tmuxStatusError",
        &[("message", state.message.clone())],
    );
    assert_eq!(
        blocked_reason,
        "tmux status error: apt-get update failed: broken"
    );
}

#[test]
fn localizes_terminal_error_from_english_and_wraps_unknown() {
    let translator = Translator::new("zh-CN");
    assert_eq!(
        localize_terminal_error(&translator, "Failed to resize terminal: tmux failed"),
        "终端尺寸调整失败: tmux failed"
    );
    assert_eq!(
        localize_terminal_error(&translator, "run process tmux: No such file or directory"),
        "终端操作失败：run process tmux: No such file or directory"
    );
}

#[test]
fn relay_command_uses_shell_paths_without_node_runtime() {
    let command = build_relay_command(Path::new("/tmp/a b.log"), Path::new("/tmp/a.in"))
        .expect("relay command");
    assert!(command.starts_with("sh -c "));
    assert!(!command.contains("node"));
    assert!(command.contains("cat >>"));
}
