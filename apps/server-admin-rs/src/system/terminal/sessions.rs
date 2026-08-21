use super::*;

pub(super) async fn terminal_list_sessions(
    state: &AppState,
) -> anyhow::Result<Vec<TerminalSessionRecord>> {
    let _ = cleanup_expired_sessions(state).await?;
    store_list_sessions(&state.storage.store).await
}

pub(super) async fn terminal_get_session(
    state: &AppState,
    id: &str,
) -> anyhow::Result<Option<TerminalSessionRecord>> {
    let Some(session) = store_get_session(&state.storage.store, id).await? else {
        return Ok(None);
    };
    if !tmux_session_exists(&session.backend_session_name).await {
        cleanup_session_artifacts(&session).await;
        store_delete_session(&state.storage.store, id).await?;
        return Ok(None);
    }
    Ok(Some(session))
}

pub(super) async fn terminal_create_session(
    state: &AppState,
    input: CreateSessionBody,
    client_ip: &str,
) -> anyhow::Result<TerminalSessionRecord> {
    let _ = cleanup_expired_sessions(state).await?;
    assert_create_allowed(state).await?;

    let config = terminal_feature_config(state).await?;
    let translator = Translator::from_state(state).await;
    let existing = store_list_sessions(&state.storage.store).await?;
    if existing.len() as i64 >= config.max_sessions {
        return Err(anyhow!(terminal_default_text(
            "sessionLimitReached",
            &[("count", config.max_sessions.to_string())],
        )));
    }

    let shell = resolve_shell(input.shell.as_deref()).await?;
    let cwd = resolve_cwd(&config, input.cwd.as_deref()).await?;
    let cols = normalize_terminal_dimension(input.cols, 120, 40, 400);
    let rows = normalize_terminal_dimension(input.rows, 32, 12, 200);
    let id = Uuid::new_v4().to_string();
    let session_name = build_session_name(&id);
    let title = sanitize_title(input.title.as_deref())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| build_default_session_title(&existing, &translator));
    let command = build_session_shell_command(&shell);

    let create_result = run_tmux(&[
        "new-session",
        "-d",
        "-s",
        &session_name,
        "-x",
        &cols.to_string(),
        "-y",
        &rows.to_string(),
        "-c",
        path_to_str(&cwd)?,
        &command,
    ])
    .await?;
    if create_result.code != 0 {
        return Err(anyhow!(
            "{}",
            fallback_message(
                &create_result.stderr,
                &terminal_default_text("tmuxSessionCreateFailed", &[])
            )
        ));
    }

    let stream_dir = stream_directory(state);
    let session = normalize_session(TerminalSessionRecord {
        id: id.clone(),
        title,
        status: "detached".to_string(),
        created_at: now_iso(),
        updated_at: now_iso(),
        last_client_ip: client_ip.to_string(),
        shell,
        cwd: cwd.to_string_lossy().to_string(),
        cols,
        rows,
        resume_backend: "tmux".to_string(),
        backend_session_name: session_name.clone(),
        input_pipe_path: build_input_pipe_path(&stream_dir, &id)
            .to_string_lossy()
            .to_string(),
        output_log_path: build_output_log_path(&stream_dir, &id)
            .to_string_lossy()
            .to_string(),
        expires_at: iso_after_seconds(config.idle_timeout_seconds),
        ..Default::default()
    });

    match configure_session_runtime(state, session.clone()).await {
        Ok(session) => Ok(session),
        Err(error) => {
            let _ = run_tmux(&["kill-session", "-t", &session_name]).await;
            cleanup_session_artifacts(&session).await;
            Err(error)
        }
    }
}

pub(super) async fn terminal_rename_session(
    state: &AppState,
    id: &str,
    title: &str,
) -> anyhow::Result<Option<TerminalSessionRecord>> {
    let Some(session) = store_get_session(&state.storage.store, id).await? else {
        return Ok(None);
    };
    let title = sanitize_title(Some(title)).unwrap_or_default();
    if title.is_empty() {
        return Err(anyhow!(terminal_default_text("sessionTitleRequired", &[])));
    }
    save_terminal_session(
        state,
        normalize_session(TerminalSessionRecord {
            title,
            updated_at: now_iso(),
            ..session
        }),
    )
    .await
    .map(Some)
}

