use super::*;
use tower::ServiceExt;

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
    let automatic_directory = automatic_backup_directory(&state);
    std::fs::create_dir_all(&automatic_directory).expect("create automatic backup directory");
    let automatic_file = automatic_directory.join("preserved.knock");
    std::fs::write(&automatic_file, b"backup").expect("seed automatic backup file");
    state
        .storage
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
        |_| async { Ok(()) },
    )
    .await;
    assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        state
            .storage
            .store
            .get_string_value("fn_knock:test:clear-route")
            .await
            .unwrap()
            .as_deref(),
        Some("value")
    );

    let translator = Translator::from_state(&state).await;
    let applied_memory = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let applied_memory_for_call = applied_memory.clone();
    let accepted = clear_all_data_with_gateway_reset(
        state.clone(),
        ClearAllDataBody {
            confirmation: maintenance_clear_text(&translator, "confirmPhrase"),
        },
        || async { Ok(()) },
        move |settings| {
            applied_memory_for_call.lock().unwrap().push(settings);
            async { Ok(()) }
        },
    )
    .await;
    assert_eq!(accepted.status(), StatusCode::OK);
    assert!(
        state
            .storage
            .store
            .scan_keys("", 100)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(automatic_file.exists());
    assert_eq!(
        load_automatic_backup_config(&state).await.unwrap()["enabled"],
        json!(false)
    );
    assert_eq!(
        *applied_memory.lock().unwrap(),
        vec![gateway_settings::GatewayMemorySettings {
            gc_percent: gateway_settings::DEFAULT_GATEWAY_GC_PERCENT,
            memory_limit_mib: None,
        }]
    );
}

