use super::super::tests::{acme_test_state, test_application};
use super::*;
use std::collections::BTreeMap;
use tower::ServiceExt;

const NAME: &str = "_acme-challenge.example.test";

fn fixture(values: &[&str], delegation: &[&str]) -> BTreeMap<String, dns::Observation> {
    BTreeMap::from([(
        NAME.into(),
        dns::Observation {
            zone: "example.test".into(),
            values: values.iter().map(|v| v.to_string()).collect(),
            delegation: delegation.iter().map(|v| v.to_string()).collect(),
        },
    )])
}

async fn install_fixture(state: &AppState) {
    let executable = acme_executable_path(state);
    std::fs::create_dir_all(executable.parent().unwrap()).unwrap();
    // Use the real pinned hook and client for fingerprint checks only.
    let mut zip = ZipArchive::new(Cursor::new(
        include_bytes!("../../../../resources/acmesh.zip").as_slice(),
    ))
    .unwrap();
    for (entry, path) in [
        ("acmesh/acme.sh", executable),
        (
            "acmesh/dnsapi/dns_cf.sh",
            acme_home_dir(state).join("dnsapi/dns_cf.sh"),
        ),
    ] {
        let mut file = zip.by_name(entry).unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut out = File::create(path).unwrap();
        std::io::copy(&mut file, &mut out).unwrap();
    }
}

async fn post(state: &AppState, path: &str, body: Value) -> (StatusCode, Value) {
    let req = Request::post(path)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let response = acme_routes()
        .with_state(state.clone())
        .oneshot(req)
        .await
        .unwrap();
    let status = response.status();
    let value = serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap())
        .unwrap();
    (status, value)
}

#[tokio::test]
async fn manual_review_releases_lease_and_never_creates_a_job() {
    let (_dir, state) = acme_test_state().await;
    install_fixture(&state).await;
    let mut app = test_application("review", &["example.test", "*.example.test"]);
    // No real provider credentials or network writes in this test.
    app["credentials"] = json!({});
    write_acme_applications(&state, std::slice::from_ref(&app))
        .await
        .unwrap();
    let t = Translator::new("en");
    dns::TEST_DNS
        .scope(fixture(&["other"], &[]), async {
            let (status, body) = post(
                &state,
                "/api/admin/acme/applications/review/request",
                json!({}),
            )
            .await;
            assert_eq!(status, StatusCode::CONFLICT, "{body}");
            assert_eq!(body["errorCode"], "preflight_required");
            let report = &body["data"];
            assert_eq!(report["status"], "review");
            assert_eq!(report["groups"].as_array().unwrap().len(), 1);
            assert_eq!(report["groups"][0]["domains"].as_array().unwrap().len(), 2);
            assert_eq!(
                get_active_acme_runtime_lock(&state).await.unwrap()["locked"],
                false
            );
            assert!(
                find_acme_application(&state, "review")
                    .await
                    .unwrap()
                    .unwrap()["latestJobId"]
                    .is_null()
            );
            let token = report["snapshot"].as_str().unwrap();
            dns::TEST_DNS
                .scope(fixture(&["different"], &[]), async {
                    assert!(
                        reserve_acme_application_job(
                            &state,
                            &app,
                            "manual_request",
                            Some(token),
                            &t
                        )
                        .await
                        .unwrap_err()
                        .is::<NeedsReview>()
                    );
                })
                .await;
            let (job, lock) =
                reserve_acme_application_job(&state, &app, "manual_request", Some(token), &t)
                    .await
                    .unwrap();
            assert!(
                reserve_acme_application_job(&state, &app, "auto_renew", None, &t)
                    .await
                    .is_err()
            );
            fail_reserved_acme_application_job(&state, &app, &job, &lock, "test finished", &t)
                .await
                .unwrap();
            assert!(
                reserve_acme_application_job(&state, &app, "manual_request", Some(token), &t)
                    .await
                    .is_err()
            );
            let (job, lock) = reserve_acme_application_job(&state, &app, "auto_renew", None, &t)
                .await
                .unwrap();
            fail_reserved_acme_application_job(&state, &app, &job, &lock, "test finished", &t)
                .await
                .unwrap();
        })
        .await;
}

