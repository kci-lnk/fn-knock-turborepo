use super::*;

async fn maintenance_test_state() -> (tempfile::TempDir, AppState) {
    let directory = tempfile::tempdir().expect("create maintenance test directory");
    let mut settings = {
        let _environment = crate::test_support::EnvGuard::new(&[]);
        crate::settings::Settings::from_env()
    };
    settings.runtime_target = "linux".to_string();
    settings.data_dir = directory.path().join("data");
    settings.gateway_config_dir = directory.path().join("gateway");
    settings.sqlite_path = directory.path().join("fn-knock.sqlite3");
    settings.legacy_redis_url = String::new();
    settings.go_backend_grpc_addr = "http://127.0.0.1:1".to_string();
    settings.internal_rpc_token = "maintenance-test-token".to_string();
    settings.request_timeout = std::time::Duration::from_millis(100);
    let state = AppState::new(settings)
        .await
        .expect("create maintenance test state");
    (directory, state)
}

#[tokio::test]
async fn clear_all_data_requires_the_localized_confirmation_phrase() {
    let (_directory, state) = maintenance_test_state().await;
    state
        .store
        .set_string_value("fn_knock:test:clear-route", "value")
        .await
        .expect("seed route data");

    let rejected = clear_all_data_with_gateway_reset(
        state.clone(),
        ClearAllDataBody {
            confirmation: "wrong phrase".to_string(),
        },
        || async { Ok(()) },
    )
    .await;
    assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        state
            .store
            .get_string_value("fn_knock:test:clear-route")
            .await
            .unwrap()
            .as_deref(),
        Some("value")
    );

    let translator = Translator::from_state(&state).await;
    let accepted = clear_all_data_with_gateway_reset(
        state.clone(),
        ClearAllDataBody {
            confirmation: maintenance_clear_text(&translator, "confirmPhrase"),
        },
        || async { Ok(()) },
    )
    .await;
    assert_eq!(accepted.status(), StatusCode::OK);
    assert!(state.store.scan_keys("", 100).await.unwrap().is_empty());
}

#[tokio::test]
async fn clear_all_data_keeps_storage_when_gateway_reset_fails() {
    let (_directory, state) = maintenance_test_state().await;
    state
        .store
        .set_string_value("fn_knock:test:clear-route", "value")
        .await
        .expect("seed route data");
    let translator = Translator::from_state(&state).await;

    let response = clear_all_data_with_gateway_reset(
        state.clone(),
        ClearAllDataBody {
            confirmation: maintenance_clear_text(&translator, "confirmPhrase"),
        },
        || async { anyhow::bail!("gateway reset failed") },
    )
    .await;

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        state
            .store
            .get_string_value("fn_knock:test:clear-route")
            .await
            .unwrap()
            .as_deref(),
        Some("value")
    );
}

#[test]
fn filters_backup_keys_like_node() {
    assert!(should_export_backup_key("fn_knock:config"));
    assert!(!should_export_backup_key(
        "fn_knock:config:host_mappings:generation"
    ));
    for prefix in BACKUP_EXCLUDED_KEY_PREFIXES {
        assert!(
            !should_export_backup_key(&format!("{prefix}sample")),
            "expected prefix {prefix} to be excluded"
        );
    }
    for key in [
        "fn_knock:acme:runtime-lock",
        "fn_knock:ddns:last_ip",
        "fn_knock:ddns:last_check",
        "fn_knock:ddns:logs",
        "fn_knock:ddns:logs:seq",
        "fn_knock:ddns:v2:target:home:last_ip",
        "fn_knock:ddns:v2:target:home:last_check",
        "fn_knock:frpc:v2:instance:main:runtime",
        "fn_knock:frpc:v2:instance:main:logs",
        "fn_knock:frpc:v2:instance:main:logs:seq",
    ] {
        assert!(
            !should_export_backup_key(key),
            "expected {key} to be excluded"
        );
    }
    assert!(should_export_backup_key(
        "fn_knock:ddns:v2:target:home:config"
    ));
    assert!(should_export_backup_key(
        "fn_knock:frpc:v2:instance:main:config"
    ));
}

