use super::*;

async fn acme_test_state_with_data_dir(data_dir: PathBuf, runtime_target: &str) -> AppState {
    let mut settings = {
        let _environment = crate::test_support::EnvGuard::new(&[]);
        crate::settings::Settings::from_env()
    };
    settings.runtime_target = runtime_target.to_string();
    settings.gateway_config_dir = data_dir.join("gateway");
    settings.sqlite_path = data_dir.join("fn-knock.sqlite3");
    settings.data_dir = data_dir;
    settings.legacy_redis_url = String::new();
    settings.go_backend_grpc_addr = "127.0.0.1:1".to_string();
    settings.internal_rpc_token = "test-internal-rpc-token".to_string();
    settings.request_timeout = std::time::Duration::from_millis(100);
    AppState::new(settings)
        .await
        .expect("create ACME test state")
}

async fn acme_test_state() -> (tempfile::TempDir, AppState) {
    let directory = tempfile::tempdir().expect("create ACME test directory");
    let state = acme_test_state_with_data_dir(directory.path().join("data"), "linux").await;
    state
        .storage
        .store
        .set_string_value(ACME_MIGRATION_VERSION_KEY, "1")
        .await
        .expect("mark ACME data migrated");
    (directory, state)
}

#[cfg(unix)]
#[tokio::test]
async fn acme_command_preserves_spaced_paths_and_uses_unspaced_http_workspace() {
    let directory = tempfile::tempdir().expect("create ACME path test directory");
    let state = acme_test_state_with_data_dir(
        directory.path().join("Application Support/FnKnock/data"),
        "macos",
    )
    .await;

    let executable = acme_executable_path(&state);
    tokio::fs::create_dir_all(executable.parent().expect("ACME executable parent"))
        .await
        .expect("create ACME executable parent");
    tokio::fs::write(
        &executable,
        r#"#!/bin/sh
{
  printf 'HTTP_HEADER=%s\n' "$HTTP_HEADER"
  printf 'LE_TEMP_DIR=%s\n' "$LE_TEMP_DIR"
  for argument in "$@"; do
    printf 'ARG=%s\n' "$argument"
  done
} > "$FN_KNOCK_TEST_ACME_RECORD"
"#,
    )
    .await
    .expect("write ACME argument fixture");
    crate::fs_utils::chmod_executable(&executable);

    let record_path = state.settings.data_dir.join("acme-command-record.txt");
    let mut extra_env = Map::new();
    extra_env.insert(
        "FN_KNOCK_TEST_ACME_RECORD".to_string(),
        json!(record_path.to_string_lossy()),
    );
    let args = shared_acme_args(&state, Some("letsencrypt"));
    let result = run_acme_command(&state, args.clone(), Some(&extra_env))
        .await
        .expect("run ACME argument fixture");
    assert_eq!(result.exit_code, 0);

    let record = tokio::fs::read_to_string(&record_path)
        .await
        .expect("read ACME argument record");
    let value = |prefix: &str| {
        record
            .lines()
            .find_map(|line| line.strip_prefix(prefix))
            .expect("recorded ACME environment value")
    };
    let http_header = value("HTTP_HEADER=");
    let temp_dir = value("LE_TEMP_DIR=");
    assert!(!http_header.chars().any(char::is_whitespace));
    assert!(!temp_dir.chars().any(char::is_whitespace));
    assert_eq!(Path::new(http_header).parent(), Some(Path::new(temp_dir)));
    assert!(!Path::new(temp_dir).exists(), "workspace must be removed");

    let recorded_args = record
        .lines()
        .filter_map(|line| line.strip_prefix("ARG="))
        .map(str::to_string)
        .collect::<Vec<_>>();
    assert_eq!(recorded_args[0], "--home");
    assert_eq!(recorded_args[2], "--config-home");
    assert_eq!(recorded_args[1], recorded_args[3]);
    assert!(!recorded_args[1].chars().any(char::is_whitespace));
    assert_eq!(
        Path::new(&recorded_args[1]).parent(),
        Some(Path::new(temp_dir))
    );
    assert!(
        !Path::new(&recorded_args[1]).exists(),
        "home link must be removed"
    );
    assert_eq!(&recorded_args[4..], &args[4..]);
    assert!(args[1].contains("Application Support"));
    assert!(acme_home_dir(&state).join("acme.sh").is_file());
}

#[cfg(unix)]
#[tokio::test]
async fn bundled_acme_script_persists_state_through_spaced_data_path() {
    let directory = tempfile::tempdir().expect("create bundled ACME path test directory");
    let state = acme_test_state_with_data_dir(
        directory.path().join("Application Support/FnKnock/data"),
        "macos",
    )
    .await;

    install_from_bundled_zip_blocking(&state).expect("install bundled acme.sh");
    set_default_certificate_authority(&state, "letsencrypt", &Translator::new("zh-CN"))
        .await
        .expect("run bundled acme.sh through an unspaced workspace");

    let account_conf = tokio::fs::read_to_string(acme_home_dir(&state).join("account.conf"))
        .await
        .expect("read persisted bundled acme.sh account config");
    assert_eq!(default_certificate_authority(&state), "letsencrypt");
    assert!(!account_conf.contains("fn-knock-acme-"));
    assert!(acme_home_dir(&state).join("acme.sh").is_file());
}

#[test]
fn acme_command_log_quotes_paths_with_spaces() {
    let executable = Path::new("/Library/Application Support/FnKnock/data/.acme.sh/acme.sh");
    let args = vec![
        "--home".to_string(),
        "/Library/Application Support/FnKnock/data/.acme.sh".to_string(),
        "-d".to_string(),
        "*.example.test".to_string(),
    ];
    assert_eq!(
        format_acme_command_for_log(executable, &args),
        "'/Library/Application Support/FnKnock/data/.acme.sh/acme.sh' --home '/Library/Application Support/FnKnock/data/.acme.sh' -d '*.example.test'"
    );
}

fn test_application(id: &str, domains: &[&str]) -> Value {
    json!({
        "id": id,
        "name": format!("Application {id}"),
        "domains": domains,
        "primaryDomain": domains.first().copied().unwrap_or_default(),
        "dnsType": "dns_cf",
        "credentials": { "CF_Token": "secret" },
        "renewEnabled": true,
        "createdAt": "2026-07-01T00:00:00.000Z",
        "updatedAt": "2026-07-01T00:00:00.000Z",
        "latestJobStatus": "idle"
    })
}

fn test_cert_info(domains: &[&str], serial_number: &str) -> Value {
    json!({
        "issuer": "CN=ACME Test CA",
        "subject": format!("CN={}", domains.first().copied().unwrap_or_default()),
        "validFrom": "Jul  1 00:00:00 2026 GMT",
        "validTo": "Jul  1 00:00:00 2036 GMT",
        "dnsNames": domains,
        "serialNumber": serial_number
    })
}

fn test_managed_certificate(
    id: &str,
    source: &str,
    source_ref_id: Option<&str>,
    primary_domain: &str,
    cert: &str,
    key: &str,
) -> Value {
    let mut certificate = json!({
        "id": id,
        "label": format!("Certificate {id}"),
        "source": source,
        "primary_domain": primary_domain,
        "cert": cert,
        "key": key,
        "created_at": "2026-07-01T00:00:00.000Z",
        "updated_at": "2026-07-01T00:00:00.000Z"
    });
    if let Some(source_ref_id) = source_ref_id {
        certificate["source_ref_id"] = json!(source_ref_id);
    }
    certificate
}

async fn save_test_ssl_config(state: &AppState, ssl: Value) {
    let mut config = state
        .storage
        .store
        .get_config()
        .await
        .expect("load test config");
    config["ssl"] = ssl;
    state
        .storage
        .store
        .save_config(&config)
        .await
        .expect("save test SSL config");
}

fn generate_acme_test_cert_pair(common_name: &str) -> Option<(String, String)> {
    use std::process::{Command as StdCommand, Stdio as StdStdio};

    if !StdCommand::new("openssl")
        .arg("version")
        .stdin(StdStdio::null())
        .output()
        .ok()?
        .status
        .success()
    {
        return None;
    }
    let temp_dir = tempfile::tempdir().ok()?;
    let key_path = temp_dir.path().join("key.pem");
    let cert_path = temp_dir.path().join("cert.pem");
    let output = StdCommand::new("openssl")
        .args([
            "req", "-x509", "-newkey", "rsa:2048", "-sha256", "-days", "1", "-nodes", "-keyout",
        ])
        .arg(&key_path)
        .arg("-out")
        .arg(&cert_path)
        .arg("-subj")
        .arg(format!("/CN={common_name}"))
        .stdin(StdStdio::null())
        .stdout(StdStdio::null())
        .stderr(StdStdio::null())
        .status()
        .ok()?;
    if !output.success() {
        return None;
    }
    Some((
        std::fs::read_to_string(cert_path).ok()?,
        std::fs::read_to_string(key_path).ok()?,
    ))
}

