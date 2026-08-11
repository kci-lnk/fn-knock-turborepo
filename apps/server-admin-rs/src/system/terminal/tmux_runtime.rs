use super::*;

pub(super) fn terminal_text(
    translator: &Translator,
    key: &str,
    params: &[(&str, String)],
) -> String {
    let full_key = format!("server.terminal.{key}");
    if params.is_empty() {
        translator.t(&full_key)
    } else {
        translator.t_params(&full_key, params)
    }
}

pub(super) fn terminal_default_text(key: &str, params: &[(&str, String)]) -> String {
    terminal_text(&Translator::new(crate::i18n::DEFAULT_LOCALE), key, params)
}

pub(super) fn localize_terminal_error(translator: &Translator, raw_message: &str) -> String {
    let raw_message = raw_message.trim();
    if raw_message.is_empty() {
        return terminal_text(translator, "operationFailed", &[]);
    }
    if let Some(message) = localize_known_terminal_message(translator, raw_message) {
        return message;
    }
    terminal_text(
        translator,
        "operationFailedWithMessage",
        &[("message", raw_message.to_string())],
    )
}

pub(super) fn localize_known_terminal_message(
    translator: &Translator,
    raw_message: &str,
) -> Option<String> {
    localize_terminal_parameterized_message(translator, raw_message)
        .or_else(|| localize_terminal_simple_message(translator, raw_message))
}

pub(super) fn localize_terminal_simple_message(
    translator: &Translator,
    raw_message: &str,
) -> Option<String> {
    for &key in TERMINAL_SIMPLE_ERROR_KEYS {
        for locale in TERMINAL_MESSAGE_LOCALES {
            let source = terminal_text(&Translator::new(locale), key, &[]);
            if source == raw_message {
                return Some(terminal_text(translator, key, &[]));
            }
            if let Some(detail) = raw_message.strip_prefix(&format!("{source}:")) {
                return Some(format!(
                    "{}:{}",
                    terminal_text(translator, key, &[]),
                    detail
                ));
            }
        }
    }
    None
}

pub(super) fn localize_terminal_parameterized_message(
    translator: &Translator,
    raw_message: &str,
) -> Option<String> {
    const MARKER: &str = "__fn_knock_terminal_param__";
    for &(key, param) in TERMINAL_PARAMETERIZED_ERROR_KEYS {
        for locale in TERMINAL_MESSAGE_LOCALES {
            let template = terminal_text(&Translator::new(locale), key, &[(param, MARKER.into())]);
            if let Some(value) = extract_single_template_value(&template, raw_message, MARKER) {
                return Some(terminal_text(translator, key, &[(param, value)]));
            }
        }
    }
    None
}

pub(super) fn extract_single_template_value(
    template: &str,
    raw_message: &str,
    marker: &str,
) -> Option<String> {
    let (prefix, suffix) = template.split_once(marker)?;
    if !raw_message.starts_with(prefix) || !raw_message.ends_with(suffix) {
        return None;
    }
    let value_end = raw_message.len().checked_sub(suffix.len())?;
    if value_end < prefix.len() {
        return None;
    }
    Some(raw_message[prefix.len()..value_end].to_string())
}

pub(super) async fn runtime_status(state: &AppState) -> anyhow::Result<TerminalRuntimeStatus> {
    let translator = Translator::from_state(state).await;
    let config = terminal_feature_config(state).await?;
    let mut install_state = get_tmux_install_state().await;
    localize_tmux_install_state(&mut install_state, &translator);
    let tmux_available = install_state.status == "installed";
    let running_as_root = is_running_as_root();
    let blocked_reason = if !config.enabled {
        translator.t("server.terminal.webTerminalDisabled")
    } else if install_state.status == "installing" {
        translator.t("server.terminal.tmuxInstallingWait")
    } else if !tmux_available {
        if install_state.status == "error" {
            translator.t_params(
                "server.terminal.tmuxStatusError",
                &[("message", install_state.message.clone())],
            )
        } else {
            translator.t("server.terminal.tmuxMissingCannotCreate")
        }
    } else if running_as_root && !config.dangerously_run_as_current_user {
        translator.t("server.terminal.rootRunRequiresDangerToggle")
    } else {
        String::new()
    };

    Ok(TerminalRuntimeStatus {
        enabled: config.enabled,
        tmux_available,
        tmux_executable_path: install_state.executable_path.clone(),
        tmux_detection_source: install_state.detection_source.clone(),
        tmux_version: install_state.version.clone(),
        tmux_install_state: install_state,
        http_polling_available: true,
        running_as_root,
        blocked_reason,
    })
}