#[tokio::test]
async fn renewal_pauses_on_delegation_errors_and_unverified_hooks_then_resumes() {
    let (_dir, state) = acme_test_state().await;
    install_fixture(&state).await;
    let mut app = test_application("auto", &["example.test"]);
    app["dnsType"] = json!("dns_duckdns");
    let t = Translator::new("en");
    for (observations, status) in [
        (fixture(&["foreign"], &[]), "unsafe"),
        (fixture(&[], &["NS ns.example.test."]), "delegated"),
        (BTreeMap::new(), "unknown"),
    ] {
        dns::TEST_DNS
            .scope(observations, async {
                let error = guard(&state, &app, "auto_renew", None, &t)
                    .await
                    .unwrap_err();
                let report = &error.downcast_ref::<NeedsReview>().unwrap().0;
                assert!(report.renewal_pending);
                assert_eq!(report.groups[0].status, status);
            })
            .await;
    }
    dns::TEST_DNS
        .scope(fixture(&[], &[]), async {
            assert_eq!(
                guard(&state, &app, "auto_renew", None, &t)
                    .await
                    .unwrap()
                    .status,
                "ready"
            );
        })
        .await;
}

#[tokio::test]
async fn cooldown_survives_app_reload_and_is_enforced_before_job_creation() {
    let (_dir, state) = acme_test_state().await;
    install_fixture(&state).await;
    let app = test_application("limited", &["example.test"]);
    write_acme_applications(&state, std::slice::from_ref(&app))
        .await
        .unwrap();
    record_cooldown(
        &state,
        &app,
        &[
            json!("urn:ietf:params:acme:error:rateLimited too many new orders per account"),
            json!("Retry-After: 7200"),
        ],
    )
    .await
    .unwrap();
    let other = test_application("other", &["other.test"]);
    assert!(active_cooldown(&state, &other).await.unwrap().is_some());
    dns::TEST_DNS
        .scope(fixture(&[], &[]), async {
            let (status, body) = post(
                &state,
                "/api/admin/acme/applications/limited/request",
                json!({}),
            )
            .await;
            assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
            assert_eq!(body["data"]["cooldown"]["source"], "ca");
            assert_eq!(body["data"]["cooldown"]["estimated"], false);
        })
        .await;
}

#[tokio::test]
async fn submitted_deletion_keeps_all_issuance_blocked_until_authorities_agree() {
    let (_dir, state) = acme_test_state().await;
    install_fixture(&state).await;
    let t = Translator::new("en");
    let mut app = test_application("propagation", &["example.test"]);
    app["credentials"] = json!({});
    record_deletion(&state, &config_signature(&app), NAME, "removed")
        .await
        .unwrap();
    dns::TEST_DNS
        .scope(fixture(&["removed"], &[]), async {
            let error = guard(&state, &app, "auto_renew", None, &t)
                .await
                .unwrap_err();
            assert_eq!(
                error.downcast_ref::<NeedsReview>().unwrap().0.groups[0].status,
                "propagating"
            );
        })
        .await;
    dns::TEST_DNS
        .scope(fixture(&[], &[]), async {
            assert_eq!(
                guard(&state, &app, "auto_renew", None, &t)
                    .await
                    .unwrap()
                    .status,
                "ready"
            );
        })
        .await;
}

#[tokio::test]
async fn create_edit_and_legacy_requests_preserve_configuration_when_dns_is_unknown() {
    let (_dir, state) = acme_test_state().await;
    install_fixture(&state).await;
    let body = json!({"domains":["example.test"],"dnsType":"dns_cf","credentials":{"CF_Token":"test"},"submitNow":true});
    dns::TEST_DNS
        .scope(BTreeMap::new(), async {
            let (status, created) =
                post(&state, "/api/admin/acme/applications", body.clone()).await;
            assert_eq!(status, StatusCode::CONFLICT, "{created}");
            let id = created["data"]["applicationId"].as_str().unwrap();
            let mut edited = body.clone();
            edited["name"] = json!("edited");
            let req = Request::patch(format!("/api/admin/acme/applications/{id}"))
                .header("content-type", "application/json")
                .body(Body::from(edited.to_string()))
                .unwrap();
            let resp = acme_routes()
                .with_state(state.clone())
                .oneshot(req)
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::CONFLICT);
            assert_eq!(
                find_acme_application(&state, id).await.unwrap().unwrap()["name"],
                "edited"
            );
            let (status, legacy) = post(&state, "/api/admin/acme/request", body).await;
            assert_eq!(status, StatusCode::CONFLICT, "{legacy}");
            assert_eq!(legacy["data"]["applicationId"], id);
            assert_eq!(read_acme_applications(&state).await.unwrap().len(), 1);
            assert!(
                find_acme_application(&state, id).await.unwrap().unwrap()["latestJobId"].is_null()
            );
            assert_eq!(
                get_active_acme_runtime_lock(&state).await.unwrap()["locked"],
                false
            );
        })
        .await;
}

