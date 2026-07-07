use super::*;

pub(super) fn normalize_session(mut session: TerminalSessionRecord) -> TerminalSessionRecord {
    let now = now_iso();
    session.id = clean_string(&session.id, "");
    session.cwd = clean_string(&session.cwd, "~");
    let default_title = terminal_default_text("defaultTitle", &[]);
    let title_fallback = path_basename(&session.cwd)
        .filter(|value| !value.is_empty())
        .unwrap_or(default_title.as_str());
    session.title = clean_string(&session.title, title_fallback);
    if !matches!(
        session.status.as_str(),
        "attached" | "detached" | "stopped" | "error"
    ) {
        session.status = "created".to_string();
    }
    session.created_at = normalize_iso(&session.created_at).unwrap_or_else(|| now.clone());
    session.updated_at = normalize_iso(&session.updated_at).unwrap_or_else(|| now.clone());
    session.last_attached_at = normalize_iso(&session.last_attached_at).unwrap_or_default();
    session.last_detached_at = normalize_iso(&session.last_detached_at).unwrap_or_default();
    session.last_client_ip = clean_string(&session.last_client_ip, "");
    session.shell = clean_string(
        &session.shell,
        &env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string()),
    );
    session.cols = if session.cols <= 0 { 120 } else { session.cols }.clamp(20, 400);
    session.rows = if session.rows <= 0 { 32 } else { session.rows }.clamp(8, 200);
    session.resume_backend = "tmux".to_string();
    session.backend_session_name = clean_string(&session.backend_session_name, "");
    session.pane_tty_path = clean_string(&session.pane_tty_path, "");
    session.input_pipe_path = clean_string(&session.input_pipe_path, "");
    session.output_log_path = clean_string(&session.output_log_path, "");
    session.expires_at = normalize_iso(&session.expires_at).unwrap_or_default();
    session.last_frame_revision = clean_string(&session.last_frame_revision, "");
    session
}

pub(super) fn normalize_attachment(
    mut attachment: TerminalAttachmentRecord,
) -> TerminalAttachmentRecord {
    let now = now_iso();
    attachment.id = clean_string(&attachment.id, "");
    attachment.session_id = clean_string(&attachment.session_id, "");
    attachment.transport = "http-polling".to_string();
    attachment.created_at = normalize_iso(&attachment.created_at).unwrap_or_else(|| now.clone());
    attachment.updated_at = normalize_iso(&attachment.updated_at).unwrap_or_else(|| now.clone());
    attachment.expires_at = normalize_iso(&attachment.expires_at).unwrap_or(now);
    attachment
}

pub(super) fn normalize_terminal_dimension(
    value: Option<f64>,
    fallback: i64,
    min: i64,
    max: i64,
) -> i64 {
    let selected = match value {
        Some(value) if value.is_finite() && value != 0.0 => value,
        _ => fallback as f64,
    };
    let floored = selected.floor();
    let parsed = if floored <= i64::MIN as f64 {
        i64::MIN
    } else if floored >= i64::MAX as f64 {
        i64::MAX
    } else {
        floored as i64
    };
    parsed.clamp(min, max)
}

pub(super) fn normalize_terminal_poll_timeout_ms(value: Option<f64>) -> u64 {
    let selected = match value {
        Some(value) if value.is_finite() && value != 0.0 => value,
        _ => DEFAULT_POLL_TIMEOUT_MS as f64,
    };
    selected.clamp(1_000.0, 20_000.0).floor() as u64
}

pub(super) fn parse_output_cursor_like_node(value: Option<&str>) -> i64 {
    let Some(parsed) = crate::node_compat::parse_i64_prefix(value.unwrap_or("").trim_start())
    else {
        return 0;
    };
    parsed.max(0)
}

pub(super) fn clean_string(value: &str, fallback: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}

pub(super) fn normalize_iso(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    parse_iso_ms(trimmed).map(|_| trimmed.to_string())
}

pub(super) fn session_data_key(id: &str) -> String {
    format!("{SESSION_DATA_PREFIX}{id}")
}

pub(super) fn session_attachments_key(session_id: &str) -> String {
    format!("{SESSION_ATTACHMENTS_PREFIX}{session_id}")
}

pub(super) fn attachment_data_key(id: &str) -> String {
    format!("{ATTACHMENT_DATA_PREFIX}{id}")
}