#[derive(Default)]
struct FailThenSucceedSslDeployment {
    calls: Vec<Value>,
}

#[async_trait::async_trait]
impl AcmeSslDeployment for FailThenSucceedSslDeployment {
    async fn sync(&mut self, _state: &AppState, config: &Value) -> anyhow::Result<()> {
        self.calls.push(config.clone());
        if self.calls.len() == 1 {
            anyhow::bail!("simulated replacement deployment failure");
        }
        Ok(())
    }
}

struct ConcurrentSslWriteThenSucceedDeployment {
    calls: Vec<Value>,
    concurrent_ssl: Value,
}

#[async_trait::async_trait]
impl AcmeSslDeployment for ConcurrentSslWriteThenSucceedDeployment {
    async fn sync(&mut self, state: &AppState, config: &Value) -> anyhow::Result<()> {
        self.calls.push(config.clone());
        if self.calls.len() == 1 {
            save_test_ssl_config(state, self.concurrent_ssl.clone()).await;
        }
        Ok(())
    }
}

#[test]
fn normalizes_acme_application_like_node() {
    let value = normalize_acme_application(json!({
        "id": " app ",
        "domains": ["Example.com", "example.com", "www.example.com"],
        "dnsType": " dns_cf ",
        "credentials": { " CF_Key ": " secret ", "empty": "" },
        "createdAt": "2026-07-05T01:02:03.946511792Z",
        "updatedAt": "2026-07-05T09:02:03+08:00",
        "latestJobStatus": "bad"
    }))
    .expect("application");
    assert_eq!(value["id"], json!("app"));
    assert_eq!(value["primaryDomain"], json!("example.com"));
    assert_eq!(value["domains"], json!(["example.com", "www.example.com"]));
    assert_eq!(value["credentials"], json!({ "CF_Key": "secret" }));
    assert_eq!(value["renewEnabled"], json!(true));
    assert_eq!(value["createdAt"], json!("2026-07-05T01:02:03.946Z"));
    assert_eq!(value["updatedAt"], json!("2026-07-05T01:02:03.000Z"));
    assert_eq!(value["latestJobStatus"], Value::Null);
}

#[test]
fn normalizes_acme_timestamps_to_node_iso_shape() {
    assert_eq!(
        normalize_timestamp("2026-07-07T10:18:23.946511792Z"),
        Some("2026-07-07T10:18:23.946Z".to_string())
    );
    assert_eq!(
        normalize_timestamp("2026-07-07T18:18:23+08:00"),
        Some("2026-07-07T10:18:23.000Z".to_string())
    );
    assert_eq!(normalize_timestamp("not-a-date"), None);
}

#[test]
fn acme_renew_interval_prefers_node_cron_env() {
    assert_eq!(
        acme_renew_interval_from_values(None, None).as_secs(),
        6 * 3600
    );
    assert_eq!(
        acme_renew_interval_from_values(Some("0 */6 * * *"), Some("7200")).as_secs(),
        6 * 3600
    );
    assert_eq!(
        acme_renew_interval_from_values(Some("*/30 * * * *"), None).as_secs(),
        30 * 60
    );
    assert_eq!(
        acme_renew_interval_from_values(None, Some("7200")).as_secs(),
        7200
    );
}

#[test]
fn parses_persisted_and_rfc3339_certificate_expirations() {
    let expected = parse_certificate_unix_timestamp("2036-07-01T20:51:37Z")
        .expect("RFC 3339 certificate expiration");
    assert_eq!(
        parse_certificate_unix_timestamp("Jul  1 20:51:37 2036 GMT"),
        Some(expected)
    );
    assert_eq!(
        parse_certificate_unix_timestamp("Jul 1 20:51:37 2036 UTC"),
        Some(expected)
    );
    assert_eq!(
        parse_certificate_unix_timestamp("Feb 29 00:00:00 2035 GMT"),
        None
    );
    assert_eq!(parse_certificate_unix_timestamp("not-a-date"), None);
}

#[test]
fn classifies_persisted_certificate_at_renewal_boundary() {
    let certificate = json!({
        "certInfo": test_cert_info(&["example.test"], "serial")
    });
    let valid_to = parse_acme_certificate_expiration(&certificate)
        .expect("persisted OpenSSL expiration should parse");
    let threshold = 30 * 24 * 60 * 60;

    assert!(!certificate_due_for_renewal(
        valid_to,
        valid_to - threshold - 1,
        threshold
    ));
    assert!(certificate_due_for_renewal(
        valid_to,
        valid_to - threshold,
        threshold
    ));
    assert!(certificate_due_for_renewal(
        valid_to,
        valid_to + 1,
        threshold
    ));
}

#[tokio::test]
async fn acme_renew_ticker_fires_immediately_then_waits_for_interval() {
    let mut ticker = acme_renew_ticker(std::time::Duration::from_secs(60));
    tokio_time::timeout(std::time::Duration::from_secs(1), ticker.tick())
        .await
        .expect("the startup renewal scan should not wait for the first interval");
    assert!(
        tokio_time::timeout(std::time::Duration::from_millis(20), ticker.tick())
            .await
            .is_err(),
        "the next renewal scan should wait for the configured interval"
    );
}

#[tokio::test]
async fn auto_renew_releases_owned_scan_lease_on_early_return() {
    let (_directory, state) = acme_test_state().await;

    run_acme_auto_renew_once(state.clone())
        .await
        .expect("uninstalled ACME should be a successful no-op");

    assert_eq!(
        state
            .storage
            .store
            .get_json_value(ACME_RENEW_LOCK_KEY)
            .await
            .expect("read renewal lease"),
        None
    );
}

#[tokio::test]
async fn auto_renew_does_not_release_another_scan_owner() {
    let (_directory, state) = acme_test_state().await;
    state
        .storage
        .store
        .set_json_value_ex(ACME_RENEW_LOCK_KEY, &json!({ "lockId": "other-scan" }), 60)
        .await
        .expect("seed foreign renewal lease");

    run_acme_auto_renew_once(state.clone())
        .await
        .expect("a foreign renewal scan should be a successful no-op");

    assert_eq!(
        state
            .storage
            .store
            .get_json_value(ACME_RENEW_LOCK_KEY)
            .await
            .expect("read foreign renewal lease")
            .and_then(|lease| lease.get("lockId").cloned()),
        Some(json!("other-scan"))
    );
}

#[tokio::test]
async fn terminal_and_orphaned_stopped_runtime_locks_are_cleaned() {
    let (_directory, state) = acme_test_state().await;
    let t = Translator::new("zh-CN");
    let application = test_application("app-1", &["example.test"]);
    let mut job = build_queued_acme_job(&application, "auto_renew", &t).expect("queued job");
    job["status"] = json!("succeeded");
    create_acme_job(&state, &job, &t)
        .await
        .expect("seed completed job");
    let completed_lock =
        with_runtime_lock_lease(build_acme_runtime_lock(&application, &job, "auto_renew"));
    state
        .storage
        .store
        .set_json_value(ACME_RUNTIME_LOCK_KEY, &completed_lock)
        .await
        .expect("seed completed runtime lock");

    assert_eq!(
        get_active_acme_runtime_lock(&state).await.unwrap()["locked"],
        json!(false)
    );
    assert_eq!(
        state
            .storage
            .store
            .get_json_value(ACME_RUNTIME_LOCK_KEY)
            .await
            .unwrap(),
        None
    );

    job["status"] = json!("stopped");
    create_acme_job(&state, &job, &t)
        .await
        .expect("seed stopped job");
    let stopped_lock =
        with_runtime_lock_lease(build_acme_runtime_lock(&application, &job, "auto_renew"));
    state
        .storage
        .store
        .set_json_value(ACME_RUNTIME_LOCK_KEY, &stopped_lock)
        .await
        .expect("seed stopped runtime lock");

    assert_eq!(
        get_active_acme_runtime_lock(&state).await.unwrap()["locked"],
        json!(false)
    );
    assert!(
        state
            .storage
            .store
            .get_json_value(ACME_RUNTIME_LOCK_KEY)
            .await
            .unwrap()
            .is_none()
    );

    let controlled_lock =
        with_runtime_lock_lease(build_acme_runtime_lock(&application, &job, "auto_renew"));
    state
        .storage
        .store
        .set_json_value(ACME_RUNTIME_LOCK_KEY, &controlled_lock)
        .await
        .expect("seed controlled stopped lock");
    let job_id = job["id"].as_str().unwrap().to_string();
    state
        .register_acme_job_control(&job_id)
        .await
        .expect("register stopped job owner");
    assert_eq!(
        get_active_acme_runtime_lock(&state).await.unwrap()["locked"],
        json!(true),
        "a stopped job remains locked while its executor is still registered"
    );
    state.finish_acme_job_control(&job_id).await;
    assert_eq!(
        get_active_acme_runtime_lock(&state).await.unwrap()["locked"],
        json!(false)
    );

    job["status"] = json!("running");
    create_acme_job(&state, &job, &t)
        .await
        .expect("seed abandoned running job");
    let abandoned_lock =
        with_runtime_lock_lease(build_acme_runtime_lock(&application, &job, "auto_renew"));
    state
        .storage
        .store
        .set_json_value(ACME_RUNTIME_LOCK_KEY, &abandoned_lock)
        .await
        .expect("seed abandoned running lock");
    let abandoned_control = state
        .register_acme_job_control(&job_id)
        .await
        .expect("register abandoned job owner");
    abandoned_control.finished.cancel();
    assert_eq!(
        get_active_acme_runtime_lock(&state).await.unwrap()["locked"],
        json!(false),
        "a completed executor cannot leave its runtime lease behind"
    );
    assert_eq!(
        get_acme_job(&state, &job_id).await.unwrap().unwrap()["status"],
        json!("stopped")
    );
    assert!(state.acme_job_control(&job_id).await.is_none());
}