#[tokio::test]
async fn windows_profile_requires_manual_cleanup_of_existing_values() {
    let dir = tempfile::tempdir().unwrap();
    let state =
        super::super::tests::acme_test_state_with_data_dir(dir.path().join("windows"), "windows")
            .await;
    let mut app = test_application("win", &["example.test"]);
    app["credentials"] = json!({});
    dns::TEST_DNS
        .scope(fixture(&["existing"], &[]), async {
            let t = Translator::new("en");
            assert!(!audited_hook(&state, "dns_cf").await);
            let err = guard(&state, &app, "auto_renew", None, &t)
                .await
                .unwrap_err();
            assert_eq!(
                err.downcast_ref::<NeedsReview>().unwrap().0.groups[0].status,
                "unsafe"
            );
        })
        .await;
}

#[tokio::test]
async fn restart_keeps_cooldown_and_recovers_dns_leftovers_from_stopped_job() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("restart");
    let app = test_application("recover", &["example.test"]);
    let t = Translator::new("en");
    let id;
    {
        let state = super::super::tests::acme_test_state_with_data_dir(path.clone(), "linux").await;
        state
            .storage
            .store
            .set_string_value(ACME_MIGRATION_VERSION_KEY, "1")
            .await
            .unwrap();
        write_acme_applications(&state, std::slice::from_ref(&app))
            .await
            .unwrap();
        let job = build_queued_acme_job(&app, "manual_request", &t).unwrap();
        id = job["id"].as_str().unwrap().to_owned();
        create_acme_job(&state, &job, &t).await.unwrap();
        state
            .storage
            .store
            .set_json_value(
                ACME_RUNTIME_LOCK_KEY,
                &with_runtime_lock_lease(build_acme_runtime_lock(&app, &job, "manual_request")),
            )
            .await
            .unwrap();
        state.storage.store.set_json_value(&format!("fn_knock:acme:dns-baseline:{id}"),&json!({"groups":[{"name":NAME,"values":[]}],"configSignature":config_signature(&app)})).await.unwrap();
        mark_acme_job_stopped(&state, &id, "stopped").await.unwrap();
        record_cooldown(
            &state,
            &app,
            &[json!("DNS API HTTP 429"), json!("Retry-After: 3600")],
        )
        .await
        .unwrap();
    }
    let state = super::super::tests::acme_test_state_with_data_dir(path, "linux").await;
    dns::TEST_DNS
        .scope(fixture(&["unowned"], &[]), async {
            assert!(recover_orphaned_acme_runtime_job(&state, &t).await.unwrap());
            assert_eq!(
                get_acme_job(&state, &id).await.unwrap().unwrap()["cleanupPending"],
                true
            );
            assert_eq!(
                get_active_acme_runtime_lock(&state).await.unwrap()["locked"],
                false
            );
            assert_eq!(
                active_cooldown(&state, &app).await.unwrap().unwrap().source,
                "dns"
            );
        })
        .await;
}

