use super::*;
use crate::store::{DockerAdminPasswordRecord, DockerAdminSessionRecord};
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use tower::ServiceExt;

async fn test_state() -> (tempfile::TempDir, AppState) {
    let directory = tempfile::tempdir().unwrap();
    let mut settings = {
        let _environment = crate::test_support::EnvGuard::new(&[]);
        crate::settings::Settings::from_env()
    };
    settings.data_dir = directory.path().join("data");
    settings.runtime_target = "linux".into();
    settings.gateway_config_dir = directory.path().join("gateway");
    settings.sqlite_path = directory.path().join("test.sqlite3");
    settings.legacy_redis_url = String::new();
    settings.go_backend_grpc_addr = "http://127.0.0.1:1".into();
    settings.internal_rpc_token = "terminal-access-test".into();
    settings.request_timeout = Duration::from_millis(100);
    (directory, AppState::new(settings).await.unwrap())
}

async fn change(
    state: &AppState,
    enabled: bool,
    password: Option<&str>,
    clear_password: bool,
) -> WebTerminalSettings {
    update(
        state,
        WebTerminalSettingsInput {
            enabled,
            password: password.map(str::to_string),
            clear_password,
            revision: settings(state).await.unwrap().revision,
        },
    )
    .await
    .unwrap()
}
fn headers(cookie: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(header::COOKIE, cookie.parse().unwrap());
    headers
}
async fn unlock(
    state: &AppState,
    headers: &HeaderMap,
    secret: &str,
) -> TerminalResult<Option<String>> {
    verify(
        state,
        headers,
        WebTerminalVerifyInput {
            password: secret.into(),
        },
    )
    .await
}

#[tokio::test]
async fn busy_password_admission_refunds_attempt_and_returns_retryable_response() {
    let mut attempts = HashMap::new();
    for _ in 0..8 {
        let result = password_attempt(&mut attempts, "client", async {
            Err(TerminalError::new(TerminalErrorCode::ResourceBusy, "busy"))
        })
        .await;
        let response = terminal_error(result.unwrap_err());
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(response.headers()[header::RETRY_AFTER], "3");
        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["errorCode"], "resource_busy");
        assert!(!attempts.contains_key("client"));
    }
    assert!(
        !password_attempt(&mut attempts, "client", async { Ok(false) })
            .await
            .unwrap()
    );
    let started = attempts["client"].1;
    assert!(
        password_attempt(&mut attempts, "client", async {
            Err(TerminalError::new(TerminalErrorCode::ResourceBusy, "busy"))
        })
        .await
        .is_err()
    );
    assert_eq!(
        attempts["client"],
        (1, started),
        "busy preserves earlier failed attempts"
    );
}

#[tokio::test]
async fn defaults_persist_without_secrets_and_preserve_password_when_disabled() {
    let (_directory, state) = test_state().await;
    let initial = settings(&state).await.unwrap();
    assert!(initial.enabled && !initial.password_configured);
    assert!(status(&state, &HeaderMap::new()).await.unwrap().authorized);
    let saved = change(&state, true, Some("secret"), false).await;
    let public = serde_json::to_string(&saved).unwrap();
    assert!(!public.contains("secret") && !public.contains("hash") && !public.contains("salt"));
    let disabled = change(&state, false, None, false).await;
    assert!(!disabled.enabled && disabled.password_configured);
    assert_ne!(disabled.revision, saved.revision);
    // A separate AppState over the same database reads the new settings.
    let restarted = AppState::new(state.settings.clone()).await.unwrap();
    assert!(!settings(&restarted).await.unwrap().enabled);
    let enabled = change(&state, true, Some(""), false).await;
    assert!(enabled.password_configured);
    let cleared = change(&state, true, None, true).await;
    assert!(!cleared.password_configured);
    assert!(status(&state, &HeaderMap::new()).await.unwrap().authorized);
}

#[tokio::test]
async fn browser_grant_is_revoked_by_password_changes_and_cannot_be_forged() {
    let (_directory, state) = test_state().await;
    change(&state, true, Some("secret"), false).await;
    assert!(!status(&state, &HeaderMap::new()).await.unwrap().authorized);
    let token = unlock(&state, &HeaderMap::new(), "secret")
        .await
        .unwrap()
        .unwrap();
    let browser = headers(&format!("{GRANT_COOKIE}={token}"));
    assert!(status(&state, &browser).await.unwrap().authorized);
    assert!(
        !status(
            &state,
            &headers(&format!("{GRANT_COOKIE}={}", Uuid::new_v4()))
        )
        .await
        .unwrap()
        .authorized
    );
    change(&state, true, Some("changed"), false).await;
    assert!(!status(&state, &browser).await.unwrap().authorized);
    assert!(unlock(&state, &browser, "secret").await.is_err());
    let rotated = unlock(&state, &browser, "changed").await.unwrap().unwrap();
    assert_ne!(rotated, token);
    assert!(!status(&state, &browser).await.unwrap().authorized);
    let browser = headers(&format!("{GRANT_COOKIE}={rotated}"));
    assert!(status(&state, &browser).await.unwrap().authorized);
    change(&state, false, None, false).await;
    change(&state, true, None, false).await;
    assert!(!status(&state, &browser).await.unwrap().authorized);
    assert!(!browser_cookie(&token, true).contains("Max-Age"));
    assert!(browser_cookie(&token, true).contains("HttpOnly; SameSite=Strict; Secure"));
}

