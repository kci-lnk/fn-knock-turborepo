use super::*;

async fn generic_linux_test_state() -> (tempfile::TempDir, AppState) {
    let directory = tempfile::tempdir().unwrap();
    let mut settings = {
        let _environment = crate::test_support::EnvGuard::new(&[]);
        crate::settings::Settings::from_env()
    };
    settings.runtime_target = "linux".to_string();
    settings.data_dir = directory.path().join("data");
    settings.gateway_config_dir = directory.path().join("gateway");
    settings.sqlite_path = directory.path().join("fn-knock.sqlite3");
    settings.legacy_redis_url = String::new();
    settings.internal_rpc_token = "test-internal-rpc-token".to_string();

    let state = AppState::new(settings).await.unwrap();
    (directory, state)
}

#[tokio::test]
async fn generic_linux_root_rejects_clock_sync_and_dnsmasq_install() {
    let profile = crate::runtime_profile::RuntimeProfile {
        deployment_target: "linux".to_string(),
        is_docker: false,
        is_linux: true,
        is_macos: false,
        is_windows: false,
        is_root_process: true,
    };
    assert!(!super::runtime::system_clock_sync_available_for_profile(
        &profile
    ));
    assert!(!super::runtime::smart_connect_available_for_profile(
        &profile
    ));

    let (_directory, state) = generic_linux_test_state().await;
    let clock_response = clock_sync(axum::extract::State(state.clone())).await;
    let dnsmasq_response = dnsmasq_install(axum::extract::State(state)).await;

    assert_eq!(clock_response.status(), axum::http::StatusCode::FORBIDDEN);
    assert_eq!(dnsmasq_response.status(), axum::http::StatusCode::FORBIDDEN);
}

#[test]
fn detects_frp_platform_names_like_node() {
    let platform = detect_frp_platform();
    assert!(
        [
            "darwin-arm64",
            "darwin-amd64",
            "linux-amd64",
            "linux-arm64",
            "linux-arm",
            "unsupported"
        ]
        .contains(&platform)
    );
}

#[test]
fn validates_downloaded_darwin_binary_architectures() {
    assert!(downloads::downloaded_architecture_matches(
        "darwin-arm64",
        "Mach-O 64-bit executable arm64"
    ));
    assert!(downloads::downloaded_architecture_matches(
        "darwin-amd64",
        "Mach-O 64-bit executable x86_64"
    ));
    assert!(!downloads::downloaded_architecture_matches(
        "darwin-arm64",
        "Mach-O 64-bit executable x86_64"
    ));
    assert!(!downloads::downloaded_architecture_matches(
        "darwin-arm64",
        "Mach-O universal binary with 2 architectures: [x86_64] [arm64]"
    ));
    assert!(downloads::downloaded_windows_architecture_matches(
        "windows-amd64",
        0x8664
    ));
    assert!(downloads::downloaded_windows_architecture_matches(
        "windows-386",
        0x014c
    ));
    assert!(!downloads::downloaded_windows_architecture_matches(
        "windows-amd64",
        0x014c
    ));
}

#[test]
fn rejects_unsafe_frp_archive_entries() {
    assert!(downloads::frp_archive_entry_path_is_safe(
        "frp_0.67.0_darwin_arm64",
        "frp_0.67.0_darwin_arm64/frpc"
    ));
    for unsafe_path in [
        "../frpc",
        "/tmp/frpc",
        "another-root/frpc",
        "frp_0.67.0_darwin_arm64/../frpc",
        "frp_0.67.0_darwin_arm64//frpc",
    ] {
        assert!(!downloads::frp_archive_entry_path_is_safe(
            "frp_0.67.0_darwin_arm64",
            unsafe_path
        ));
    }
    assert!(downloads::frp_archive_entry_type_is_safe(
        "-rwxr-xr-x root/wheel frpc"
    ));
    assert!(downloads::frp_archive_entry_type_is_safe(
        "drwxr-xr-x root/wheel directory"
    ));
    for unsafe_type in ['l', 'h', 'c', 'b', 'p'] {
        assert!(!downloads::frp_archive_entry_type_is_safe(&format!(
            "{unsafe_type}rwxr-xr-x root/wheel entry"
        )));
    }
}

