use super::*;
use std::os::unix::fs::PermissionsExt;
use tokio::io::AsyncReadExt;

async fn headers<S: tokio::io::AsyncRead + Unpin>(socket: &mut S) -> String {
    let mut bytes = Vec::new();
    while !bytes.ends_with(b"\r\n\r\n") {
        bytes.push(socket.read_u8().await.unwrap());
        assert!(bytes.len() < 16384);
    }
    String::from_utf8(bytes).unwrap()
}

#[tokio::test]
async fn update_proxy_trusted_tls_roundtrips_for_connect_and_https_proxy() {
    // The cURL trust wrapper changes PATH. Run it in a dedicated process so
    // parallel DDNS transport tests cannot inherit a temporary certificate path.
    const CHILD: &str = "FN_KNOCK_DDNS_TLS_TEST_CHILD";
    if env::var_os(CHILD).is_none() {
        let module = module_path!().split_once("::").unwrap().1;
        let test =
            format!("{module}::update_proxy_trusted_tls_roundtrips_for_connect_and_https_proxy");
        let output = tokio::process::Command::new(env::current_exe().unwrap())
            .args(["--exact", &test, "--nocapture"])
            .env(CHILD, "1")
            .output()
            .await
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
        return;
    }
    let certificate =
        rcgen::generate_simple_self_signed(vec!["localhost".into(), "provider.invalid".into()])
            .unwrap();
    let tls = rustls::ServerConfig::builder_with_provider(std::sync::Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(
        vec![certificate.cert.der().clone()],
        rustls::pki_types::PrivatePkcs8KeyDer::from(certificate.signing_key.serialize_der()).into(),
    )
    .unwrap();
    let acceptor = tokio_rustls::TlsAcceptor::from(std::sync::Arc::new(tls));
    let root = reqwest::Certificate::from_pem(certificate.cert.pem().as_bytes()).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let ca = directory.path().join("ca.pem");
    std::fs::write(&ca, certificate.cert.pem()).unwrap();
    // Give the subprocess only our ephemeral test trust root. Production still uses
    // normal certificate verification and the exact same request execution path.
    let environment = crate::test_support::EnvGuard::new(&["PATH"]);
    let original_path = env::var_os("PATH").unwrap();
    let curl = env::split_paths(&original_path)
        .map(|dir| dir.join("curl"))
        .find(|file| file.is_file())
        .unwrap()
        .canonicalize()
        .unwrap();
    let quote =
        |path: &std::path::Path| format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"));
    let wrapper = directory.path().join("curl");
    std::fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\nexec {} \"$@\" --cacert {} --proxy-cacert {}\n",
            quote(&curl),
            quote(&ca),
            quote(&ca)
        ),
    )
    .unwrap();
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o700)).unwrap();
    let paths =
        std::iter::once(directory.path().to_path_buf()).chain(env::split_paths(&original_path));
    environment.set("PATH", env::join_paths(paths).unwrap());

    for transport in ["node", "curl"] {
        for https_proxy in [false, true] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let port = listener.local_addr().unwrap().port();
            let acceptor = acceptor.clone();
            let expected_auth = format!(
                "proxy-authorization: Basic {}",
                BASE64_STANDARD.encode("user:p@ss:word\\\"%")
            );
            let proxy = tokio::spawn(async move {
                let (mut socket, _) = listener.accept().await.unwrap();
                if !https_proxy {
                    let connect = headers(&mut socket).await;
                    assert!(connect.starts_with("CONNECT provider.invalid:443"));
                    assert!(
                        connect
                            .lines()
                            .any(|line| line.eq_ignore_ascii_case(&expected_auth))
                    );
                    socket
                        .write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")
                        .await
                        .unwrap();
                }
                let mut socket = acceptor.accept(socket).await.unwrap();
                let request = headers(&mut socket).await;
                if https_proxy {
                    assert!(request.starts_with("POST http://provider.invalid/records"));
                    assert!(
                        request
                            .lines()
                            .any(|line| line.eq_ignore_ascii_case(&expected_auth))
                    );
                } else {
                    assert!(request.starts_with("POST /records"));
                    assert!(!request.to_lowercase().contains("proxy-authorization"));
                }
                assert!(
                    request
                        .to_lowercase()
                        .contains("authorization: bearer provider-token")
                );
                let mut body = vec![0; b"2001:db8::123".len()];
                socket.read_exact(&mut body).await.unwrap();
                assert_eq!(body, b"2001:db8::123");
                socket
                    .write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
                    )
                    .await
                    .unwrap();
                socket.shutdown().await.unwrap();
            });
            let scheme = if https_proxy { "https" } else { "http" };
            let mut proxy_config = DDNSUpdateProxy {
                enabled: true,
                url: format!("{scheme}://localhost:{port}"),
                username: "user".into(),
                password: "p@ss:word\\\"%".into(),
            };
            proxy_config.validate(true).unwrap();
            let options = DDNSHttpClientOptions {
                transport,
                update_proxy: Some(proxy_config.clone()),
                ..Default::default()
            };
            let mut client = ddns_http_client(&Translator::new("en"), &options).unwrap();
            if transport == "node" {
                // Add only test trust, keeping the production transport builder and proxy.
                client.reqwest_attempts = vec![
                    ddns_reqwest_client_builder(5_000, true)
                        .proxy(proxy_config.reqwest_proxy().unwrap())
                        .tls_certs_merge([root.clone()])
                        .build()
                        .unwrap(),
                ];
            }
            let origin_scheme = if https_proxy { "http" } else { "https" };
            let response = client
                .post(format!("{origin_scheme}://provider.invalid/records"))
                .bearer_auth("provider-token")
                .body("2001:db8::123")
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), reqwest::StatusCode::OK);
            assert_eq!(response_text(response).await.unwrap(), "ok");
            tokio::time::timeout(Duration::from_secs(5), proxy)
                .await
                .unwrap()
                .unwrap();
        }
    }
}