pub(super) async fn terminal_kill_session(state: &AppState, id: &str) -> anyhow::Result<()> {
    let Some(session) = store_get_session(&state.storage.store, id).await? else {
        return Ok(());
    };
    let _ = run_tmux(&["kill-session", "-t", &session.backend_session_name]).await;
    cleanup_session_artifacts(&session).await;
    store_delete_session(&state.storage.store, id).await?;
    Ok(())
}

pub(super) async fn terminal_create_attachment(
    state: &AppState,
    session_id: &str,
    client_ip: &str,
) -> anyhow::Result<TerminalAttachmentRecord> {
    let Some(session) = terminal_get_session(state, session_id).await? else {
        return Err(anyhow!(terminal_default_text(
            "sessionMissingOrExpired",
            &[]
        )));
    };
    let status = runtime_status(state).await?;
    if !status.enabled {
        return Err(anyhow!(terminal_default_text("webTerminalDisabled", &[])));
    }
    if !status.tmux_available {
        return Err(anyhow!(terminal_default_text(
            "tmuxMissingCannotAttach",
            &[]
        )));
    }

    let runtime_session = ensure_session_runtime(state, session).await?;
    let now = now_iso();
    let config = terminal_feature_config(state).await?;
    save_terminal_session(
        state,
        normalize_session(TerminalSessionRecord {
            status: "attached".to_string(),
            updated_at: now.clone(),
            last_attached_at: now.clone(),
            last_client_ip: client_ip.to_string(),
            expires_at: iso_after_seconds(config.idle_timeout_seconds),
            ..runtime_session.clone()
        }),
    )
    .await?;

    store_save_attachment(
        &state.storage.store,
        normalize_attachment(TerminalAttachmentRecord {
            id: Uuid::new_v4().to_string(),
            session_id: runtime_session.id,
            transport: "http-polling".to_string(),
            created_at: now.clone(),
            updated_at: now,
            expires_at: iso_after_seconds(DEFAULT_ATTACHMENT_TTL_SECONDS),
        }),
        DEFAULT_ATTACHMENT_TTL_SECONDS,
    )
    .await
}

pub(super) async fn terminal_detach_attachment(
    state: &AppState,
    attachment_id: &str,
) -> anyhow::Result<()> {
    let Some(attachment) = store_get_attachment(&state.storage.store, attachment_id).await? else {
        return Ok(());
    };
    store_delete_attachment(&state.storage.store, attachment_id).await?;
    let remaining =
        store_list_attachment_ids_for_session(&state.storage.store, &attachment.session_id).await?;
    if remaining.is_empty() {
        mark_session_detached(state, &attachment.session_id).await?;
    }
    Ok(())
}

pub(super) async fn terminal_send_input(
    state: &AppState,
    attachment_id: &str,
    data_base64: &str,
) -> anyhow::Result<()> {
    let Some(attachment) = store_get_attachment(&state.storage.store, attachment_id).await? else {
        return Err(anyhow!(terminal_default_text("attachmentExpired", &[])));
    };
    let Some(session) = store_get_session(&state.storage.store, &attachment.session_id).await?
    else {
        return Err(anyhow!(terminal_default_text(
            "sessionMissingOrExpired",
            &[]
        )));
    };
    let data = general_purpose::STANDARD
        .decode(data_base64.as_bytes())
        .unwrap_or_default();
    if data.is_empty() {
        return Ok(());
    }

    let runtime_session = if session.input_pipe_path.trim().is_empty() {
        ensure_session_runtime(state, session).await?
    } else {
        session
    };

    if let Err(error) = write_input_pipe(&runtime_session.input_pipe_path, data.clone()).await {
        let Some(confirmed) = terminal_get_session(state, &runtime_session.id).await? else {
            return Err(anyhow!(terminal_default_text(
                "sessionMissingOrExpired",
                &[]
            )));
        };
        let refreshed = configure_session_runtime(state, confirmed).await?;
        write_input_pipe(&refreshed.input_pipe_path, data)
            .await
            .map_err(|retry_error| {
                anyhow!(
                    "{}: {retry_error}",
                    terminal_default_text("inputSendFailed", &[])
                )
            })?;
        tracing::warn!(session_id = %refreshed.id, %error, "terminal input pipe recovered after runtime refresh");
        touch_session_activity(state, refreshed, false).await?;
    } else {
        touch_session_activity(state, runtime_session, false).await?;
    }
    Ok(())
}

