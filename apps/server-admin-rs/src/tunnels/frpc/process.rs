use super::*;

#[cfg(unix)]
const FORCE_TERMINATE_SIGNAL: libc::c_int = libc::SIGKILL;
#[cfg(not(unix))]
const FORCE_TERMINATE_SIGNAL: libc::c_int = libc::SIGTERM;

pub(super) fn pid_path_for_meta(meta: &FrpcInstanceMeta) -> PathBuf {
    PathBuf::from(&meta.work_dir).join("frpc.pid")
}

pub(super) async fn read_pid_file(path: &Path) -> Option<u32> {
    fs::read_to_string(path)
        .await
        .ok()
        .and_then(|content| content.trim().parse::<u32>().ok())
        .filter(|pid| *pid > 0)
}

pub(super) async fn write_pid_file(path: &Path, pid: u32) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent).await;
    }
    let _ = fs::write(path, format!("{pid}\n")).await;
}

pub(super) async fn remove_pid_file(path: &Path) {
    let _ = fs::remove_file(path).await;
}

pub(super) async fn terminate_pid(pid: u32) -> FrpcResult<()> {
    if pid == std::process::id() || !is_process_alive(pid) {
        return Ok(());
    }
    send_signal(pid, libc::SIGTERM);
    for _ in 0..20 {
        if !is_process_alive(pid) {
            return Ok(());
        }
        sleep(Duration::from_millis(100)).await;
    }
    send_signal(pid, FORCE_TERMINATE_SIGNAL);
    for _ in 0..10 {
        if !is_process_alive(pid) {
            return Ok(());
        }
        sleep(Duration::from_millis(100)).await;
    }
    if is_process_alive(pid) {
        return Err(frpc_internal(format!(
            "frpc process is still running: {pid}"
        )));
    }
    Ok(())
}

pub(super) fn send_signal(pid: u32, signal: libc::c_int) {
    if let Ok(pid) = i32::try_from(pid) {
        let _ = crate::unix::send_signal(pid, signal);
    }
}

pub(super) fn is_process_alive(pid: u32) -> bool {
    i32::try_from(pid).is_ok_and(crate::unix::process_exists)
}

pub(super) async fn is_owned_frpc_pid(pid: u32, config_path: &str) -> bool {
    if !is_process_alive(pid) {
        return false;
    }
    let args = read_process_args(pid).await;
    args.as_deref()
        .is_some_and(|args| is_frpc_process_args_for_config(args, config_path))
}

pub(super) async fn find_frpc_pid_by_config_path(config_path: &str) -> Option<u32> {
    let entries = std::fs::read_dir("/proc").ok()?;
    for entry in entries.flatten() {
        let pid = entry
            .file_name()
            .to_str()
            .and_then(|value| value.parse::<u32>().ok());
        let Some(pid) = pid else {
            continue;
        };
        if pid == std::process::id() || !is_process_alive(pid) {
            continue;
        }
        let args = read_proc_cmdline_args(pid).await;
        if args
            .as_deref()
            .is_some_and(|args| is_frpc_process_args_for_config(args, config_path))
        {
            return Some(pid);
        }
    }
    None
}

pub(super) async fn read_process_args(pid: u32) -> Option<Vec<String>> {
    read_proc_cmdline_args(pid)
        .await
        .or_else(|| read_ps_command_args(pid))
}

pub(super) async fn read_proc_cmdline_args(pid: u32) -> Option<Vec<String>> {
    let bytes = fs::read(format!("/proc/{pid}/cmdline")).await.ok()?;
    if bytes.is_empty() {
        return None;
    }
    let args = bytes
        .split(|byte| *byte == 0)
        .filter_map(|part| {
            let value = String::from_utf8_lossy(part).trim().to_string();
            (!value.is_empty()).then_some(value)
        })
        .collect::<Vec<_>>();
    (!args.is_empty()).then_some(args)
}

pub(super) fn read_ps_command_args(pid: u32) -> Option<Vec<String>> {
    let output = std::process::Command::new("ps")
        .args(["-ww", "-p", &pid.to_string(), "-o", "args="])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let command = String::from_utf8_lossy(&output.stdout);
    let args = split_command_line(command.trim());
    (!args.is_empty()).then_some(args)
}

pub(super) fn split_command_line(command: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut in_arg = false;
    for ch in command.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            in_arg = true;
            continue;
        }
        if ch == '\\' && quote != Some('\'') {
            escaped = true;
            in_arg = true;
            continue;
        }
        if let Some(active_quote) = quote {
            if ch == active_quote {
                quote = None;
            } else {
                current.push(ch);
            }
            in_arg = true;
            continue;
        }
        if ch == '\'' || ch == '"' {
            quote = Some(ch);
            in_arg = true;
            continue;
        }
        if ch.is_whitespace() {
            if in_arg {
                args.push(std::mem::take(&mut current));
                in_arg = false;
            }
            continue;
        }
        current.push(ch);
        in_arg = true;
    }
    if escaped {
        current.push('\\');
    }
    if in_arg {
        args.push(current);
    }
    args
}

pub(super) fn is_frpc_process_args_for_config(args: &[String], config_path: &str) -> bool {
    let Some(first) = args.first() else {
        return false;
    };
    let executable = Path::new(first)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if executable != "frpc" && executable != "frpc.exe" {
        return false;
    }
    for (index, arg) in args.iter().enumerate().skip(1) {
        if matches!(arg.as_str(), "-c" | "--config" | "--config-file") {
            return args
                .get(index + 1)
                .is_some_and(|candidate| same_path(candidate, config_path));
        }
        if let Some(candidate) = arg.strip_prefix("--config=") {
            return same_path(candidate, config_path);
        }
        if let Some(candidate) = arg.strip_prefix("--config-file=") {
            return same_path(candidate, config_path);
        }
    }
    false
}

pub(super) fn same_path(left: &str, right: &str) -> bool {
    normalize_path(left) == normalize_path(right)
}

pub(super) fn normalize_path(value: &str) -> String {
    let path = PathBuf::from(value);
    let absolute = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    };
    absolute.to_string_lossy().replace('\\', "/")
}