pub(super) fn stream_directory(state: &AppState) -> PathBuf {
    state.settings.data_dir.join(TERMINAL_STREAM_DIR_NAME)
}

pub(super) async fn ensure_stream_directory(state: &AppState) -> anyhow::Result<()> {
    fs::create_dir_all(stream_directory(state)).await?;
    Ok(())
}

pub(super) fn build_session_name(id: &str) -> String {
    let compact = id.replace('-', "");
    format!("fnk_{}", compact.chars().take(16).collect::<String>())
}

pub(super) fn build_output_log_path(stream_directory: &Path, id: &str) -> PathBuf {
    stream_directory.join(format!("{id}.log"))
}

pub(super) fn build_input_pipe_path(stream_directory: &Path, id: &str) -> PathBuf {
    stream_directory.join(format!("{id}.in"))
}

pub(super) fn pane_target(session: &TerminalSessionRecord) -> String {
    format!(
        "{}{}",
        session.backend_session_name, TMUX_TARGET_PANE_SUFFIX
    )
}

pub(super) fn sanitize_title(raw: Option<&str>) -> Option<String> {
    raw.map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

pub(super) fn build_default_session_title(
    existing_sessions: &[TerminalSessionRecord],
    translator: &Translator,
) -> String {
    let prefix = terminal_text(translator, "defaultSessionTitlePrefix", &[]);
    let mut used = BTreeSet::new();
    for session in existing_sessions {
        let title = session.title.trim();
        let suffix = title
            .strip_prefix(&prefix)
            .or_else(|| title.strip_prefix(LEGACY_DEFAULT_SESSION_TITLE_PREFIX));
        let Some(suffix) = suffix else { continue };
        if let Ok(index) = suffix.parse::<i64>() {
            if index > 0 {
                used.insert(index);
            }
        }
    }
    let mut next = 1;
    while used.contains(&next) {
        next += 1;
    }
    format!("{prefix}{next}")
}

pub(super) async fn resolve_shell(shell: Option<&str>) -> anyhow::Result<String> {
    let requested = shell.map(str::trim).filter(|value| !value.is_empty());
    if let Some(requested) = requested {
        if can_start_shell(requested).await {
            return Ok(requested.to_string());
        }
        return Err(anyhow!(terminal_default_text(
            "requestedShellUnavailable",
            &[("shell", requested.to_string())],
        )));
    }

    for candidate in auto_shell_candidates() {
        if can_start_shell(&candidate).await {
            return Ok(candidate);
        }
    }
    Err(anyhow!(terminal_default_text("noShellDetected", &[])))
}

pub(super) fn auto_shell_candidates() -> Vec<String> {
    auto_shell_candidates_from_env(&env::var("SHELL").unwrap_or_default())
}

pub(super) fn auto_shell_candidates_from_env(env_shell: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    if is_zsh_shell(env_shell) {
        candidates.push(env_shell.to_string());
    }
    candidates.extend(["zsh", "/bin/zsh", "/usr/bin/zsh"].map(String::from));
    candidates.push(env_shell.to_string());
    candidates.extend(
        [
            "bash",
            "/bin/bash",
            "/usr/bin/bash",
            "sh",
            "/bin/sh",
            "/usr/bin/sh",
        ]
        .map(String::from),
    );
    dedupe_strings(candidates)
}

pub(super) async fn can_start_shell(command: &str) -> bool {
    run_process(command, &["-c", "exit 0"], None, true)
        .await
        .is_ok_and(|result| result.code == 0)
}

pub(super) fn build_session_shell_command(shell: &str) -> String {
    if is_zsh_shell(shell) {
        format!("exec {} -il", shell_quote(shell))
    } else {
        format!("exec {}", shell_quote(shell))
    }
}

pub(super) fn is_zsh_shell(shell: &str) -> bool {
    Path::new(shell)
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("zsh"))
}

pub(super) async fn resolve_cwd(
    config: &TerminalFeatureConfig,
    cwd: Option<&str>,
) -> anyhow::Result<PathBuf> {
    let configured = normalize_terminal_default_cwd(Some(&config.default_cwd));
    let requested = cwd.map(str::trim).filter(|value| !value.is_empty());
    let next = requested
        .map(|value| normalize_terminal_default_cwd(Some(value)))
        .unwrap_or(configured);
    let next = next.trim();
    let resolved = if next.is_empty() || next == "~" {
        home_dir()
    } else if let Some(rest) = next.strip_prefix("~/") {
        home_dir().join(rest)
    } else {
        PathBuf::from(next)
    };
    let metadata = fs::metadata(&resolved)
        .await
        .with_context(|| format!("working directory is unavailable: {}", resolved.display()))?;
    if metadata.is_dir() {
        Ok(resolved)
    } else {
        Err(anyhow!(
            "working directory is unavailable: {}",
            resolved.display()
        ))
    }
}