#[test]
fn restores_disabled_waf_before_other_runtime_steps() {
    assert!(should_restore_waf_before_other_runtime_steps(&json!({
        "waf": {"enabled": false}
    })));
    assert!(should_restore_waf_before_other_runtime_steps(&json!({})));
    assert!(!should_restore_waf_before_other_runtime_steps(&json!({
        "waf": {"enabled": true}
    })));
}

#[test]
fn builds_node_compatible_backup_filename() {
    assert_eq!(
        build_backup_filename("2026-07-05T01:02:03.456Z"),
        "fn-knock-backup-2026-07-05T01-02-03-456Z.knock"
    );
}

#[test]
fn writes_encrypted_zip_headers() {
    let zip = create_password_protected_zip(
        KNOCK_BACKUP_JSON_FILENAME,
        br#"{"ok":true}"#,
        KNOCK_BACKUP_PASSWORD,
        1_704_067_200_000,
    )
    .unwrap();
    assert_eq!(&zip[0..4], &[0x50, 0x4b, 0x03, 0x04]);
    assert!(
        zip.windows(KNOCK_BACKUP_JSON_FILENAME.len())
            .any(|window| window == KNOCK_BACKUP_JSON_FILENAME.as_bytes())
    );
    assert!(
        zip.windows(4)
            .any(|window| window == [0x50, 0x4b, 0x05, 0x06])
    );
}

#[test]
fn parses_import_payload_with_supported_redis_types() {
    let payload = json!({
        "version": APP_BACKUP_SCHEMA_VERSION,
        "app_version": APP_LOCAL_VERSION,
        "prefix": KNOCK_BACKUP_PREFIX,
        "exported_at": "2026-07-05T00:00:00Z",
        "entries": [
            {"key":"fn_knock:string","type":"string","ttl_ms":1000,"value":"v"},
            {"key":"fn_knock:hash","type":"hash","ttl_ms":null,"value":{"a":"b"}},
            {"key":"fn_knock:list","type":"list","ttl_ms":null,"value":["a","b"]},
            {"key":"fn_knock:set","type":"set","ttl_ms":null,"value":["a"]},
            {"key":"fn_knock:zset","type":"zset","ttl_ms":null,"value":[{"member":"a","score":1.5}]},
            {"key":"fn_knock:stream","type":"stream","ttl_ms":null,"value":[{"id":"1-0","fields":["a","b"]}]}
        ]
    });
    let parsed = parse_backup_payload(&payload.to_string()).unwrap();
    assert_eq!(parsed["entry_count"], json!(6));
    assert_eq!(parsed["entries"][0]["ttl_ms"], json!(1000));
}

#[test]
fn parses_import_payload_with_legacy_backup_schema_version_field() {
    let payload = json!({
        "backupSchemaVersion": APP_BACKUP_SCHEMA_VERSION,
        "app_version": APP_LOCAL_VERSION,
        "prefix": KNOCK_BACKUP_PREFIX,
        "exported_at": "2026-07-05T00:00:00Z",
        "entries": [
            {"key":"fn_knock:string","type":"string","ttl_ms":null,"value":"v"}
        ]
    });
    let parsed = parse_backup_payload(&payload.to_string()).unwrap();
    assert_eq!(parsed["version"], json!(APP_BACKUP_SCHEMA_VERSION));
    assert_eq!(parsed["entry_count"], json!(1));
}