pub(super) async fn terminal_resize_attachment(
    state: &AppState,
    attachment_id: &str,
    cols: f64,
    rows: f64,
) -> anyhow::Result<TerminalSessionRecord> {
    let Some(attachment) = store_refresh_attachment(
        &state.storage.store,
        attachment_id,
        DEFAULT_ATTACHMENT_TTL_SECONDS,
    )
    .await?
    else {
        return Err(anyhow!(terminal_default_text("attachmentExpired", &[])));
    };
    let Some(session) = terminal_get_session(state, &attachment.session_id).await? else {
        return Err(anyhow!(terminal_default_text(
            "sessionMissingOrExpired",
            &[]
        )));
    };
    let next_cols = normalize_terminal_dimension(Some(cols), session.cols, 40, 400);
    let next_rows = normalize_terminal_dimension(Some(rows), session.rows, 12, 200);
    let resize_result = run_tmux(&[
        "resize-window",
        "-t",
        &session.backend_session_name,
        "-x",
        &next_cols.to_string(),
        "-y",
        &next_rows.to_string(),
    ])
    .await?;
    if resize_result.code != 0 {
        return Err(anyhow!(
            "{}",
            fallback_message(
                &resize_result.stderr,
                &terminal_default_text("resizeFailed", &[])
            )
        ));
    }
    refresh_session_expiry(
        state,
        normalize_session(TerminalSessionRecord {
            cols: next_cols,
            rows: next_rows,
            updated_at: now_iso(),
            ..session
        }),
    )
    .await
}

pub(super) async fn terminal_wait_for_output(
    state: &AppState,
    attachment_id: &str,
    cursor: i64,
    timeout_ms: Option<f64>,
) -> anyhow::Result<TerminalPollResult> {
    let Some(attachment) = store_refresh_attachment(
        &state.storage.store,
        attachment_id,
        DEFAULT_ATTACHMENT_TTL_SECONDS,
    )
    .await?
    else {
        return Err(anyhow!(terminal_default_text("attachmentExpired", &[])));
    };
    let Some(session) = terminal_get_session(state, &attachment.session_id).await? else {
        return Err(anyhow!(terminal_default_text(
            "sessionMissingOrExpired",
            &[]
        )));
    };
    let runtime_session = ensure_session_runtime(state, session).await?;
    let requested_cursor = cursor.max(0);
    let timeout = normalize_terminal_poll_timeout_ms(timeout_ms);
    let deadline = Instant::now() + Duration::from_millis(timeout);

    while Instant::now() < deadline {
        if let Some(chunk) = read_output_chunk(&runtime_session, requested_cursor).await? {
            return Ok(TerminalPollResult {
                changed: true,
                chunk: Some(chunk),
            });
        }
        sleep(Duration::from_millis(DEFAULT_POLL_INTERVAL_MS)).await;
    }

    Ok(TerminalPollResult {
        changed: false,
        chunk: None,
    })
}

async fn save_terminal_session(
    state: &AppState,
    session: TerminalSessionRecord,
) -> anyhow::Result<TerminalSessionRecord> {
    let saved = store_save_session(&state.storage.store, session).await?;
    state.request_terminal_cleanup();
    Ok(saved)
}

pub(super) async fn cleanup_expired_sessions(state: &AppState) -> anyhow::Result<Option<i64>> {
    let sessions = store_list_sessions(&state.storage.store).await?;
    let now = now_ms();
    let mut next_expiry_ms = None;
    for session in sessions {
        if parse_iso_ms(&session.expires_at).is_some_and(|expires_at| expires_at <= now) {
            if let Err(error) = terminal_kill_session(state, &session.id).await {
                tracing::warn!(session_id = %session.id, %error, "failed to cleanup expired terminal session");
            }
            continue;
        }
        if !tmux_session_exists(&session.backend_session_name).await {
            cleanup_session_artifacts(&session).await;
            store_delete_session(&state.storage.store, &session.id).await?;
            continue;
        }
        if let Some(expires_at) = parse_iso_ms(&session.expires_at) {
            next_expiry_ms =
                Some(next_expiry_ms.map_or(expires_at, |current: i64| current.min(expires_at)));
        }
        if session.status == "attached" {
            let attachments =
                store_list_attachment_ids_for_session(&state.storage.store, &session.id).await?;
            if attachments.is_empty() {
                mark_session_detached(state, &session.id).await?;
            }
        }
    }
    Ok(next_expiry_ms)
}

pub(super) async fn assert_create_allowed(state: &AppState) -> anyhow::Result<()> {
    let status = runtime_status(state).await?;
    if status.blocked_reason.is_empty() {
        Ok(())
    } else {
        Err(anyhow!(status.blocked_reason))
    }
}

