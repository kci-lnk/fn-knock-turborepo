use super::super::tests::{acme_test_state, test_application, test_cert_info};
use super::*;

#[test]
fn output_configuration_and_collision_validation() {
    assert_eq!(
        normalize_file_output(None),
        json!({"enabled": false, "directory": ""})
    );
    for directory in ["", "relative/path", "/tmp/../etc", "/tmp/\ninvalid"] {
        assert!(
            validate_file_output_input(&json!({"enabled": true, "directory": directory})).is_err()
        );
    }
    assert!(validate_file_output_input(&json!({"enabled": "true", "directory": "/tmp"})).is_err());
    let dir = tempfile::tempdir().unwrap();
    let mut wildcard = test_application("a", &["*.example.test"]);
    wildcard["fileOutput"] = json!({"enabled": true, "directory": dir.path().join("new")});
    let mut literal = test_application("b", &["wildcard.example.test"]);
    literal["fileOutput"] = wildcard["fileOutput"].clone();
    assert!(check_output_conflicts(&wildcard, &[literal]).is_err());
    let (cert, key) = output_paths(&wildcard);
    assert_eq!(cert.file_name().unwrap(), "wildcard.example.test.cert.pem");
    assert_eq!(key.file_name().unwrap(), "wildcard.example.test.key.pem");
}

#[test]
fn pair_replacement_restores_previous_certificate_on_second_failure() {
    let dir = tempfile::tempdir().unwrap();
    let cert = dir.path().join("cert.pem");
    let key = dir.path().join("key.pem");
    write_output_pair(&cert, &key, "old cert", "old key").unwrap();
    let mut calls = 0;
    assert!(
        write_output_pair_with(&cert, &key, "new cert", "new key", |file, path| {
            calls += 1;
            if calls == 2 {
                anyhow::bail!("injected second-file failure");
            }
            replace_output_file(file, path)
        })
        .is_err()
    );
    assert_eq!(fs::read_to_string(&cert).unwrap(), "old cert");
    assert_eq!(fs::read_to_string(&key).unwrap(), "old key");
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    write_output_pair(&cert, &key, "new cert", "new key").unwrap();
    assert_eq!(fs::read_to_string(&cert).unwrap(), "new cert");
    assert_eq!(fs::read_to_string(&key).unwrap(), "new key");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(key).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

#[test]
fn first_output_failure_leaves_no_half_pair() {
    let dir = tempfile::tempdir().unwrap();
    let cert = dir.path().join("cert.pem");
    let key = dir.path().join("key.pem");
    assert!(
        write_output_pair_with(&cert, &key, "cert", "key", |file, path| {
            if path == key {
                anyhow::bail!("injected failure");
            }
            replace_output_file(file, path)
        })
        .is_err()
    );
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    fs::create_dir(&key).unwrap();
    assert!(write_output_pair(&cert, &key, "cert", "key").is_err());
    assert!(!cert.exists());
}

#[cfg(unix)]
#[test]
fn symlink_targets_rejected_and_directory_aliases_conflict() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    let cert = dir.path().join("cert.pem");
    let key = dir.path().join("key.pem");
    let other = dir.path().join("other");
    fs::write(&other, "keep").unwrap();
    symlink(&other, &key).unwrap();
    assert!(write_output_pair(&cert, &key, "cert", "key").is_err());
    assert_eq!(fs::read_to_string(other).unwrap(), "keep");
    let alias = dir.path().join("alias");
    symlink(dir.path(), &alias).unwrap();
    let mut a = test_application("a", &["*.example.test"]);
    a["fileOutput"] = json!({"enabled": true, "directory": dir.path().join("new")});
    let mut b = test_application("b", &["wildcard.example.test"]);
    b["fileOutput"] = json!({"enabled": true, "directory": alias.join("new")});
    assert!(check_output_conflicts(&a, &[b]).is_err());
}

async fn seed_certificate(state: &AppState, cert: &str, key: &str) {
    save_acme_issued_certificate(
        state,
        "a",
        "example.test",
        cert,
        key,
        test_cert_info(&["example.test"], cert),
    )
    .await
    .unwrap();
}