#[test]
fn builds_frp_binary_path_for_supported_platforms() {
    let path = frp_binary_path(Path::new("/tmp/data"), "linux-amd64", "frpc").unwrap();
    assert!(path.ends_with("frp/frp_0.67.0_linux_amd64/frpc"));
    assert!(frp_binary_path(Path::new("/tmp/data"), "unsupported", "frpc").is_none());
}

#[test]
fn builds_dnsmasq_bootstrap_config_like_node() {
    let config = dnsmasq_bootstrap_config();
    assert!(config.contains("local-ttl=30"));
    assert!(config.contains("listen-address=127.0.0.1"));
    assert!(config.contains("bind-interfaces"));
    assert!(config.ends_with('\n'));
}

#[test]
fn localizes_system_asset_and_dnsmasq_messages() {
    let zh = Translator::new("zh-CN");
    assert_eq!(
        tunnel_manager_text(&zh, "cloudflared", "downloadStarted"),
        "已开始下载 Cloudflared"
    );
    assert_eq!(
        cloudflared_delete_unsupported_message(&zh, "unsupported").as_deref(),
        Some("当前平台不受支持")
    );
    assert_eq!(
        cloudflared_delete_unsupported_message(&zh, "darwin-amd64"),
        None
    );
    assert_eq!(
        cloudflared_delete_unsupported_message(&zh, "windows-amd64"),
        None
    );
    assert_eq!(
        tunnel_manager_text_params(
            &zh,
            "frp",
            "deleteFailed",
            &[("detail", "权限不足".to_string())]
        ),
        "删除 FRP 失败：权限不足"
    );
    assert_eq!(dnsmasq_ready_message(&zh, "2.90"), "dnsmasq 已就绪：2.90");
    assert_eq!(
        dnsmasq_detected_message(&zh, "2.90", true),
        "dnsmasq 已检测到：2.90，等待初始化或启动服务"
    );
    assert_eq!(
        dnsmasq_detected_message(&zh, "2.90", false),
        "缺少系统服务，初始化时会自动补全"
    );
    assert_eq!(
        dnsmasq_install_state_to_json(&DnsmasqInstallState::default(), &zh)["message"],
        "未检测到 dnsmasq，请先完成安装"
    );
    assert_eq!(
        normalize_dnsmasq_error(
            &zh,
            "failed to create listening socket for port 53: Address already in use",
            "restartFailed",
        ),
        "DNS 53 端口不可用，请先释放端口后重试：failed to create listening socket for port 53: Address already in use"
    );

    let en = Translator::new("en");
    assert_eq!(
        tunnel_manager_text(&en, "frp", "deleteSuccess"),
        "FRP deleted"
    );
    assert_eq!(
        dnsmasq_text(&en, "checkingEnvironment"),
        "Checking dnsmasq environment..."
    );
}

#[test]
fn resolves_dnsmasq_install_state_like_node() {
    let zh = Translator::new("zh-CN");

    let installing = resolve_dnsmasq_install_state(
        &zh,
        Some("2.90"),
        true,
        true,
        true,
        dnsmasq_state("installing", 42, "installing now".to_string()),
    );
    assert_eq!(installing.status, "installing");
    assert_eq!(installing.progress, 42);
    assert_eq!(installing.message, "installing now");

    let previous_error = dnsmasq_state("error", 0, "bind failed".to_string());
    let preserved_error = resolve_dnsmasq_install_state(
        &zh,
        Some("2.90"),
        false,
        false,
        true,
        previous_error.clone(),
    );
    assert_eq!(preserved_error.status, "error");
    assert_eq!(preserved_error.message, "bind failed");

    let ready_overrides_error =
        resolve_dnsmasq_install_state(&zh, Some("2.90"), true, true, true, previous_error);
    assert_eq!(ready_overrides_error.status, "installed");
    assert_eq!(ready_overrides_error.message, "dnsmasq 已就绪：2.90");

    let missing_service = resolve_dnsmasq_install_state(
        &zh,
        Some("2.90"),
        false,
        false,
        false,
        DnsmasqInstallState::default(),
    );
    assert_eq!(missing_service.status, "installed");
    assert_eq!(missing_service.message, "缺少系统服务，初始化时会自动补全");
}

