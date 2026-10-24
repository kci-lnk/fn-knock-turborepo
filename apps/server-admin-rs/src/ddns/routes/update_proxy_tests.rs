use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn merge_proxy(current: &DDNSUpdateProxy, value: Value) -> anyhow::Result<DDNSUpdateProxy> {
    current.merge(serde_json::from_value(value).unwrap())
}

#[test]
fn update_proxy_defaults_validation_merge_and_redaction() {
    let proxy = DDNSUpdateProxy::from_settings(&parse_settings(None));
    assert!(!proxy.enabled);
    let corrupt = parse_settings(Some(r#"{"updateProxy":{"enabled":true,"url":123}}"#));
    let options = DDNSHttpClientOptions::from_settings_and_config(&corrupt, &HashMap::new());
    assert!(ddns_http_client(&Translator::new("en"), &options).is_err());
    assert_eq!(
        public_ddns_settings(parse_settings(None))["updateProxy"],
        json!({
            "enabled":false, "url":"", "username":"", "passwordConfigured":false
        })
    );
    for url in [
        "ftp://localhost:80",
        "socks5://localhost",
        "http://localhost:0",
        "http://user:pass@localhost:80",
        "http://localhost:80/path",
        "http://localhost:80/?x=1",
        "https://localhost/#x",
        "http://localhost:65536",
    ] {
        assert!(merge_proxy(&proxy, json!({"url":url})).is_err(), "{url}");
    }
    assert!(merge_proxy(&proxy, json!({"enabled":true})).is_err());
    let stored = merge_proxy(&proxy, json!({"enabled":true,"url":"socks5://[::1]:1080","username":"test","password":"private-marker"})).unwrap();
    assert_eq!(stored.url, "socks5h://[::1]:1080");
    let retained = merge_proxy(&stored, json!({"enabled":false,"password":""})).unwrap();
    assert_eq!(retained.password, "private-marker");
    let replaced = merge_proxy(&retained, json!({"password":"replacement"})).unwrap();
    assert_eq!(replaced.password, "replacement");
    assert!(merge_proxy(&stored, json!({"password":"new","clearPassword":true})).is_err());
    assert!(merge_proxy(&stored, json!({"username":"bad\nname"})).is_err());
    assert!(merge_proxy(&stored, json!({"username":""})).is_err());
    assert!(
        merge_proxy(&stored, json!({"clearPassword":true}))
            .unwrap()
            .password
            .is_empty()
    );
    let settings = parse_settings(Some(&json!({"updateProxy":stored}).to_string()));
    assert_eq!(
        DDNSUpdateProxy::from_settings(&settings).password,
        "private-marker"
    );
    assert!(
        !public_ddns_settings(settings)
            .to_string()
            .contains("private-marker")
    );
    assert!(!format!("{stored:?}").contains("private-marker"));
}

#[test]
fn update_proxy_target_precedence_and_missing_proxy_fail_closed() {
    for enabled in [false, true] {
        for mode in ["inherit", "direct", "proxy"] {
            let settings = json!({"updateProxy":{"enabled":enabled,"url":"http://127.0.0.1:7890"}});
            let config = HashMap::from([
                ("update_proxy_mode".into(), mode.into()),
                ("network_interface".into(), "missing-test-interface".into()),
            ]);
            let options = DDNSHttpClientOptions::from_settings_and_config(&settings, &config);
            let expected = mode == "proxy" || (mode == "inherit" && enabled);
            assert_eq!(options.update_proxy.is_some(), expected);
            if expected {
                // Proxy connections do not bind the address-source interface.
                assert!(ddns_http_client(&Translator::new("en"), &options).is_ok());
            }
        }
    }
    let options = DDNSHttpClientOptions::from_settings_and_config(
        &json!({}),
        &HashMap::from([("update_proxy_mode".into(), "proxy".into())]),
    );
    assert!(ddns_http_client(&Translator::new("en"), &options).is_err());
}

async fn read_headers<S: tokio::io::AsyncRead + Unpin>(socket: &mut S) -> String {
    let mut bytes = Vec::new();
    while !bytes.ends_with(b"\r\n\r\n") {
        bytes.push(socket.read_u8().await.unwrap());
        assert!(bytes.len() < 16384);
    }
    String::from_utf8(bytes).unwrap()
}

async fn reply<S: tokio::io::AsyncWrite + Unpin>(socket: &mut S, body: &str) {
    socket
        .write_all(
            format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .as_bytes(),
        )
        .await
        .unwrap();
}

fn options(transport: &str, address: &str) -> DDNSHttpClientOptions {
    DDNSHttpClientOptions::from_settings_and_config(
        &json!({
            "httpTransport":transport,"updateProxy":{"enabled":true,"url":address,"username":"proxy-user","password":"proxy-secret"}
        }),
        &HashMap::new(),
    )
}

#[tokio::test]
async fn update_proxy_http_query_and_write_use_proxy_without_local_dns() {
    for transport in ["node", "curl"] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let proxy = tokio::spawn(async move {
            for method in ["GET", "POST"] {
                let (mut socket, _) = listener.accept().await.unwrap();
                let headers = read_headers(&mut socket).await;
                assert!(headers.starts_with(&format!("{method} http://provider.invalid/records")));
                assert!(
                    headers.to_lowercase().contains(
                        "proxy-authorization: basic cHJveHktdXNlcjpwcm94eS1zZWNyZXQ="
                            .to_lowercase()
                            .as_str()
                    )
                );
                if method == "POST" {
                    let mut body = vec![0; "192.0.2.123".len()];
                    socket.read_exact(&mut body).await.unwrap();
                    assert_eq!(body, b"192.0.2.123");
                }
                reply(&mut socket, "ok").await;
            }
        });
        let client = ddns_http_client(
            &Translator::new("en"),
            &options(transport, &format!("http://{address}")),
        )
        .unwrap();
        assert!(
            client
                .get("http://provider.invalid/records")
                .send()
                .await
                .unwrap()
                .status()
                .is_success()
        );
        assert!(
            client
                .post("http://provider.invalid/records")
                .body("192.0.2.123")
                .send()
                .await
                .unwrap()
                .status()
                .is_success()
        );
        tokio_time::timeout(Duration::from_secs(5), proxy)
            .await
            .unwrap()
            .unwrap();
    }
}

#[tokio::test]
async fn update_proxy_socks_auth_and_remote_dns_preserve_ipv6_update_payload() {
    for transport in ["node", "curl"] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let proxy = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            assert_eq!(socket.read_u8().await.unwrap(), 5);
            let count = socket.read_u8().await.unwrap();
            let mut methods = vec![0; count as usize];
            socket.read_exact(&mut methods).await.unwrap();
            assert!(methods.contains(&2));
            socket.write_all(&[5, 2]).await.unwrap();
            assert_eq!(socket.read_u8().await.unwrap(), 1);
            for expected in [b"proxy-user".as_slice(), b"proxy-secret".as_slice()] {
                let len = socket.read_u8().await.unwrap();
                let mut value = vec![0; len as usize];
                socket.read_exact(&mut value).await.unwrap();
                assert_eq!(value, expected);
            }
            socket.write_all(&[1, 0]).await.unwrap();
            let mut head = [0; 4];
            socket.read_exact(&mut head).await.unwrap();
            assert_eq!(head, [5, 1, 0, 3]);
            let len = socket.read_u8().await.unwrap();
            let mut host = vec![0; len as usize];
            socket.read_exact(&mut host).await.unwrap();
            assert_eq!(host, b"provider.invalid");
            assert_eq!(socket.read_u16().await.unwrap(), 80);
            socket
                .write_all(&[5, 0, 0, 1, 127, 0, 0, 1, 0, 80])
                .await
                .unwrap();
            let headers = read_headers(&mut socket).await;
            assert!(headers.starts_with("POST /records"));
            assert!(!headers.to_lowercase().contains("proxy-authorization"));
            let mut body = vec![0; b"2001:db8::123".len()];
            socket.read_exact(&mut body).await.unwrap();
            assert_eq!(body, b"2001:db8::123");
            reply(&mut socket, "ok").await;
        });
        let client = ddns_http_client(
            &Translator::new("en"),
            &options(transport, &format!("socks5://{address}")),
        )
        .unwrap();
        assert!(
            client
                .post("http://provider.invalid/records")
                .body("2001:db8::123")
                .send()
                .await
                .unwrap()
                .status()
                .is_success()
        );
        tokio_time::timeout(Duration::from_secs(5), proxy)
            .await
            .unwrap()
            .unwrap();
    }
}