#[test]
fn running_job_message_matches_automatic_renew_trigger() {
    let t = Translator::new("zh-CN");
    assert_eq!(
        acme_job_running_message(&t, Some("auto_renew")),
        "正在自动续期证书"
    );
    assert_eq!(
        acme_job_running_message(&t, Some("manual_request")),
        "正在申请证书"
    );
}

#[tokio::test]
async fn runtime_lock_heartbeat_stops_immediately_when_cancelled() {
    let (_directory, state) = acme_test_state().await;
    let application = test_application("app-1", &["example.test"]);
    let t = Translator::new("zh-CN");
    let job = build_queued_acme_job(&application, "auto_renew", &t).expect("queued job");
    let lock = with_runtime_lock_lease(build_acme_runtime_lock(&application, &job, "auto_renew"));
    let stop = CancellationToken::new();
    let heartbeat = start_acme_lock_heartbeat(state, lock, stop.clone());

    stop.cancel();

    tokio_time::timeout(std::time::Duration::from_secs(1), heartbeat)
        .await
        .expect("cancelled heartbeat should not wait for its sleep interval")
        .expect("heartbeat task should exit cleanly");
}

#[test]
fn detects_issued_certificate_compatibility_by_domain_set() {
    let application = json!({
        "primaryDomain": "example.com",
        "domains": ["example.com", "www.example.com"],
    });
    let certificate = json!({
        "primaryDomain": "example.com",
        "certInfo": { "dnsNames": ["www.example.com", "example.com"] },
    });
    assert!(issued_certificate_compatible(&application, &certificate));
}

#[test]
fn builds_stable_legacy_application_id() {
    assert_eq!(
        build_application_id(Some("Example.com")),
        build_application_id(Some("example.com"))
    );
    assert!(build_application_id(Some("example.com")).starts_with("acme_app_"));
}

#[test]
fn normalizes_log_limit_bounds() {
    assert_eq!(normalize_log_limit(None), DEFAULT_ACME_LOG_LIMIT);
    assert_eq!(normalize_log_limit(Some("")), 1);
    assert_eq!(normalize_log_limit(Some("   ")), 1);
    assert_eq!(normalize_log_limit(Some("0")), 1);
    assert_eq!(normalize_log_limit(Some("-5")), 1);
    assert_eq!(normalize_log_limit(Some("2000")), MAX_ACME_LOG_LIMIT);
    assert_eq!(normalize_log_limit(Some("10")), 10);
    assert_eq!(normalize_log_limit(Some("3.9")), 3);
    assert_eq!(normalize_log_limit(Some("10x")), DEFAULT_ACME_LOG_LIMIT);
}

#[test]
fn localizes_queued_job_domain_validation() {
    let t = Translator::new("zh-CN");
    let error = build_queued_acme_job(&json!({ "domains": [] }), "manual_request", &t)
        .expect_err("empty domains should be rejected");
    assert_eq!(error.to_string(), "域名列表不能为空或格式无效");

    let job = build_queued_acme_job(
        &json!({
            "id": "app-1",
            "domains": ["Example.com"],
            "dnsType": "dns_cf"
        }),
        "auto_renew",
        &t,
    )
    .expect("valid job");
    assert_eq!(job["status"], json!("queued"));
    assert_eq!(job["message"], json!("queued for renew"));
}

#[test]
fn builds_pending_application_for_submit_now_update_like_node() {
    let existing = json!({
        "id": "app-1",
        "name": "Old name",
        "domains": ["old.example.com"],
        "primaryDomain": "old.example.com",
        "dnsType": "dns_cf",
        "credentials": { "CF_Token": "old" },
        "renewEnabled": true,
        "latestJobId": "job-1"
    });
    let normalized = NormalizedAcmeRequest {
        domains: vec!["example.com".to_string(), "*.example.com".to_string()],
        dns_type: "dns_ali".to_string(),
        credentials: json!({ "Ali_Key": "key", "Ali_Secret": "secret" }),
    };
    let pending = build_pending_acme_application_for_update(
        &existing,
        &json!({
            "name": "  ",
            "renewEnabled": false
        }),
        &normalized,
    );

    assert_eq!(pending["id"], json!("app-1"));
    assert!(pending.get("name").is_none());
    assert_eq!(pending["domains"], json!(["example.com", "*.example.com"]));
    assert_eq!(pending["primaryDomain"], json!("example.com"));
    assert_eq!(pending["dnsType"], json!("dns_ali"));
    assert_eq!(
        pending["credentials"],
        json!({ "Ali_Key": "key", "Ali_Secret": "secret" })
    );
    assert_eq!(pending["renewEnabled"], json!(false));
    assert_eq!(pending["latestJobId"], json!("job-1"));
}

#[tokio::test]
async fn domain_changes_and_failed_or_stopped_jobs_preserve_active_certificate() {
    let (_directory, state) = acme_test_state().await;
    let translator = Translator::new("en");
    let application = test_application("app-1", &["example.test"]);
    write_acme_applications(&state, std::slice::from_ref(&application))
        .await
        .expect("seed application");
    let issued = json!({
        "applicationId": "app-1",
        "primaryDomain": "example.test",
        "cert": "OLD CERT",
        "key": "OLD KEY",
        "certInfo": test_cert_info(&["example.test"], "old"),
        "createdAt": "2026-07-01T00:00:00.000Z",
        "updatedAt": "2026-07-01T00:00:00.000Z",
        "libraryCertificateId": "cert-old",
        "libraryLinkedAt": "2026-07-01T00:00:00.000Z"
    });
    state
        .storage
        .store
        .set_json_value(
            ACME_ISSUED_CERTIFICATES_KEY,
            &Value::Array(vec![issued.clone()]),
        )
        .await
        .expect("seed issued certificate");
    let ssl = json!({
        "cert": "OLD CERT",
        "key": "OLD KEY",
        "active_cert_id": "cert-old",
        "deployment_mode": "single_active",
        "certificates": [
            test_managed_certificate(
                "cert-old",
                "acme",
                Some("app-1"),
                "example.test",
                "OLD CERT",
                "OLD KEY"
            )
        ]
    });
    save_test_ssl_config(&state, ssl.clone()).await;
    state
        .storage
        .store
        .set_json_value(
            &format!("{ACME_CERT_PREFIX}example.test"),
            &json!({ "cert": "OLD CERT", "key": "OLD KEY" }),
        )
        .await
        .expect("seed ACME certificate pair");
    let old_directory = state.settings.data_dir.join("ssl").join("example.test");
    tokio::fs::create_dir_all(&old_directory)
        .await
        .expect("create old certificate directory");
    tokio::fs::write(old_directory.join("fullchain.cer"), "OLD CERT")
        .await
        .expect("seed old certificate file");

    let added_domain = save_acme_application_with_effects(
        &state,
        &translator,
        SaveAcmeApplicationInput {
            id: Some("app-1".to_string()),
            name: Some("Application app-1".to_string()),
            name_provided: true,
            domains: vec!["example.test".to_string(), "alt.example.test".to_string()],
            dns_type: "dns_cf".to_string(),
            credentials: json!({ "CF_Token": "secret" }),
            renew_enabled: Some(true),
        },
    )
    .await
    .expect("save added domain");

    assert_eq!(
        state.storage.store.get_config().await.unwrap()["ssl"],
        ssl,
        "saving a domain change must not alter the deployed SSL configuration"
    );
    assert_eq!(
        read_issued_certificates(&state).await.unwrap(),
        vec![normalize_issued_certificate(issued.clone()).unwrap()]
    );
    assert!(
        get_usable_issued_certificate_for_application(&state, &added_domain.application)
            .await
            .unwrap()
            .is_none(),
        "the preserved old certificate must not be presented as covering the new domain set"
    );
    assert!(old_directory.exists());
    assert!(
        state
            .storage
            .store
            .get_json_value(&format!("{ACME_CERT_PREFIX}example.test"))
            .await
            .unwrap()
            .is_some()
    );

    let failed_job = json!({
        "id": "job-failed",
        "status": "failed",
        "trigger": "manual_request",
        "createdAt": "2026-07-02T00:00:00.000Z",
        "finishedAt": "2026-07-02T00:01:00.000Z",
        "message": "DNS validation failed"
    });
    update_acme_application_job_state(&state, &added_domain.application, &failed_job)
        .await
        .expect("record failed job");
    assert_eq!(state.storage.store.get_config().await.unwrap()["ssl"], ssl);
    assert_eq!(
        read_issued_certificates(&state).await.unwrap(),
        vec![normalize_issued_certificate(issued.clone()).unwrap()]
    );

    let stopped_job = json!({
        "id": "job-stopped",
        "status": "stopped",
        "trigger": "manual_request",
        "createdAt": "2026-07-02T00:02:00.000Z",
        "finishedAt": "2026-07-02T00:03:00.000Z",
        "message": "stopped"
    });
    update_acme_application_job_state(&state, &added_domain.application, &stopped_job)
        .await
        .expect("record stopped job");
    assert_eq!(state.storage.store.get_config().await.unwrap()["ssl"], ssl);
    assert_eq!(
        read_issued_certificates(&state).await.unwrap(),
        vec![normalize_issued_certificate(issued).unwrap()]
    );
    assert!(old_directory.exists());
}