#[tokio::test]
async fn clear_all_data_keeps_storage_when_gateway_reset_fails() {
    let (_directory, state) = maintenance_test_state().await;
    state
        .storage
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
        |_| async { panic!("memory settings must not be applied when gateway reset fails") },
    )
    .await;

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        state
            .storage
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
    assert!(should_export_backup_key(
        "fn_knock:patch:reverse-proxy-throttle-default:v2"
    ));
    assert!(should_export_backup_key("fn_knock:wol:local-relay"));
    assert!(should_export_backup_key("fn_knock:wol:relay:relay-id"));
    assert!(should_export_backup_key("fn_knock:wol:target:target-id"));
    assert!(should_export_backup_key(
        "fn_knock:wol:target-status:target-id"
    ));
    assert!(!should_export_backup_key(
        "fn_knock:wol:runtime:cooldown:target-id"
    ));
    assert!(!should_export_backup_key(
        "fn_knock:config:host_mappings:generation"
    ));
    assert!(!should_export_backup_key(
        "fn_knock:auth:subdomain_rule_grant_active:app.example.com"
    ));
    assert!(should_export_backup_key("fn_knock:terminal:targets"));
    assert!(!should_export_backup_key(
        "fn_knock:terminal:local-settings"
    ));
    assert!(!should_export_backup_key(
        "fn_knock:terminal:session:data:legacy-session"
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
        "fn_knock:ldap:invite:temporary",
        "fn_knock:passkey:state:challenge",
        "fn_knock:runtime:session:management",
        "fn_knock:cleanup:legacy-auth-logs:v1:lock",
        "fn_knock:auth:expired_session_cleanup:abc",
        "fn_knock:cloudflared:managed:state:v1",
        "fn_knock:ddns:edgeone:overseas_access:edgeone",
        "fn_knock:future:operation:lease",
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
fn rejects_exports_that_cannot_be_imported_again() {
    assert!(ensure_backup_export_size(MAX_BACKUP_ARCHIVE_SIZE).is_ok());
    assert!(ensure_backup_export_size(MAX_BACKUP_ARCHIVE_SIZE + 1).is_err());
}

#[tokio::test]
async fn excluded_runtime_data_does_not_consume_the_export_budget() {
    let (_directory, state) = maintenance_test_state().await;
    state
        .storage
        .store
        .set_string_value("fn_knock:events:large", &"x".repeat(4096))
        .await
        .unwrap();
    state
        .storage
        .store
        .set_string_value("fn_knock:config:test", "small")
        .await
        .unwrap();

    let entries = state
        .storage
        .store
        .export_backup_entries_by_prefix_limited(
            KNOCK_BACKUP_PREFIX,
            1024,
            should_export_backup_key,
        )
        .await
        .unwrap();

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["key"], json!("fn_knock:config:test"));
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

#[tokio::test]
async fn backup_destination_never_overwrites_an_existing_archive() {
    let directory = tempfile::tempdir().unwrap();
    let requested = "fn-knock-backup-2026-07-05T01-02-03-456Z.knock";
    std::fs::write(directory.path().join(requested), b"existing").unwrap();

    let (filename, destination) = unique_backup_destination(directory.path(), requested).await;

    assert_ne!(filename, requested);
    assert_eq!(destination.parent(), Some(directory.path()));
    assert!(filename.ends_with(KNOCK_BACKUP_EXTENSION));
    assert!(!destination.exists());
    assert_eq!(
        std::fs::read(directory.path().join(requested)).unwrap(),
        b"existing"
    );
}

#[test]
fn writes_encrypted_zip_headers() {
    let payload = br#"{"ok":true}"#;
    let zip = create_password_protected_zip(
        KNOCK_BACKUP_JSON_FILENAME,
        payload,
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
    assert_eq!(
        read_backup_json_from_archive_native(&zip)
            .unwrap()
            .as_bytes(),
        payload
    );
}

#[test]
fn parses_backup_json_directly_from_the_encrypted_archive_stream() {
    let payload = json!({
        "version": APP_BACKUP_SCHEMA_VERSION,
        "app_version": APP_LOCAL_VERSION,
        "prefix": KNOCK_BACKUP_PREFIX,
        "exported_at": "2026-07-05T01:02:03.456Z",
        "entries": [{
            "key": "fn_knock:config:test",
            "type": "string",
            "ttl_ms": null,
            "value": "ok"
        }]
    });
    let zip = create_password_protected_zip(
        KNOCK_BACKUP_JSON_FILENAME,
        payload.to_string().as_bytes(),
        KNOCK_BACKUP_PASSWORD,
        1_704_067_200_000,
    )
    .unwrap();

    let parsed = parse_backup_payload_from_archive_native(&zip).unwrap();

    assert_eq!(parsed["entry_count"], json!(1));
    assert_eq!(parsed["entries"][0]["value"], json!("ok"));
}

#[test]
fn rejects_zip_payloads_larger_than_the_decompressed_limit() {
    let payload = vec![b'a'; 2048];
    let zip = create_password_protected_zip(
        KNOCK_BACKUP_JSON_FILENAME,
        &payload,
        KNOCK_BACKUP_PASSWORD,
        1_704_067_200_000,
    )
    .unwrap();
    let error = read_backup_json_from_archive_native_with_limit(&zip, 1024).unwrap_err();
    assert_eq!(error.status, StatusCode::BAD_REQUEST);
    assert_eq!(error.message, "Backup JSON payload is too large");
}

#[test]
fn ages_ttls_and_drops_entries_that_expired_after_export() {
    let persistent = json!({"key":"fn_knock:persistent","ttl_ms":null});
    assert_eq!(
        age_backup_entry_ttl(persistent.clone(), 10_000),
        Some(persistent)
    );
    assert_eq!(
        age_backup_entry_ttl(json!({"key":"fn_knock:live","ttl_ms":15_000}), 10_000).unwrap()["ttl_ms"],
        json!(5_000)
    );
    assert!(
        age_backup_entry_ttl(json!({"key":"fn_knock:expired","ttl_ms":10_000}), 10_000).is_none()
    );
    assert!(
        age_backup_entry_ttl(json!({"key":"fn_knock:legacy","ttl_ms":10_000}), i64::MAX).is_none()
    );
}

#[test]
fn reads_the_pre_import_config_from_the_atomic_snapshot() {
    let entries = vec![json!({
        "key": "fn_knock:config",
        "type": "string",
        "ttl_ms": null,
        "value": "{\"fnos_network_tuning\":{\"bbr_enabled\":true}}"
    })];
    assert_eq!(
        backup_config_from_entries(&entries).unwrap()["fnos_network_tuning"]["bbr_enabled"],
        json!(true)
    );
    assert!(
        backup_config_from_entries(&[json!({
            "key": "fn_knock:config",
            "value": "not-json"
        })])
        .is_none()
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
        localize_backup_error_message(&translator, "Backup export is too large"),
        "备份数据过大，无法导出"
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
fn automatic_backup_defaults_and_validation_are_stable() {
    assert_eq!(
        normalize_automatic_backup_config(None),
        json!({
            "enabled": false,
            "interval_hours": 24,
            "retention_days": 7,
            "updated_at": null,
        })
    );
    assert!(
        validate_automatic_backup_config(&UpdateAutomaticBackupBody {
            enabled: true,
            interval_hours: 1,
            retention_days: 1,
        })
        .is_ok()
    );
    assert!(
        validate_automatic_backup_config(&UpdateAutomaticBackupBody {
            enabled: true,
            interval_hours: 8760,
            retention_days: 3650,
        })
        .is_ok()
    );
    for (interval_hours, retention_days) in [(0, 7), (8761, 7), (24, 0), (24, 3651)] {
        assert!(
            validate_automatic_backup_config(&UpdateAutomaticBackupBody {
                enabled: true,
                interval_hours,
                retention_days,
            })
            .is_err()
        );
    }
}

#[test]
fn automatic_backup_rescheduling_runs_overdue_work_once() {
    let now = time_utils::parse_iso_ms("2026-07-24T12:00:00Z").unwrap();
    assert_eq!(
        next_backup_after_last_success(Some("2026-07-24T11:30:00Z"), 1, now),
        "2026-07-24T12:30:00Z"
    );
    assert_eq!(
        next_backup_after_last_success(Some("2026-07-20T00:00:00Z"), 24, now),
        "2026-07-24T12:00:00Z"
    );
    assert_eq!(
        next_backup_after_last_success(None, 24, now),
        "2026-07-24T12:00:00Z"
    );
    assert_eq!(
        next_backup_after_failure(Some("2027-07-24T12:00:00Z"), 8760, now),
        "2026-07-24T13:00:00Z"
    );
}

#[tokio::test]
async fn automatic_backup_writes_to_the_cross_platform_data_directory() {
    let (_directory, state) = maintenance_test_state().await;
    assert_eq!(
        automatic_backup_directory(&state),
        state.settings.data_dir.join("backups").join("automatic")
    );
    let details = save_automatic_backup_config(
        &state,
        UpdateAutomaticBackupBody {
            enabled: true,
            interval_hours: 24,
            retention_days: 7,
        },
    )
    .await
    .expect("enable automatic backups");
    let next = details["status"]["next_backup_at"]
        .as_str()
        .and_then(time_utils::parse_iso_ms)
        .expect("immediate next backup");
    assert!(next <= time_utils::now_ms() + 1_000);

    let result = run_automatic_backup_once(&state)
        .await
        .expect("run automatic backup");
    let filename = result["filename"].as_str().expect("backup filename");
    assert!(automatic_backup_directory(&state).join(filename).is_file());
    assert!(
        std::fs::read_dir(automatic_backup_directory(&state))
            .unwrap()
            .all(|entry| {
                !entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(AUTOMATIC_BACKUP_TEMP_PREFIX)
            })
    );
    let files = automatic_backup_files_payload(&state)
        .await
        .expect("list automatic backups");
    assert_eq!(files["files"].as_array().unwrap().len(), 1);
    let runtime = load_automatic_backup_runtime(&state).await.unwrap();
    assert!(runtime["last_success_at"].is_string());
    assert!(runtime["last_error"].is_null());
    assert!(
        time_utils::parse_iso_ms(runtime["next_backup_at"].as_str().unwrap()).unwrap()
            > time_utils::now_ms()
    );
}

#[tokio::test]
async fn automatic_backup_scheduler_runs_the_first_backup_immediately() {
    let (_directory, state) = maintenance_test_state().await;
    save_automatic_backup_config(
        &state,
        UpdateAutomaticBackupBody {
            enabled: true,
            interval_hours: 24,
            retention_days: 7,
        },
    )
    .await
    .unwrap();

    let worker_state = state.clone();
    let worker = tokio::spawn(async move {
        automatic_backup_scheduler(worker_state).await;
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let payload = automatic_backup_files_payload(&state).await.unwrap();
            if !payload["files"].as_array().unwrap().is_empty() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("automatic scheduler should create the first backup");
    state.shutdown.cancel();
    worker.await.unwrap();
}

#[tokio::test]
async fn automatic_backup_scheduler_honors_a_persisted_future_deadline() {
    let (_directory, state) = maintenance_test_state().await;
    state
        .storage
        .store
        .set_json_value(
            AUTOMATIC_BACKUP_CONFIG_KEY,
            &json!({
                "enabled": true,
                "interval_hours": 24,
                "retention_days": 7,
                "updated_at": time_utils::now_iso(),
            }),
        )
        .await
        .unwrap();
    state
        .storage
        .store
        .set_json_value(
            AUTOMATIC_BACKUP_RUNTIME_KEY,
            &json!({
                "last_attempt_at": null,
                "last_success_at": null,
                "last_error": null,
                "last_filename": null,
                "next_backup_at": time_utils::iso_after_seconds(3600),
            }),
        )
        .await
        .unwrap();

    let worker_state = state.clone();
    let worker = tokio::spawn(async move {
        automatic_backup_scheduler(worker_state).await;
    });
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let payload = automatic_backup_files_payload(&state).await.unwrap();
    assert!(payload["files"].as_array().unwrap().is_empty());
    state.shutdown.cancel();
    worker.await.unwrap();
}

#[tokio::test]
async fn changing_the_interval_keeps_a_failed_backup_within_the_retry_cap() {
    let (_directory, state) = maintenance_test_state().await;
    state
        .storage
        .store
        .set_json_value(
            AUTOMATIC_BACKUP_CONFIG_KEY,
            &json!({
                "enabled": true,
                "interval_hours": 24,
                "retention_days": 7,
                "updated_at": time_utils::now_iso(),
            }),
        )
        .await
        .unwrap();
    state
        .storage
        .store
        .set_json_value(
            AUTOMATIC_BACKUP_RUNTIME_KEY,
            &json!({
                "last_attempt_at": time_utils::now_iso(),
                "last_success_at": "2026-01-01T00:00:00Z",
                "last_error": "disk full",
                "last_filename": "previous.knock",
                "next_backup_at": time_utils::iso_after_seconds(3600),
            }),
        )
        .await
        .unwrap();
    let before = time_utils::now_ms();

    let details = save_automatic_backup_config(
        &state,
        UpdateAutomaticBackupBody {
            enabled: true,
            interval_hours: 8760,
            retention_days: 7,
        },
    )
    .await
    .unwrap();
    let next = details["status"]["next_backup_at"]
        .as_str()
        .and_then(time_utils::parse_iso_ms)
        .unwrap();
    assert!(next >= before);
    assert!(next <= before + 3_600_000);
}

#[tokio::test]
async fn automatic_backup_file_listing_does_not_hide_retained_archives() {
    let (_directory, state) = maintenance_test_state().await;
    let directory = automatic_backup_directory(&state);
    std::fs::create_dir_all(&directory).unwrap();
    for index in 0..501 {
        std::fs::write(directory.join(format!("{index:03}.knock")), b"backup").unwrap();
    }

    let payload = automatic_backup_files_payload(&state).await.unwrap();
    assert_eq!(payload["files"].as_array().unwrap().len(), 501);
}

#[tokio::test]
async fn automatic_backup_config_rejects_non_integer_json_with_bad_request() {
    let (_directory, state) = maintenance_test_state().await;
    let response = maintenance_routes()
        .with_state(state)
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri("/api/admin/maintenance/backup/automatic")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"enabled":true,"interval_hours":1.5,"retention_days":7}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn local_backup_import_accepts_json_bodies_above_axums_default_limit() {
    let (_directory, state) = maintenance_test_state().await;
    let response = maintenance_routes()
        .with_state(state)
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/admin/maintenance/backup/import")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "filename": "large.knock",
                        "archive_base64": "A".repeat(3 * 1024 * 1024),
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn automatic_backup_pruning_only_removes_expired_knock_files() {
    let (_directory, state) = maintenance_test_state().await;
    let directory = automatic_backup_directory(&state);
    std::fs::create_dir_all(&directory).unwrap();
    let expired = directory.join("expired.knock");
    let recent = directory.join("recent.knock");
    let unrelated = directory.join("expired.txt");
    std::fs::write(&expired, b"old").unwrap();
    std::fs::write(&recent, b"new").unwrap();
    std::fs::write(&unrelated, b"old but unrelated").unwrap();
    let old_time = SystemTime::now() - std::time::Duration::from_secs(2 * 24 * 3600);
    std::fs::File::options()
        .write(true)
        .open(&expired)
        .unwrap()
        .set_modified(old_time)
        .unwrap();
    std::fs::File::options()
        .write(true)
        .open(&unrelated)
        .unwrap()
        .set_modified(old_time)
        .unwrap();

    prune_automatic_backup_directory(&state, 1)
        .await
        .expect("prune automatic backups");
    assert!(!expired.exists());
    assert!(recent.exists());
    assert!(unrelated.exists());
}

#[tokio::test]
async fn automatic_backup_import_rejects_unsafe_paths_and_symlinks() {
    let (_directory, state) = maintenance_test_state().await;
    for value in [
        "",
        "..",
        "../backup.knock",
        "nested/backup.knock",
        r"nested\backup.knock",
        "/tmp/backup.knock",
        "backup.zip",
    ] {
        let error = resolve_automatic_backup_archive_path(&state, value)
            .await
            .unwrap_err();
        assert_eq!(error.status, StatusCode::BAD_REQUEST);
    }

    #[cfg(unix)]
    {
        let directory = automatic_backup_directory(&state);
        std::fs::create_dir_all(&directory).unwrap();
        let target = directory.join("target.knock");
        let link = directory.join("link.knock");
        std::fs::write(&target, b"not an archive").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let translator = Translator::from_state(&state).await;
        let error =
            import_backup_archive_from_automatic_directory(&state, "link.knock", &translator)
                .await
                .unwrap_err();
        assert_eq!(error.status, StatusCode::BAD_REQUEST);
    }
}

#[cfg(unix)]
#[tokio::test]
async fn backup_path_validation_rejects_an_intermediate_symlink_escape() {
    let directory = tempfile::tempdir().unwrap();
    let backup_root = directory.path().join("backup");
    let outside = directory.path().join("outside");
    std::fs::create_dir_all(&backup_root).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("escaped.knock"), b"not an archive").unwrap();
    std::os::unix::fs::symlink(&outside, backup_root.join("linked")).unwrap();

    let resolved = backup_root.join("linked").join("escaped.knock");
    let error = validate_existing_backup_path(&backup_root, resolved)
        .await
        .unwrap_err();
    assert_eq!(error.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn failed_automatic_backup_records_an_hourly_retry() {
    let (_directory, state) = maintenance_test_state().await;
    let backup_directory = automatic_backup_directory(&state);
    std::fs::create_dir_all(backup_directory.parent().unwrap()).unwrap();
    std::fs::write(backup_directory, b"blocks directory creation").unwrap();
    save_automatic_backup_config(
        &state,
        UpdateAutomaticBackupBody {
            enabled: true,
            interval_hours: 24,
            retention_days: 7,
        },
    )
    .await
    .unwrap();
    assert!(run_automatic_backup_once(&state).await.is_err());
    let runtime = load_automatic_backup_runtime(&state).await.unwrap();
    let next = time_utils::parse_iso_ms(runtime["next_backup_at"].as_str().unwrap()).unwrap();
    let delay = next - time_utils::now_ms();
    assert!(runtime["last_error"].is_string());
    assert!(delay > 0 && delay <= 3_600_000);
}

#[tokio::test]
async fn backup_restore_preserves_automatic_backup_settings_atomically() {
    let (_directory, state) = maintenance_test_state().await;
    state
        .storage
        .store
        .set_string_value("fn_knock:test:included", "original")
        .await
        .unwrap();
    let archive = export_backup_archive(&state).await.unwrap();
    save_automatic_backup_config(
        &state,
        UpdateAutomaticBackupBody {
            enabled: true,
            interval_hours: 12,
            retention_days: 30,
        },
    )
    .await
    .unwrap();
    state
        .storage
        .store
        .set_string_value("fn_knock:test:included", "changed")
        .await
        .unwrap();
    let cloudflared_directory = state.settings.data_dir.join("cloudflared");
    std::fs::create_dir_all(&cloudflared_directory).unwrap();
    std::fs::write(
        cloudflared_directory.join("cloudflared.json"),
        r#"{"token":"restore-secret","protocol":"auto"}"#,
    )
    .unwrap();

    let translator = Translator::from_state(&state).await;
    let result = import_backup_archive_buffer(&state, archive.buffer, &translator)
        .await
        .expect("restore backup");
    assert_eq!(result["imported_keys"], json!(1));
    assert_eq!(
        state
            .storage
            .store
            .get_string_value("fn_knock:test:included")
            .await
            .unwrap()
            .as_deref(),
        Some("original")
    );
    let config = load_automatic_backup_config(&state).await.unwrap();
    assert_eq!(config["enabled"], json!(true));
    assert_eq!(config["interval_hours"], json!(12));
    assert_eq!(config["retention_days"], json!(30));
    let cloudflared_config =
        std::fs::read_to_string(cloudflared_directory.join("cloudflared.json")).unwrap();
    assert!(!cloudflared_config.contains("restore-secret"));
    assert!(
        !cloudflared_directory
            .join("cloudflare-api-token.enc")
            .exists()
    );
    assert!(!cloudflared_directory.join("tunnel-token.enc").exists());
}

#[tokio::test]
async fn backup_restore_round_trips_terminal_and_wol_credentials_without_plaintext() {
    let (_directory, state) = maintenance_test_state().await;
    let terminal_password_id = Uuid::new_v4().to_string();
    let terminal_key_id = Uuid::new_v4().to_string();
    state
        .storage
        .store
        .set_json_value(
            "fn_knock:terminal:targets",
            &json!([
                {
                    "id": terminal_password_id,
                    "name": "Password target",
                    "host": "192.0.2.10",
                    "port": 22,
                    "username": "root",
                    "authMethod": "password",
                    "trustedHostKey": null,
                    "revision": 1,
                    "lastVerifiedAt": null,
                    "createdAt": "2026-08-28T00:00:00Z",
                    "updatedAt": "2026-08-28T00:00:00Z"
                },
                {
                    "id": terminal_key_id,
                    "name": "Private key target",
                    "host": "192.0.2.11",
                    "port": 22,
                    "username": "root",
                    "authMethod": "privateKey",
                    "trustedHostKey": null,
                    "revision": 7,
                    "lastVerifiedAt": null,
                    "createdAt": "2026-08-28T00:00:00Z",
                    "updatedAt": "2026-08-28T00:00:00Z"
                }
            ]),
        )
        .await
        .unwrap();
    terminal::write_backup_test_credential(
        &state,
        &terminal_password_id,
        terminal::domain::AuthMethod::Password,
        1,
        Some(b"terminal-password-secret"),
        None,
        None,
    );
    terminal::write_backup_test_credential(
        &state,
        &terminal_key_id,
        terminal::domain::AuthMethod::PrivateKey,
        7,
        None,
        Some(b"-----BEGIN OPENSSH PRIVATE KEY-----terminal-key-secret"),
        Some(b"terminal-passphrase-secret"),
    );

    let wol_relay_id = "relay-backup-test";
    let wol_target_id = "target-backup-test";
    state
        .storage
        .store
        .set_string_and_zadd(
            &format!("fn_knock:wol:relay:{wol_relay_id}"),
            &json!({
                "id": wol_relay_id,
                "name": "Relay",
                "address": "192.0.2.20",
                "port": 40009,
                "enabled": true,
                "key_version": 3,
                "created_at": "2026-08-28T00:00:00Z",
                "updated_at": "2026-08-28T00:00:00Z"
            })
            .to_string(),
            "fn_knock:wol:relays:index",
            wol_relay_id,
            1,
        )
        .await
        .unwrap();
    state
        .storage
        .store
        .set_string_and_zadd(
            &format!("fn_knock:wol:target:{wol_target_id}"),
            &json!({
                "id": wol_target_id,
                "name": "NAS",
                "mac": "02:11:22:33:44:55",
                "relay_id": wol_relay_id,
                "broadcast_address": null,
                "ip_address": "192.0.2.30",
                "integrations": {
                    "blinker": { "enabled": true, "bindComponent": false, "skipTlsVerify": true },
                    "bemfa": { "enabled": true, "topic": "nas", "skipTlsVerify": true }
                },
                "ssh": {
                    "enabled": true,
                    "host": "192.0.2.30",
                    "port": 22,
                    "username": "root",
                    "platform": "linux",
                    "authMethod": "privateKey",
                    "hostKeyAlgorithm": "ssh-ed25519",
                    "hostKeyFingerprint": "SHA256:test"
                },
                "enabled": true,
                "created_at": "2026-08-28T00:00:00Z",
                "updated_at": "2026-08-28T00:00:00Z"
            })
            .to_string(),
            "fn_knock:wol:targets:index",
            wol_target_id,
            1,
        )
        .await
        .unwrap();
    wol::write_backup_test_relay_secret(&state, wol_relay_id, 3, b"relay-psk-secret");
    let wol_secrets: [&[u8]; 5] = [
        b"blinker-device-secret",
        b"bemfa-private-secret",
        b"wol-ssh-password-secret",
        b"-----BEGIN OPENSSH PRIVATE KEY-----wol-key-secret",
        b"wol-key-passphrase-secret",
    ];
    wol::write_backup_test_target_secrets(&state, wol_target_id, wol_secrets);

    let archive = export_backup_archive(&state).await.unwrap();
    let backup_json = read_backup_json_from_archive_native(&archive.buffer).unwrap();
    for secret in [
        "terminal-password-secret",
        "terminal-key-secret",
        "terminal-passphrase-secret",
        "relay-psk-secret",
        "blinker-device-secret",
        "bemfa-private-secret",
        "wol-ssh-password-secret",
        "wol-key-secret",
        "wol-key-passphrase-secret",
    ] {
        assert!(!backup_json.contains(secret), "backup leaked {secret}");
    }
    assert!(backup_json.contains("protected_credentials"));

    terminal::write_backup_test_credential(
        &state,
        &terminal_password_id,
        terminal::domain::AuthMethod::Password,
        1,
        Some(b"changed"),
        None,
        None,
    );
    terminal::write_backup_test_credential(
        &state,
        &terminal_key_id,
        terminal::domain::AuthMethod::PrivateKey,
        7,
        None,
        Some(b"changed"),
        None,
    );
    wol::write_backup_test_relay_secret(&state, wol_relay_id, 3, b"changed");
    wol::write_backup_test_target_secrets(&state, wol_target_id, [b"changed"; 5]);

    let translator = Translator::from_state(&state).await;
    import_backup_archive_buffer(&state, archive.buffer, &translator)
        .await
        .expect("restore credential-bearing backup");

    assert_eq!(
        terminal::read_backup_test_credential(&state, &terminal_password_id),
        [Some(b"terminal-password-secret".to_vec()), None, None]
    );
    assert_eq!(
        terminal::read_backup_test_credential(&state, &terminal_key_id),
        [
            None,
            Some(b"-----BEGIN OPENSSH PRIVATE KEY-----terminal-key-secret".to_vec()),
            Some(b"terminal-passphrase-secret".to_vec())
        ]
    );
    assert_eq!(
        wol::read_backup_test_relay_secret(&state, wol_relay_id, 3),
        Some(b"relay-psk-secret".to_vec())
    );
    assert_eq!(
        wol::read_backup_test_target_secrets(&state, wol_target_id),
        wol_secrets.map(|value| Some(value.to_vec()))
    );
}

#[tokio::test]
async fn automatic_backup_waits_for_the_maintenance_mutex() {
    let (_directory, state) = maintenance_test_state().await;
    save_automatic_backup_config(
        &state,
        UpdateAutomaticBackupBody {
            enabled: true,
            interval_hours: 24,
            retention_days: 7,
        },
    )
    .await
    .unwrap();
    let guard = state.maintenance.automatic_backup_lock.lock().await;
    let worker_state = state.clone();
    let mut worker = tokio::spawn(async move { run_automatic_backup_once(&worker_state).await });
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(30), &mut worker)
            .await
            .is_err()
    );
    drop(guard);
    worker.await.unwrap().unwrap();
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
        maintenance_backup_text(&zh, "syncSteps.trustedClientIps"),
        "网关可信客户端 IP"
    );
    assert_eq!(
        maintenance_backup_text(&zh, "syncSteps.systemResourceMonitorReset"),
        "系统资源监控状态重置"
    );
    assert_eq!(
        maintenance_backup_text(&en, "syncSteps.runModeGatewayRoutes"),
        "Run mode and gateway routes"
    );
    assert_eq!(
        maintenance_backup_text(&zh, "syncSteps.fnosNetworkTuning"),
        "飞牛网络调优"
    );
}