#[test]
fn parses_import_payload_number_coercions_like_node() {
    let payload = json!({
        "version": APP_BACKUP_SCHEMA_VERSION,
        "app_version": APP_LOCAL_VERSION,
        "prefix": KNOCK_BACKUP_PREFIX,
        "exported_at": " ",
        "entries": [
            {"key":"fn_knock:string","type":"string","ttl_ms":"1000.9","value":"v"},
            {"key":"fn_knock:hash","type":"hash","ttl_ms":0.5,"value":{"a":"b"}},
            {"key":"fn_knock:list","type":"list","ttl_ms":true,"value":["a"]},
            {"key":"fn_knock:zset","type":"zset","ttl_ms":null,"value":[
                {"member":"string-score","score":"1.5"},
                {"member":"null-score","score":null},
                {"member":"bool-score","score":true},
                {"member":"array-score","score":["2.75"]}
            ]}
        ]
    });

    let parsed = parse_backup_payload(&payload.to_string()).unwrap();

    assert_eq!(parsed["entries"][0]["ttl_ms"], json!(1000));
    assert_eq!(parsed["entries"][1]["ttl_ms"], json!(0));
    assert_eq!(parsed["entries"][2]["ttl_ms"], json!(1));
    assert_eq!(parsed["entries"][3]["value"][0]["score"], json!(1.5));
    assert_eq!(parsed["entries"][3]["value"][1]["score"], json!(0.0));
    assert_eq!(parsed["entries"][3]["value"][2]["score"], json!(1.0));
    assert_eq!(parsed["entries"][3]["value"][3]["score"], json!(2.75));
    assert_eq!(parsed["exported_at"], json!(" "));
}

#[test]
fn rejects_import_payload_invalid_number_coercions_like_node() {
    let invalid_ttl = json!({
        "version": APP_BACKUP_SCHEMA_VERSION,
        "app_version": APP_LOCAL_VERSION,
        "prefix": KNOCK_BACKUP_PREFIX,
        "exported_at": "2026-07-05T00:00:00Z",
        "entries": [
            {"key":"fn_knock:string","type":"string","ttl_ms":false,"value":"v"}
        ]
    });
    assert!(parse_backup_payload(&invalid_ttl.to_string()).is_err());

    let invalid_score = json!({
        "version": APP_BACKUP_SCHEMA_VERSION,
        "app_version": APP_LOCAL_VERSION,
        "prefix": KNOCK_BACKUP_PREFIX,
        "exported_at": "2026-07-05T00:00:00Z",
        "entries": [
            {"key":"fn_knock:zset","type":"zset","ttl_ms":null,"value":[{"member":"a","score":{}}]}
        ]
    });
    assert!(parse_backup_payload(&invalid_score.to_string()).is_err());
}

#[test]
fn validates_base64_like_node_regex() {
    assert!(is_node_base64("Zm9v"));
    assert!(is_node_base64("Zm8="));
    assert!(is_node_base64("Zg=="));
    assert!(!is_node_base64(""));
    assert!(!is_node_base64("Zg"));
    assert!(!is_node_base64("Z==="));
    assert!(!is_node_base64("Zm9v\n"));
    assert!(!is_node_base64("Zm9v-"));
}

#[test]
fn rejects_duplicate_import_keys() {
    let payload = json!({
        "version": APP_BACKUP_SCHEMA_VERSION,
        "app_version": APP_LOCAL_VERSION,
        "prefix": KNOCK_BACKUP_PREFIX,
        "exported_at": "2026-07-05T00:00:00Z",
        "entries": [
            {"key":"fn_knock:dup","type":"string","ttl_ms":null,"value":"a"},
            {"key":"fn_knock:dup","type":"string","ttl_ms":null,"value":"b"}
        ]
    });
    let error = parse_backup_payload(&payload.to_string()).unwrap_err();
    assert_eq!(error.status, StatusCode::BAD_REQUEST);
}

#[test]
fn validates_backup_version_range_like_node() {
    assert!(backup_app_version_supported(APP_BACKUP_IMPORT_MIN_VERSION));
    assert!(backup_app_version_supported(APP_LOCAL_VERSION));
    assert!(!backup_app_version_supported("1.3.9"));
    assert!(!backup_app_version_supported("99.0.0"));
}

