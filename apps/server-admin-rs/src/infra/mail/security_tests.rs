use super::*;
use std::sync::Arc;
use tokio::{io::AsyncWriteExt, net::TcpListener};
use tokio_rustls::TlsAcceptor;

async fn tls_fixture() -> (TcpListener, TlsAcceptor, reqwest::Certificate) {
    let certificate = rcgen::generate_simple_self_signed(vec!["127.0.0.1".into()]).unwrap();
    let root = reqwest::Certificate::from_der(certificate.cert.der()).unwrap();
    let key =
        rustls::pki_types::PrivateKeyDer::Pkcs8(certificate.signing_key.serialize_der().into());
    let config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(vec![certificate.cert.der().clone()], key)
    .unwrap();
    (
        TcpListener::bind("127.0.0.1:0").await.unwrap(),
        TlsAcceptor::from(Arc::new(config)),
        root,
    )
}

#[tokio::test]
async fn security_tls_https_requires_a_trusted_certificate() {
    let (listener, acceptor, root) = tls_fixture().await;
    let url = format!("https://{}/", listener.local_addr().unwrap());
    let server = async move {
        for trusted in [false, true] {
            let (stream, _) = listener.accept().await.unwrap();
            let handshake = acceptor.accept(stream).await;
            assert_eq!(handshake.is_ok(), trusted);
            if let Ok(mut tls) = handshake {
                tls.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
                )
                .await
                .unwrap();
                tls.shutdown().await.unwrap();
            }
        }
    };
    let client = async {
        let untrusted = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap()
            .get(&url)
            .send()
            .await
            .unwrap_err();
        assert!(untrusted.is_connect());
        let response = reqwest::Client::builder()
            .no_proxy()
            .add_root_certificate(root)
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap()
            .get(&url)
            .send()
            .await
            .unwrap();
        assert_eq!(response.text().await.unwrap(), "ok");
    };
    tokio::time::timeout(Duration::from_secs(10), async {
        tokio::join!(server, client);
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn security_tls_smtp_rejects_untrusted_certificate_before_authentication() {
    let (listener, acceptor, _) = tls_fixture().await;
    let config = SmtpConfig {
        host: "127.0.0.1".into(),
        port: listener.local_addr().unwrap().port(),
        username: "must-not-be-sent".into(),
        ..SmtpConfig::default()
    };
    let server = async move {
        let (stream, _) = listener.accept().await.unwrap();
        assert!(acceptor.accept(stream).await.is_err());
    };
    let mailer = transport(&config, "must-not-be-sent").unwrap();
    let client = async {
        assert!(mailer.test_connection().await.is_err());
    };
    tokio::time::timeout(Duration::from_secs(5), async {
        tokio::join!(server, client);
    })
    .await
    .unwrap();
}
