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
async fn backup_routes_reject_busy_requests_before_polling_upload_bodies() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use tokio_stream::StreamExt;

    let (_directory, state) = maintenance_test_state().await;
    let admission = routes::BackupAdmission::try_acquire(&state).unwrap();
    let app = maintenance_routes().with_state(state.clone());
    for (method, path) in [
        ("POST", "/api/admin/maintenance/backup/export/fnos"),
        ("POST", "/api/admin/maintenance/backup/import"),
        ("POST", "/api/admin/maintenance/backup/import/fnos"),
        ("POST", "/api/admin/maintenance/backup/import/automatic"),
    ] {
        let polled = Arc::new(AtomicBool::new(false));
        let stream_polled = polled.clone();
        let body = Body::from_stream(
            tokio_stream::once(Ok::<_, io::Error>(bytes::Bytes::from_static(
                b"invalid JSON",
            )))
            .map(move |chunk| {
                stream_polled.store(true, Ordering::Release);
                chunk
            }),
        );
        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method(method)
                    .uri(path)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(body)
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::SERVICE_UNAVAILABLE,
            "{method} {path}"
        );
        assert_eq!(response.headers()[header::RETRY_AFTER], "1");
        assert!(
            !polled.load(Ordering::Acquire),
            "{method} {path} must reject before reading its body"
        );
    }
    drop(admission);
    assert!(routes::BackupAdmission::try_acquire(&state).is_ok());
}