#[test]
fn detects_backup_archive_extension_case_insensitively() {
    assert!(is_backup_archive_file("backup.KNOCK"));
    assert!(!is_backup_archive_file("backup.zip"));
}

#[test]
fn resolves_backup_archive_paths_like_node() {
    let root = Path::new("/share/backup");

    assert_eq!(
        resolve_backup_archive_path_like_node(root, "sub/../file.knock")
            .unwrap()
            .as_path(),
        Path::new("/share/backup/file.knock")
    );
    assert_eq!(
        resolve_backup_archive_path_like_node(root, "./nested/file.knock")
            .unwrap()
            .as_path(),
        Path::new("/share/backup/nested/file.knock")
    );

    for value in ["", "   ", "/", "..", "../file.knock", "a/../.."] {
        let error = resolve_backup_archive_path_like_node(root, value).unwrap_err();
        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(error.message, "Invalid backup path");
    }
}

#[test]
fn summarizes_command_failures_like_node() {
    assert_eq!(
        summarize_command_failure(b"out1\nout2\nout3\nout4", b"err1\nerr2\nerr3\nerr4").as_deref(),
        Some("err2 | err3 | err4")
    );
    assert_eq!(
        summarize_command_failure(b"out1\nout2\nout3\nout4", b"").as_deref(),
        Some("out2 | out3 | out4")
    );
    assert_eq!(summarize_command_failure(b"stdout", b"   "), None);
}

#[test]
fn localizes_backup_error_messages() {
    let translator = Translator::new("zh-CN");

    assert_eq!(
        localize_backup_error_message(&translator, "Backup JSON payload is invalid"),
        "备份文件 JSON 无法解析"
    );
    assert_eq!(
        localize_backup_error_message(&translator, "Backup archive file must end with .knock"),
        "仅支持导入 .knock 备份文件"
    );
    assert_eq!(
        localize_backup_error_message(
            &translator,
            "Unsupported Redis type for backup: bitmap (fn_knock:sample)"
        ),
        "不支持导出的 Redis 数据类型: bitmap (fn_knock:sample)"
    );
    assert_eq!(
        localize_backup_error_message(&translator, "entries[2].value[3].fields is invalid"),
        "entries[2].value[3].fields 必须是偶数长度且非空的字符串数组"
    );
    assert_eq!(
        localize_backup_error_message(&translator, "Backup file not found"),
        "未找到要导入的备份文件"
    );
    assert_eq!(
        localize_backup_error_message(&translator, "Backup directory import archive is too large"),
        "备份文件过大，无法从飞牛目录导入"
    );
    assert_eq!(
        localize_backup_error_message(
            &translator,
            &backup_error_key_message("commandMissing", &[("command", "unzip".to_string())])
        ),
        "系统环境缺少 unzip 命令"
    );
    let command_error = backup_command_error_message(
        backup_error_key_message("readArchiveFailed", &[]),
        9,
        Some("cannot find fn-knock-backup.json".to_string()),
    );
    assert_eq!(
        localize_backup_error_message(&translator, &command_error),
        "读取 .knock 备份归档失败（退出码: 9）: cannot find fn-knock-backup.json"
    );
}

#[test]
fn localizes_runtime_sync_step_labels() {
    let zh = Translator::new("zh-CN");
    let en = Translator::new("en");

    assert_eq!(
        maintenance_backup_text(&zh, "syncSteps.runModeGatewayRoutes"),
        "运行模式与网关路由"
    );
    assert_eq!(
        maintenance_backup_text(&zh, "syncSteps.directModeWhitelist"),
        "直连模式白名单"
    );
    assert_eq!(
        maintenance_backup_text(&zh, "syncSteps.systemResourceMonitorReset"),
        "系统资源监控状态重置"
    );
    assert_eq!(
        maintenance_backup_text(&en, "syncSteps.runModeGatewayRoutes"),
        "Run mode and gateway routes"
    );
}