#[tokio::test]
async fn successful_issue_replaces_in_place_and_preserves_deployment_role() {
    let Some((old_cert, old_key)) = generate_acme_test_cert_pair("old.example.test") else {
        if cfg!(windows) {
            return;
        }
        panic!("openssl is required for ACME replacement tests");
    };
    let Some((new_cert, new_key)) = generate_acme_test_cert_pair("new.example.test") else {
        if cfg!(windows) {
            return;
        }
        panic!("openssl is required for ACME replacement tests");
    };
    let (_directory, state) = acme_test_state().await;
    let translator = Translator::new("en");
    let old_application = test_application("app-1", &["example.test"]);
    write_acme_applications(&state, std::slice::from_ref(&old_application))
        .await
        .expect("seed old application");
    save_acme_issued_certificate(
        &state,
        "app-1",
        "example.test",
        &old_cert,
        &old_key,
        test_cert_info(&["example.test"], "old"),
    )
    .await
    .expect("seed old issued certificate");
    link_issued_certificate_to_library(&state, "app-1", "cert-old")
        .await
        .expect("link old issued certificate");
    let old_library_certificate = test_managed_certificate(
        "cert-old",
        "acme",
        Some("app-1"),
        "example.test",
        &old_cert,
        &old_key,
    );
    let original_ssl = json!({
        "cert": old_cert,
        "key": old_key,
        "active_cert_id": "cert-old",
        "deployment_mode": "single_active",
        "certificates": [old_library_certificate],
        "future_compatible_field": { "preserve": true }
    });
    save_test_ssl_config(&state, original_ssl.clone()).await;

    let saved = save_acme_application_with_effects(
        &state,
        &translator,
        SaveAcmeApplicationInput {
            id: Some("app-1".to_string()),
            name: Some("Preserved label".to_string()),
            name_provided: true,
            domains: vec!["example.test".to_string(), "alt.example.test".to_string()],
            dns_type: "dns_cf".to_string(),
            credentials: json!({ "CF_Token": "secret" }),
            renew_enabled: Some(true),
        },
    )
    .await
    .expect("save replacement domain set");
    save_acme_issued_certificate(
        &state,
        "app-1",
        "example.test",
        &new_cert,
        &new_key,
        test_cert_info(&["example.test", "alt.example.test"], "new"),
    )
    .await
    .expect("save replacement issued certificate");
    assert_eq!(
        read_issued_certificates(&state).await.unwrap()[0]["libraryCertificateId"],
        json!("cert-old"),
        "replacement issuance must retain the existing library identity"
    );
    link_issued_certificate_to_library(&state, "app-1", "stale-library-id")
        .await
        .expect("seed stale issued-library link");

    let prepared = prepare_acme_library_after_issue(&state, &saved.application, &translator)
        .await
        .expect("prepare active replacement");
    assert_eq!(prepared.kind, AcmeLibraryUpdateKind::Replaced);
    assert!(prepared.should_sync_gateway);
    assert_eq!(prepared.previous_ssl, Some(original_ssl.clone()));
    assert_eq!(
        prepared.next_config.pointer("/ssl/active_cert_id"),
        Some(&json!("cert-old"))
    );
    assert_eq!(
        prepared.next_config.pointer("/ssl/certificates/0/id"),
        Some(&json!("cert-old"))
    );
    assert_eq!(
        prepared.next_config.pointer("/ssl/certificates/0/cert"),
        Some(&json!(new_cert.trim()))
    );
    assert_eq!(
        prepared.next_config.pointer("/ssl/certificates/0/label"),
        Some(&json!("Certificate cert-old"))
    );
    assert_eq!(
        prepared
            .next_config
            .pointer("/ssl/certificates")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1),
        "replacement must update the existing entry rather than add a duplicate"
    );
    assert_eq!(
        read_issued_certificates(&state).await.unwrap()[0]["libraryCertificateId"],
        json!("cert-old"),
        "the source-linked library certificate must repair stale issued metadata"
    );

    let restored = restore_ssl_after_failed_acme_deployment(
        &state,
        prepared.next_config.get("ssl"),
        prepared.previous_ssl.as_ref(),
    )
    .await
    .expect("restore previous SSL snapshot");
    let AcmeSslRollbackOutcome::RestoredPrevious(restored) = restored else {
        panic!("unmodified SSL state must restore the previous snapshot");
    };
    assert_eq!(restored["ssl"], original_ssl);
    assert_eq!(
        state.storage.store.get_config().await.unwrap()["ssl"],
        original_ssl
    );

    let scripted_prepared =
        prepare_acme_library_after_issue(&state, &saved.application, &translator)
            .await
            .expect("prepare scripted gateway replacement");
    let mut scripted_deployment = FailThenSucceedSslDeployment::default();
    let scripted_error = sync_ssl_deployment_with_rollback_using(
        &state,
        scripted_prepared.previous_ssl.as_ref(),
        &scripted_prepared.next_config,
        &mut scripted_deployment,
    )
    .await
    .expect_err("the first scripted deployment must fail");
    assert!(
        scripted_error
            .to_string()
            .contains("restored and reapplied the previous SSL configuration")
    );
    assert_eq!(scripted_deployment.calls.len(), 2);
    assert_eq!(
        scripted_deployment.calls[0].pointer("/ssl/certificates/0/cert"),
        Some(&json!(new_cert.trim()))
    );
    assert_eq!(
        scripted_deployment.calls[1]
            .pointer("/ssl/certificates/0/cert")
            .and_then(Value::as_str)
            .map(str::trim),
        Some(old_cert.trim())
    );
    assert_eq!(
        state.storage.store.get_config().await.unwrap()["ssl"],
        original_ssl
    );

    let concurrent_success_prepared =
        prepare_acme_library_after_issue(&state, &saved.application, &translator)
            .await
            .expect("prepare replacement before successful concurrent write");
    let mut concurrent_success_ssl = concurrent_success_prepared.next_config["ssl"].clone();
    concurrent_success_ssl["concurrent_writer"] = json!("newer-successful-write");
    let mut concurrent_success_deployment = ConcurrentSslWriteThenSucceedDeployment {
        calls: Vec::new(),
        concurrent_ssl: concurrent_success_ssl.clone(),
    };
    sync_ssl_deployment_with_rollback_using(
        &state,
        concurrent_success_prepared.previous_ssl.as_ref(),
        &concurrent_success_prepared.next_config,
        &mut concurrent_success_deployment,
    )
    .await
    .expect("a concurrent SSL write must be applied after the ACME deployment succeeds");
    assert_eq!(concurrent_success_deployment.calls.len(), 2);
    assert_eq!(
        concurrent_success_deployment.calls[1]["ssl"],
        concurrent_success_ssl
    );
    assert_eq!(
        state.storage.store.get_config().await.unwrap()["ssl"],
        concurrent_success_ssl
    );
    save_test_ssl_config(&state, original_ssl.clone()).await;

    let unrelated_prepared =
        prepare_acme_library_after_issue(&state, &saved.application, &translator)
            .await
            .expect("prepare replacement before unrelated config write");
    let mut config_with_unrelated_write = state.storage.store.get_config().await.unwrap();
    config_with_unrelated_write["concurrent_unrelated_section"] = json!({ "preserve": true });
    state
        .storage
        .store
        .save_config(&config_with_unrelated_write)
        .await
        .expect("save unrelated config write");
    let unrelated_outcome = restore_ssl_after_failed_acme_deployment(
        &state,
        unrelated_prepared.next_config.get("ssl"),
        unrelated_prepared.previous_ssl.as_ref(),
    )
    .await
    .expect("restore SSL around unrelated write");
    let AcmeSslRollbackOutcome::RestoredPrevious(unrelated_config) = unrelated_outcome else {
        panic!("an unrelated top-level write must not conflict with SSL rollback");
    };
    assert_eq!(
        unrelated_config["concurrent_unrelated_section"],
        json!({ "preserve": true })
    );
    assert_eq!(unrelated_config["ssl"], original_ssl);

    let concurrent_prepared =
        prepare_acme_library_after_issue(&state, &saved.application, &translator)
            .await
            .expect("prepare replacement before concurrent SSL write");
    let mut concurrent_ssl = concurrent_prepared.next_config["ssl"].clone();
    concurrent_ssl["deployment_mode"] = json!("multi_sni");
    concurrent_ssl["concurrent_writer"] = json!("preserve-me");
    save_test_ssl_config(&state, concurrent_ssl.clone()).await;
    let concurrent_outcome = restore_ssl_after_failed_acme_deployment(
        &state,
        concurrent_prepared.next_config.get("ssl"),
        concurrent_prepared.previous_ssl.as_ref(),
    )
    .await
    .expect("detect concurrent SSL write");
    let AcmeSslRollbackOutcome::PreservedConcurrent(latest_config) = concurrent_outcome else {
        panic!("a concurrent SSL write must not be replaced by the ACME rollback");
    };
    assert_eq!(latest_config["ssl"], concurrent_ssl);
    assert_eq!(
        state.storage.store.get_config().await.unwrap()["ssl"],
        concurrent_ssl
    );

    let mut legacy_mirror_ssl = original_ssl.clone();
    legacy_mirror_ssl
        .as_object_mut()
        .expect("SSL object")
        .remove("active_cert_id");
    save_test_ssl_config(&state, legacy_mirror_ssl.clone()).await;
    let legacy_mirror_update =
        prepare_acme_library_after_issue(&state, &saved.application, &translator)
            .await
            .expect("prepare legacy mirrored active replacement");
    assert!(
        legacy_mirror_update.should_sync_gateway,
        "the legacy top-level PEM mirror must still identify the linked certificate as active"
    );
    assert_eq!(
        legacy_mirror_update
            .next_config
            .pointer("/ssl/active_cert_id"),
        Some(&json!("cert-old"))
    );

    let manual_active = test_managed_certificate(
        "manual-active",
        "manual",
        None,
        "manual.example.test",
        &old_cert,
        &old_key,
    );
    let linked_inactive = test_managed_certificate(
        "cert-old",
        "acme",
        Some("app-1"),
        "example.test",
        &old_cert,
        &old_key,
    );
    let inactive_single_ssl = json!({
        "cert": old_cert,
        "key": old_key,
        "active_cert_id": "manual-active",
        "deployment_mode": "single_active",
        "certificates": [manual_active, linked_inactive]
    });
    save_test_ssl_config(&state, inactive_single_ssl.clone()).await;
    let inactive_update = prepare_acme_library_after_issue(&state, &saved.application, &translator)
        .await
        .expect("prepare inactive replacement");
    assert_eq!(inactive_update.kind, AcmeLibraryUpdateKind::Replaced);
    assert!(!inactive_update.should_sync_gateway);
    assert_eq!(
        inactive_update.next_config.pointer("/ssl/active_cert_id"),
        Some(&json!("manual-active"))
    );

    restore_ssl_after_failed_acme_deployment(
        &state,
        inactive_update.next_config.get("ssl"),
        Some(&inactive_single_ssl),
    )
    .await
    .expect("restore inactive single-active snapshot");
    let mut multi_sni_ssl = inactive_single_ssl.clone();
    multi_sni_ssl["deployment_mode"] = json!("multi_sni");
    save_test_ssl_config(&state, multi_sni_ssl).await;
    let multi_sni_update =
        prepare_acme_library_after_issue(&state, &saved.application, &translator)
            .await
            .expect("prepare multi-SNI replacement");
    assert_eq!(multi_sni_update.kind, AcmeLibraryUpdateKind::Replaced);
    assert!(
        multi_sni_update.should_sync_gateway,
        "an inactive certificate still participates in a multi-SNI deployment"
    );

    restore_ssl_after_failed_acme_deployment(
        &state,
        multi_sni_update.next_config.get("ssl"),
        Some(&inactive_single_ssl),
    )
    .await
    .expect("restore single-active config before adding an unrelated certificate");
    let new_application = test_application("app-2", &["new-only.example.test"]);
    write_acme_applications(
        &state,
        &[saved.application.clone(), new_application.clone()],
    )
    .await
    .expect("add unrelated application");
    save_acme_issued_certificate(
        &state,
        "app-2",
        "new-only.example.test",
        &new_cert,
        &new_key,
        test_cert_info(&["new-only.example.test"], "new-only"),
    )
    .await
    .expect("save unrelated issued certificate");
    let added_update = prepare_acme_library_after_issue(&state, &new_application, &translator)
        .await
        .expect("prepare unrelated certificate addition");
    assert_eq!(added_update.kind, AcmeLibraryUpdateKind::Added);
    assert!(!added_update.should_sync_gateway);
    assert_eq!(
        added_update.next_config.pointer("/ssl/active_cert_id"),
        Some(&json!("manual-active")),
        "adding a separate certificate must not steal the active role"
    );

    let mut stopped_job = build_queued_acme_job(&saved.application, "manual_request", &translator)
        .expect("build stopped boundary job");
    stopped_job["id"] = json!("job-stopped-boundary");
    stopped_job["status"] = json!("stopped");
    stopped_job["progress"] = json!(100);
    create_acme_job(&state, &stopped_job, &translator)
        .await
        .expect("seed stopped boundary job");
    save_test_ssl_config(&state, original_ssl.clone()).await;
    let stopped_error = sync_acme_library_after_issue(
        &state,
        &saved.application,
        "job-stopped-boundary",
        &translator,
    )
    .await
    .expect_err("a stopped job must not cross the SSL deployment boundary");
    assert_eq!(
        stopped_error.to_string(),
        translator.t("server.acmeJobRunner.manualStop")
    );
    assert_eq!(
        state.storage.store.get_config().await.unwrap()["ssl"],
        original_ssl
    );

    save_test_ssl_config(&state, original_ssl.clone()).await;
    let deployment_error =
        sync_acme_library_after_issue(&state, &saved.application, "job-rollback", &translator)
            .await
            .expect_err("the unavailable test gateway must reject deployment");
    assert!(
        deployment_error
            .to_string()
            .contains("failed to restore or reapply a safe SSL configuration"),
        "the error should report that gateway recovery could not be completed"
    );
    assert_eq!(
        state.storage.store.get_config().await.unwrap()["ssl"],
        original_ssl,
        "a gateway deployment failure must restore the old active certificate and compatibility fields"
    );

    let reconcile_error = reconcile_acme_ssl_deployment(&state)
        .await
        .expect_err("the unavailable gateway must reject reconciliation");
    assert!(!reconcile_error.to_string().is_empty());
    assert_eq!(
        state.storage.store.get_config().await.unwrap()["ssl"],
        original_ssl,
        "automatic reconciliation must not undo the failed deployment rollback"
    );
}