pub(super) async fn tmux_session_exists(session_name: &str) -> bool {
    if session_name.trim().is_empty() {
        return false;
    }
    run_tmux(&["has-session", "-t", session_name])
        .await
        .is_ok_and(|result| result.code == 0)
}

pub(super) async fn read_pane_runtime_metadata(
    session: &TerminalSessionRecord,
) -> anyhow::Result<(String, i64, i64)> {
    let result = run_tmux(&[
        "display-message",
        "-p",
        "-t",
        &pane_target(session),
        "#{pane_tty}\t#{pane_width}\t#{pane_height}",
    ])
    .await?;
    if result.code != 0 {
        return Err(anyhow!(
            "{}",
            fallback_message(
                &result.stderr,
                &terminal_default_text("paneMetadataReadFailed", &[])
            )
        ));
    }
    let mut parts = result.stdout.split('\t');
    let pane_tty_path = parts.next().unwrap_or("").trim().to_string();
    if pane_tty_path.is_empty() {
        return Err(anyhow!(terminal_default_text("paneTtyParseFailed", &[])));
    }
    let cols = parse_tmux_number(parts.next().unwrap_or(""), session.cols);
    let rows = parse_tmux_number(parts.next().unwrap_or(""), session.rows);
    Ok((pane_tty_path, cols, rows))
}

pub(super) async fn is_relay_pipe_active(session: &TerminalSessionRecord) -> bool {
    let Ok(result) = run_tmux(&[
        "display-message",
        "-p",
        "-t",
        &pane_target(session),
        "#{?pane_pipe,1,0}",
    ])
    .await
    else {
        return false;
    };
    result.code == 0 && result.stdout.trim() == "1"
}

pub(super) async fn ensure_output_log_path(
    state: &AppState,
    session: &TerminalSessionRecord,
) -> anyhow::Result<PathBuf> {
    ensure_stream_directory(state).await?;
    let path = if session.output_log_path.trim().is_empty() {
        build_output_log_path(&stream_directory(state), &session.id)
    } else {
        PathBuf::from(session.output_log_path.trim())
    };
    let _file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .await
        .with_context(|| format!("open terminal output log {}", path.display()))?;
    Ok(path)
}

pub(super) async fn ensure_input_pipe_path(
    state: &AppState,
    session: &TerminalSessionRecord,
) -> anyhow::Result<PathBuf> {
    ensure_stream_directory(state).await?;
    let path = if session.input_pipe_path.trim().is_empty() {
        build_input_pipe_path(&stream_directory(state), &session.id)
    } else {
        PathBuf::from(session.input_pipe_path.trim())
    };
    if let Ok(metadata) = fs::metadata(&path).await {
        if is_fifo(&metadata) {
            return Ok(path);
        }
        let _ = fs::remove_file(&path).await;
    }
    let result = run_process("mkfifo", &[path_to_str(&path)?], None, true).await?;
    if result.code != 0 {
        return Err(anyhow!(
            "{}",
            fallback_message(
                &result.stderr,
                &terminal_default_text("inputPipeCreateFailed", &[])
            )
        ));
    }
    Ok(path)
}

pub(super) async fn configure_relay_pipe(
    session: &TerminalSessionRecord,
    output_log_path: &Path,
    input_pipe_path: &Path,
) -> anyhow::Result<()> {
    let relay = build_relay_command(output_log_path, input_pipe_path)?;
    let result = run_tmux(&["pipe-pane", "-I", "-O", "-t", &pane_target(session), &relay]).await?;
    if result.code != 0 {
        return Err(anyhow!(
            "{}",
            fallback_message(
                &result.stderr,
                &terminal_default_text("ioRelayCreateFailed", &[])
            )
        ));
    }
    Ok(())
}

pub(super) async fn configure_session_runtime(
    state: &AppState,
    session: TerminalSessionRecord,
) -> anyhow::Result<TerminalSessionRecord> {
    let output_log_path = ensure_output_log_path(state, &session).await?;
    let input_pipe_path = ensure_input_pipe_path(state, &session).await?;
    let (pane_tty_path, cols, rows) = read_pane_runtime_metadata(&session).await?;
    configure_relay_pipe(&session, &output_log_path, &input_pipe_path).await?;
    save_terminal_session(
        state,
        normalize_session(TerminalSessionRecord {
            cols,
            rows,
            pane_tty_path,
            input_pipe_path: input_pipe_path.to_string_lossy().to_string(),
            output_log_path: output_log_path.to_string_lossy().to_string(),
            updated_at: now_iso(),
            ..session
        }),
    )
    .await
}

