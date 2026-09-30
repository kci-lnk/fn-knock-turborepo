//! Verify the configured local fnOS TLS listener without trusting CA/hostname validity.
//! The leaf fingerprint must match the certificate just synchronized.
use super::*;
use std::{
    io::Read,
    process::{Child, ExitStatus},
    time::Instant,
};

const SETTINGS_PATH: &str = "/usr/trim/etc/network_gateway_setting.conf";
const SETTINGS_MAX_BYTES: u64 = 64 * 1024;
const PROBE_TIMEOUT: Duration = Duration::from_secs(8);
const PROBE_ATTEMPTS: usize = 3;
const RETRY_DELAY: Duration = Duration::from_millis(250);
const CAPTURE_MAX_BYTES: u64 = 64 * 1024;

pub(super) fn configured_port() -> anyhow::Result<u16> {
    read_port(Path::new(SETTINGS_PATH))
}

fn read_port(path: &Path) -> anyhow::Result<u16> {
    let result = (|| {
        validate_fixed_regular_file(path)?;
        let file = fs::File::open(path)?;
        if file.metadata()?.len() > SETTINGS_MAX_BYTES {
            bail!("gateway settings exceed 64 KiB")
        }
        let mut bytes = Vec::new();
        file.take(SETTINGS_MAX_BYTES + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > SETTINGS_MAX_BYTES {
            bail!("gateway settings exceed 64 KiB")
        }
        parse_port(&bytes)
    })();
    // Callers persist Display, so keep the actionable cause in the top-level message.
    result.map_err(|error: anyhow::Error| {
        anyhow!(
            "Cannot read fnOS HTTPS port from {}: {error}",
            path.display()
        )
    })
}

fn parse_port(bytes: &[u8]) -> anyhow::Result<u16> {
    let settings: Value = serde_json::from_slice(bytes)
        .map_err(|_| anyhow!("gateway settings contain invalid JSON"))?;
    settings
        .pointer("/schema/https/port")
        .and_then(Value::as_u64)
        .and_then(|port| u16::try_from(port).ok())
        .filter(|port| *port != 0)
        .ok_or_else(|| anyhow!("schema.https.port must be an integer between 1 and 65535"))
}

pub(super) fn verify_configured_port(expected: u16) -> anyhow::Result<()> {
    if configured_port()? != expected {
        bail!("stale certificate synchronization plan: fnOS HTTPS port changed")
    }
    Ok(())
}

fn configure_command(command: &mut Command, host: &str, port: u16) {
    command.args(["s_client", "-connect", &format!("127.0.0.1:{port}")]);
    if host == "fallback" {
        command.arg("-noservername");
    } else {
        command.args(["-servername", host]);
    }
}

struct ProbeOutput {
    status: ExitStatus,
    timed_out: bool,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

// Always reap the direct OpenSSL child, including on I/O errors. No shell or timeout
// subprocess is involved, so killing this child cannot leave a TLS probe running.
struct ProbeChild(Child);
impl Drop for ProbeChild {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
}

// Keep memory bounded while continuing to drain both streams. Stopping reads at the
// cap would let OpenSSL block on a full pipe; spooling to disk would not bound storage.
fn capture_output(mut reader: impl Read) -> std::io::Result<Vec<u8>> {
    let mut captured = Vec::new();
    let mut buffer = [0; 8192];
    loop {
        let count = match reader.read(&mut buffer) {
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if count == 0 {
            return Ok(captured);
        }
        let keep = count.min((CAPTURE_MAX_BYTES as usize).saturating_sub(captured.len()));
        captured.extend_from_slice(&buffer[..keep]);
    }
}

fn run_probe(command: &mut Command, timeout: Duration) -> anyhow::Result<ProbeOutput> {
    let child = ProbeChild(
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("process stage (exit unavailable): could not start OpenSSL")?,
    );
    let deadline = Instant::now() + timeout;
    std::thread::scope(|scope| {
        // Own the guard inside the scope: every error must kill/reap OpenSSL before
        // scope teardown joins readers waiting for EOF from that same child.
        let mut child = child;
        let stdout = child
            .0
            .stdout
            .take()
            .ok_or_else(|| anyhow!("Missing TLS probe stdout"))?;
        let stderr = child
            .0
            .stderr
            .take()
            .ok_or_else(|| anyhow!("Missing TLS probe stderr"))?;
        let stdout =
            std::thread::Builder::new().spawn_scoped(scope, move || capture_output(stdout))?;
        let stderr =
            std::thread::Builder::new().spawn_scoped(scope, move || capture_output(stderr))?;
        let result = (|| -> anyhow::Result<(ExitStatus, bool)> {
            let mut input = child
                .0
                .stdin
                .take()
                .ok_or_else(|| anyhow!("Missing TLS probe input"))?;
            // Q ends s_client after its handshake; early exit can close stdin first.
            if let Err(error) = input.write_all(b"Q\n")
                && error.kind() != std::io::ErrorKind::BrokenPipe
            {
                return Err(error).context("process stage: could not write TLS probe input");
            }
            drop(input);
            loop {
                if let Some(status) = child.0.try_wait()? {
                    return Ok((status, false));
                }
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    child
                        .0
                        .kill()
                        .context("process stage: could not stop timed-out OpenSSL")?;
                    return Ok((child.0.wait()?, true));
                }
                std::thread::sleep(remaining.min(Duration::from_millis(10)));
            }
        })();
        drop(child);
        let stdout = stdout
            .join()
            .map_err(|_| anyhow!("TLS probe stdout reader panicked"))?;
        let stderr = stderr
            .join()
            .map_err(|_| anyhow!("TLS probe stderr reader panicked"))?;
        let (status, timed_out) = result?;
        Ok(ProbeOutput {
            status,
            timed_out,
            stdout: stdout.context("process stage: could not read TLS probe stdout")?,
            stderr: stderr.context("process stage: could not read TLS probe stderr")?,
        })
    })
}

fn diagnostic(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr);
    let mut in_pem = false;
    let mut lines = Vec::new();
    for line in text.lines() {
        if line.contains("-----BEGIN ") {
            in_pem = true;
        }
        if in_pem {
            if line.contains("-----END ") {
                in_pem = false;
            }
            continue;
        }
        // Exclude certificate subjects, PEM/base64 data, and unrelated output.
        let lower = line.to_ascii_lowercase();
        if ![
            "error",
            "errno",
            "failed",
            "alert",
            "wrong version",
            "unknown option",
            "unrecognized option",
        ]
        .iter()
        .any(|word| lower.contains(word))
        {
            continue;
        }
        let line = line
            .split_whitespace()
            .map(|word| {
                if word.len() >= 64
                    && word
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"+/=".contains(&b))
                {
                    "[redacted]"
                } else {
                    word
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        lines.push(line.chars().filter(|c| !c.is_control()).collect::<String>());
    }
    lines.join("; ").chars().take(1024).collect()
}

fn check_output(output: &ProbeOutput, expected: &ParsedCertificate) -> anyhow::Result<()> {
    let status = output.status.to_string();
    if output.timed_out {
        bail!("timeout stage ({status}): OpenSSL exceeded the probe deadline")
    }
    if !output.status.success() {
        let detail = diagnostic(&output.stderr);
        let stage = if detail.contains("BIO_connect") || detail.contains("connect:errno") {
            "connection"
        } else {
            "handshake/process"
        };
        bail!(
            "{stage} stage ({status}): {}",
            if detail.is_empty() {
                "OpenSSL failed without an error summary"
            } else {
                &detail
            }
        )
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let start = text.find("-----BEGIN CERTIFICATE-----").ok_or_else(|| {
        anyhow!("certificate stage ({status}): TLS probe returned no certificate")
    })?;
    let end = start
        + text[start..]
            .find("-----END CERTIFICATE-----")
            .ok_or_else(|| anyhow!("certificate stage ({status}): incomplete TLS certificate"))?
        + "-----END CERTIFICATE-----".len();
    let actual = parse_certificate(&text[start..end])
        .map_err(|_| anyhow!("certificate stage ({status}): invalid TLS certificate"))?;
    if actual.fingerprint != expected.fingerprint {
        bail!(
            "fingerprint stage ({status}): fnOS TLS fingerprint mismatch (expected {}, received {})",
            expected.fingerprint,
            actual.fingerprint
        )
    }
    Ok(())
}

fn verify_with_retry(
    host: &str,
    port: u16,
    expected: &ParsedCertificate,
    mut probe: impl FnMut() -> anyhow::Result<ProbeOutput>,
    mut sleep: impl FnMut(Duration),
) -> anyhow::Result<()> {
    for attempt in 1..=PROBE_ATTEMPTS {
        let error = match probe().and_then(|output| check_output(&output, expected)) {
            Ok(()) => return Ok(()),
            Err(error) => error,
        };
        if attempt == PROBE_ATTEMPTS {
            let name = if host == "fallback" {
                "fallback (no SNI)".to_owned()
            } else {
                format!(
                    "SNI {}",
                    host.chars()
                        .filter(|c| !c.is_control())
                        .take(253)
                        .collect::<String>()
                )
            };
            bail!(
                "fnOS TLS verification failed for {name} at 127.0.0.1:{port} after {attempt} attempts: {error:#}"
            )
        }
        sleep(RETRY_DELAY);
    }
    unreachable!("at least one probe attempt")
}

pub(super) fn verify_host(
    host: &str,
    port: u16,
    expected: &ParsedCertificate,
) -> anyhow::Result<()> {
    verify_with_retry(
        host,
        port,
        expected,
        || {
            let mut command = Command::new("openssl");
            configure_command(&mut command, host, port);
            run_probe(&mut command, PROBE_TIMEOUT)
        },
        std::thread::sleep,
    )
}

#[cfg(all(test, unix))]
mod tests;