#[tokio::test]
async fn reads_stop_and_startup_cannot_steal_another_instances_lease() {
    let (_dir, state) = acme_test_state().await;
    let app = test_application("remote", &["remote.test"]);
    write_acme_applications(&state, std::slice::from_ref(&app))
        .await
        .unwrap();
    let t = Translator::new("en");
    let job = build_queued_acme_job(&app, "auto_renew", &t).unwrap();
    create_acme_job(&state, &job, &t).await.unwrap();
    update_acme_application_job_state(&state, &app, &job)
        .await
        .unwrap();
    let lock = with_runtime_lock_lease(build_acme_runtime_lock(&app, &job, "auto_renew"));
    state
        .storage
        .store
        .set_json_value_ex(ACME_RUNTIME_LOCK_KEY, &lock, 300)
        .await
        .unwrap();
    assert_eq!(
        get_active_acme_runtime_lock(&state).await.unwrap()["locked"],
        true
    );
    assert!(stop_active_acme_job(&state, &t).await.is_err());
    assert!(!recover_orphaned_acme_runtime_job(&state, &t).await.unwrap());
    assert_eq!(
        get_acme_job(&state, job["id"].as_str().unwrap())
            .await
            .unwrap()
            .unwrap()["status"],
        "queued"
    );
    assert!(
        !state
            .storage
            .store
            .set_json_value_nx_ex(ACME_RUNTIME_LOCK_KEY, &json!({"lockId":"delete"}), 300)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn recreated_reordered_sans_share_interrupted_429_even_after_log_clear() {
    let (_dir, state) = acme_test_state().await;
    let original = test_application("original", &["a.test", "b.test"]);
    let replacement = test_application("replacement", &["b.test", "a.test"]);
    state
        .storage
        .store
        .set_json_value_ex(
            ACME_RUNTIME_LOCK_KEY,
            &json!({"locked":true,"lockId":"test-lease","jobId":"interrupted"}),
            300,
        )
        .await
        .unwrap();
    let rates = JobRates::new(&state, "interrupted", &original, "letsencrypt");
    rates
        .observe("HTTP/1.1 429 Too Many Requests")
        .await
        .unwrap();
    rates.observe("Retry-After: 120").await.unwrap();
    for _ in 0..1025 {
        rates.observe("ordinary cleanup output").await.unwrap();
    }
    clear_acme_logs(&state, "interrupted").await.unwrap();
    let limit = active_cooldown(&state, &replacement)
        .await
        .unwrap()
        .unwrap();
    assert!(!limit.estimated);
    assert!((119..=120).contains(&(limit.retry_at - time_utils::now_ms() / 1000)));
    state
        .storage
        .store
        .delete_lock_if_owned(ACME_RUNTIME_LOCK_KEY, "test-lease")
        .await
        .unwrap();
    // A stale process must not overwrite the current owner's journal.
    rates.observe("Retry-After: 1").await.unwrap();
    recover_orphaned_acme_runtime_job(&state, &Translator::new("en"))
        .await
        .unwrap();
    assert_eq!(
        active_cooldown(&state, &replacement)
            .await
            .unwrap()
            .unwrap()
            .retry_at,
        limit.retry_at
    );
    assert!(
        active_cooldown(&state, &test_application("unrelated", &["other.test"]))
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn shorter_concurrent_cooldowns_never_overwrite_a_longer_one() {
    let (_dir, state) = acme_test_state().await;
    let app = test_application("same", &["example.test"]);
    let now = time_utils::now_ms() / 1000;
    let mut writers = tokio::task::JoinSet::new();
    for seconds in [8000, 60, 500, 20, 25, 7000, 15, 10] {
        let app = app.clone();
        let state = state.clone();
        writers.spawn(async move {
            record_cooldown(
                &state,
                &app,
                &[
                    json!("DNS API HTTP 429"),
                    json!(format!("Retry-After: {seconds}")),
                ],
            )
            .await
            .unwrap();
        });
    }
    while let Some(done) = writers.join_next().await {
        done.unwrap();
    }
    assert!(
        active_cooldown(&state, &app)
            .await
            .unwrap()
            .unwrap()
            .retry_at
            >= now + 8000
    );
}

#[cfg(unix)]
#[tokio::test]
async fn scheduled_scan_renews_after_dns_recovers_and_rate_deadline_expires() {
    use super::super::tests::{generate_acme_test_cert_pair, test_cert_info};
    let (_dir, state) = acme_test_state().await;
    install_fixture(&state).await;
    let mut app = test_application("scheduled", &["example.test"]);
    app["credentials"] = json!({});
    app["dnsType"] = json!("dns_gd");
    app["latestJobStatus"] = json!("failed");
    app["latestJobId"] = json!("previous");
    app["latestJobAt"] = json!(now_node_iso());
    write_acme_applications(&state, std::slice::from_ref(&app))
        .await
        .unwrap();
    let mut info = test_cert_info(&["example.test"], "old");
    info["validTo"] = json!("Jul  1 00:00:00 2020 GMT");
    state.storage.store.set_json_value(ACME_ISSUED_CERTIFICATES_KEY,&json!([{"applicationId":"scheduled","primaryDomain":"example.test","cert":"old","key":"old","createdAt":now_node_iso(),"certInfo":info}])).await.unwrap();
    let Some((cert, key)) = generate_acme_test_cert_pair("example.test") else {
        return;
    };
    let dir = state.settings.data_dir.join("ssl/example.test");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("fullchain.cer"), cert).unwrap();
    std::fs::write(dir.join("example.test.key"), key).unwrap();
    let exe = acme_executable_path(&state);
    std::fs::write(&exe, "#!/bin/sh\nexit 0\n").unwrap();
    crate::fs_utils::chmod_executable(&exe);
    let mut limit = Cooldown {
        source: "unknown".into(),
        retry_at: time_utils::now_ms() / 1000 + 120,
        estimated: false,
    };
    mark_limited_job(&state, "previous", &limit).await.unwrap();
    dns::TEST_DNS
        .scope(fixture(&[], &[]), run_acme_auto_renew_once(state.clone()))
        .await
        .unwrap();
    assert_eq!(
        find_acme_application(&state, "scheduled")
            .await
            .unwrap()
            .unwrap()["latestJobId"],
        "previous"
    );
    limit.retry_at = time_utils::now_ms() / 1000 - 1;
    mark_limited_job(&state, "previous", &limit).await.unwrap();
    dns::TEST_DNS
        .scope(BTreeMap::new(), run_acme_auto_renew_once(state.clone()))
        .await
        .unwrap();
    assert_eq!(
        last_report(&state, "scheduled").await.unwrap().unwrap()["renewalPending"],
        true
    );
    // Opening or editing the review panel cannot reintroduce ordinary failure backoff.
    state
        .storage
        .store
        .set_json_value(&super::key("scheduled"), &json!({"status":"ready"}))
        .await
        .unwrap();
    dns::TEST_DNS
        .scope(fixture(&[], &[]), run_acme_auto_renew_once(state.clone()))
        .await
        .unwrap();
    let app = find_acme_application(&state, "scheduled")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(app["latestJobStatus"], "succeeded", "{app}");
    assert_eq!(app["latestJobTrigger"], "auto_renew");
}

#[tokio::test]
async fn account_settings_cannot_reregister_through_ca_cooldown() {
    let (_dir, state) = acme_test_state().await;
    install_fixture(&state).await;
    let until = time_utils::now_ms() / 1000 + 600;
    persist_registration_limit(
        &state,
        "letsencrypt",
        &Cooldown {
            source: "ca".into(),
            retry_at: until,
            estimated: false,
        },
    )
    .await
    .unwrap();
    let req = Request::post("/api/admin/acme/client-settings")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"certificateAuthority":"letsencrypt","accountEmail":"another@example.test"})
                .to_string(),
        ))
        .unwrap();
    let response = acme_routes()
        .with_state(state.clone())
        .oneshot(req)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(response.headers().contains_key("Retry-After"));
    assert_eq!(
        cooldown_for_ca(
            &state,
            &test_application("unrelated", &["other.test"]),
            "letsencrypt"
        )
        .await
        .unwrap()
        .unwrap()
        .retry_at,
        until
    );
}