pub(super) async fn ensure_session_runtime(
    state: &AppState,
    session: TerminalSessionRecord,
) -> anyhow::Result<TerminalSessionRecord> {
    let stream_dir = stream_directory(state);
    let output_log_path = if session.output_log_path.trim().is_empty() {
        build_output_log_path(&stream_dir, &session.id)
    } else {
        PathBuf::from(session.output_log_path.trim())
    };
    let input_pipe_path = if session.input_pipe_path.trim().is_empty() {
        build_input_pipe_path(&stream_dir, &session.id)
    } else {
        PathBuf::from(session.input_pipe_path.trim())
    };
    let output_exists = fs::metadata(&output_log_path)
        .await
        .is_ok_and(|metadata| metadata.is_file());
    let input_exists = fs::metadata(&input_pipe_path)
        .await
        .is_ok_and(|metadata| is_fifo(&metadata));
    let relay_active = !session.pane_tty_path.trim().is_empty()
        && output_exists
        && input_exists
        && is_relay_pipe_active(&session).await;

    if relay_active {
        if !session.output_log_path.trim().is_empty() && !session.input_pipe_path.trim().is_empty()
        {
            return Ok(session);
        }
        return save_terminal_session(
            state,
            normalize_session(TerminalSessionRecord {
                input_pipe_path: input_pipe_path.to_string_lossy().to_string(),
                output_log_path: output_log_path.to_string_lossy().to_string(),
                updated_at: now_iso(),
                ..session
            }),
        )
        .await;
    }

    configure_session_runtime(
        state,
        normalize_session(TerminalSessionRecord {
            input_pipe_path: input_pipe_path.to_string_lossy().to_string(),
            output_log_path: output_log_path.to_string_lossy().to_string(),
            ..session
        }),
    )
    .await
}

pub(super) async fn refresh_session_expiry(
    state: &AppState,
    session: TerminalSessionRecord,
) -> anyhow::Result<TerminalSessionRecord> {
    let config = terminal_feature_config(state).await?;
    save_terminal_session(
        state,
        normalize_session(TerminalSessionRecord {
            updated_at: now_iso(),
            expires_at: iso_after_seconds(config.idle_timeout_seconds),
            ..session
        }),
    )
    .await
}

pub(super) async fn touch_session_activity(
    state: &AppState,
    session: TerminalSessionRecord,
    force: bool,
) -> anyhow::Result<TerminalSessionRecord> {
    let now = now_ms();
    let next_allowed = {
        let deadlines = SESSION_TOUCH_DEADLINES.lock().await;
        deadlines.get(&session.id).copied().unwrap_or(0)
    };
    let normalized = normalize_session(TerminalSessionRecord {
        updated_at: now_iso(),
        ..session
    });
    if !force && now < next_allowed {
        return Ok(normalized);
    }
    let saved = refresh_session_expiry(state, normalized).await?;
    SESSION_TOUCH_DEADLINES
        .lock()
        .await
        .insert(saved.id.clone(), now + INPUT_SESSION_TOUCH_THROTTLE_MS);
    Ok(saved)
}

pub(super) async fn mark_session_detached(
    state: &AppState,
    session_id: &str,
) -> anyhow::Result<Option<TerminalSessionRecord>> {
    let Some(session) = store_get_session(&state.storage.store, session_id).await? else {
        return Ok(None);
    };
    let saved = refresh_session_expiry(
        state,
        normalize_session(TerminalSessionRecord {
            status: "detached".to_string(),
            updated_at: now_iso(),
            last_detached_at: now_iso(),
            ..session
        }),
    )
    .await?;
    Ok(Some(saved))
}

pub(super) async fn cleanup_session_artifacts(session: &TerminalSessionRecord) {
    SESSION_TOUCH_DEADLINES.lock().await.remove(&session.id);
    if !session.input_pipe_path.trim().is_empty() {
        let _ = fs::remove_file(session.input_pipe_path.trim()).await;
    }
    if !session.output_log_path.trim().is_empty() {
        let _ = fs::remove_file(session.output_log_path.trim()).await;
    }
}

