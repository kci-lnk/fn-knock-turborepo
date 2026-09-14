use super::super::ssh::HostKeyHandler;
use super::{MetricReason, MetricsCollector, OUTPUT_LIMIT, SCRIPT, TIMEOUT};
use async_trait::async_trait;
use russh::{ChannelMsg, client};
use std::{sync::Arc, time::Duration};
use tokio_util::sync::CancellationToken;

pub(in crate::system::terminal) struct SshCollector(pub Arc<client::Handle<HostKeyHandler>>);

#[async_trait]
impl MetricsCollector for SshCollector {
    async fn collect(&self, cancel: &CancellationToken) -> Result<String, MetricReason> {
        let deadline = tokio::time::Instant::now() + TIMEOUT;
        let mut channel = tokio::select! {
            biased;
            _ = cancel.cancelled() => return Err(MetricReason::SessionInactive),
            result = tokio::time::timeout_at(deadline, self.0.channel_open_session()) =>
                result.map_err(|_| MetricReason::Timeout)?.map_err(|_| MetricReason::ExecRejected)?,
        };
        // The script is fixed and shell-quoted, never built from user input.
        let command = format!("sh -c '{}'", SCRIPT.replace('\'', "'\\''"));
        let result = tokio::select! {
            biased;
            _ = cancel.cancelled() => Err(MetricReason::SessionInactive),
            result = tokio::time::timeout_at(deadline, async {
                channel.exec(true, command).await.map_err(|_| MetricReason::ExecRejected)?;
                let mut bytes = Vec::new();
                let mut size = 0usize;
                let mut accepted = false;
                let mut failed = false;
                while let Some(message) = channel.wait().await {
                    match message {
                        ChannelMsg::Success => accepted = true,
                        ChannelMsg::Failure => return Err(MetricReason::ExecRejected),
                        ChannelMsg::Data { data } => {
                            size = size.saturating_add(data.len());
                            if size > OUTPUT_LIMIT { return Err(MetricReason::OutputLimit); }
                            bytes.extend_from_slice(&data);
                        }
                        ChannelMsg::ExtendedData { data, .. } => {
                            size = size.saturating_add(data.len());
                            if size > OUTPUT_LIMIT { return Err(MetricReason::OutputLimit); }
                        }
                        ChannelMsg::ExitStatus { exit_status } => failed = exit_status != 0,
                        ChannelMsg::ExitSignal { .. } => return Err(MetricReason::CollectionFailed),
                        // EOF ends stdout, not the command; exit status may arrive later.
                        ChannelMsg::Close => break,
                        _ => {}
                    }
                }
                if !accepted { return Err(MetricReason::ExecRejected); }
                if failed { return Err(MetricReason::CollectionFailed); }
                let raw = String::from_utf8(bytes).map_err(|_| MetricReason::InvalidOutput)?;
                if !raw.contains("__FN_METRIC_end__") { return Err(MetricReason::InvalidOutput); }
                Ok(raw)
            }) => result.unwrap_or(Err(MetricReason::Timeout)),
        };
        let _ = tokio::time::timeout(Duration::from_millis(250), channel.close()).await;
        result
    }
}

#[cfg(unix)]
pub(in crate::system::terminal) struct LocalCollector;

#[cfg(unix)]
#[async_trait]
impl MetricsCollector for LocalCollector {
    async fn collect(&self, cancel: &CancellationToken) -> Result<String, MetricReason> {
        collect_local_script(SCRIPT, TIMEOUT, cancel).await
    }
}