pub(super) fn home_dir() -> PathBuf {
    let env_home = env::var("HOME").ok();
    let platform_home = platform_home_dir();
    let current_dir = env::current_dir().ok();
    resolve_home_dir(env_home.as_deref(), platform_home, current_dir.as_deref())
}

pub(super) fn resolve_home_dir(
    env_home: Option<&str>,
    platform_home: Option<PathBuf>,
    current_dir: Option<&Path>,
) -> PathBuf {
    if let Some(home) = env_home.map(str::trim).filter(|value| !value.is_empty()) {
        let env_home = PathBuf::from(home);
        if (current_dir.is_some_and(|cwd| cwd == env_home) || is_terminal_runtime_cwd(home))
            && let Some(platform_home) = platform_home.as_ref()
            && !platform_home.as_os_str().is_empty()
            && platform_home != &env_home
        {
            return platform_home.clone();
        }
        return env_home;
    }
    platform_home
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| PathBuf::from("/"))
}

#[cfg(unix)]
pub(super) fn platform_home_dir() -> Option<PathBuf> {
    unsafe {
        let passwd = libc::getpwuid(libc::geteuid());
        if passwd.is_null() || (*passwd).pw_dir.is_null() {
            return None;
        }
        let value = std::ffi::CStr::from_ptr((*passwd).pw_dir)
            .to_string_lossy()
            .trim()
            .to_string();
        (!value.is_empty()).then(|| PathBuf::from(value))
    }
}

#[cfg(not(unix))]
pub(super) fn platform_home_dir() -> Option<PathBuf> {
    env::var("USERPROFILE").ok().map(PathBuf::from).or_else(|| {
        let drive = env::var("HOMEDRIVE").ok()?;
        let path = env::var("HOMEPATH").ok()?;
        Some(PathBuf::from(format!("{drive}{path}")))
    })
}

pub(super) fn path_basename(path: &str) -> Option<&str> {
    Path::new(path).file_name().and_then(|value| value.to_str())
}

pub(super) fn path_to_str(path: &Path) -> anyhow::Result<&str> {
    path.to_str()
        .ok_or_else(|| anyhow!("path is not valid UTF-8: {}", path.display()))
}

pub(super) fn parse_tmux_number(value: &str, fallback: i64) -> i64 {
    value.trim().parse::<i64>().unwrap_or(fallback)
}

pub(super) fn fallback_message<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback
    } else {
        trimmed
    }
}

pub(super) fn dedupe_strings(values: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for value in values {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }
        if seen.insert(trimmed.to_string()) {
            result.push(trimmed.to_string());
        }
    }
    result
}

pub(super) fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

pub(super) fn build_relay_command(
    output_log_path: &Path,
    input_pipe_path: &Path,
) -> anyhow::Result<String> {
    let log = shell_quote(path_to_str(output_log_path)?);
    let input = shell_quote(path_to_str(input_pipe_path)?);
    Ok(format!(
        "sh -c 'log=$1; input=$2; exec 3<> \"$input\"; cat <&3 & input_pid=$!; cat >> \"$log\"; kill \"$input_pid\" 2>/dev/null || true' fnk-relay {log} {input}"
    ))
}

pub(super) fn detect_client_ip(headers: &HeaderMap) -> String {
    if let Some(forwarded) = headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
    {
        if let Some(first) = forwarded
            .split(',')
            .next()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return first.to_string();
        }
    }
    headers
        .get("x-real-ip")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown")
        .to_string()
}

#[cfg(unix)]
pub(super) fn is_fifo(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::FileTypeExt;
    metadata.file_type().is_fifo()
}

#[cfg(not(unix))]
pub(super) fn is_fifo(_metadata: &std::fs::Metadata) -> bool {
    false
}

#[cfg(unix)]
pub(super) fn is_running_as_root() -> bool {
    unsafe extern "C" {
        fn getuid() -> u32;
    }
    unsafe { getuid() == 0 }
}

#[cfg(not(unix))]
pub(super) fn is_running_as_root() -> bool {
    false
}