fn save_input(directory: &Path, enabled: bool) -> SaveAcmeApplicationInput {
    SaveAcmeApplicationInput {
        id: Some("a".into()),
        name: None,
        name_provided: false,
        domains: vec!["example.test".into()],
        dns_type: "dns_cf".into(),
        credentials: json!({}),
        renew_enabled: None,
        file_output: Some(json!({"enabled": enabled, "directory": directory})),
    }
}

#[tokio::test]
async fn settings_sync_existing_certificate_renewal_retry_and_disable() {
    let (dir, state) = acme_test_state().await;
    let output = dir.path().join("output");
    seed_certificate(&state, "cert-1", "key-1").await;
    let saved = save_acme_application_with_effects(
        &state,
        &Translator::new("en"),
        save_input(&output, true),
    )
    .await
    .unwrap();
    assert_eq!(saved.application["fileOutputStatus"]["status"], "saved");
    assert_eq!(
        fs::read_to_string(output.join("example.test.cert.pem")).unwrap(),
        "cert-1"
    );
    seed_certificate(&state, "cert-2", "key-2").await;
    assert_eq!(
        file_output_status(&state, &saved.application)
            .await
            .unwrap()["status"],
        "pending"
    );
    assert_eq!(
        sync_file_output(&state, "a").await.unwrap()["status"],
        "saved"
    );
    assert_eq!(
        fs::read_to_string(output.join("example.test.key.pem")).unwrap(),
        "key-2"
    );
    let blocked = dir.path().join("blocked");
    fs::write(&blocked, "not a directory").unwrap();
    let saved = save_acme_application_with_effects(
        &state,
        &Translator::new("en"),
        save_input(&blocked, true),
    )
    .await
    .unwrap();
    assert_eq!(saved.application["fileOutputStatus"]["status"], "error");
    assert_eq!(
        read_issued_certificates(&state).await.unwrap()[0]["cert"],
        "cert-2"
    );
    fs::remove_file(&blocked).unwrap();
    assert_eq!(
        sync_file_output(&state, "a").await.unwrap()["status"],
        "saved"
    );
    let mut input = save_input(&blocked, false);
    input.file_output = None;
    assert_eq!(
        save_acme_application_with_effects(&state, &Translator::new("en"), input)
            .await
            .unwrap()
            .application["fileOutput"]["enabled"],
        true
    );
    save_acme_application_with_effects(&state, &Translator::new("en"), save_input(&blocked, false))
        .await
        .unwrap();
    seed_certificate(&state, "cert-3", "key-3").await;
    assert_eq!(
        sync_file_output(&state, "a").await.unwrap()["status"],
        "disabled"
    );
    assert_eq!(
        fs::read_to_string(blocked.join("example.test.cert.pem")).unwrap(),
        "cert-2"
    );
    assert!(output.join("example.test.cert.pem").exists());
}

#[tokio::test]
async fn waiting_and_changed_domains_never_export_old_certificate() {
    let (dir, state) = acme_test_state().await;
    let output = dir.path().join("output");
    let saved = save_acme_application_with_effects(
        &state,
        &Translator::new("en"),
        save_input(&output, true),
    )
    .await
    .unwrap();
    assert_eq!(saved.application["fileOutputStatus"]["status"], "waiting");
    assert!(!output.exists());
    seed_certificate(&state, "cert", "key").await;
    let mut input = save_input(&output, true);
    input.domains = vec!["new.example.test".into()];
    save_acme_application_with_effects(&state, &Translator::new("en"), input)
        .await
        .unwrap();
    assert_eq!(
        sync_file_output(&state, "a").await.unwrap()["status"],
        "waiting"
    );
    assert!(!output.exists());
}