#[cfg(unix)]
async fn collect_local_script(
    script: &str,
    timeout: Duration,
    cancel: &CancellationToken,
) -> Result<String, MetricReason> {
    use std::process::Stdio;
    use tokio::io::AsyncReadExt;
    // Local terminal runs under the service's effective uid/gid; preserve
    // that identity, but do not inherit credentials or login-shell hooks.
    let mut command = tokio::process::Command::new("/bin/sh");
    command
        .arg("-c")
        .arg(script)
        .env_clear()
        .current_dir("/")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .process_group(0);
    let mut child = command
        .spawn()
        .map_err(|_| MetricReason::CollectionFailed)?;
    let mut group = ProcessGroup(child.id());
    let stdout = child.stdout.take().ok_or(MetricReason::CollectionFailed)?;
    let result = tokio::select! {
        biased;
        _ = cancel.cancelled() => Err(MetricReason::SessionInactive),
        result = tokio::time::timeout(timeout, async {
            let mut bytes = Vec::new();
            stdout.take(OUTPUT_LIMIT as u64 + 1).read_to_end(&mut bytes).await.map_err(|_| MetricReason::CollectionFailed)?;
            if bytes.len() > OUTPUT_LIMIT { return Err(MetricReason::OutputLimit); }
            let status = child.wait().await.map_err(|_| MetricReason::CollectionFailed)?;
            // wait() reaped the PID; never signal a possibly reused process group.
            group.0 = None;
            if !status.success() { return Err(MetricReason::CollectionFailed); }
            let raw = String::from_utf8(bytes).map_err(|_| MetricReason::InvalidOutput)?;
            if !raw.contains("__FN_METRIC_end__") { return Err(MetricReason::InvalidOutput); }
            Ok(raw)
        }) => result.unwrap_or(Err(MetricReason::Timeout)),
    };
    // Kill the whole command group (including top), then reap the shell.
    if result.is_err() {
        group.kill();
        let _ = tokio::time::timeout(Duration::from_millis(250), child.kill()).await;
    }
    // Successful wait already reaped the process; avoid killing a reused pid.
    group.0 = None;
    result
}

#[cfg(unix)]
struct ProcessGroup(Option<u32>);
#[cfg(unix)]
impl ProcessGroup {
    fn kill(&self) {
        if let Some(pid) = self.0.and_then(|v| i32::try_from(v).ok()) {
            // SAFETY: the process is spawned above as leader of its own group.
            unsafe {
                libc::kill(-pid, libc::SIGKILL);
            }
        }
    }
}
#[cfg(unix)]
impl Drop for ProcessGroup {
    fn drop(&mut self) {
        self.kill();
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    #[tokio::test]
    async fn local_macos_collects_without_dependencies() {
        let raw = LocalCollector
            .collect(&CancellationToken::new())
            .await
            .unwrap();
        let metrics = super::super::parser::parse(&raw, &mut None);
        assert_eq!(metrics.status, super::super::MetricsStatus::Available);
        assert!(metrics.cpu.value.is_some());
        assert!(metrics.memory.total_bytes.unwrap() > 0);
    }
}

#[cfg(all(test, unix))]
mod local_tests {
    use super::*;
    #[tokio::test]
    async fn local_output_is_bounded() {
        let script = "i=0; while [ $i -lt 10000 ]; do printf '0123456789'; i=$((i + 1)); done";
        assert_eq!(
            collect_local_script(script, TIMEOUT, &CancellationToken::new())
                .await
                .unwrap_err(),
            MetricReason::OutputLimit
        );
    }
    #[tokio::test]
    async fn local_nonzero_exit_and_incomplete_output_are_errors() {
        for script in ["exit 7", "printf incomplete"] {
            assert!(
                collect_local_script(script, TIMEOUT, &CancellationToken::new())
                    .await
                    .is_err()
            );
        }
    }
    #[tokio::test]
    async fn local_timeout_cleans_up_descendant_processes() {
        let directory = tempfile::tempdir().unwrap();
        let pidfile = directory.path().join("child.pid");
        let script = format!("sleep 30 & printf '%s' $! > '{}'; wait", pidfile.display());
        assert_eq!(
            collect_local_script(
                &script,
                Duration::from_millis(150),
                &CancellationToken::new()
            )
            .await
            .unwrap_err(),
            MetricReason::Timeout
        );
        let pid: i32 = std::fs::read_to_string(pidfile).unwrap().parse().unwrap();
        let mut alive = true;
        for _ in 0..50 {
            // SAFETY: signal 0 observes process existence; it does not signal it.
            alive = unsafe { libc::kill(pid, 0) } == 0;
            if !alive {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(
            !alive,
            "sampling must not leave sleep/top descendants behind"
        );
    }
}