pub(super) async fn write_input_pipe(path: &str, data: Vec<u8>) -> io::Result<()> {
    let path = PathBuf::from(path.trim());
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut last_error: Option<io::Error> = None;
    while Instant::now() < deadline {
        let path_for_write = path.clone();
        let data_for_write = data.clone();
        match task::spawn_blocking(move || {
            write_input_pipe_blocking(path_for_write, data_for_write)
        })
        .await
        .map_err(|error| io::Error::other(error.to_string()))?
        {
            Ok(()) => return Ok(()),
            Err(error) if error.raw_os_error() == Some(libc::ENXIO) => {
                last_error = Some(error);
                sleep(Duration::from_millis(50)).await;
            }
            Err(error) => return Err(error),
        }
    }
    Err(last_error
        .unwrap_or_else(|| io::Error::other(terminal_default_text("inputPipeNotReady", &[]))))
}

#[cfg(unix)]
pub(super) fn write_input_pipe_blocking(path: PathBuf, data: Vec<u8>) -> io::Result<()> {
    use std::{io::Write, os::unix::fs::OpenOptionsExt};

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)?;
    file.write_all(&data)
}

#[cfg(not(unix))]
pub(super) fn write_input_pipe_blocking(path: PathBuf, data: Vec<u8>) -> io::Result<()> {
    use std::io::Write;

    let mut file = std::fs::OpenOptions::new().write(true).open(path)?;
    file.write_all(&data)
}

pub(super) async fn read_output_chunk(
    session: &TerminalSessionRecord,
    requested_cursor: i64,
) -> anyhow::Result<Option<TerminalOutputChunk>> {
    let output_log_path = session.output_log_path.trim();
    let updated_at = now_iso();
    if output_log_path.is_empty() {
        return capture_pane_snapshot_chunk(session, 0, updated_at)
            .await
            .map(Some);
    }

    let Ok(metadata) = fs::metadata(output_log_path).await else {
        return capture_pane_snapshot_chunk(session, 0, updated_at)
            .await
            .map(Some);
    };
    if !metadata.is_file() {
        return capture_pane_snapshot_chunk(session, 0, updated_at)
            .await
            .map(Some);
    }
    let size = i64::try_from(metadata.len()).unwrap_or(i64::MAX);
    if requested_cursor <= 0 || requested_cursor > size {
        return capture_pane_snapshot_chunk(session, size, updated_at)
            .await
            .map(Some);
    }
    if requested_cursor >= size {
        return Ok(None);
    }

    let bytes_to_read = (size - requested_cursor).min(TERMINAL_STREAM_CHUNK_MAX_BYTES);
    let mut file = fs::File::open(output_log_path).await?;
    file.seek(std::io::SeekFrom::Start(requested_cursor as u64))
        .await?;
    let mut buffer = vec![0_u8; bytes_to_read as usize];
    let bytes_read = file.read(&mut buffer).await?;
    if bytes_read == 0 {
        return Ok(None);
    }
    buffer.truncate(bytes_read);
    Ok(Some(TerminalOutputChunk {
        cursor: requested_cursor + bytes_read as i64,
        data_base64: general_purpose::STANDARD.encode(&buffer),
        reset: false,
        updated_at,
    }))
}

pub(super) async fn capture_pane_snapshot_chunk(
    session: &TerminalSessionRecord,
    cursor: i64,
    updated_at: String,
) -> anyhow::Result<TerminalOutputChunk> {
    let rows = session
        .rows
        .max((session.rows * 2).min(TERMINAL_SNAPSHOT_SCROLLBACK_ROWS));
    let result = run_tmux_raw(&[
        "capture-pane",
        "-p",
        "-e",
        "-t",
        &pane_target(session),
        "-S",
        &format!("-{rows}"),
    ])
    .await
    .ok();
    let snapshot = result
        .filter(|result| result.code == 0)
        .map(|result| normalize_pane_snapshot_output(&result.stdout))
        .unwrap_or_default();
    Ok(TerminalOutputChunk {
        cursor,
        data_base64: general_purpose::STANDARD.encode(snapshot.as_bytes()),
        reset: true,
        updated_at,
    })
}

pub(super) fn normalize_pane_snapshot_output(output: &str) -> String {
    let trimmed = output.trim_end_matches([' ', '\t', '\r', '\n']);
    if trimmed.is_empty() {
        String::new()
    } else {
        let mut normalized = String::with_capacity(trimmed.len());
        let mut chars = trimmed.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '\r' && chars.peek().is_some_and(|next| *next == '\n') {
                chars.next();
                normalized.push_str("\r\n");
            } else if ch == '\n' {
                normalized.push_str("\r\n");
            } else {
                normalized.push(ch);
            }
        }
        normalized
    }
}
