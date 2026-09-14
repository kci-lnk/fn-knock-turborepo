//! Opt-in real SSH fixture. See README.md for the isolated Dropbear container.
use super::super::{
    domain::{AuthMethod, TargetRecord, TrustedHostKey},
    ssh,
};
use super::*;

#[tokio::test]
#[ignore = "requires the localhost Dropbear fixture and FN_TERMINAL_SSH_TEST_PORT"]
async fn busybox_dropbear_metrics_keep_interactive_shell_usable() {
    let port: u16 = std::env::var("FN_TERMINAL_SSH_TEST_PORT")
        .unwrap()
        .parse()
        .unwrap();
    let probe = ssh::probe_host_key("127.0.0.1", port).await.unwrap();
    let target = TargetRecord {
        id: "metrics-fixture".into(),
        name: "metrics-fixture".into(),
        host: "127.0.0.1".into(),
        port,
        username: "metrics".into(),
        auth_method: AuthMethod::Password,
        trusted_host_key: Some(TrustedHostKey {
            algorithm: probe.algorithm,
            fingerprint: probe.fingerprint,
        }),
        revision: 1,
        last_verified_at: None,
        created_at: String::new(),
        updated_at: String::new(),
    };
    let mut shell = ssh::open_shell(
        &target,
        ssh::SshCredential::Password("metrics-fixture".into()),
        80,
        24,
        None,
    )
    .await
    .unwrap();
    let collector = shell.metrics_collector().unwrap();
    let cancel = CancellationToken::new();
    let mut previous = None;
    let first = collector.collect(&cancel).await.unwrap();
    let first = parser::parse(&first, &mut previous);
    assert_eq!(first.platform, MetricsPlatform::Linux);
    assert_eq!(first.cpu.reason, Some(MetricReason::WarmingUp));
    tokio::time::sleep(Duration::from_millis(100)).await;
    let second = collector.collect(&cancel).await.unwrap();
    let second = parser::parse(&second, &mut previous);
    assert_eq!(second.status, MetricsStatus::Available);
    assert!(second.memory.total_bytes.unwrap() > 0);
    assert!(second.disk.total_bytes.unwrap() > 0);
    let detail = collector.collect_disks(&cancel).await.unwrap();
    let detail = disks::parse(&detail);
    assert_eq!(detail.status, MetricsStatus::Available);
    assert!(detail.disks.iter().any(|disk| disk.mount_point == "/"));
    assert!(detail.disks.len() > 1);
    shell.resize(100, 30).await.unwrap();
    shell
        .input(b"printf 'interactive-%s\\n' ok\r".to_vec())
        .await
        .unwrap();
    let output = tokio::time::timeout(Duration::from_secs(3), async {
        let mut output = Vec::new();
        loop {
            if let super::super::shell::ShellEvent::Data(bytes) = shell.next_event().await {
                output.extend(bytes);
                if String::from_utf8_lossy(&output).contains("interactive-ok") {
                    break output;
                }
            }
        }
    })
    .await
    .unwrap();
    assert!(!String::from_utf8_lossy(&output).contains("__FN_METRIC_"));
    shell.close().await;
    shell.disconnect().await;
}