#[test]
fn localizes_asset_progress_errors() {
    let zh = Translator::new("zh-CN");
    assert_eq!(
        localize_asset_progress_error(&zh, "frp", "Download cancelled"),
        "下载已取消"
    );
    assert_eq!(
        localize_asset_progress_error(&zh, "cloudflared", "Cloudflared platform is unsupported"),
        "当前平台不受支持"
    );
    assert_eq!(
        localize_asset_progress_error(&zh, "frp", "FRP package extraction failed with code 2"),
        "解压失败，退出码 2"
    );
    assert_eq!(
        localize_asset_progress_error(&zh, "frp", "FRP download failed: HTTP 503"),
        "下载失败：HTTP 503"
    );
    assert_eq!(
        localize_asset_progress_error(&zh, "frp", "Download failed"),
        "下载失败：未知错误"
    );
    assert_eq!(
        localize_asset_progress_error(
            &zh,
            "frp",
            "FRP download failed: Download response timed out after 120s without receiving data"
        ),
        "下载失败：连接超时"
    );
    assert_eq!(
        localize_asset_progress_error(
            &zh,
            "frp",
            "FRP download failed: Download connection timed out after 30s"
        ),
        "下载失败：连接超时"
    );
    assert_eq!(
        localize_asset_progress_error(
            &zh,
            "frp",
            "FRP download failed: Download timed out after 1800s total"
        ),
        "下载失败：连接超时"
    );
    assert_eq!(
        localize_asset_progress_error(&zh, "cloudflared", "Download response body is unreadable"),
        "下载响应体不可读"
    );
}

#[test]
fn preserves_clock_sync_metadata_across_status_refresh() {
    let previous = json!({
        "syncInProgress": true,
        "lastSyncAt": "2026-07-07T01:02:03Z",
        "lastSyncError": "boom",
        "syncSummary": "done"
    });
    let mut status = initial_clock_status();

    preserve_clock_sync_metadata_from(&mut status, Some(&previous));

    assert_eq!(status["syncInProgress"], true);
    assert_eq!(status["lastSyncAt"], "2026-07-07T01:02:03Z");
    assert_eq!(status["lastSyncError"], "boom");
    assert_eq!(status["syncSummary"], "done");
}

#[test]
fn calculates_clock_sync_target_like_node() {
    assert_eq!(clock_sync_target_epoch_ms(10_000, 1_000, 3_500), 12_500);
    assert_eq!(clock_sync_target_epoch_ms(10_000, 3_500, 1_000), 10_000);
}

#[test]
fn rounds_network_latency_compensation_like_node() {
    assert_eq!(network_latency_compensation_ms(0), 0);
    assert_eq!(network_latency_compensation_ms(1), 1);
    assert_eq!(network_latency_compensation_ms(2), 1);
    assert_eq!(network_latency_compensation_ms(3), 2);
}

#[test]
fn formats_drift_with_node_rounding() {
    let zh = Translator::new("zh-CN");
    assert_eq!(format_drift(90_100, &zh), "1 分 30 秒");
    assert_eq!(format_drift(90_500, &zh), "1 分 31 秒");
}

#[test]
fn summarizes_process_output_tail_like_node() {
    let summary = summarize_process_output(
            b"stdout-1\nstdout-2\n",
            b"stderr-1\nstderr-2\nstderr-3\nstderr-4\nstderr-5\nstderr-6\nstderr-7\nstderr-8\nstderr-9\n",
        );
    assert!(!summary.contains("stderr-1"));
    assert!(summary.contains("stderr-9"));
    assert!(summary.contains("stdout-2"));
}

#[test]
fn formats_epoch_as_localized_beijing_time_like_node() {
    assert_eq!(
        format_beijing_time(0, "zh-CN").as_deref(),
        Some("1970/01/01 08:00:00")
    );
    assert_eq!(
        format_beijing_time(0, "en").as_deref(),
        Some("01/01/1970, 08:00:00")
    );
    assert_eq!(
        format_beijing_time(0, "zh-Hant").as_deref(),
        Some("1970/01/01\u{2009}08:00:00")
    );
    assert_eq!(
        format_beijing_time(0, "ko-KR").as_deref(),
        Some("1970. 01. 01. 08:00:00")
    );
}