#[tokio::test]
async fn verification_is_bound_to_the_valid_login_session() {
    let (_directory, state) = test_state().await;
    let now = time_utils::now_iso();
    state
        .storage
        .store
        .set_docker_admin_password(&DockerAdminPasswordRecord {
            algorithm: "scrypt".into(),
            salt: "salt".into(),
            hash: "hash".into(),
            n: 16384,
            r: 8,
            p: 1,
            key_length: 64,
            created_at: now.clone(),
            updated_at: now.clone(),
        })
        .await
        .unwrap();
    let mut session = DockerAdminSessionRecord {
        id: Uuid::new_v4().to_string(),
        created_at: now.clone(),
        updated_at: now,
        expires_at: time_utils::iso_after_seconds(3600),
        ttl_seconds: 3600,
        password_revision: String::new(),
        ip: String::new(),
        user_agent: String::new(),
    };
    state
        .storage
        .store
        .set_docker_admin_session(&session)
        .await
        .unwrap();
    change(&state, true, Some("secret"), false).await;
    let login = headers(&format!(
        "{}={}",
        cookies::ADMIN_PANEL_SESSION_COOKIE_NAME,
        session.id
    ));
    assert!(unlock(&state, &login, "secret").await.unwrap().is_none());
    assert!(status(&state, &login).await.unwrap().authorized);
    // A fresh request (refresh/new tab) with the same session remains authorized.
    assert!(status(&state, &login.clone()).await.unwrap().authorized);
    state
        .storage
        .store
        .delete_docker_admin_session(&session.id)
        .await
        .unwrap();
    assert!(!status(&state, &login).await.unwrap().authorized);
    expire_grants(&state).await.unwrap();
    assert!(
        state
            .storage
            .store
            .get_string_value(&grant_key(
                cookies::ADMIN_PANEL_SESSION_COOKIE_NAME,
                &session.id
            ))
            .await
            .unwrap()
            .is_none()
    );
    session.id = Uuid::new_v4().to_string();
    state
        .storage
        .store
        .set_docker_admin_session(&session)
        .await
        .unwrap();
    let next_login = headers(&format!(
        "{}={}",
        cookies::ADMIN_PANEL_SESSION_COOKIE_NAME,
        session.id
    ));
    assert!(!status(&state, &next_login).await.unwrap().authorized);
}

#[tokio::test]
async fn limits_failed_password_attempts_and_rejects_stale_settings_updates() {
    let (_directory, state) = test_state().await;
    let initial = settings(&state).await.unwrap();
    change(&state, true, Some("secret"), false).await;
    let conflict = update(
        &state,
        WebTerminalSettingsInput {
            enabled: false,
            revision: initial.revision,
            password: None,
            clear_password: false,
        },
    )
    .await
    .err()
    .unwrap();
    assert_eq!(conflict.code, TerminalErrorCode::Conflict);
    for _ in 0..5 {
        assert_eq!(
            unlock(&state, &HeaderMap::new(), "wrong")
                .await
                .err()
                .unwrap()
                .code,
            TerminalErrorCode::AccessPasswordRequired
        );
    }
    assert_eq!(
        unlock(&state, &HeaderMap::new(), "secret")
            .await
            .err()
            .unwrap()
            .code,
        TerminalErrorCode::AccessRateLimited
    );
    state
        .terminal
        .access
        .attempts
        .lock()
        .await
        .values_mut()
        .for_each(|(_, start)| *start = Instant::now() - Duration::from_secs(61));
    assert!(unlock(&state, &HeaderMap::new(), "secret").await.is_ok());
}