#[tokio::test]
async fn update_proxy_failed_connect_does_not_fall_back_or_leak_secrets() {
    for transport in ["node", "curl"] {
        let origin = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let unused = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = unused.local_addr().unwrap();
        drop(unused);
        let client = ddns_http_client(
            &Translator::new("en"),
            &options(transport, &format!("http://{address}")),
        )
        .unwrap();
        let error = client
            .get(format!("http://{}/records", origin.local_addr().unwrap()))
            .send()
            .await
            .err()
            .unwrap()
            .to_string();
        assert!(!error.contains("proxy-secret"));
        assert!(!error.contains("proxy-user"));
        assert!(
            tokio_time::timeout(Duration::from_millis(100), origin.accept())
                .await
                .is_err()
        );
    }
}

#[tokio::test]
async fn update_proxy_connect_tunnels_and_preserves_tls_verification() {
    for transport in ["node", "curl"] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let proxy = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let headers = read_headers(&mut socket).await;
            assert!(headers.starts_with("CONNECT provider.invalid:443"));
            socket
                .write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")
                .await
                .unwrap();
            // The tunneled connection must be TLS, never plaintext HTTP.
            assert_eq!(socket.read_u8().await.unwrap(), 22);
        });
        let client = ddns_http_client(
            &Translator::new("en"),
            &options(transport, &format!("http://{address}")),
        )
        .unwrap();
        assert!(
            client
                .get("https://provider.invalid/records")
                .send()
                .await
                .is_err()
        );
        tokio_time::timeout(Duration::from_secs(5), proxy)
            .await
            .unwrap()
            .unwrap();
    }
}