#[test]
fn replacement_lookup_falls_back_to_issued_library_id() {
    let certificate =
        test_managed_certificate("cert-old", "acme", None, "old.example.test", "CERT", "KEY");
    let config = json!({
        "ssl": {
            "certificates": [certificate]
        }
    });
    let issued = json!({ "libraryCertificateId": "cert-old" });
    assert_eq!(
        replacement_library_certificate(&config, "app-1", &issued)
            .and_then(|certificate| certificate.get("id").cloned()),
        Some(json!("cert-old"))
    );
}

#[tokio::test]
async fn issued_snapshot_rollback_restores_previous_artifacts_and_removes_new_artifacts() {
    let (_directory, state) = acme_test_state().await;
    let previous = save_acme_issued_certificate(
        &state,
        "app-1",
        "old.example.test",
        "OLD CERT",
        "OLD KEY",
        test_cert_info(&["old.example.test"], "old"),
    )
    .await
    .expect("save previous issued certificate");
    let new_directory = state.settings.data_dir.join("ssl").join("new.example.test");
    tokio::fs::create_dir_all(&new_directory)
        .await
        .expect("create new issued directory");
    tokio::fs::write(new_directory.join("fullchain.cer"), "NEW CERT")
        .await
        .expect("write new issued file");
    save_acme_issued_certificate(
        &state,
        "app-1",
        "new.example.test",
        "NEW CERT",
        "NEW KEY",
        test_cert_info(&["new.example.test"], "new"),
    )
    .await
    .expect("save replacement issued certificate");

    restore_acme_issued_certificate_snapshot(&state, "app-1", "new.example.test", Some(&previous))
        .await
        .expect("restore previous issued snapshot");

    assert_eq!(
        read_issued_certificates(&state).await.unwrap(),
        vec![previous.clone()]
    );
    assert!(
        state
            .storage
            .store
            .get_json_value(&format!("{ACME_CERT_PREFIX}new.example.test"))
            .await
            .unwrap()
            .is_none()
    );
    assert!(!new_directory.exists());
    assert_eq!(
        state
            .storage
            .store
            .get_json_value(&format!("{ACME_CERT_PREFIX}old.example.test"))
            .await
            .unwrap(),
        Some(json!({ "cert": "OLD CERT", "key": "OLD KEY" }))
    );
    let old_directory = state.settings.data_dir.join("ssl").join("old.example.test");
    assert_eq!(
        tokio::fs::read_to_string(old_directory.join("fullchain.cer"))
            .await
            .unwrap(),
        "OLD CERT"
    );
    assert_eq!(
        tokio::fs::read_to_string(old_directory.join("old.example.test.key"))
            .await
            .unwrap(),
        "OLD KEY"
    );

    let reused_previous = save_acme_issued_certificate(
        &state,
        "app-reuse",
        "reused.example.test",
        "STALE CERT",
        "STALE KEY",
        test_cert_info(&["reused.example.test"], "stale"),
    )
    .await
    .expect("save issued snapshot for a subsequently reused domain");
    save_acme_issued_certificate(
        &state,
        "app-reuse",
        "replacement.example.test",
        "REPLACEMENT CERT",
        "REPLACEMENT KEY",
        test_cert_info(&["replacement.example.test"], "replacement"),
    )
    .await
    .expect("save replacement for reused-domain application");
    write_acme_applications(
        &state,
        &[
            test_application("app-reuse", &["replacement.example.test"]),
            test_application("app-owner", &["reused.example.test"]),
        ],
    )
    .await
    .expect("claim the previous domain from another application");
    state
        .storage
        .store
        .set_json_value(
            &format!("{ACME_CERT_PREFIX}reused.example.test"),
            &json!({ "cert": "OWNER CERT", "key": "OWNER KEY" }),
        )
        .await
        .expect("seed reused-domain owner pair");
    let reused_directory = state
        .settings
        .data_dir
        .join("ssl")
        .join("reused.example.test");
    tokio::fs::create_dir_all(&reused_directory)
        .await
        .expect("create reused-domain owner directory");
    tokio::fs::write(reused_directory.join("fullchain.cer"), "OWNER CERT")
        .await
        .expect("write reused-domain owner certificate");
    tokio::fs::write(
        reused_directory.join("reused.example.test.key"),
        "OWNER KEY",
    )
    .await
    .expect("write reused-domain owner key");
    restore_acme_issued_certificate_snapshot(
        &state,
        "app-reuse",
        "replacement.example.test",
        Some(&reused_previous),
    )
    .await
    .expect("restore issued metadata without overwriting a reused domain");
    assert_eq!(
        state
            .storage
            .store
            .get_json_value(&format!("{ACME_CERT_PREFIX}reused.example.test"))
            .await
            .unwrap(),
        Some(json!({ "cert": "OWNER CERT", "key": "OWNER KEY" }))
    );
    assert_eq!(
        tokio::fs::read_to_string(reused_directory.join("fullchain.cer"))
            .await
            .unwrap(),
        "OWNER CERT"
    );

    save_acme_issued_certificate(
        &state,
        "app-2",
        "only-new.example.test",
        "ONLY NEW CERT",
        "ONLY NEW KEY",
        test_cert_info(&["only-new.example.test"], "only-new"),
    )
    .await
    .expect("save first issued certificate for second application");
    let only_new_directory = state
        .settings
        .data_dir
        .join("ssl")
        .join("only-new.example.test");
    tokio::fs::create_dir_all(&only_new_directory)
        .await
        .expect("create only-new directory");
    restore_acme_issued_certificate_snapshot(&state, "app-2", "only-new.example.test", None)
        .await
        .expect("remove newly issued snapshot without predecessor");
    assert!(
        read_issued_certificates(&state)
            .await
            .unwrap()
            .iter()
            .all(
                |certificate| certificate.get("applicationId").and_then(Value::as_str)
                    != Some("app-2")
            )
    );
    assert!(!only_new_directory.exists());

    save_acme_issued_certificate(
        &state,
        "app-current-reuse",
        "claimed-current.example.test",
        "TRANSIENT CERT",
        "TRANSIENT KEY",
        test_cert_info(&["claimed-current.example.test"], "transient"),
    )
    .await
    .expect("save transient issued certificate");
    write_acme_applications(
        &state,
        &[
            test_application("app-current-reuse", &["moved-away.example.test"]),
            test_application("app-current-owner", &["claimed-current.example.test"]),
        ],
    )
    .await
    .expect("claim the transient current domain from another application");
    let claimed_current_key = format!("{ACME_CERT_PREFIX}claimed-current.example.test");
    state
        .storage
        .store
        .set_json_value(
            &claimed_current_key,
            &json!({ "cert": "CURRENT OWNER CERT", "key": "CURRENT OWNER KEY" }),
        )
        .await
        .expect("seed current-domain owner pair");
    let claimed_current_directory = state
        .settings
        .data_dir
        .join("ssl")
        .join("claimed-current.example.test");
    tokio::fs::create_dir_all(&claimed_current_directory)
        .await
        .expect("create current-domain owner directory");
    tokio::fs::write(
        claimed_current_directory.join("fullchain.cer"),
        "CURRENT OWNER CERT",
    )
    .await
    .expect("write current-domain owner certificate");
    restore_acme_issued_certificate_snapshot(
        &state,
        "app-current-reuse",
        "claimed-current.example.test",
        None,
    )
    .await
    .expect("remove issued metadata without deleting a reused current domain");
    assert_eq!(
        state
            .storage
            .store
            .get_json_value(&claimed_current_key)
            .await
            .unwrap(),
        Some(json!({
            "cert": "CURRENT OWNER CERT",
            "key": "CURRENT OWNER KEY"
        }))
    );
    assert_eq!(
        tokio::fs::read_to_string(claimed_current_directory.join("fullchain.cer"))
            .await
            .unwrap(),
        "CURRENT OWNER CERT"
    );
}