async fn assert_backup_waiter_pending<F: Future>(mut future: std::pin::Pin<&mut F>) {
    std::future::poll_fn(|cx| {
        assert!(future.as_mut().poll(cx).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
}

#[tokio::test]
async fn backup_export_queue_has_three_waiters_and_preserves_fifo() {
    let (_directory, state) = maintenance_test_state().await;
    let active = routes::BackupExportAdmission::acquire(&state)
        .await
        .unwrap();
    assert_eq!(
        state.maintenance.backup_export_waiters.available_permits(),
        3
    );
    let mut waiters = (0..3)
        .map(|_| Box::pin(routes::BackupExportAdmission::acquire(&state)))
        .collect::<Vec<_>>();
    for waiter in &mut waiters {
        // Polling once establishes the exact FIFO order, without sleeps or
        // scheduling assumptions about concurrently spawned tasks.
        assert_backup_waiter_pending(waiter.as_mut()).await;
    }
    assert_eq!(
        state.maintenance.backup_export_waiters.available_permits(),
        0
    );
    let response = maintenance_routes()
        .with_state(state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/admin/maintenance/backup/export")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(response.headers()[header::RETRY_AFTER], "1");
    assert_eq!(
        state.maintenance.backup_export_waiters.available_permits(),
        0
    );

    drop(active);
    for expected_available in 1..=3 {
        // A newly arriving immediate request cannot take the permit reserved
        // for the oldest waiter, even before that waiter is polled again.
        assert!(routes::BackupAdmission::try_acquire(&state).is_err());
        for later in waiters.iter_mut().skip(1) {
            assert_backup_waiter_pending(later.as_mut()).await;
        }
        let active = waiters.remove(0).await.unwrap();
        assert_eq!(
            state.maintenance.backup_export_waiters.available_permits(),
            expected_available,
        );
        assert!(routes::BackupAdmission::try_acquire(&state).is_err());
        drop(active);
    }
    assert!(routes::BackupAdmission::try_acquire(&state).is_ok());
}

#[tokio::test]
async fn cancelled_or_timed_out_export_waiter_releases_capacity_without_starting_work() {
    use std::sync::atomic::{AtomicBool, Ordering};

    let (_directory, state) = maintenance_test_state().await;
    let active = routes::BackupAdmission::try_acquire(&state).unwrap();
    let started = AtomicBool::new(false);
    let mut cancelled = Box::pin(async {
        let admission = routes::BackupExportAdmission::acquire(&state).await?;
        started.store(true, Ordering::Release);
        Ok::<_, BackupImportError>(admission)
    });
    assert_backup_waiter_pending(cancelled.as_mut()).await;
    assert_eq!(
        state.maintenance.backup_export_waiters.available_permits(),
        2
    );
    drop(cancelled);
    assert!(!started.load(Ordering::Acquire));
    assert_eq!(
        state.maintenance.backup_export_waiters.available_permits(),
        3
    );

    let error =
        routes::BackupExportAdmission::acquire_with_timeout(&state, std::time::Duration::ZERO)
            .await
            .err()
            .expect("busy zero-deadline waiter must time out");
    assert_eq!(error.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        state.maintenance.backup_export_waiters.available_permits(),
        3
    );
    assert!(routes::BackupAdmission::try_acquire(&state).is_err());
    drop(active);
    assert!(!started.load(Ordering::Acquire));
    assert!(routes::BackupExportAdmission::acquire(&state).await.is_ok());
}

#[tokio::test]
async fn shutdown_cancels_export_waiters_without_releasing_an_active_backup() {
    let (_directory, state) = maintenance_test_state().await;
    let active = routes::BackupAdmission::try_acquire(&state).unwrap();
    let mut waiting = Box::pin(routes::BackupExportAdmission::acquire(&state));
    assert_backup_waiter_pending(waiting.as_mut()).await;
    state.shutdown.cancel();
    let error = waiting.await.err().expect("shutdown rejects the waiter");
    assert_eq!(error.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        state.maintenance.backup_export_waiters.available_permits(),
        3
    );
    assert!(routes::BackupAdmission::try_acquire(&state).is_err());
    drop(active);
    assert!(
        routes::BackupExportAdmission::acquire(&state)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn queued_export_waits_for_the_response_body_and_last_bytes_owner() {
    use http_body_util::BodyExt;

    let (_directory, state) = maintenance_test_state().await;
    let routes::BackupExportAdmission(admission) = routes::BackupExportAdmission::acquire(&state)
        .await
        .unwrap();
    let response = binary_archive_response(
        BackupArchive {
            buffer: BackupArchiveBuffer::from_bytes(&[1, 2, 3]),
            exported_at: "2026-09-05T00:00:00Z".to_string(),
            filename: "test.knock".to_string(),
        },
        admission,
        &Translator::new("en"),
    );
    let mut waiting = Box::pin(routes::BackupExportAdmission::acquire(&state));
    assert_backup_waiter_pending(waiting.as_mut()).await;
    let mut body = response.into_body();
    let frame = body.frame().await.unwrap().unwrap().into_data().unwrap();
    let retained = frame.clone();
    drop(frame);
    assert!(body.frame().await.is_none());
    assert_backup_waiter_pending(waiting.as_mut()).await;
    drop(body);
    assert_backup_waiter_pending(waiting.as_mut()).await;
    drop(retained);
    let next = waiting.await.unwrap();
    assert_eq!(
        state.maintenance.backup_export_waiters.available_permits(),
        3
    );
    assert!(routes::BackupAdmission::try_acquire(&state).is_err());
    drop(next);
    assert!(routes::BackupAdmission::try_acquire(&state).is_ok());
}

#[tokio::test]
async fn queued_get_export_returns_an_archive_after_the_previous_download_drops() {
    let (_directory, state) = maintenance_test_state().await;
    let app = maintenance_routes().with_state(state.clone());
    let request = || {
        axum::http::Request::builder()
            .uri("/api/admin/maintenance/backup/export")
            .body(Body::empty())
            .unwrap()
    };
    let first = app.clone().oneshot(request()).await.unwrap();
    assert_eq!(first.status(), StatusCode::OK);
    let mut second = Box::pin(app.oneshot(request()));
    assert_backup_waiter_pending(second.as_mut()).await;
    assert_eq!(
        state.maintenance.backup_export_waiters.available_permits(),
        2
    );
    drop(first);
    let second = second.await.unwrap();
    assert_eq!(second.status(), StatusCode::OK);
    assert!(second.headers().contains_key(header::CONTENT_LENGTH));
    let bytes = axum::body::to_bytes(second.into_body(), MAX_BACKUP_ARCHIVE_SIZE)
        .await
        .unwrap();
    assert_eq!(&bytes[..4], b"PK\x03\x04");
    assert_eq!(
        state.maintenance.backup_export_waiters.available_permits(),
        3
    );
    drop(bytes);
    assert!(routes::BackupAdmission::try_acquire(&state).is_ok());
}

#[tokio::test]
async fn cancelled_backup_upload_releases_admission_before_work_starts() {
    let (_directory, state) = maintenance_test_state().await;
    let body = Body::from_stream(tokio_stream::pending::<Result<bytes::Bytes, io::Error>>());
    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/api/admin/maintenance/backup/import")
        .header(header::CONTENT_TYPE, "application/json")
        .body(body)
        .unwrap();
    let mut request = Box::pin(
        maintenance_routes()
            .with_state(state.clone())
            .oneshot(request),
    );
    std::future::poll_fn(|cx| {
        assert!(request.as_mut().poll(cx).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    assert!(routes::BackupAdmission::try_acquire(&state).is_err());
    drop(request);
    assert!(routes::BackupAdmission::try_acquire(&state).is_ok());
}

#[tokio::test]
async fn cancelled_manual_backup_finishes_work_before_releasing_admission() {
    let (_directory, state) = maintenance_test_state().await;
    let admission = routes::BackupAdmission::try_acquire(&state).unwrap();
    let request_state = state.clone();
    let work_state = state.clone();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = tokio::sync::oneshot::channel();
    let request = tokio::spawn(async move {
        routes::run_backup_operation(&request_state, admission, async move {
            started_tx.send(()).unwrap();
            release_rx.await.unwrap();
            work_state
                .storage
                .store
                .set_string_value("fn_knock:test:completed-backup", "yes")
                .await
                .map_err(|error| BackupImportError::internal(error.to_string()))?;
            Ok(())
        })
        .await
    });
    started_rx.await.unwrap();
    request.abort();
    let _ = request.await;
    assert!(routes::BackupAdmission::try_acquire(&state).is_err());
    release_tx.send(()).unwrap();
    let _guard = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        state.maintenance.backup_request_lock.lock(),
    )
    .await
    .unwrap();
    assert_eq!(
        state
            .storage
            .store
            .get_string_value("fn_knock:test:completed-backup")
            .await
            .unwrap()
            .as_deref(),
        Some("yes")
    );
}

#[tokio::test]
async fn backup_download_admission_follows_retained_response_bytes() {
    let (_directory, state) = maintenance_test_state().await;
    let admission = routes::BackupAdmission::try_acquire(&state).unwrap();
    let response = binary_archive_response(
        BackupArchive {
            buffer: BackupArchiveBuffer::from_bytes(&[1, 2, 3]),
            exported_at: "2026-09-05T00:00:00Z".to_string(),
            filename: "test.knock".to_string(),
        },
        admission,
        &Translator::new("en"),
    );
    assert_eq!(response.status(), StatusCode::OK);
    assert!(routes::BackupAdmission::try_acquire(&state).is_err());
    let bytes = axum::body::to_bytes(response.into_body(), 1024)
        .await
        .unwrap();
    assert_eq!(&bytes[..], &[1, 2, 3]);
    let retained = bytes.clone();
    drop(bytes);
    assert!(routes::BackupAdmission::try_acquire(&state).is_err());
    drop(retained);
    assert!(routes::BackupAdmission::try_acquire(&state).is_ok());
}

#[tokio::test]
async fn backup_download_admission_follows_stream_body_and_last_chunk() {
    use http_body_util::BodyExt;

    let (_directory, state) = maintenance_test_state().await;
    let content = vec![42; 2 * 64 * 1024 + 17];
    for retain_body in [true, false] {
        let admission = routes::BackupAdmission::try_acquire(&state).unwrap();
        let response = binary_archive_response(
            BackupArchive {
                buffer: BackupArchiveBuffer::from_bytes(&content),
                exported_at: "2026-09-05T00:00:00Z".to_string(),
                filename: "test.knock".to_string(),
            },
            admission,
            &Translator::new("en"),
        );
        assert_eq!(
            response.headers()[header::CONTENT_LENGTH],
            content.len().to_string()
        );
        let mut body = response.into_body();
        let mut last = None;
        let mut received = 0;
        let mut chunks = 0;
        while let Some(frame) = body.frame().await {
            let data = frame.unwrap().into_data().unwrap();
            assert!(data.len() <= 64 * 1024);
            assert!(data.iter().all(|byte| *byte == 42));
            received += data.len();
            chunks += 1;
            last = Some(data);
        }
        assert_eq!(received, content.len());
        assert_eq!(chunks, 3);
        assert!(routes::BackupAdmission::try_acquire(&state).is_err());
        if retain_body {
            drop(last);
            assert!(routes::BackupAdmission::try_acquire(&state).is_err());
            drop(body);
        } else {
            drop(body);
            let last = last.unwrap();
            let retained = last.clone();
            drop(last);
            assert!(routes::BackupAdmission::try_acquire(&state).is_err());
            drop(retained);
        }
        assert!(routes::BackupAdmission::try_acquire(&state).is_ok());
    }
}

#[test]
fn backup_base64_decode_checks_padded_size_before_allocating() {
    for data in [b"1".as_slice(), b"12", b"123", b"1234", b"12345"] {
        let body = || ImportBackupBody {
            filename: Some("test.knock".to_string()),
            archive_base64: format!(" {} ", STANDARD.encode(data)),
        };
        assert_eq!(
            decode_backup_archive_body(body(), data.len()).unwrap(),
            data
        );
        let error = decode_backup_archive_body(body(), data.len() - 1).unwrap_err();
        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(error.message, "Backup archive is too large");
    }
    let error = decode_backup_archive_body(
        ImportBackupBody {
            filename: None,
            archive_base64: "a===".to_string(),
        },
        128,
    )
    .unwrap_err();
    assert_eq!(error.message, "Backup archive base64 is invalid");
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
    assert!(should_export_backup_key(
        "fn_knock:terminal:feature-settings-v2"
    ));
    assert!(!should_export_backup_key(
        "fn_knock:terminal:access-grant:browser"
    ));
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

#[tokio::test]
async fn backup_export_budget_counts_escaped_json_exactly() {
    let (_directory, state) = maintenance_test_state().await;
    let key = "fn_knock:backup-budget-test";
    let value = "\0\n\"\\\u{4e2d}\u{6587}";
    state
        .storage
        .store
        .set_string_value(key, value)
        .await
        .unwrap();
    let entries = state
        .storage
        .store
        .export_backup_entries_by_prefix_limited(key, 1024, |_| true)
        .await
        .unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["value"], value);
    let exact_size = serde_json::to_vec(&entries[0]).unwrap().len();
    let exact = state
        .storage
        .store
        .export_backup_entries_by_prefix_limited(key, exact_size, |_| true)
        .await
        .unwrap();
    assert_eq!(exact, entries);
    assert!(
        state
            .storage
            .store
            .export_backup_entries_by_prefix_limited(key, exact_size - 1, |_| true)
            .await
            .is_err()
    );
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
fn encrypted_zip_round_trips_incompressible_and_empty_payloads() {
    let mut random = 0x1234_5678_9abc_def0_u64;
    let payload = (0..256 * 1024)
        .map(|_| {
            random ^= random << 13;
            random ^= random >> 7;
            random ^= random << 17;
            b' ' + (random % 95) as u8
        })
        .collect::<Vec<_>>();
    for content in [payload.as_slice(), &[]] {
        let archive = create_password_protected_zip(
            KNOCK_BACKUP_JSON_FILENAME,
            content,
            KNOCK_BACKUP_PASSWORD,
            1_704_067_200_000,
        )
        .unwrap();
        assert_eq!(
            read_backup_json_from_archive_native(&archive)
                .unwrap()
                .as_bytes(),
            content
        );
    }
}

#[tokio::test]
async fn cancelled_backup_request_keeps_archive_lock_until_work_finishes() {
    let lock = std::sync::Arc::new(tokio::sync::Mutex::new(()));
    let guard = lock.clone().lock_owned().await;
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let task = tokio::spawn(run_backup_archive_work(guard, move || {
        let _ = started_tx.send(());
        release_rx.recv().unwrap();
    }));
    tokio::time::timeout(std::time::Duration::from_secs(1), started_rx)
        .await
        .unwrap()
        .unwrap();
    task.abort();
    let _ = task.await;
    assert!(lock.try_lock().is_err());
    release_tx.send(()).unwrap();
    let _guard = tokio::time::timeout(std::time::Duration::from_secs(1), lock.lock())
        .await
        .unwrap();
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
    let result = import_backup_archive_buffer(&state, archive.buffer.into_bytes(), &translator)
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
    let archive_bytes = archive.buffer.into_bytes();
    let backup_json = read_backup_json_from_archive_native(&archive_bytes).unwrap();
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
    import_backup_archive_buffer(&state, archive_bytes, &translator)
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