#[tokio::test]
async fn every_business_endpoint_enforces_the_gate_before_processing_input() {
    let (_directory, state) = test_state().await;
    let document = super::super::http::routes().into_openapi();
    let router = crate::terminal::terminal_routes().with_state(state.clone());
    for (enabled, expected) in [
        (true, "access_password_required"),
        (false, "feature_disabled"),
    ] {
        change(&state, enabled, Some("secret"), false).await;
        let paths = serde_json::to_value(&document).unwrap();
        for (path, item) in paths["paths"].as_object().unwrap() {
            if path.ends_with("/settings") || path.contains("/access") {
                continue;
            }
            for method in item.as_object().unwrap().keys() {
                let response = router
                    .clone()
                    .oneshot(
                        Request::builder()
                            .method(method.to_uppercase().as_str())
                            .uri(path.replace("{id}", "00000000-0000-0000-0000-000000000001"))
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                assert_eq!(response.status(), StatusCode::FORBIDDEN, "{method} {path}");
                let body: serde_json::Value =
                    serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap())
                        .unwrap();
                assert_eq!(body["errorCode"], expected, "{method} {path}");
            }
        }
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/admin/terminal/settings")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}

#[cfg(unix)]
#[tokio::test]
async fn disabling_waits_for_inflight_creation_then_ends_every_session() {
    use super::super::{
        domain::{CreateSessionInput, LocalTerminalSettingsInput},
        service,
    };
    let (_directory, state) = test_state().await;
    service::update_local_terminal(
        &state,
        LocalTerminalSettingsInput {
            enabled: true,
            revision: 0,
            acknowledge_risk: true,
        },
        false,
        None,
    )
    .await
    .unwrap();
    let (mut parts, _) = Request::builder().body(()).unwrap().into_parts();
    let access = TerminalAccess::from_request_parts(&mut parts, &state)
        .await
        .unwrap();
    let mut update_request = Box::pin(update(
        &state,
        WebTerminalSettingsInput {
            enabled: false,
            revision: settings(&state).await.unwrap().revision,
            password: None,
            clear_password: false,
        },
    ));
    std::future::poll_fn(|cx| {
        assert!(std::future::Future::poll(update_request.as_mut(), cx).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    service::create_local_session(
        &state,
        CreateSessionInput {
            cols: Some(80),
            rows: Some(24),
            title: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(state.terminal.list().await.sessions.len(), 1);
    drop(access);
    assert!(!update_request.await.unwrap().enabled);
    assert!(state.terminal.list().await.sessions.is_empty());
    assert!(
        TerminalAccess::from_request_parts(&mut parts, &state)
            .await
            .is_err()
    );
    assert!(
        service::local_terminal_status(&state)
            .await
            .unwrap()
            .enabled
    );
}

#[cfg(unix)]
#[tokio::test]
async fn accepted_disable_finishes_after_the_http_request_is_cancelled() {
    use super::super::{
        domain::{CreateSessionInput, LocalTerminalSettingsInput},
        service,
    };
    let (_directory, state) = test_state().await;
    service::update_local_terminal(
        &state,
        LocalTerminalSettingsInput {
            enabled: true,
            revision: 0,
            acknowledge_risk: true,
        },
        false,
        None,
    )
    .await
    .unwrap();
    service::create_local_session(
        &state,
        CreateSessionInput {
            cols: Some(80),
            rows: Some(24),
            title: None,
        },
    )
    .await
    .unwrap();
    let mut request = Box::pin(update(
        &state,
        WebTerminalSettingsInput {
            enabled: false,
            revision: settings(&state).await.unwrap().revision,
            password: None,
            clear_password: false,
        },
    ));
    // Dispatch the owned operation, then simulate the client dropping its response.
    std::future::poll_fn(|cx| {
        assert!(std::future::Future::poll(request.as_mut(), cx).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    drop(request);
    let _guard = state.terminal.access.policy.read().await;
    assert!(!settings(&state).await.unwrap().enabled);
    assert!(state.terminal.list().await.sessions.is_empty());
}

#[test]
fn cancelled_password_checks_still_consume_the_attempt_budget() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    runtime.block_on(async {
        let (_directory, state) = test_state().await;
        change(&state, true, Some("secret"), false).await;
        // Park the hash worker so every admitted check is cancelled before its
        // result can be delivered. The storage executor uses its own thread.
        let (release, blocked) = std::sync::mpsc::channel::<()>();
        let (started, ready) = tokio::sync::oneshot::channel();
        let worker = tokio::task::spawn_blocking(move || {
            let _ = started.send(());
            let _ = blocked.recv();
        });
        ready.await.unwrap();
        let headers = HeaderMap::new();
        let mut charged = 0;
        for _ in 0..50 {
            tokio::select! {
                result = unlock(&state, &headers, "wrong") => panic!("hash worker is parked: {result:?}"),
                _ = tokio::time::sleep(Duration::from_millis(10)) => {},
            }
            charged = state.terminal.access.attempts.lock().await
                .get("").map(|(count, _)| *count).unwrap_or(0);
            if charged == 5 { break; }
        }
        assert_eq!(charged, 5, "cancelled requests must be charged before hashing");
        assert_eq!(unlock(&state, &headers, "secret").await.err().unwrap().code,
            TerminalErrorCode::AccessRateLimited);
        drop(release);
        worker.await.unwrap();
    });
}

#[cfg(unix)]
mod cgi;