pub(super) fn localize_tmux_install_state(
    state: &mut TerminalTmuxInstallState,
    translator: &Translator,
) {
    state.message = match state.status.as_str() {
        "installed" => translator.t_params(
            "server.terminal.tmuxReadyWithVersion",
            &[("version", state.version.clone())],
        ),
        "installing" if state.progress < 30 => translator.t("server.terminal.refreshingApt"),
        "installing" if state.progress < 90 => translator.t("server.terminal.installingTmux"),
        "installing" => translator.t("server.terminal.verifyingTmuxInstall"),
        "uninstalled" => translator.t("server.terminal.tmuxNotDetectedInstallFirst"),
        "error" => localize_known_terminal_message(translator, &state.message)
            .unwrap_or_else(|| state.message.clone()),
        _ => state.message.clone(),
    };
}

pub(super) async fn terminal_feature_config(
    state: &AppState,
) -> anyhow::Result<TerminalFeatureConfig> {
    let config = state.storage.store.get_config().await?;
    Ok(normalize_terminal_feature(config.get("terminal_feature")))
}

pub(super) fn normalize_terminal_feature(value: Option<&Value>) -> TerminalFeatureConfig {
    TerminalFeatureConfig {
        enabled: bool_field(value, "enabled", false),
        default_cwd: normalize_terminal_default_cwd(
            value
                .and_then(|value| value.get("default_cwd"))
                .and_then(Value::as_str),
        ),
        max_sessions: int_field(value, "max_sessions", 3, 1, 12),
        idle_timeout_seconds: int_field(
            value,
            "idle_timeout_seconds",
            24 * 60 * 60,
            60,
            7 * 24 * 60 * 60,
        ),
        resume_backend: "tmux".to_string(),
        allow_mobile_toolbar: bool_field(value, "allow_mobile_toolbar", true),
        dangerously_run_as_current_user: bool_field(value, "dangerously_run_as_current_user", true),
    }
}

pub(super) fn bool_field(value: Option<&Value>, key: &str, fallback: bool) -> bool {
    value
        .and_then(|value| value.get(key))
        .and_then(Value::as_bool)
        .unwrap_or(fallback)
}

pub(super) fn int_field(
    value: Option<&Value>,
    key: &str,
    fallback: i64,
    min: i64,
    max: i64,
) -> i64 {
    value
        .and_then(|value| value.get(key))
        .and_then(parse_int_field_value)
        .unwrap_or(fallback)
        .clamp(min, max)
}

pub(super) fn parse_int_field_value(value: &Value) -> Option<i64> {
    crate::node_compat::parse_i64_from_json_like_node(value)
}

pub(super) async fn start_tmux_install(
    app_state: &AppState,
) -> anyhow::Result<TerminalTmuxInstallState> {
    let current = get_tmux_install_state().await;
    if current.status == "installed" || current.status == "installing" {
        return Ok(current);
    }

    if TMUX_INSTALL_RUNNING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        {
            let mut state = TMUX_INSTALL_STATE.lock().await;
            *state = TerminalTmuxInstallState {
                status: "installing".to_string(),
                progress: 15,
                message: terminal_default_text("refreshingApt", &[]),
                executable_path: String::new(),
                detection_source: None,
                version: String::new(),
            };
        }
        app_state.spawn_background("tmux-install", async {
            install_tmux_in_background().await;
            TMUX_INSTALL_RUNNING.store(false, Ordering::SeqCst);
        });
    }

    Ok(TMUX_INSTALL_STATE.lock().await.clone())
}

pub(super) async fn install_tmux_in_background() {
    if let Err(error) = do_install_tmux().await {
        reset_tmux_probe_cache().await;
        let mut state = TMUX_INSTALL_STATE.lock().await;
        *state = TerminalTmuxInstallState {
            status: "error".to_string(),
            progress: 0,
            message: error.to_string(),
            executable_path: String::new(),
            detection_source: None,
            version: String::new(),
        };
    }
}

pub(super) async fn do_install_tmux() -> anyhow::Result<()> {
    set_install_state(
        "installing",
        15,
        &terminal_default_text("refreshingApt", &[]),
        None,
    )
    .await;
    ensure_process_succeeded(
        DEBIAN_APT_GET_PATH,
        &["update"],
        &terminal_default_text("aptUpdateFailed", &[]),
    )
    .await?;

    set_install_state(
        "installing",
        60,
        &terminal_default_text("installingTmux", &[]),
        None,
    )
    .await;
    ensure_process_succeeded(
        DEBIAN_APT_GET_PATH,
        &["install", "-y", "tmux"],
        &terminal_default_text("aptInstallTmuxFailed", &[]),
    )
    .await?;

    set_install_state(
        "installing",
        90,
        &terminal_default_text("verifyingTmuxInstall", &[]),
        None,
    )
    .await;
    reset_tmux_probe_cache().await;
    let Some(tmux) = detect_tmux_executable().await else {
        return Err(anyhow!(terminal_default_text(
            "tmuxMissingAfterInstall",
            &[]
        )));
    };
    let ready_message = terminal_default_text(
        "tmuxInstallCompleteWithVersion",
        &[("version", tmux.version.clone())],
    );
    set_install_state("installed", 100, &ready_message, Some(tmux)).await;
    Ok(())
}

