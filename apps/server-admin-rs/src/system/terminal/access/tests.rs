use super::*;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use std::time::Duration;
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

async fn change(state: &AppState, enabled: bool) -> WebTerminalSettings {
    update(
        state,
        WebTerminalSettingsInput {
            enabled,
            revision: settings(state).await.unwrap().revision,
        },
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn defaults_and_switch_persist_without_password_fields() {
    let (_directory, state) = test_state().await;
    assert!(settings(&state).await.unwrap().enabled);
    let disabled = change(&state, false).await;
    let restarted = AppState::new(state.settings.clone()).await.unwrap();
    assert!(!settings(&restarted).await.unwrap().enabled);
    let enabled = change(&state, true).await;
    assert_ne!(enabled.revision, disabled.revision);
    let raw = state
        .storage
        .store
        .get_json_value(SETTINGS_KEY)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(raw.as_object().unwrap().len(), 2);
    assert!(raw.get("password").is_none());
    assert!(
        update(
            &state,
            WebTerminalSettingsInput {
                enabled: false,
                revision: disabled.revision
            }
        )
        .await
        .is_err()
    );
}

#[tokio::test]
async fn legacy_password_is_ignored_and_removed_without_changing_the_switch() {
    let (_directory, state) = test_state().await;
    let router = crate::terminal::terminal_routes().with_state(state.clone());
    for enabled in [false, true] {
        state.storage.store.set_json_value(SETTINGS_KEY, &serde_json::json!({"enabled": enabled, "revision": "legacy", "password": {"hash": "obsolete"}})).await.unwrap();
        state
            .storage
            .store
            .set_string_value_with_optional_ttl(
                "fn_knock:terminal:access-grant:legacy",
                "obsolete",
                None,
            )
            .await
            .unwrap();
        // An old hash, even if invalid, no longer gates terminal requests.
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/admin/terminal/targets")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            if enabled {
                StatusCode::OK
            } else {
                StatusCode::FORBIDDEN
            }
        );
        cleanup_retired_password(&state).await.unwrap();
        let raw = state
            .storage
            .store
            .get_json_value(SETTINGS_KEY)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            raw,
            serde_json::json!({"enabled": enabled, "revision": "legacy"})
        );
        assert!(
            state
                .storage
                .store
                .scan_keys("fn_knock:terminal:access-grant:", 200)
                .await
                .unwrap()
                .is_empty()
        );
    }
    for (method, path) in [
        ("GET", "/api/admin/terminal/access"),
        ("POST", "/api/admin/terminal/access/verify"),
    ] {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}

#[tokio::test]
async fn every_business_endpoint_enforces_the_switch_before_processing_input() {
    let (_directory, state) = test_state().await;
    change(&state, false).await;
    let document = super::super::http::routes().into_openapi();
    let router = crate::terminal::terminal_routes().with_state(state.clone());
    let paths = serde_json::to_value(&document).unwrap();
    for (path, item) in paths["paths"].as_object().unwrap() {
        if path.ends_with("/settings") {
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
            assert_eq!(body["errorCode"], "feature_disabled", "{method} {path}");
        }
    }
    let response = router
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

#[cfg(unix)]
mod cgi;