#[tokio::test]
async fn superseded_domain_cleanup_avoids_equal_or_reused_domains() {
    let (_directory, state) = acme_test_state().await;
    let current = test_application("app-1", &["new.example.test"]);
    write_acme_applications(&state, std::slice::from_ref(&current))
        .await
        .expect("seed current application");

    let old_key = format!("{ACME_CERT_PREFIX}old.example.test");
    let old_directory = state.settings.data_dir.join("ssl").join("old.example.test");
    state
        .storage
        .store
        .set_json_value(&old_key, &json!({ "cert": "OLD", "key": "KEY" }))
        .await
        .expect("seed old pair");
    tokio::fs::create_dir_all(&old_directory)
        .await
        .expect("create old directory");

    assert!(
        cleanup_superseded_acme_domain_artifacts(
            &state,
            "app-1",
            "old.example.test",
            "new.example.test"
        )
        .await
        .expect("clean unclaimed old domain")
    );
    assert!(
        state
            .storage
            .store
            .get_json_value(&old_key)
            .await
            .unwrap()
            .is_none()
    );
    assert!(!old_directory.exists());

    state
        .storage
        .store
        .set_json_value(&old_key, &json!({ "cert": "REUSED", "key": "KEY" }))
        .await
        .expect("reseed reused pair");
    tokio::fs::create_dir_all(&old_directory)
        .await
        .expect("recreate reused directory");
    let reused = test_application("app-2", &["old.example.test"]);
    write_acme_applications(&state, &[current.clone(), reused])
        .await
        .expect("seed application reusing old domain");

    assert!(
        !cleanup_superseded_acme_domain_artifacts(
            &state,
            "app-1",
            "old.example.test",
            "new.example.test"
        )
        .await
        .expect("skip reused old domain")
    );
    assert!(
        state
            .storage
            .store
            .get_json_value(&old_key)
            .await
            .unwrap()
            .is_some()
    );
    assert!(old_directory.exists());

    assert!(
        !cleanup_superseded_acme_domain_artifacts(
            &state,
            "app-1",
            "OLD.EXAMPLE.TEST.",
            "old.example.test"
        )
        .await
        .expect("skip normalized equal domains")
    );
    assert!(
        state
            .storage
            .store
            .get_json_value(&old_key)
            .await
            .unwrap()
            .is_some()
    );
    assert!(old_directory.exists());
}