pub(super) async fn set_install_state(
    status: &str,
    progress: i64,
    message: &str,
    tmux: Option<TmuxExecutableInfo>,
) {
    let mut state = TMUX_INSTALL_STATE.lock().await;
    *state = TerminalTmuxInstallState {
        status: status.to_string(),
        progress,
        message: message.to_string(),
        executable_path: tmux
            .as_ref()
            .map(|value| value.path.clone())
            .unwrap_or_default(),
        detection_source: tmux.as_ref().map(|value| value.detection_source.clone()),
        version: tmux.map(|value| value.version).unwrap_or_default(),
    };
}

pub(super) async fn get_tmux_install_state() -> TerminalTmuxInstallState {
    if TMUX_INSTALL_RUNNING.load(Ordering::SeqCst) {
        return TMUX_INSTALL_STATE.lock().await.clone();
    }

    if let Some(tmux) = detect_tmux_executable().await {
        let ready_message =
            terminal_default_text("tmuxReadyWithVersion", &[("version", tmux.version.clone())]);
        let state = TerminalTmuxInstallState {
            status: "installed".to_string(),
            progress: 100,
            message: ready_message,
            executable_path: tmux.path,
            detection_source: Some(tmux.detection_source),
            version: tmux.version,
        };
        *TMUX_INSTALL_STATE.lock().await = state.clone();
        return state;
    }

    let current = TMUX_INSTALL_STATE.lock().await.clone();
    if current.status == "error" {
        current
    } else {
        default_tmux_install_state()
    }
}

pub(super) fn default_tmux_install_state() -> TerminalTmuxInstallState {
    TerminalTmuxInstallState {
        status: "uninstalled".to_string(),
        progress: 0,
        message: terminal_default_text("tmuxNotDetectedInstallFirst", &[]),
        executable_path: String::new(),
        detection_source: None,
        version: String::new(),
    }
}

pub(super) async fn detect_tmux_executable() -> Option<TmuxExecutableInfo> {
    if let Some(cached) = TMUX_CACHE.lock().await.clone() {
        return Some(cached);
    }

    let candidates = [
        ("tmux", "env-path"),
        (TMUX_ABSOLUTE_FALLBACK_PATH, "absolute-path"),
    ];
    for (path, detection_source) in candidates {
        let Ok(result) = run_process(path, &["-V"], None, true).await else {
            continue;
        };
        if result.code == 0 {
            let info = TmuxExecutableInfo {
                path: path.to_string(),
                detection_source: detection_source.to_string(),
                version: if result.stdout.trim().is_empty() {
                    "tmux".to_string()
                } else {
                    result.stdout
                },
            };
            *TMUX_CACHE.lock().await = Some(info.clone());
            return Some(info);
        }
    }
    None
}

pub(super) async fn reset_tmux_probe_cache() {
    *TMUX_CACHE.lock().await = None;
}

pub(super) async fn run_tmux(args: &[&str]) -> anyhow::Result<ExecResult> {
    let tmux = detect_tmux_executable().await;
    run_process(
        tmux.as_ref()
            .map(|value| value.path.as_str())
            .unwrap_or("tmux"),
        args,
        None,
        true,
    )
    .await
}

pub(super) async fn run_tmux_raw(args: &[&str]) -> anyhow::Result<ExecResult> {
    let tmux = detect_tmux_executable().await;
    run_process(
        tmux.as_ref()
            .map(|value| value.path.as_str())
            .unwrap_or("tmux"),
        args,
        None,
        false,
    )
    .await
}

pub(super) async fn run_process(
    command: &str,
    args: &[&str],
    cwd: Option<&Path>,
    trim_output: bool,
) -> anyhow::Result<ExecResult> {
    let mut cmd = Command::new(command);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(cwd) = cwd {
        cmd.current_dir(cwd);
    }
    let output = cmd
        .output()
        .await
        .with_context(|| format!("run process {command}"))?;
    let mut stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let mut stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if trim_output {
        stdout = stdout.trim_end().to_string();
        stderr = stderr.trim_end().to_string();
    }
    Ok(ExecResult {
        code: output.status.code().unwrap_or(-1),
        stdout,
        stderr,
    })
}

pub(super) async fn ensure_process_succeeded(
    command: &str,
    args: &[&str],
    failure_message: &str,
) -> anyhow::Result<ExecResult> {
    let result = run_process(command, args, None, false).await?;
    if result.code == 0 {
        return Ok(ExecResult {
            code: result.code,
            stdout: result.stdout.trim_end().to_string(),
            stderr: result.stderr.trim_end().to_string(),
        });
    }
    let detail = summarize_process_output(&result);
    if detail.is_empty() {
        Err(anyhow!(failure_message.to_string()))
    } else {
        Err(anyhow!("{failure_message}: {detail}"))
    }
}

pub(super) fn summarize_process_output(result: &ExecResult) -> String {
    let detail = format!("{}\n{}", result.stderr, result.stdout);
    let lines = detail
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    lines
        .iter()
        .rev()
        .take(8)
        .copied()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join(" | ")
        .chars()
        .take(500)
        .collect()
}