#[tokio::test]
async fn queued_sync_reads_latest_configuration_after_commit_or_disable() {
    let (dir, state) = acme_test_state().await;
    let output = dir.path().join("output");
    seed_certificate(&state, "old cert", "old key").await;
    save_acme_application_with_effects(&state, &Translator::new("en"), save_input(&output, true))
        .await
        .unwrap();
    let guard = state.gateway.acme_output_lock.lock().await;
    let mut pending = Box::pin(sync_file_output(&state, "a"));
    tokio::select! {
        biased;
        _ = &mut pending => panic!("sync must wait for the certificate commit"),
        _ = tokio::task::yield_now() => {}
    }
    seed_certificate(&state, "new cert", "new key").await;
    drop(guard);
    assert_eq!(pending.await.unwrap()["status"], "saved");
    assert_eq!(
        fs::read_to_string(output.join("example.test.cert.pem")).unwrap(),
        "new cert"
    );

    let guard = state.gateway.acme_output_lock.lock().await;
    let mut pending = Box::pin(sync_file_output(&state, "a"));
    tokio::select! {
        biased;
        _ = &mut pending => panic!("sync must wait for the configuration change"),
        _ = tokio::task::yield_now() => {}
    }
    let mut applications = read_acme_applications(&state).await.unwrap();
    applications[0]["fileOutput"]["enabled"] = json!(false);
    write_acme_applications(&state, &applications)
        .await
        .unwrap();
    seed_certificate(&state, "never export", "never export key").await;
    drop(guard);
    assert_eq!(pending.await.unwrap()["status"], "disabled");
    assert_eq!(
        fs::read_to_string(output.join("example.test.cert.pem")).unwrap(),
        "new cert"
    );
}

#[tokio::test]
async fn queued_sync_cannot_observe_rolled_back_issued_certificate() {
    let (dir, state) = acme_test_state().await;
    let output = dir.path().join("output");
    seed_certificate(&state, "old cert", "old key").await;
    save_acme_application_with_effects(&state, &Translator::new("en"), save_input(&output, true))
        .await
        .unwrap();
    let guard = state.gateway.acme_output_lock.lock().await;
    let previous = read_issued_certificates(&state).await.unwrap().remove(0);
    seed_certificate(&state, "uncommitted cert", "uncommitted key").await;
    let mut pending = Box::pin(sync_file_output(&state, "a"));
    tokio::select! {
        biased;
        _ = &mut pending => panic!("sync must wait for the certificate commit"),
        _ = tokio::task::yield_now() => {}
    }
    restore_acme_issued_certificate_snapshot(&state, "a", "example.test", Some(&previous))
        .await
        .unwrap();
    drop(guard);
    assert_eq!(pending.await.unwrap()["status"], "saved");
    assert_eq!(
        fs::read_to_string(output.join("example.test.cert.pem")).unwrap(),
        "old cert"
    );
}

#[tokio::test]
async fn manual_sync_returns_public_status_without_certificate_content() {
    let (dir, state) = acme_test_state().await;
    let output = dir.path().join("output");
    seed_certificate(&state, "secret cert", "secret key").await;
    save_acme_application_with_effects(&state, &Translator::new("en"), save_input(&output, true))
        .await
        .unwrap();
    let response =
        handlers::sync_application_file_output(State(state.clone()), AxumPath("a".into())).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 65536).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["data"]["status"], "saved");
    assert!(body["data"].get("fingerprint").is_none());
    assert!(!body.to_string().contains("secret key"));
    let response =
        handlers::sync_application_file_output(State(state.clone()), AxumPath("missing".into()))
            .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    delete_acme_application_internal(&state, "a").await.unwrap();
    assert!(output.join("example.test.cert.pem").exists());
    assert!(output.join("example.test.key.pem").exists());
}

#[tokio::test]
async fn internal_cleanup_directories_cannot_be_used_for_external_exports() {
    let (_dir, state) = acme_test_state().await;
    let output = state.settings.data_dir.join("ssl").join("example.test");
    let error = save_acme_application_with_effects(
        &state,
        &Translator::new("en"),
        save_input(&output, true),
    )
    .await
    .err()
    .unwrap();
    assert!(error.to_string().contains("outside the internal"));
    assert!(read_acme_applications(&state).await.unwrap().is_empty());
}

#[cfg(unix)]
#[test]
fn unwritable_directory_preserves_existing_pair() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let cert = dir.path().join("cert.pem");
    let key = dir.path().join("key.pem");
    write_output_pair(&cert, &key, "old cert", "old key").unwrap();
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o500)).unwrap();
    let result = write_output_pair(&cert, &key, "new cert", "new key");
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
    // A privileged test runner can legitimately bypass directory mode bits.
    if result.is_ok() {
        return;
    }
    assert_eq!(fs::read_to_string(cert).unwrap(), "old cert");
    assert_eq!(fs::read_to_string(key).unwrap(), "old key");
}