#[test]
fn provider_catalog_contains_node_dns_types() {
    let t = Translator::new("en");
    let providers = acme_dns_providers(&t);
    let dns_types = providers
        .iter()
        .filter_map(|item| item.get("dnsType").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    assert!(dns_types.contains("dns_cf"));
    assert!(dns_types.contains("dns_azure"));
    assert!(dns_types.contains("dns_opnsense"));
    assert_eq!(
        providers
            .iter()
            .find(|item| item.get("dnsType").and_then(Value::as_str) == Some("dns_cf"))
            .and_then(|item| item.get("credentialSchemes"))
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2)
    );
}

#[test]
fn validates_acme_request_with_alias_and_filters_credentials() {
    let t = Translator::new("en");
    let normalized = validate_acme_request(
        &json!({
            "domains": ["Example.com", "bad host", "*.example.com", "example.com"],
            "dnsType": "aliyun",
            "credentials": {
                "Ali_Key": " key ",
                "Ali_Secret": " secret ",
                "Ignored": "value"
            }
        }),
        &t,
    )
    .expect("valid request");

    assert_eq!(normalized.domains, vec!["example.com", "*.example.com"]);
    assert_eq!(normalized.dns_type, "dns_ali");
    assert_eq!(
        normalized.credentials,
        json!({ "Ali_Key": "key", "Ali_Secret": "secret" })
    );
}

#[test]
fn validates_netlify_credential_alias() {
    let t = Translator::new("en");
    let normalized = validate_acme_request(
        &json!({
            "domains": ["example.com"],
            "provider": "netlify",
            "credentials": {
                "NETLIFY_TOKEN": "token"
            }
        }),
        &t,
    )
    .expect("valid request");

    assert_eq!(normalized.dns_type, "dns_netlify");
    assert_eq!(
        normalized.credentials,
        json!({ "NETLIFY_ACCESS_TOKEN": "token" })
    );
}

#[test]
fn rejects_missing_acme_credentials() {
    let t = Translator::new("en");
    let error = validate_acme_request(
        &json!({
            "domains": ["example.com"],
            "dnsType": "dns_ali",
            "credentials": {
                "Ali_Key": "key"
            }
        }),
        &t,
    )
    .expect_err("credentials should be incomplete");

    assert!(error.contains("DNS API credentials are missing"));
    assert!(error.contains("Ali_Secret"));
}

#[test]
fn localizes_acme_route_errors() {
    let t = Translator::new("zh-CN");
    assert_eq!(acme_route_text(&t, "invalidRequestBody"), "请求体不正确");
    assert_eq!(acme_route_text(&t, "loadJobFailed"), "读取 ACME 任务失败");
    assert_eq!(
        acme_route_text(&t, "createCertificateZipFailed"),
        "创建 ACME 证书压缩包失败"
    );
    assert_eq!(
        acme_route_text(&t, "updateApplicationFailed"),
        "更新 ACME 申请项失败"
    );
    assert_eq!(
        acme_route_text(&t, "saveClientSettingsFailed"),
        "保存 ACME 客户端设置失败"
    );
    assert_eq!(
        acme_route_text(&t, "syncLibraryFailed"),
        "同步 ACME 证书到证书库失败"
    );
    assert_eq!(
        acme_route_text(&t, "deployCertificateFailed"),
        "部署 ACME 证书失败"
    );
    assert_eq!(acme_route_text(&t, "stopJobFailed"), "停止 ACME 任务失败");
}

#[test]
fn detects_submit_now_requests_for_fallback() {
    assert!(submit_now_requested(&json!({ "submitNow": true })));
    assert!(!submit_now_requested(&json!({ "submitNow": false })));
    assert!(!submit_now_requested(&json!({})));
}

#[test]
fn validates_acme_domain_like_node() {
    assert!(is_valid_acme_domain("example.com"));
    assert!(is_valid_acme_domain("*.example.com"));
    assert!(!is_valid_acme_domain("example"));
    assert!(!is_valid_acme_domain("deep.*.example.com"));
    assert!(!is_valid_acme_domain("bad host.example.com"));
}

#[test]
fn wildcard_domains_cover_single_label_subdomains_only() {
    let domains = vec!["example.com".to_string(), "*.example.com".to_string()];
    assert!(is_requirement_covered_by_certificate_domains(
        "app.example.com",
        &domains
    ));
    assert!(is_requirement_covered_by_certificate_domains(
        "example.com",
        &domains
    ));
    assert!(!is_requirement_covered_by_certificate_domains(
        "deep.app.example.com",
        &domains
    ));
}

#[test]
fn windows_wildcard_certificate_uses_safe_paths_and_apex_issue_storage() {
    let application = json!({
        "primaryDomain": "*.fs.wxlnk.com",
        "domains": ["*.fs.wxlnk.com", "fs.wxlnk.com"],
    });
    assert_eq!(
        acme_data_dir_name_for_target("*.fs.wxlnk.com", true),
        "wildcard_fs.wxlnk.com"
    );
    assert_eq!(
        acme_issued_storage_domain_for_target(&application, true),
        "fs.wxlnk.com"
    );
    assert_eq!(
        acme_issued_storage_domain_for_target(&application, false),
        "*.fs.wxlnk.com"
    );
}

#[test]
fn acme_zip_entry_names_preserve_requested_domain_like_node() {
    let bytes = zip_acme_cert_pair("Example.COM", "CERT", "KEY").expect("zip should build");
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).expect("zip should parse");

    assert!(archive.by_name("Example.COM.cert.pem").is_ok());
    assert!(archive.by_name("Example.COM.key.pem").is_ok());
}

#[test]
fn acme_zip_uses_portable_names_and_non_empty_entries_for_wildcards() {
    use std::io::Read as _;

    assert_eq!(
        acme_certificate_archive_stem("*.Example.COM."),
        "wildcard.Example.COM"
    );
    let bytes = zip_acme_cert_pair("*.example.com", "CERT", "KEY").expect("zip should build");
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).expect("zip should parse");
    assert_eq!(archive.len(), 2);

    let mut cert = String::new();
    archive
        .by_name("wildcard.example.com.cert.pem")
        .expect("certificate entry")
        .read_to_string(&mut cert)
        .expect("certificate contents");
    assert_eq!(cert, "CERT");

    let mut key = String::new();
    archive
        .by_name("wildcard.example.com.key.pem")
        .expect("private key entry")
        .read_to_string(&mut key)
        .expect("private key contents");
    assert_eq!(key, "KEY");
}

#[test]
fn acme_init_payload_matches_node_shape() {
    let payload = build_init_acme_payload(
        PathBuf::from("/data/.acme.sh/acme.sh"),
        &json!({
            "certificateAuthority": "letsencrypt",
            "updatedAt": "2026-07-07T00:00:00Z",
        }),
    );
    let keys = payload
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<Vec<_>>();

    assert_eq!(keys, vec!["certificateAuthority", "executablePath"]);
    assert_eq!(payload["certificateAuthority"], json!("letsencrypt"));
    assert_eq!(payload["executablePath"], json!("/data/.acme.sh/acme.sh"));
    assert!(payload.get("state").is_none());
}

#[test]
fn analyzes_cloudflare_invalid_key_like_node() {
    let t = Translator::new("en");
    let logs = vec![
        json!("Cloudflare API request failed"),
        json!("{\"code\":6103,\"message\":\"Invalid format for X-Auth-Key header\"}"),
    ];
    let analysis = analyze_acme_logs(&json!({ "provider": "dns_cf" }), &logs, &t);

    assert_eq!(analysis["reason"], json!("dns_credentials_invalid"));
    assert_eq!(analysis["provider"], json!("dns_cf"));
    assert!(analysis["message"].as_str().unwrap().contains("Cloudflare"));
    assert_eq!(analysis["evidence"].as_array().unwrap().len(), 1);
}

