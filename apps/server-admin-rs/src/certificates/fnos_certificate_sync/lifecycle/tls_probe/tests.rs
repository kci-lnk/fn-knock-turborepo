use super::*;
use rcgen::generate_simple_self_signed;
use std::{
    net::TcpListener,
    os::unix::process::ExitStatusExt,
    sync::{Arc, Mutex},
};

fn expected() -> (String, ParsedCertificate) {
    let pem = generate_simple_self_signed(vec!["sync.example.test".into()])
        .unwrap()
        .cert
        .pem();
    let parsed = parse_certificate(&pem).unwrap();
    (pem, parsed)
}

fn output(code: i32, stdout: &str, stderr: &str, timed_out: bool) -> ProbeOutput {
    ProbeOutput {
        status: ExitStatus::from_raw(code << 8),
        stdout: stdout.as_bytes().to_vec(),
        stderr: stderr.as_bytes().to_vec(),
        timed_out,
    }
}

#[test]
fn reads_each_machines_https_port_without_guessing() {
    for port in [443, 5667, 19123, 65535] {
        assert_eq!(
            parse_port(&serde_json::to_vec(&json!({"schema":{"https":{"port":port}}})).unwrap())
                .unwrap(),
            port
        );
    }
    for bytes in [
        "",
        "not-json",
        "null",
        "{}",
        r#"{"schema":{"http":{"port":443}}}"#,
        r#"{"schema":{"https":{"port":0}}}"#,
        r#"{"schema":{"https":{"port":65536}}}"#,
        r#"{"schema":{"https":{"port":-1}}}"#,
        r#"{"schema":{"https":{"port":"443"}}}"#,
        r#"{"schema":{"https":{"port":443.5}}}"#,
        r#"{"schema":{"https":{"port":null}}}"#,
    ] {
        assert!(parse_port(bytes.as_bytes()).is_err(), "accepted {bytes}");
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gateway.conf");
    assert!(
        read_port(&path)
            .unwrap_err()
            .to_string()
            .contains("Cannot read fnOS HTTPS port")
    );
    fs::write(
        &path,
        br#"{"schema":{"https":{"port":5667}},"redirect":false}"#,
    )
    .unwrap();
    assert_eq!(read_port(&path).unwrap(), 5667);
    fs::write(&path, vec![b' '; SETTINGS_MAX_BYTES as usize + 1]).unwrap();
    assert!(read_port(&path).unwrap_err().to_string().contains("64 KiB"));
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&path, &link).unwrap();
    assert!(read_port(&link).is_err());
    assert!(read_port(dir.path()).is_err());
}

#[test]
fn retries_transient_failure_or_old_certificate_and_stops_on_success() {
    let (pem, cert) = expected();
    let (old_pem, _) = expected();
    for first in [
        output(
            1,
            "",
            "BIO_connect:Connection refused\nconnect:errno=111",
            false,
        ),
        output(1, "", "error:SSL routines:wrong version number", false),
        output(0, "", "", false),
        output(0, &old_pem, "", false),
        output(1, "", "", true),
    ] {
        let mut outputs = vec![first, output(0, &pem, "", false)].into_iter();
        let mut attempts = 0;
        let mut delays = Vec::new();
        verify_with_retry(
            "fallback",
            5667,
            &cert,
            || {
                attempts += 1;
                Ok(outputs.next().expect("no probe after success"))
            },
            |delay| delays.push(delay),
        )
        .unwrap();
        assert_eq!(attempts, 2);
        assert_eq!(delays, vec![RETRY_DELAY]);
    }
    let mut attempts = 0;
    verify_with_retry(
        "sync.example.test",
        5667,
        &cert,
        || {
            attempts += 1;
            Ok(output(0, &pem, "", false))
        },
        |_| panic!("successful probe must not sleep"),
    )
    .unwrap();
    assert_eq!(attempts, 1);
}