#[test]
fn failed_rollback_keeps_a_recoverable_previous_certificate() {
    let dir = tempfile::tempdir().unwrap();
    let cert = dir.path().join("cert.pem");
    let key = dir.path().join("key.pem");
    write_output_pair(&cert, &key, "old cert", "old key").unwrap();
    let error = write_output_pair_with(&cert, &key, "new cert", "new key", |file, path| {
        if path == key {
            // Simulate a target changing before rollback, which must refuse to
            // overwrite it but retain the backup instead of dropping it.
            fs::remove_file(&cert)?;
            fs::create_dir(&cert)?;
            anyhow::bail!("injected second-file failure");
        }
        replace_output_file(file, path)
    })
    .unwrap_err();
    let recovery = fs::read_dir(dir.path())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path != &cert && path != &key)
        .expect("the previous certificate backup must survive failed rollback");
    assert_eq!(fs::read_to_string(&recovery).unwrap(), "old cert");
    assert_eq!(fs::read_to_string(&key).unwrap(), "old key");
    assert!(error.to_string().contains(recovery.to_str().unwrap()));
}

#[tokio::test]
async fn cancelled_sync_keeps_lock_and_persists_worker_outcome() {
    for fail in [false, true] {
        let (dir, state) = acme_test_state().await;
        let output = dir.path().join("output");
        seed_certificate(&state, "cert", "key").await;
        let saved = save_acme_application_with_effects(
            &state,
            &Translator::new("en"),
            save_input(&output, true),
        )
        .await
        .unwrap();
        let guard = std::sync::Arc::new(state.gateway.acme_output_lock.clone().lock_owned().await);
        let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
        let (resume_tx, resume_rx) = std::sync::mpsc::channel();
        let mut sync = Box::pin(sync_file_output_locked_with(
            &state,
            "a",
            &guard,
            move |cert, key, cert_content, key_content| {
                entered_tx.send(()).unwrap();
                resume_rx.recv_timeout(std::time::Duration::from_secs(5))?;
                anyhow::ensure!(!fail, "injected write failure");
                write_output_pair(cert, key, cert_content, key_content)
            },
        ));
        tokio::select! {
            result = &mut sync => panic!("worker must wait before completing: {result:?}"),
            entered = entered_rx => entered.unwrap(),
        }
        drop(sync);
        drop(guard);
        assert!(state.gateway.acme_output_lock.try_lock().is_err());
        let pending = file_output_status(&state, &saved.application)
            .await
            .unwrap();
        assert_eq!(pending["status"], "pending");
        assert_eq!(
            pending["lastSuccessAt"],
            saved.application["fileOutputStatus"]["lastSuccessAt"]
        );
        resume_tx.send(()).unwrap();
        let _guard = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            state.gateway.acme_output_lock.lock(),
        )
        .await
        .unwrap();
        let status = file_output_status(&state, &saved.application)
            .await
            .unwrap();
        assert_eq!(status["status"], if fail { "error" } else { "saved" });
        if fail {
            assert!(
                status["error"]
                    .as_str()
                    .unwrap()
                    .contains("injected write failure")
            );
        }
    }
}

#[tokio::test]
async fn worker_panic_is_reported_as_save_failure() {
    let (dir, state) = acme_test_state().await;
    let output = dir.path().join("output");
    seed_certificate(&state, "cert", "key").await;
    let saved = save_acme_application_with_effects(
        &state,
        &Translator::new("en"),
        save_input(&output, true),
    )
    .await
    .unwrap();
    let guard = std::sync::Arc::new(state.gateway.acme_output_lock.clone().lock_owned().await);
    let status = sync_file_output_locked_with(&state, "a", &guard, |_, _, _, _| {
        panic!("injected worker panic");
    })
    .await
    .unwrap();
    assert_eq!(status["status"], "error");
    assert_eq!(
        file_output_status(&state, &saved.application)
            .await
            .unwrap(),
        status
    );
}