#[tokio::test]
async fn update_proxy_https_uses_tls_to_proxy() {
    for transport in ["node", "curl"] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let proxy = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            assert_eq!(socket.read_u8().await.unwrap(), 22);
        });
        let client = ddns_http_client(
            &Translator::new("en"),
            &options(transport, &format!("https://{address}")),
        )
        .unwrap();
        assert!(
            client
                .get("http://provider.invalid/records")
                .send()
                .await
                .is_err()
        );
        tokio_time::timeout(Duration::from_secs(5), proxy)
            .await
            .unwrap()
            .unwrap();
    }
}

#[tokio::test]
async fn update_proxy_settings_routes_mask_password_and_merge_partial_writes() {
    let (_directory, state) = ddns_test_state().await;
    let update: SettingsBody=serde_json::from_value(json!({"updateProxy":{"enabled":true,"url":"http://localhost:7890","username":"user","password":"stored-secret"}})).unwrap();
    let response = update_settings(State(state.clone()), Json(update)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains("stored-secret"));
    let interval: SettingsBody =
        serde_json::from_value(json!({"updateIntervalMinutes":15})).unwrap();
    assert_eq!(
        update_settings(State(state.clone()), Json(interval))
            .await
            .status(),
        StatusCode::OK
    );
    let raw = state
        .storage
        .store
        .get_string_value(DDNS_SETTINGS)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        DDNSUpdateProxy::from_settings(&parse_settings(Some(&raw))).password,
        "stored-secret"
    );
    let status = build_ddns_status(&state, &Translator::new("en"))
        .await
        .unwrap();
    assert!(!status.to_string().contains("stored-secret"));
    assert_eq!(status["updateProxy"]["passwordConfigured"], true);
    let response = get_settings(State(state)).await;
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains("stored-secret"));
}

#[tokio::test]
async fn update_proxy_public_check_test_is_direct_with_global_proxy_enabled() {
    let (_directory, state) = ddns_test_state().await;
    let unused_proxy = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    for transport in ["node", "curl"] {
        state.storage.store.set_string_value(DDNS_SETTINGS,&json!({"httpTransport":transport,"publicDnsProvider":"none","updateProxy":{"enabled":true,"url":format!("http://{}",unused_proxy.local_addr().unwrap())}}).to_string()).await.unwrap();
        let origin = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = origin.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = origin.accept().await.unwrap();
            read_headers(&mut socket).await;
            reply(&mut socket, "192.0.2.123").await;
        });
        let body = serde_json::from_value(
            json!({"publicCheckSources":{"ipv4":[format!("http://{address}/ip")],"ipv6":[]}}),
        )
        .unwrap();
        let response = test_public_check_sources(State(state.clone()), Json(body)).await;
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["data"]["results"][0]["ip"], "192.0.2.123");
        server.await.unwrap();
    }
    assert!(
        tokio_time::timeout(Duration::from_millis(100), unused_proxy.accept())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn update_proxy_address_resolution_for_updates_stays_direct() {
    for transport in ["node", "curl"] {
        let unused_proxy = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = origin.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = origin.accept().await.unwrap();
            read_headers(&mut socket).await;
            reply(&mut socket, "192.0.2.123").await;
        });
        let settings = json!({"httpTransport":transport,"publicDnsProvider":"none",
            "publicCheckSources":{"ipv4":[format!("http://{address}/ip")],"ipv6":[]},
            "updateProxy":{"enabled":true,"url":format!("http://{}",unused_proxy.local_addr().unwrap())}});
        let mut target = default_primary_target();
        target
            .config
            .insert("update_scope".into(), "ipv4_only".into());
        let ips = resolve_target_ips(&target, &settings, &Translator::new("en"))
            .await
            .unwrap();
        assert_eq!(ips.ipv4.as_deref(), Some("192.0.2.123"));
        assert_eq!(ips.ipv6, None);
        server.await.unwrap();
        assert!(
            tokio_time::timeout(Duration::from_millis(100), unused_proxy.accept())
                .await
                .is_err()
        );
    }
}

