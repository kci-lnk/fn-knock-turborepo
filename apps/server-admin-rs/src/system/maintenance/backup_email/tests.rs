use super::*;
use crate::infra::mail::test_support::smtp;

fn update(port: u16) -> BackupEmailUpdate {
    BackupEmailUpdate {
        config: BackupEmailConfig {
            enabled: true,
            smtp: SmtpConfig {
                host: "127.0.0.1".into(),
                port,
                security: "none".into(),
                auth_mode: "none".into(),
                timeout_seconds: 2,
                ..Default::default()
            },
            from_address: "backup@example.com".into(),
            to_addresses: vec!["one@example.com".into(), "two@example.com".into()],
            ..Default::default()
        },
        password: None,
        clear_password: false,
    }
}
async fn enable(state: &AppState, email: Option<BackupEmailUpdate>) -> Value {
    save_automatic_backup_config(
        state,
        UpdateAutomaticBackupBody {
            enabled: true,
            interval_hours: 24,
            retention_days: 7,
            email,
        },
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn credentials_are_write_only_excluded_and_legacy_updates_keep_them() {
    let (_dir, state) = super::super::tests::maintenance_test_state().await;
    let mut config = update(25);
    config.password = Some("secret-do-not-export".into());
    let response = enable(&state, Some(config)).await;
    assert!(
        response["config"]["email"]["password_configured"]
            .as_bool()
            .unwrap()
    );
    assert!(!response.to_string().contains("secret-do-not-export"));
    let email = load(&state).await.unwrap();
    assert_eq!(password(&state, &email).unwrap(), "secret-do-not-export");
    assert!(
        !serde_json::to_string(&email)
            .unwrap()
            .contains("secret-do-not-export")
    );
    let payload = export_backup_payload(&state).await.unwrap();
    assert!(!payload.to_string().contains(EMAIL_KEY));
    enable(&state, None).await;
    assert_eq!(
        password(&state, &load(&state).await.unwrap()).unwrap(),
        "secret-do-not-export"
    );
    let mut clear = update(25);
    clear.clear_password = true;
    enable(&state, Some(clear)).await;
    assert!(load(&state).await.unwrap().secret_id.is_none());
}

#[tokio::test]
async fn delivery_sends_original_archive_and_records_success() {
    let (_dir, state) = super::super::tests::maintenance_test_state().await;
    let (port, server) = smtp("250 recipient OK\r\n").await;
    enable(&state, Some(update(port))).await;
    let archive = run_automatic_backup_once(&state).await.unwrap();
    let filename = archive["filename"].as_str().unwrap();
    let bytes = fs::read(automatic_backup_directory(&state).join(filename))
        .await
        .unwrap();
    worker::tick(&state, true).await.unwrap();
    let captured = server.await.unwrap();
    assert!(captured.contains(filename));
    assert!(captured.contains("one@example.com") && captured.contains("two@example.com"));
    let encoded = STANDARD.encode(&bytes);
    assert!(captured.replace("\r\n", "").contains(&encoded));
    let email = load(&state).await.unwrap();
    assert!(email.jobs[0].status == JobStatus::Sent);
    assert!(email.last_success_at.is_some());
    assert!(email.last_error.is_none());
    let translator = Translator::from_state(&state).await;
    import_backup_archive_buffer(&state, bytes, &translator)
        .await
        .unwrap();
    assert!(load(&state).await.unwrap().jobs[0].status == JobStatus::Sent);
}

#[tokio::test]
async fn transient_failures_retry_same_archive_and_permanent_failures_stop() {
    let (_dir, state) = super::super::tests::maintenance_test_state().await;
    let (port, server) = smtp("450 temporary failure\r\n").await;
    enable(&state, Some(update(port))).await;
    let archive = run_automatic_backup_once(&state).await.unwrap();
    worker::tick(&state, true).await.unwrap();
    server.await.unwrap();
    let mut email = load(&state).await.unwrap();
    assert!(email.jobs[0].status == JobStatus::Pending);
    assert_eq!(email.jobs[0].attempts, 1);
    assert!(email.jobs[0].next_ms - time_utils::now_ms() > 50_000);
    let id = email.jobs[0].id.clone();
    let (port, server) = smtp("550 invalid recipient\r\n").await;
    email.config.smtp.port = port;
    email.jobs[0].next_ms = 0;
    persist(&state, &email).await.unwrap();
    worker::tick(&state, false).await.unwrap();
    server.await.unwrap();
    let email = load(&state).await.unwrap();
    assert!(email.jobs[0].status == JobStatus::Failed);
    assert_eq!(email.jobs[0].id, id);
    assert_eq!(email.jobs[0].filename, archive["filename"]);
    assert_eq!(
        automatic_backup_files_payload(&state).await.unwrap()["files"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(load_automatic_backup_runtime(&state).await.unwrap()["last_error"].is_null());
}

#[tokio::test]
async fn recovered_sending_job_is_retried_and_configuration_changes_cancel_pending() {
    let (_dir, state) = super::super::tests::maintenance_test_state().await;
    let (port, server) = smtp("250 OK\r\n").await;
    enable(&state, Some(update(port))).await;
    run_automatic_backup_once(&state).await.unwrap();
    let mut email = load(&state).await.unwrap();
    email.jobs[0].status = JobStatus::Sending;
    email.jobs[0].attempts = 1;
    persist(&state, &email).await.unwrap();
    worker::tick(&state, true).await.unwrap();
    server.await.unwrap();
    assert!(load(&state).await.unwrap().jobs[0].status == JobStatus::Sent);
    run_automatic_backup_once(&state).await.unwrap();
    let mut config = update(port);
    config.config.from_name = "Changed".into();
    enable(&state, Some(config)).await;
    assert!(
        load(&state)
            .await
            .unwrap()
            .jobs
            .iter()
            .all(|job| !job.active())
    );
}

#[tokio::test]
async fn missing_files_and_expired_jobs_fail_without_regenerating_backup() {
    let (_dir, state) = super::super::tests::maintenance_test_state().await;
    enable(&state, Some(update(25))).await;
    let archive = run_automatic_backup_once(&state).await.unwrap();
    fs::remove_file(automatic_backup_directory(&state).join(archive["filename"].as_str().unwrap()))
        .await
        .unwrap();
    worker::tick(&state, true).await.unwrap();
    assert_eq!(
        load(&state).await.unwrap().last_error.as_deref(),
        Some("backup_missing")
    );
    run_automatic_backup_once(&state).await.unwrap();
    let mut email = load(&state).await.unwrap();
    email.jobs[0].created_ms = 0;
    persist(&state, &email).await.unwrap();
    assert!(pinned_files(&state).await.unwrap().is_empty());
    worker::tick(&state, false).await.unwrap();
    assert!(load(&state).await.unwrap().jobs[0].status == JobStatus::Failed);
}

#[tokio::test]
async fn oversized_attachment_sends_explanation_but_is_not_delivery_success() {
    let (_dir, state) = super::super::tests::maintenance_test_state().await;
    let (port, server) = smtp("250 OK\r\n").await;
    let mut config = update(port);
    config.config.attachment_limit_mib = 1;
    enable(&state, Some(config)).await;
    let archive = run_automatic_backup_once(&state).await.unwrap();
    fs::write(
        automatic_backup_directory(&state).join(archive["filename"].as_str().unwrap()),
        vec![0; 1024 * 1024 + 1],
    )
    .await
    .unwrap();
    worker::tick(&state, true).await.unwrap();
    let captured = server.await.unwrap();
    assert!(!captured.contains("Content-Disposition: attachment"));
    assert_eq!(
        load(&state).await.unwrap().last_error.as_deref(),
        Some("attachment_too_large")
    );
}

#[tokio::test]
async fn failed_atomic_commit_neither_publishes_archive_nor_changes_password() {
    let (_dir, state) = super::super::tests::maintenance_test_state().await;
    let mut config = update(25);
    config.password = Some("original".into());
    enable(&state, Some(config)).await;
    let connection =
        tokio_rusqlite::rusqlite::Connection::open(&state.settings.sqlite_path).unwrap();
    connection.execute_batch("CREATE TRIGGER reject_backup_mail BEFORE INSERT ON kv_strings WHEN NEW.key = 'fn_knock:config:backup:automatic:email' BEGIN SELECT RAISE(ABORT, 'test persistence failure'); END;").unwrap();
    assert!(run_automatic_backup_once(&state).await.is_err());
    assert!(load(&state).await.unwrap().jobs.is_empty());
    assert!(
        automatic_backup_files_payload(&state).await.unwrap()["files"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let mut config = update(25);
    config.password = Some("replacement".into());
    let result = save_automatic_backup_config(
        &state,
        UpdateAutomaticBackupBody {
            enabled: true,
            interval_hours: 24,
            retention_days: 7,
            email: Some(config),
        },
    )
    .await;
    assert!(result.is_err());
    assert_eq!(
        password(&state, &load(&state).await.unwrap()).unwrap(),
        "original"
    );
}

#[tokio::test]
async fn pending_archives_are_pinned_until_cancelled() {
    let (_dir, state) = super::super::tests::maintenance_test_state().await;
    enable(&state, Some(update(25))).await;
    let archive = run_automatic_backup_once(&state).await.unwrap();
    let path = automatic_backup_directory(&state).join(archive["filename"].as_str().unwrap());
    let file = std::fs::File::options().write(true).open(&path).unwrap();
    file.set_times(std::fs::FileTimes::new().set_modified(SystemTime::UNIX_EPOCH))
        .unwrap();
    prune_automatic_backup_directory(&state, 1).await.unwrap();
    assert!(path.exists());
    save_automatic_backup_config(
        &state,
        UpdateAutomaticBackupBody {
            enabled: false,
            interval_hours: 24,
            retention_days: 7,
            email: None,
        },
    )
    .await
    .unwrap();
    prune_automatic_backup_directory(&state, 1).await.unwrap();
    assert!(!path.exists());
}

#[tokio::test]
async fn test_email_uses_draft_without_saving_or_exporting_backup() {
    let (_dir, state) = super::super::tests::maintenance_test_state().await;
    let (port, server) = smtp("250 OK\r\n").await;
    test_email(&state, update(port)).await.unwrap();
    let captured = server.await.unwrap();
    assert!(captured.contains("backup-email-test.txt"));
    assert!(!load(&state).await.unwrap().config.enabled);
    assert!(load(&state).await.unwrap().jobs.is_empty());
}

#[tokio::test]
async fn retry_budget_is_four_attempts_and_keeps_the_message_id() {
    let (_dir, state) = super::super::tests::maintenance_test_state().await;
    enable(&state, Some(update(25))).await;
    run_automatic_backup_once(&state).await.unwrap();
    let original_id = load(&state).await.unwrap().jobs[0].id.clone();
    for attempt in 1..=4 {
        let (port, server) = smtp("450 temporary failure\r\n").await;
        let mut email = load(&state).await.unwrap();
        email.config.smtp.port = port;
        email.jobs[0].next_ms = 0;
        persist(&state, &email).await.unwrap();
        worker::tick(&state, false).await.unwrap();
        server.await.unwrap();
        let email = load(&state).await.unwrap();
        assert_eq!(email.jobs[0].attempts, attempt);
        assert_eq!(email.jobs[0].id, original_id);
        if attempt < 4 {
            assert!(email.jobs[0].status == JobStatus::Pending);
            let delay = [60_000, 300_000, 1_800_000][usize::from(attempt - 1)];
            assert!(email.jobs[0].next_ms - time_utils::now_ms() > delay - 5_000);
        } else {
            assert!(email.jobs[0].status == JobStatus::Failed);
        }
    }
}

#[tokio::test]
async fn smtp_timeout_does_not_hold_the_maintenance_lock() {
    let (_dir, state) = super::super::tests::maintenance_test_state().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let mut config = update(port);
    config.config.smtp.timeout_seconds = 1;
    enable(&state, Some(config)).await;
    run_automatic_backup_once(&state).await.unwrap();
    let worker_state = state.clone();
    let task = tokio::spawn(async move { worker::tick(&worker_state, true).await });
    let (_stream, _) = tokio::time::timeout(std::time::Duration::from_secs(5), listener.accept())
        .await
        .unwrap()
        .unwrap();
    let guard = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        state.maintenance.automatic_backup_lock.lock(),
    )
    .await
    .expect("SMTP must not hold the maintenance lock");
    drop(guard);
    save_automatic_backup_config(
        &state,
        UpdateAutomaticBackupBody {
            enabled: false,
            interval_hours: 24,
            retention_days: 7,
            email: None,
        },
    )
    .await
    .unwrap();
    task.await.unwrap().unwrap();
    let email = load(&state).await.unwrap();
    assert_eq!(email.last_error.as_deref(), Some("smtp_timeout"));
    assert!(!email.jobs[0].active());
}

#[tokio::test]
async fn empty_password_update_preserves_pending_jobs_and_revision() {
    let (_dir, state) = super::super::tests::maintenance_test_state().await;
    enable(&state, Some(update(25))).await;
    run_automatic_backup_once(&state).await.unwrap();
    let before = load(&state).await.unwrap();
    let mut config = update(25);
    config.password = Some(String::new());
    enable(&state, Some(config)).await;
    let after = load(&state).await.unwrap();
    assert_eq!(before.revision, after.revision);
    assert!(after.jobs[0].status == JobStatus::Pending);
}

#[tokio::test]
async fn known_delivery_result_survives_storage_failures_without_resending() {
    let (_dir, state) = super::super::tests::maintenance_test_state().await;
    enable(&state, Some(update(25))).await;
    run_automatic_backup_once(&state).await.unwrap();
    let mut email = load(&state).await.unwrap();
    email.jobs[0].status = JobStatus::Sending;
    email.jobs[0].attempts = 1;
    persist(&state, &email).await.unwrap();
    let mut worker = worker::DeliveryWorker::new(false);
    worker.completion = Some(worker::Completion {
        job: email.jobs[0].clone(),
        result: Ok(()),
    });
    let connection =
        tokio_rusqlite::rusqlite::Connection::open(&state.settings.sqlite_path).unwrap();
    connection.execute_batch("CREATE TRIGGER fail_delivery_result BEFORE INSERT ON kv_strings WHEN NEW.key = 'fn_knock:config:backup:automatic:email' BEGIN SELECT RAISE(ABORT, 'failed bookkeeping'); END;").unwrap();
    assert!(worker.tick(&state).await.is_err());
    assert!(worker.tick(&state).await.is_err());
    assert!(worker.completion.is_some());
    connection
        .execute_batch("DROP TRIGGER fail_delivery_result;")
        .unwrap();
    worker.tick(&state).await.unwrap();
    let after = load(&state).await.unwrap();
    assert!(after.jobs[0].status == JobStatus::Sent);
    assert_eq!(after.jobs[0].attempts, 1);
    assert!(worker.completion.is_none());
}
