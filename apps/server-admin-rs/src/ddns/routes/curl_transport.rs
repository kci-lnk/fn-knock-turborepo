use std::{process::ExitStatus, time::Duration};

const PROXY_ENV_KEYS: [&str; 8] = [
    "http_proxy",
    "https_proxy",
    "all_proxy",
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "ALL_PROXY",
    "no_proxy",
    "NO_PROXY",
];

pub(super) fn command(
    timeout: Duration,
    max_response_bytes: usize,
    follow_redirects: bool,
) -> tokio::process::Command {
    let mut command = tokio::process::Command::new("curl");
    command
        .arg("-q")
        .arg("--silent")
        .arg("--show-error")
        .arg("--max-time")
        .arg(format!("{:.3}", timeout.as_secs_f64().max(0.001)))
        .arg("--max-filesize")
        .arg(max_response_bytes.to_string());
    if follow_redirects {
        command.arg("--location");
    }
    for key in PROXY_ENV_KEYS {
        command.env_remove(key);
    }
    command
}

pub(super) fn bind_network_interface(
    command: &mut tokio::process::Command,
    network_interface: &str,
) {
    if !network_interface.is_empty()
        && !network_interface.starts_with(super::DOCKER_HOST_INTERFACE_PREFIX)
    {
        command.arg("--interface").arg(network_interface);
    }
}

pub(super) fn failure_detail(status: ExitStatus, stderr: &[u8]) -> String {
    let detail = String::from_utf8_lossy(stderr).trim().to_string();
    if detail.is_empty() {
        status
            .code()
            .map(|code| format!("exit {code}"))
            .unwrap_or_else(|| "terminated".to_string())
    } else {
        detail
    }
}