#[tokio::test]
async fn update_proxy_target_saves_require_configuration() {
    let (_directory, state) = ddns_test_state().await;
    let config = HashMap::from([("update_proxy_mode".into(), "proxy".into())]);
    assert!(validate_target_update_proxy(&state, &config).await.is_err());
    state
        .storage
        .store
        .set_string_value(
            DDNS_SETTINGS,
            r#"{"updateProxy":{"url":"http://127.0.0.1:7890","enabled":false}}"#,
        )
        .await
        .unwrap();
    assert!(validate_target_update_proxy(&state, &config).await.is_ok());
    assert!(
        validate_target_update_proxy(
            &state,
            &HashMap::from([("update_proxy_mode".into(), "invalid".into())])
        )
        .await
        .is_err()
    );
}

#[tokio::test]
async fn update_proxy_never_uses_provider_address_autodetection() {
    assert_eq!(
        noip_ip_parameter(Some("192.0.2.1"), None).as_deref(),
        Some("192.0.2.1")
    );
    assert_eq!(
        noip_ip_parameter(None, Some("2001:db8::1")).as_deref(),
        Some("2001:db8::1")
    );
    assert_eq!(
        noip_ip_parameter(Some("192.0.2.1"), Some("2001:db8::1")).as_deref(),
        Some("192.0.2.1,2001:db8::1")
    );
    for prefix in ["auto", "6to4"] {
        let config = HashMap::from([
            ("token".into(), "offline-token".into()),
            ("zone".into(), "test.invalid".into()),
            ("ipv6prefix".into(), prefix.into()),
        ]);
        let result = update_dynv6(
            &Translator::new("en"),
            &config,
            &options("node", "http://127.0.0.1:1"),
            None,
            Some("2001:db8::1"),
        )
        .await;
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("explicit IPv6 prefix")
        );
    }
}

fn untrusted_tls_acceptor() -> tokio_rustls::TlsAcceptor {
    let cert =
        rcgen::generate_simple_self_signed(vec!["localhost".into(), "provider.invalid".into()])
            .unwrap();
    let key = rustls::pki_types::PrivatePkcs8KeyDer::from(cert.signing_key.serialize_der());
    let config = rustls::ServerConfig::builder_with_provider(std::sync::Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(vec![cert.cert.der().clone()], key.into())
    .unwrap();
    tokio_rustls::TlsAcceptor::from(std::sync::Arc::new(config))
}

#[tokio::test]
async fn update_proxy_rejects_untrusted_proxy_and_origin_certificates() {
    for transport in ["node", "curl"] {
        for tls_proxy in [false, true] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let port = listener.local_addr().unwrap().port();
            let acceptor = untrusted_tls_acceptor();
            let server = tokio::spawn(async move {
                let (mut socket, _) = listener.accept().await.unwrap();
                if !tls_proxy {
                    let headers = read_headers(&mut socket).await;
                    assert!(headers.starts_with("CONNECT provider.invalid:443"));
                    socket
                        .write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")
                        .await
                        .unwrap();
                }
                // No test CA is trusted: both proxy TLS and tunneled origin TLS must reject it.
                if let Ok(mut tls) = acceptor.accept(socket).await {
                    let mut first = [0];
                    assert!(
                        tls.read(&mut first).await.is_err(),
                        "client accepted an untrusted certificate"
                    );
                }
            });
            let scheme = if tls_proxy { "https" } else { "http" };
            let client = ddns_http_client(
                &Translator::new("en"),
                &options(transport, &format!("{scheme}://127.0.0.1:{port}")),
            )
            .unwrap();
            assert!(
                client
                    .get("https://provider.invalid/records")
                    .send()
                    .await
                    .is_err()
            );
            tokio_time::timeout(Duration::from_secs(5), server)
                .await
                .unwrap()
                .unwrap();
        }
    }
}

#[tokio::test]
async fn update_proxy_authentication_failures_are_reported_without_credentials() {
    for transport in ["node", "curl"] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            read_headers(&mut socket).await;
            socket.write_all(b"HTTP/1.1 407 Proxy Authentication Required\r\nContent-Length: 12\r\nConnection: close\r\n\r\nproxy-secret").await.unwrap();
        });
        let client = ddns_http_client(
            &Translator::new("en"),
            &options(transport, &format!("http://{address}")),
        )
        .unwrap();
        let error = client
            .get("http://provider.invalid/records")
            .send()
            .await
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("authentication failed"));
        assert!(!error.contains("proxy-secret"));
        server.await.unwrap();
    }
    assert_eq!(proxy_curl_error_key(Some(4)), "updateProxyUnsupported");
    assert_eq!(proxy_curl_error_key(Some(28)), "updateProxyTimeout");
    assert_eq!(proxy_curl_error_key(Some(60)), "updateProxyTlsFailed");
}

#[path = "update_proxy_audit_tests.rs"]
mod audit_tests;