#[test]
fn sustained_failures_include_target_stage_exit_status_and_bounded_retries() {
    let (_, cert) = expected();
    let (wrong_pem, _) = expected();
    for (code, pem, stderr, timed_out, stage) in [
        (
            1,
            "",
            "BIO_connect:Connection refused\nconnect:errno=111",
            false,
            "connection stage",
        ),
        (
            1,
            "",
            "error:SSL routines:wrong version number",
            false,
            "handshake/process stage",
        ),
        (1, "", "", true, "timeout stage"),
        (0, "", "", false, "certificate stage"),
        (
            0,
            "-----BEGIN CERTIFICATE-----",
            "",
            false,
            "incomplete TLS certificate",
        ),
        (
            0,
            "-----BEGIN CERTIFICATE-----\ninvalid\n-----END CERTIFICATE-----",
            "",
            false,
            "invalid TLS certificate",
        ),
        (0, wrong_pem.as_str(), "", false, "fingerprint mismatch"),
    ] {
        let mut attempts = 0;
        let mut delays = Vec::new();
        let error = verify_with_retry(
            "fallback",
            5667,
            &cert,
            || {
                attempts += 1;
                Ok(output(code, pem, stderr, timed_out))
            },
            |delay| delays.push(delay),
        )
        .unwrap_err()
        .to_string();
        assert_eq!(attempts, 3);
        assert_eq!(delays, vec![RETRY_DELAY; 2]);
        for part in [
            stage,
            "127.0.0.1:5667",
            "fallback (no SNI)",
            "exit status:",
            "after 3 attempts",
        ] {
            assert!(error.contains(part), "{part}: {error}");
        }
        assert!(!error.contains("BEGIN CERTIFICATE"));
    }
}

#[test]
fn diagnostic_never_returns_pem_or_unbounded_control_characters() {
    let stderr = format!(
        "depth=0 CN=hidden.example\n-----BEGIN PRIVATE KEY-----\nerror secret-key\n-----END PRIVATE KEY-----\n-----BEGIN CERTIFICATE-----\nerror secret-cert\n-----END CERTIFICATE-----\nerror: {}\nerror: failure\0\r\n{}",
        "A".repeat(128),
        "error: harmless detail\n".repeat(200)
    );
    let summary = diagnostic(stderr.as_bytes());
    for absent in [
        "PRIVATE KEY",
        "CERTIFICATE",
        "secret-",
        "hidden.example",
        &"A".repeat(128),
    ] {
        assert!(!summary.contains(absent));
    }
    assert!(!summary.chars().any(char::is_control));
    assert!(summary.chars().count() <= 1024);
    assert!(summary.contains("[redacted]"));
    assert!(summary.contains("failure"));
}

fn openssl() -> Command {
    // macOS ships LibreSSL without -noservername. Production fnOS uses OpenSSL 3.
    #[cfg(target_os = "macos")]
    for path in [
        "/opt/homebrew/opt/openssl@3/bin/openssl",
        "/usr/local/opt/openssl@3/bin/openssl",
    ] {
        if Path::new(path).is_file() {
            return Command::new(path);
        }
    }
    Command::new("openssl")
}

#[derive(Debug)]
struct Resolver {
    key: Arc<rustls::sign::CertifiedKey>,
    names: Arc<Mutex<Vec<Option<String>>>>,
}
impl rustls::server::ResolvesServerCert for Resolver {
    fn resolve(
        &self,
        hello: rustls::server::ClientHello<'_>,
    ) -> Option<Arc<rustls::sign::CertifiedKey>> {
        self.names
            .lock()
            .unwrap()
            .push(hello.server_name().map(str::to_owned));
        Some(self.key.clone())
    }
}

#[test]
fn isolated_custom_port_checks_leaf_fingerprint_and_actual_sni() {
    let cert = generate_simple_self_signed(vec!["sync.example.test".into()]).unwrap();
    let expected = parse_certificate(&cert.cert.pem()).unwrap();
    let key = rustls::pki_types::PrivateKeyDer::Pkcs8(cert.signing_key.serialize_der().into());
    let key = rustls::crypto::ring::sign::any_supported_type(&key).unwrap();
    let key = Arc::new(rustls::sign::CertifiedKey::new(
        vec![cert.cert.der().clone()],
        key,
    ));
    let names = Arc::new(Mutex::new(Vec::new()));
    let config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_cert_resolver(Arc::new(Resolver {
        key,
        names: names.clone(),
    }));
    let config = Arc::new(config);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    assert_ne!(port, 443);
    listener.set_nonblocking(true).unwrap();
    let server = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(10);
        for _ in 0..2 {
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(e)
                        if e.kind() == std::io::ErrorKind::WouldBlock
                            && Instant::now() < deadline =>
                    {
                        std::thread::sleep(Duration::from_millis(5))
                    }
                    Err(e) => panic!("TLS fixture accept: {e}"),
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut connection = rustls::ServerConnection::new(config.clone()).unwrap();
            while connection.is_handshaking() {
                connection.complete_io(&mut stream).unwrap();
            }
            connection.send_close_notify();
            let _ = connection.write_tls(&mut stream);
        }
    });
    for host in ["fallback", "sync.example.test"] {
        let mut command = openssl();
        configure_command(&mut command, host, port);
        let output = run_probe(&mut command, PROBE_TIMEOUT).unwrap();
        check_output(&output, &expected).unwrap();
    }
    server.join().unwrap();
    assert_eq!(
        *names.lock().unwrap(),
        vec![None, Some("sync.example.test".into())]
    );
}

