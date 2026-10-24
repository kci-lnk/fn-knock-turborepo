use super::*;

#[test]
fn update_proxy_curl_preserves_default_and_ipv6_ports() {
    for (input, expected) in [
        ("http://localhost", "http://localhost:80"),
        ("http://localhost:80", "http://localhost:80"),
        ("https://localhost:443", "https://localhost:443"),
        ("http://[::1]:80", "http://[::1]:80"),
        ("socks5://[::1]:1080", "socks5h://[::1]:1080"),
    ] {
        let proxy = merge_proxy(&DDNSUpdateProxy::default(), json!({"url":input})).unwrap();
        assert!(
            proxy
                .curl_config()
                .unwrap()
                .starts_with(&format!("proxy = \"{expected}\"\n"))
        );
    }
}

#[tokio::test]
async fn update_proxy_connect_and_socks_authentication_errors_are_specific() {
    for transport in ["node", "curl"] {
        for socks in [false, true] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let server = tokio::spawn(async move {
                let (mut socket, _) = listener.accept().await.unwrap();
                if socks {
                    assert_eq!(socket.read_u8().await.unwrap(), 5);
                    let count = socket.read_u8().await.unwrap();
                    let mut methods = vec![0; count as usize];
                    socket.read_exact(&mut methods).await.unwrap();
                    socket.write_all(&[5, 2]).await.unwrap();
                    assert_eq!(socket.read_u8().await.unwrap(), 1);
                    for _ in 0..2 {
                        let len = socket.read_u8().await.unwrap();
                        let mut credential = vec![0; len as usize];
                        socket.read_exact(&mut credential).await.unwrap();
                    }
                    socket.write_all(&[1, 1]).await.unwrap();
                } else {
                    assert!(read_headers(&mut socket).await.starts_with("CONNECT "));
                    socket.write_all(b"HTTP/1.1 407 Proxy Authentication Required\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await.unwrap();
                }
            });
            let scheme = if socks { "socks5" } else { "http" };
            let client = ddns_http_client(
                &Translator::new("en"),
                &options(transport, &format!("{scheme}://{address}")),
            )
            .unwrap();
            let error = client
                .get("https://provider.invalid/records")
                .send()
                .await
                .err()
                .unwrap()
                .to_string();
            assert!(
                error.contains("authentication failed"),
                "{transport} {scheme}: {error}"
            );
            assert!(!error.contains("proxy-secret"));
            tokio_time::timeout(Duration::from_secs(5), server)
                .await
                .unwrap()
                .unwrap();
        }
    }
}

#[tokio::test]
async fn update_proxy_activation_revalidates_saved_target_configuration() {
    let (_directory, state) = ddns_test_state().await;
    let config = HashMap::from([("update_proxy_mode".into(), "proxy".into())]);
    // Historical drafts can outlive the configured proxy address.
    save_legacy_config_draft(&state, "duckdns", &config)
        .await
        .unwrap();
    assert!(set_primary_provider(&state, "duckdns").await.is_err());
    assert_ne!(
        primary_target(&state)
            .await
            .unwrap()
            .meta
            .provider
            .as_deref(),
        Some("duckdns")
    );
    state
        .storage
        .store
        .set_string_value(
            DDNS_SETTINGS,
            r#"{"updateProxy":{"url":"http://localhost:7890"}}"#,
        )
        .await
        .unwrap();
    set_primary_provider(&state, "duckdns").await.unwrap();
    let created = create_ddns_target(
        &state,
        TargetBody {
            name: Some("extra".into()),
            provider: "dynv6".into(),
            enabled: Some(false),
            config: Some(config),
        },
        &Translator::new("en"),
    )
    .await
    .unwrap();
    let id = created["id"].as_str().unwrap();
    state
        .storage
        .store
        .set_string_value(DDNS_SETTINGS, "{}")
        .await
        .unwrap();
    assert!(set_primary_provider(&state, "duckdns").await.is_err());
    assert!(set_ddns_target_enabled(&state, id, true).await.is_err());
    assert!(!find_target_or_err(&state, id).await.unwrap().meta.enabled);
    // Disabling still works when the proxy has been removed.
    set_ddns_target_enabled(&state, id, false).await.unwrap();
}

#[tokio::test]
async fn update_proxy_cancelled_curl_removes_credentials_and_stops_request() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let marker = uuid::Uuid::new_v4().to_string();
    let mut options = options("curl", &format!("http://{address}"));
    options.update_proxy.as_mut().unwrap().username = marker.clone();
    let client = ddns_http_client(&Translator::new("en"), &options).unwrap();
    let request =
        tokio::spawn(async move { client.get("http://provider.invalid/records").send().await });
    let (mut socket, _) = tokio_time::timeout(Duration::from_secs(5), listener.accept())
        .await
        .unwrap()
        .unwrap();
    read_headers(&mut socket).await;
    let directory = std::fs::read_dir(env::temp_dir())
        .unwrap()
        .filter_map(Result::ok)
        .find_map(|entry| {
            let path = entry.path();
            if !entry
                .file_name()
                .to_string_lossy()
                .starts_with("ddns-curl-")
            {
                return None;
            }
            std::fs::read_to_string(path.join("proxy.conf"))
                .ok()
                .filter(|text| text.contains(&marker))
                .map(|_| path)
        })
        .expect("in-flight request must have a private proxy config");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&directory).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            std::fs::metadata(directory.join("proxy.conf"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    request.abort();
    assert!(request.await.err().unwrap().is_cancelled());
    assert!(
        !directory.exists(),
        "credentials survived request cancellation"
    );
    assert_eq!(
        tokio_time::timeout(Duration::from_secs(5), socket.read_u8())
            .await
            .unwrap()
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::UnexpectedEof
    );
}