#[test]
fn analyzes_retry_after_frequency_limit_like_node() {
    let t = Translator::new("en");
    let logs = vec![
        json!("server asks retryafter=601, too large, will not retry"),
        json!("final error"),
    ];
    let analysis = analyze_acme_logs(&json!({ "provider": "dns_ali" }), &logs, &t);

    assert_eq!(analysis["reason"], json!("acme_frequency_limited"));
    assert_eq!(analysis["provider"], json!("dns_ali"));
    assert!(analysis["message"].as_str().unwrap().contains("601"));
    assert_eq!(analysis["evidence"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn startup_recovery_stops_orphaned_job_and_releases_its_lock() {
    let (_directory, state) = acme_test_state().await;
    let t = Translator::new("en");
    let application = test_application("app-orphan", &["orphan.example.test"]);
    write_acme_applications(&state, std::slice::from_ref(&application))
        .await
        .expect("seed ACME application");

    let mut job = build_queued_acme_job(&application, "auto_renew", &t).expect("queued job");
    job["status"] = json!("running");
    job["startedAt"] = json!("2026-07-01T01:00:00.000Z");
    create_acme_job(&state, &job, &t)
        .await
        .expect("seed running job");
    let lock = with_runtime_lock_lease(build_acme_runtime_lock(&application, &job, "auto_renew"));
    state
        .storage
        .store
        .set_json_value(ACME_RUNTIME_LOCK_KEY, &lock)
        .await
        .expect("seed runtime lock");

    assert!(
        recover_orphaned_acme_runtime_job(&state, &t)
            .await
            .expect("recover orphaned job")
    );
    assert_eq!(
        get_acme_job(&state, job["id"].as_str().unwrap())
            .await
            .unwrap()
            .unwrap()["status"],
        json!("stopped")
    );
    assert_eq!(
        read_acme_applications(&state).await.unwrap()[0]["latestJobStatus"],
        json!("stopped")
    );
    assert_eq!(
        get_active_acme_runtime_lock(&state).await.unwrap()["locked"],
        json!(false)
    );

    job["status"] = json!("failed");
    job["message"] = json!("provider credentials rejected");
    create_acme_job(&state, &job, &t)
        .await
        .expect("seed terminal job");
    let terminal_lock =
        with_runtime_lock_lease(build_acme_runtime_lock(&application, &job, "auto_renew"));
    state
        .storage
        .store
        .set_json_value(ACME_RUNTIME_LOCK_KEY, &terminal_lock)
        .await
        .expect("seed terminal runtime lock");
    recover_orphaned_acme_runtime_job(&state, &t)
        .await
        .expect("recover terminal lock");
    assert_eq!(
        get_acme_job(&state, job["id"].as_str().unwrap())
            .await
            .unwrap()
            .unwrap()["status"],
        json!("failed"),
        "startup recovery must not downgrade a terminal job"
    );
}

#[tokio::test]
async fn startup_recovery_reconstructs_a_missing_running_job_record() {
    let (_directory, state) = acme_test_state().await;
    let t = Translator::new("en");
    let mut application = test_application("app-missing-job", &["missing.example.test"]);
    application["latestJobId"] = json!("missing-job");
    application["latestJobStatus"] = json!("running");
    application["latestJobTrigger"] = json!("auto_renew");
    application["latestJobAt"] = json!("2026-07-01T01:00:00.000Z");
    write_acme_applications(&state, std::slice::from_ref(&application))
        .await
        .expect("seed stale application state");

    assert!(
        recover_orphaned_acme_runtime_job(&state, &t)
            .await
            .expect("recover missing job")
    );
    let job = get_acme_job(&state, "missing-job")
        .await
        .unwrap()
        .expect("reconstructed job");
    assert_eq!(job["status"], json!("stopped"));
    assert_eq!(
        read_acme_applications(&state).await.unwrap()[0]["latestJobStatus"],
        json!("stopped")
    );
}

#[tokio::test]
async fn manual_stop_waits_for_the_owned_executor_to_release_its_lock() {
    let (_directory, state) = acme_test_state().await;
    let t = Translator::new("en");
    let application = test_application("app-running", &["running.example.test"]);
    write_acme_applications(&state, std::slice::from_ref(&application))
        .await
        .expect("seed ACME application");
    let mut job = build_queued_acme_job(&application, "manual_request", &t).expect("queued job");
    job["status"] = json!("running");
    create_acme_job(&state, &job, &t)
        .await
        .expect("seed running job");
    let lock = with_runtime_lock_lease(build_acme_runtime_lock(
        &application,
        &job,
        "manual_request",
    ));
    state
        .storage
        .store
        .set_json_value(ACME_RUNTIME_LOCK_KEY, &lock)
        .await
        .expect("seed runtime lock");

    let job_id = job["id"].as_str().unwrap().to_string();
    let control = state
        .register_acme_job_control(&job_id)
        .await
        .expect("register executor control");
    let executor_state = state.clone();
    let executor_lock = lock.clone();
    let executor_job_id = job_id.clone();
    let executor = async move {
        control.cancellation.cancelled().await;
        release_acme_runtime_lock(&executor_state, &executor_lock)
            .await
            .expect("release runtime lock");
        executor_state
            .finish_acme_job_control(&executor_job_id)
            .await;
    };

    let (result, ()) = tokio::join!(stop_active_acme_job(&state, &t), executor);
    let result = result.expect("stop active job");
    assert_eq!(result["stopped"], json!(true));
    assert_eq!(result["job"]["status"], json!("stopped"));
    assert_eq!(result["lock"]["locked"], json!(false));
    assert_eq!(result["processResult"]["remainingPids"], json!([]));
}

#[tokio::test]
async fn cancellation_during_job_reservation_is_recorded_as_stopped() {
    let (_directory, state) = acme_test_state().await;
    let t = Translator::new("en");
    let application = test_application("app-reserved", &["reserved.example.test"]);
    write_acme_applications(&state, std::slice::from_ref(&application))
        .await
        .expect("seed ACME application");
    let job = build_queued_acme_job(&application, "manual_request", &t).expect("queued job");
    create_acme_job(&state, &job, &t)
        .await
        .expect("seed queued job");
    let lock = with_runtime_lock_lease(build_acme_runtime_lock(
        &application,
        &job,
        "manual_request",
    ));
    state
        .storage
        .store
        .set_json_value(ACME_RUNTIME_LOCK_KEY, &lock)
        .await
        .expect("seed runtime lock");
    let job_id = job["id"].as_str().unwrap();
    let control = state
        .register_acme_job_control(job_id)
        .await
        .expect("register job control");
    control.cancellation.cancel();

    fail_reserved_acme_application_job(
        &state,
        &application,
        &job,
        &lock,
        "runtime is shutting down",
        &t,
    )
    .await
    .expect("finalize cancelled reservation");

    let stopped = get_acme_job(&state, job_id)
        .await
        .unwrap()
        .expect("stopped job");
    assert_eq!(stopped["status"], json!("stopped"));
    assert_eq!(
        stopped["message"],
        json!(t.t("server.acmeJobRunner.manualStop"))
    );
    assert_eq!(
        get_active_acme_runtime_lock(&state).await.unwrap()["locked"],
        json!(false)
    );
    assert!(state.acme_job_control(job_id).await.is_none());
}

#[test]
fn auto_renew_failure_backoff_requires_time_or_a_configuration_change() {
    let latest_job_at = parse_certificate_unix_timestamp("2026-07-01T01:00:00Z").unwrap();
    let application = json!({
        "latestJobStatus": "failed",
        "latestJobAt": "2026-07-01T01:00:00Z",
        "updatedAt": "2026-07-01T00:00:00Z",
    });

    assert!(!auto_renew_retry_allowed_with_backoff(
        &application,
        latest_job_at + 3_599,
        3_600,
    ));
    assert!(auto_renew_retry_allowed_with_backoff(
        &application,
        latest_job_at + 3_600,
        3_600,
    ));

    let mut updated = application;
    updated["updatedAt"] = json!("2026-07-01T01:00:01Z");
    assert!(auto_renew_retry_allowed_with_backoff(
        &updated,
        latest_job_at + 1,
        3_600,
    ));
}

#[cfg(unix)]
#[tokio::test]
async fn forced_acme_stop_terminates_the_owned_process_group() {
    let mut command = Command::new("sh");
    command
        .args(["-c", "trap '' TERM; sleep 30 & echo $!; wait"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    command.process_group(0);
    let mut child = command.spawn().expect("spawn test ACME process group");
    let pid = child.id().expect("child pid");
    let mut descendant_line = String::new();
    BufReader::new(child.stdout.take().expect("child stdout"))
        .read_line(&mut descendant_line)
        .await
        .expect("read descendant pid");
    let descendant_pid = descendant_line
        .trim()
        .parse::<i32>()
        .expect("descendant pid");

    terminate_acme_child(&mut child, std::time::Duration::from_millis(50))
        .await
        .expect("terminate process group");
    assert!(!crate::unix::process_exists(pid as i32));
    for _ in 0..50 {
        if !crate::unix::process_exists(descendant_pid) {
            break;
        }
        tokio_time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(!crate::unix::process_exists(descendant_pid));
}