#[test]
fn unpublished_provider_values_are_protected_without_authorizing_deletion() {
    let mut g = ChallengeGroup {
        name: NAME.into(),
        zone: "example.test".into(),
        domains: vec!["example.test".into()],
        status: "clear".into(),
        values: vec![],
        delegation: vec![],
        selectable: vec![],
    };
    let private = TxtRecord {
        id: "1".into(),
        name: NAME.into(),
        zone: "example.test".into(),
        value: "other-client".into(),
    };
    assert!(merge_provider_records(&mut g, vec![private], false).is_empty());
    assert_eq!(g.values, ["other-client"]);
    assert_eq!(g.status, "unsafe");
    assert!(g.selectable.is_empty());
}

#[tokio::test]
async fn explicit_provider_quota_follows_credential_across_applications_only() {
    let (_dir, state) = acme_test_state().await;
    let app = test_application("one", &["one.test"]);
    let other = test_application("two", &["two.test"]);
    let mut different = test_application("three", &["three.test"]);
    different["credentials"]["CF_Token"] = json!("another-token");
    let quota = crate::ddns::acme_txt::ProviderLimit::from_body(
        Some("750".into()),
        br#"{"message":"request limit per API token"}"#,
    );
    persist_provider_limit(&state, &app, &quota).await.unwrap();
    assert!(dns_cooling(&state, &other).await.unwrap());
    assert!(!dns_cooling(&state, &different).await.unwrap());
}

#[tokio::test]
async fn multiline_acme_problem_shares_explicit_account_limit_before_job_exit() {
    let (_dir, state) = acme_test_state().await;
    state
        .storage
        .store
        .set_json_value_ex(
            ACME_RUNTIME_LOCK_KEY,
            &json!({"lockId":"owner","jobId":"problem"}),
            300,
        )
        .await
        .unwrap();
    let app = test_application("first", &["first.test"]);
    let rates = JobRates::new(&state, "problem", &app, "letsencrypt");
    for line in [
        "HTTP/2 429",
        r#""type": "urn:ietf:params:acme:error:rateLimited","#,
        r#""detail": "too many new orders from this account","#,
        "Retry-After: 180",
    ] {
        rates.observe(line).await.unwrap();
    }
    // A generic diagnostic much later in the same response cannot erase
    // the previously identified account scope.
    for _ in 0..30 {
        rates.observe("debug").await.unwrap();
    }
    rates.observe("response status: 429").await.unwrap();
    let sibling = test_application("other", &["different.test"]);
    assert!(
        cooldown_for_ca(&state, &sibling, "letsencrypt")
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        cooldown_for_ca(&state, &sibling, "zerossl")
            .await
            .unwrap()
            .is_none()
    );
}
