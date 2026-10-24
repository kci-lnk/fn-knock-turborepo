use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn response_value(response: Response) -> Value {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn proxy_probe_uses_drafts_and_saved_password_without_saving_or_updating_dns() {
    let (_directory, state) = super::super::tests::ddns_test_state().await;
    let saved = r#"{"httpTransport":"node","updateProxy":{"enabled":false,"url":"http://127.0.0.1:1","username":"user","password":"stored-secret"}}"#;
    state
        .storage
        .store
        .set_string_value(DDNS_SETTINGS, saved)
        .await
        .unwrap();
    for transport in ["node", "curl"] {
        for (password, clear, expected) in [
            ("", false, "stored-secret"),
            ("draft-secret", false, "draft-secret"),
            ("", true, ""),
        ] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let server = tokio::spawn(async move {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                while !request.ends_with(b"\r\n\r\n") {
                    request.push(socket.read_u8().await.unwrap());
                }
                let request = String::from_utf8(request).unwrap();
                assert!(request.starts_with("GET http://probe.invalid/connectivity HTTP/1.1"));
                let auth = BASE64_STANDARD.encode(format!("user:{expected}"));
                assert!(
                    request.to_ascii_lowercase().contains(
                        &format!("proxy-authorization: basic {auth}").to_ascii_lowercase()
                    )
                );
                assert!(!request.contains("/records"));
                socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 13\r\nConnection: close\r\n\r\nstored-secret").await.unwrap();
            });
            let body = serde_json::from_value(json!({"httpTransport":transport,"updateProxy":{
                "enabled":false,"url":format!("http://{address}"),"password":password,"clearPassword":clear
            }})).unwrap();
            let response =
                run_update_proxy_test(&state, body, "http://probe.invalid/connectivity").await;
            assert_eq!(response.status(), StatusCode::OK);
            let value = response_value(response).await;
            assert_eq!(value["data"]["success"], true);
            assert_eq!(value["data"]["status"], 200);
            assert!(value["data"]["elapsedMs"].is_number());
            assert!(!value.to_string().contains("secret"));
            assert_eq!(
                state
                    .storage
                    .store
                    .get_string_value(DDNS_SETTINGS)
                    .await
                    .unwrap()
                    .as_deref(),
                Some(saved)
            );
            tokio_time::timeout(Duration::from_secs(5), server)
                .await
                .unwrap()
                .unwrap();
        }
    }
}

#[tokio::test]
async fn proxy_probe_requires_address_and_never_falls_back_to_direct() {
    let (_directory, state) = super::super::tests::ddns_test_state().await;
    let missing = serde_json::from_value(json!({"updateProxy":{"enabled":false}})).unwrap();
    assert_eq!(
        test_update_proxy(State(state.clone()), Json(missing))
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    let direct = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    for transport in ["node", "curl"] {
        let closed = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = closed.local_addr().unwrap();
        drop(closed);
        let body = serde_json::from_value(
            json!({"httpTransport":transport,"updateProxy":{"url":format!("http://{address}")}}),
        )
        .unwrap();
        let response = run_update_proxy_test(
            &state,
            body,
            &format!("http://{}/", direct.local_addr().unwrap()),
        )
        .await;
        let value = response_value(response).await;
        assert_eq!(value["data"]["success"], false);
        assert!(value["data"]["error"].is_string());
    }
    assert!(
        tokio_time::timeout(Duration::from_millis(50), direct.accept())
            .await
            .is_err()
    );
    assert!(
        state
            .storage
            .store
            .get_string_value(DDNS_SETTINGS)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn proxy_probe_uses_fixed_https_destination_and_redacts_authentication_failure() {
    let (_directory, state) = super::super::tests::ddns_test_state().await;
    for transport in ["node", "curl"] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                request.push(socket.read_u8().await.unwrap());
            }
            assert!(
                String::from_utf8(request)
                    .unwrap()
                    .starts_with("CONNECT www.cloudflare.com:443")
            );
            socket.write_all(b"HTTP/1.1 407 Proxy Authentication Required\r\nContent-Length: 13\r\nConnection: close\r\n\r\nstored-secret").await.unwrap();
        });
        let body = serde_json::from_value(json!({"httpTransport":transport,"updateProxy":{
            "url":format!("http://{address}"),"username":"user","password":"stored-secret"
        }}))
        .unwrap();
        let response = test_update_proxy(State(state.clone()), Json(body)).await;
        let value = response_value(response).await;
        assert_eq!(value["data"]["success"], false);
        assert_eq!(value["data"]["url"], PROXY_TEST_URL);
        assert!(!value.to_string().contains("stored-secret"));
        server.await.unwrap();
    }
}