#[test]
fn actual_process_reports_connection_refusal_and_handshake_failure() {
    let (_, expected) = expected();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let mut command = openssl();
    configure_command(&mut command, "fallback", port);
    let output = run_probe(&mut command, PROBE_TIMEOUT).unwrap();
    assert!(
        check_output(&output, &expected)
            .unwrap_err()
            .to_string()
            .contains("connection stage")
    );

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    listener.set_nonblocking(true).unwrap();
    let server = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    stream.set_nonblocking(false).unwrap();
                    stream
                        .set_write_timeout(Some(Duration::from_secs(1)))
                        .unwrap();
                    let _ =
                        stream.write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n");
                    break;
                }
                Err(e)
                    if e.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(5))
                }
                Err(e) => panic!("HTTP fixture accept: {e}"),
            }
        }
    });
    let mut command = openssl();
    configure_command(&mut command, "sync.example.test", port);
    let output = run_probe(&mut command, PROBE_TIMEOUT).unwrap();
    server.join().unwrap();
    assert!(
        check_output(&output, &expected)
            .unwrap_err()
            .to_string()
            .contains("handshake/process stage")
    );
}

#[test]
fn bounded_capture_drains_remaining_bytes_and_retries_interrupted_reads() {
    struct InterruptedOnce {
        interrupted: bool,
        data: std::io::Cursor<Vec<u8>>,
    }
    impl Read for InterruptedOnce {
        fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
            if !self.interrupted {
                self.interrupted = true;
                return Err(std::io::ErrorKind::Interrupted.into());
            }
            self.data.read(bytes)
        }
    }
    let size = CAPTURE_MAX_BYTES as usize * 3;
    let data = (0..size).map(|i| (i % 251) as u8).collect::<Vec<_>>();
    let mut reader = InterruptedOnce {
        interrupted: false,
        data: std::io::Cursor::new(data.clone()),
    };
    let captured = capture_output(&mut reader).unwrap();
    assert_eq!(captured, data[..CAPTURE_MAX_BYTES as usize]);
    assert_eq!(reader.data.position(), size as u64);
}

#[test]
fn matching_certificate_does_not_hide_failed_or_timed_out_handshake() {
    let (pem, expected) = expected();
    assert!(
        check_output(
            &output(1, &pem, "error: handshake failed", false),
            &expected
        )
        .is_err()
    );
    assert!(check_output(&output(0, &pem, "", true), &expected).is_err());
}

#[test]
fn deadline_kills_and_reaps_process_and_pipe_output_cannot_deadlock() {
    let dir = tempfile::tempdir().unwrap();
    let pid_path = dir.path().join("pid");
    let mut command = Command::new("sh");
    command
        .args(["-c", "echo $$ > \"$1\"; exec sleep 20", "probe-test"])
        .arg(&pid_path);
    let started = Instant::now();
    let output = run_probe(&mut command, Duration::from_millis(150)).unwrap();
    assert!(output.timed_out);
    assert!(started.elapsed() < Duration::from_secs(3));
    let pid = fs::read_to_string(pid_path)
        .unwrap()
        .trim()
        .parse::<i32>()
        .unwrap();
    // waitpid reports ECHILD only once the timed-out direct child has been reaped.
    assert_eq!(
        unsafe { libc::waitpid(pid, std::ptr::null_mut(), libc::WNOHANG) },
        -1
    );
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ECHILD)
    );

    let mut command = Command::new("sh");
    command.args(["-c", "i=0; while [ $i -lt 10000 ]; do echo 'probe stdout padding'; echo 'error: stderr padding' >&2; i=$((i + 1)); done"]);
    let output = run_probe(&mut command, Duration::from_secs(5)).unwrap();
    assert!(output.status.success());
    assert!(!output.timed_out);
    assert_eq!(output.stdout.len(), CAPTURE_MAX_BYTES as usize);
    assert_eq!(output.stderr.len(), CAPTURE_MAX_BYTES as usize);

    // An endless producer must still time out even after both capture buffers fill.
    let mut command = Command::new("sh");
    command.args([
        "-c",
        "while :; do echo 'probe stdout padding'; echo 'error: stderr padding' >&2; done",
    ]);
    let started = Instant::now();
    let output = run_probe(&mut command, Duration::from_millis(200)).unwrap();
    assert!(output.timed_out);
    assert!(started.elapsed() < Duration::from_secs(3));
    assert!(output.stdout.len() <= CAPTURE_MAX_BYTES as usize);
    assert!(output.stderr.len() <= CAPTURE_MAX_BYTES as usize);
}
