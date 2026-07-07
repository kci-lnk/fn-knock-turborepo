use super::*;

pub(super) async fn build_overview(state: &AppState) -> FrpcResult<FrpcInstancesOverview> {
    let metas = all_metas(state).await?;
    let mut items = Vec::new();
    for meta in metas {
        items.push(build_status(state, &meta).await?);
    }
    Ok(FrpcInstancesOverview {
        initialized: frp_executable(state).is_some(),
        platform: detect_frp_platform().to_string(),
        primary_instance_id: FRPC_PRIMARY_INSTANCE_ID.to_string(),
        total: items.len(),
        extra_count: items.iter().filter(|item| !item.is_primary).count(),
        running_count: items.iter().filter(|item| item.running).count(),
        defaults: json!({ "local_port": std::env::var("GO_REPROXY_PORT").unwrap_or_else(|_| "7999".to_string()) }),
        items,
    })
}

pub(super) async fn build_status(
    state: &AppState,
    meta: &FrpcInstanceMeta,
) -> FrpcResult<FrpcInstanceStatus> {
    let runtime = reconcile_runtime(state, meta).await?;
    let content = read_config_for_meta(meta).await?;
    Ok(FrpcInstanceStatus {
        id: meta.id.clone(),
        name: meta.name.clone(),
        is_primary: meta.is_primary,
        config_path: meta.config_path.clone(),
        work_dir: meta.work_dir.clone(),
        created_at: meta.created_at.clone(),
        updated_at: meta.updated_at.clone(),
        sort_order: meta.sort_order,
        desired_running: runtime.0.desired_running,
        running: runtime.1,
        attached: runtime.2,
        pid: runtime.0.pid,
        started_at: runtime.0.started_at,
        stopped_at: runtime.0.stopped_at,
        last_exit_code: runtime.0.last_exit_code,
        last_message: runtime.0.last_message,
        summary: build_summary(&content),
    })
}

pub(super) async fn reconcile_runtime(
    state: &AppState,
    meta: &FrpcInstanceMeta,
) -> FrpcResult<(FrpcInstanceRuntime, bool, bool)> {
    let runtime = read_runtime(&state.redis, &meta.id).await?;
    let original_runtime = runtime.clone();
    let pid = read_candidate_pid(meta, &runtime).await;
    let attached = if let Some(pid) = pid {
        ATTACHED_PIDS.lock().await.get(&meta.id).copied() == Some(pid)
    } else {
        false
    };
    if let Some(pid) = pid {
        let next = merge_detected_frpc_runtime(runtime, pid);
        if should_persist_detected_runtime(&original_runtime, &next) {
            write_runtime(&state.redis, &meta.id, &next).await?;
        }
        write_pid_file(&pid_path_for_meta(meta), pid).await;
        return Ok((next, true, attached));
    }
    let had_pid = runtime.pid.is_some() || read_pid_file(&pid_path_for_meta(meta)).await.is_some();
    remove_pid_file(&pid_path_for_meta(meta)).await;
    if runtime.pid.is_some() || had_pid {
        let mut next = runtime;
        next.pid = None;
        if next.stopped_at.is_none() {
            next.stopped_at = Some(time_utils::now_iso());
        }
        if next.last_message.is_none() {
            next.last_message = Some("frpc pid is no longer running".to_string());
        }
        write_runtime(&state.redis, &meta.id, &next).await?;
        return Ok((next, false, false));
    }
    Ok((runtime, false, false))
}

pub(super) fn merge_detected_frpc_runtime(
    mut runtime: FrpcInstanceRuntime,
    pid: u32,
) -> FrpcInstanceRuntime {
    let preserve_message = runtime.pid == Some(pid)
        && runtime.stopped_at.is_none()
        && runtime.last_exit_code.is_none()
        && runtime.last_message.is_some();
    runtime.pid = Some(pid);
    if runtime.started_at.is_none() {
        runtime.started_at = Some(time_utils::now_iso());
    }
    runtime.stopped_at = None;
    runtime.last_exit_code = None;
    if !preserve_message {
        runtime.last_message = Some(format!("frpc process detected pid={pid}"));
    }
    runtime
}

pub(super) fn should_persist_detected_runtime(
    left: &FrpcInstanceRuntime,
    right: &FrpcInstanceRuntime,
) -> bool {
    left.pid != right.pid
        || left.started_at != right.started_at
        || left.stopped_at != right.stopped_at
        || left.last_exit_code != right.last_exit_code
        || left.last_message != right.last_message
}

pub(super) async fn read_candidate_pid(
    meta: &FrpcInstanceMeta,
    runtime: &FrpcInstanceRuntime,
) -> Option<u32> {
    if let Some(pid) = ATTACHED_PIDS.lock().await.get(&meta.id).copied() {
        if is_owned_frpc_pid(pid, &meta.config_path).await {
            return Some(pid);
        }
    }
    if let Some(pid) = runtime.pid {
        if is_owned_frpc_pid(pid, &meta.config_path).await {
            return Some(pid);
        }
    }
    if let Some(pid) = read_pid_file(&pid_path_for_meta(meta)).await {
        if is_owned_frpc_pid(pid, &meta.config_path).await {
            return Some(pid);
        }
    }
    find_frpc_pid_by_config_path(&meta.config_path).await
}

pub(super) async fn verify_frpc_config(
    state: &AppState,
    meta: &FrpcInstanceMeta,
    content: &str,
) -> FrpcResult<()> {
    let Some(bin) = frp_executable(state) else {
        return Err(frpc_validation("FRP is not initialized"));
    };
    fs::create_dir_all(&meta.work_dir).await?;
    let temp = PathBuf::from(&meta.work_dir).join(format!("frpc.verify.{}.toml", Uuid::new_v4()));
    fs::write(&temp, content).await?;
    let output = Command::new(&bin)
        .arg("verify")
        .arg("-c")
        .arg(&temp)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await;
    let _ = fs::remove_file(&temp).await;
    let output = output
        .map_err(|error| frpc_validation(format!("Failed to verify frpc config: {error}")))?;
    if output.status.success() {
        return Ok(());
    }
    let detail = normalize_verify_output(&format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    ));
    if detail.is_empty() {
        Err(frpc_validation(format!(
            "frpc config verify failed with code {}",
            output.status.code().unwrap_or(-1)
        )))
    } else {
        Err(frpc_validation(format!(
            "frpc config verify failed: {detail}"
        )))
    }
}

pub(super) fn normalize_verify_output(value: &str) -> String {
    let normalized = value
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    if normalized.len() <= 4000 {
        normalized
    } else {
        format!("{}...", &normalized[..4000])
    }
}

pub(super) async fn start_instance_inner(state: &AppState, id: &str) -> FrpcResult<u32> {
    let meta = get_meta_or_error(state, id).await?;
    let Some(bin) = frp_executable(state) else {
        return Err(frpc_validation("FRP is not initialized"));
    };
    let content = read_config_for_meta(&meta).await?;
    verify_frpc_config(state, &meta, &content).await?;
    let current = build_status(state, &meta).await?;
    if current.running {
        if let Some(pid) = current.pid {
            let mut runtime = read_runtime(&state.redis, &meta.id).await?;
            runtime.desired_running = true;
            runtime.pid = Some(pid);
            write_runtime(&state.redis, &meta.id, &runtime).await?;
            return Ok(pid);
        }
    }
    {
        let mut states = CONNECTION_STATES.lock().await;
        let connection = states.entry(meta.id.clone()).or_default();
        connection.stop_requested = false;
    }
    let mut child = Command::new(bin)
        .arg("-c")
        .arg(&meta.config_path)
        .current_dir(&meta.work_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| frpc_internal(format!("Failed to start frpc: {error}")))?;
    let pid = child
        .id()
        .ok_or_else(|| frpc_internal("Failed to read frpc pid"))?;
    if let Some(stdout) = child.stdout.take() {
        spawn_log_reader(state.clone(), meta.clone(), stdout, false);
    }
    if let Some(stderr) = child.stderr.take() {
        spawn_log_reader(state.clone(), meta.clone(), stderr, true);
    }
    ATTACHED_PIDS.lock().await.insert(meta.id.clone(), pid);
    write_pid_file(&pid_path_for_meta(&meta), pid).await;
    write_runtime(
        &state.redis,
        &meta.id,
        &FrpcInstanceRuntime {
            desired_running: true,
            pid: Some(pid),
            started_at: Some(time_utils::now_iso()),
            stopped_at: None,
            last_exit_code: None,
            last_message: Some(format!("frpc started pid={pid}")),
        },
    )
    .await?;
    append_logs(state, &meta, &[format!("frpc started pid={pid}")]).await?;
    mark_tunnel_running(state).await;
    spawn_exit_watcher(state.clone(), meta.clone(), child);
    Ok(pid)
}

pub(super) fn spawn_log_reader<R>(state: AppState, meta: FrpcInstanceMeta, reader: R, stderr: bool)
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let line = if stderr {
                format!("[ERR] {line}")
            } else {
                line
            };
            if let Err(error) = append_logs(&state, &meta, &[line]).await {
                tracing::warn!(instance_id = %meta.id, %error, "failed to append frpc process log");
            }
        }
    });
}

pub(super) fn spawn_exit_watcher(
    state: AppState,
    meta: FrpcInstanceMeta,
    mut child: tokio::process::Child,
) {
    tokio::spawn(async move {
        let pid = child.id();
        let status = child.wait().await;
        let code = status
            .as_ref()
            .ok()
            .and_then(|status| status.code())
            .unwrap_or(-1);
        let was_attached = ATTACHED_PIDS.lock().await.remove(&meta.id).is_some();
        let expected_stop = {
            let states = CONNECTION_STATES.lock().await;
            states
                .get(&meta.id)
                .map(|state| state.stop_requested)
                .unwrap_or(false)
                || !was_attached
        };
        remove_pid_file(&pid_path_for_meta(&meta)).await;
        let message = match status {
            Ok(_) => format!("frpc exited with code {code}"),
            Err(error) => format!("frpc process error: {error}"),
        };
        let mut runtime = read_runtime(&state.redis, &meta.id)
            .await
            .unwrap_or_else(|_| default_runtime());
        runtime.pid = None;
        runtime.stopped_at = Some(time_utils::now_iso());
        runtime.last_exit_code = Some(code);
        runtime.last_message = Some(message.clone());
        let _ = write_runtime(&state.redis, &meta.id, &runtime).await;
        let _ = append_logs(&state, &meta, &[message]).await;
        if !expected_stop {
            let exit_message = runtime.last_message.as_deref();
            emit_frpc_connectivity(&state, &meta, false, exit_message, pid).await;
        }
        if let Some(connection) = CONNECTION_STATES.lock().await.get_mut(&meta.id) {
            connection.stop_requested = false;
        }
        let _ = update_aggregate_tunnel_state(&state).await;
    });
}

pub(super) async fn stop_instance_inner(state: &AppState, id: &str) -> FrpcResult<()> {
    let meta = get_meta_or_error(state, id).await?;
    let status = build_status(state, &meta).await?;
    {
        let mut states = CONNECTION_STATES.lock().await;
        let connection = states.entry(meta.id.clone()).or_default();
        connection.stop_requested = true;
        connection.connected = false;
    }
    if let Some(pid) = status.pid {
        if is_owned_frpc_pid(pid, &meta.config_path).await {
            terminate_pid(pid).await?;
        }
    }
    ATTACHED_PIDS.lock().await.remove(&meta.id);
    remove_pid_file(&pid_path_for_meta(&meta)).await;
    let mut runtime = read_runtime(&state.redis, &meta.id).await?;
    runtime.desired_running = false;
    runtime.pid = None;
    runtime.stopped_at = Some(time_utils::now_iso());
    runtime.last_message = Some(
        status
            .pid
            .map(|pid| format!("frpc stopped pid={pid}"))
            .unwrap_or_else(|| "frpc already stopped".to_string()),
    );
    write_runtime(&state.redis, &meta.id, &runtime).await?;
    if let Some(pid) = status.pid {
        append_logs(state, &meta, &[format!("frpc stopped pid={pid}")]).await?;
    }
    if let Some(connection) = CONNECTION_STATES.lock().await.get_mut(&meta.id) {
        connection.stop_requested = false;
    }
    update_aggregate_tunnel_state(state).await?;
    Ok(())
}

pub(super) async fn list_logs_inner(
    state: &AppState,
    id: &str,
    limit: usize,
) -> FrpcResult<Vec<String>> {
    let meta = get_meta_or_error(state, id).await?;
    Ok(state
        .redis
        .list_log_buffer(&log_key(&meta.id), limit, log_max_len(&meta.id))
        .await?)
}

pub(super) async fn clear_logs_inner(state: &AppState, id: &str) -> FrpcResult<()> {
    let meta = get_meta_or_error(state, id).await?;
    state.redis.clear_log_buffer(&log_key(&meta.id)).await?;
    Ok(())
}

pub(super) async fn poll_inner(
    state: &AppState,
    id: &str,
    cursor: Option<&str>,
) -> FrpcResult<Value> {
    let meta = get_meta_or_error(state, id).await?;
    let logs = state
        .redis
        .poll_log_buffer(&log_key(&meta.id), cursor)
        .await?;
    let status = build_status(state, &meta).await?;
    Ok(json!({
        "cursor": logs.get("cursor").cloned().unwrap_or(json!(0)),
        "reset": logs.get("reset").cloned().unwrap_or(json!(false)),
        "logs": logs.get("items").and_then(Value::as_array).cloned().unwrap_or_default().into_iter().filter_map(|item| item.as_str().map(str::to_string)).collect::<Vec<_>>(),
        "status": status
    }))
}

pub(super) async fn append_logs(
    state: &AppState,
    meta: &FrpcInstanceMeta,
    lines: &[String],
) -> anyhow::Result<()> {
    let normalized = lines
        .iter()
        .map(|line| line.trim_end().to_string())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    state
        .redis
        .append_log_buffer(
            &log_key(&meta.id),
            &normalized,
            LOG_TTL_SEC,
            log_max_len(&meta.id),
        )
        .await?;
    handle_frpc_runtime_signals(state, meta, &normalized).await;
    Ok(())
}

pub(super) async fn handle_frpc_runtime_signals(
    state: &AppState,
    meta: &FrpcInstanceMeta,
    lines: &[String],
) {
    for line in lines {
        let Some(message) = normalize_frpc_tunnel_event_message(line) else {
            continue;
        };
        let normalized = message.to_ascii_lowercase();
        let pid = ATTACHED_PIDS.lock().await.get(&meta.id).copied();
        if FRPC_CONNECTED_PATTERNS
            .iter()
            .any(|pattern| normalized.contains(pattern))
        {
            emit_frpc_connectivity(state, meta, true, Some(&message), pid).await;
            continue;
        }
        if FRPC_DISCONNECTED_PATTERNS
            .iter()
            .any(|pattern| normalized.contains(pattern))
        {
            emit_frpc_connectivity(state, meta, false, Some(&message), pid).await;
        }
    }
}

pub(super) async fn emit_frpc_connectivity(
    state: &AppState,
    meta: &FrpcInstanceMeta,
    connected: bool,
    message: Option<&str>,
    pid: Option<u32>,
) {
    {
        let mut states = CONNECTION_STATES.lock().await;
        let connection = states.entry(meta.id.clone()).or_default();
        if connected {
            if connection.connected {
                return;
            }
            connection.connected = true;
        } else {
            if !connection.connected {
                return;
            }
            connection.connected = false;
            if connection.stop_requested {
                return;
            }
        }
    }

    let event_message = message.map(|value| format!("{}: {value}", meta.name));
    if let Err(error) = system_events::publish_tunnel_connectivity_event(
        state,
        "frp",
        connected,
        pid,
        event_message.as_deref(),
        Some(&meta.id),
        Some(&meta.name),
        Some(meta.is_primary),
    )
    .await
    {
        tracing::warn!(instance_id = %meta.id, %error, "failed to publish frpc connectivity event");
    }
}

pub(super) fn normalize_frpc_tunnel_event_message(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let line = if trimmed
        .get(..5)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("[ERR]"))
    {
        trimmed[5..].trim_start()
    } else {
        line
    };
    normalize_tunnel_event_message(line)
}

pub(super) fn normalize_tunnel_event_message(line: &str) -> Option<String> {
    let normalized = line.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return None;
    }
    if normalized.chars().count() <= 240 {
        return Some(normalized);
    }
    let truncated = normalized.chars().take(240).collect::<String>();
    Some(format!("{}...", truncated.trim()))
}

pub(super) fn log_max_len(id: &str) -> usize {
    if id == FRPC_PRIMARY_INSTANCE_ID {
        PRIMARY_LOG_MAX_LEN
    } else {
        EXTRA_LOG_MAX_LEN
    }
}

pub(super) async fn restore_on_boot(state: &AppState) -> FrpcResult<()> {
    let had_runtime = has_any_runtime_data(state).await?;
    ensure_primary_instance(state).await?;
    if !had_runtime && should_resume_tunnel(state).await {
        let mut runtime = read_runtime(&state.redis, FRPC_PRIMARY_INSTANCE_ID).await?;
        runtime.desired_running = true;
        write_runtime(&state.redis, FRPC_PRIMARY_INSTANCE_ID, &runtime).await?;
    }
    let metas = all_metas(state).await?;
    for meta in metas {
        let status = build_status(state, &meta).await?;
        if !status.desired_running || status.running {
            continue;
        }
        append_logs(state, &meta, &["resume on boot".to_string()]).await?;
        if let Err(error) = start_instance_inner(state, &meta.id).await {
            append_logs(state, &meta, &[format!("resume error: {}", error.message)]).await?;
        }
    }
    update_aggregate_tunnel_state(state).await?;
    Ok(())
}

pub(super) async fn has_any_runtime_data(state: &AppState) -> FrpcResult<bool> {
    for id in read_instance_ids(&state.redis).await? {
        if state
            .redis
            .get_json_value(&instance_key(&id, "runtime"))
            .await?
            .is_some()
        {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) async fn update_aggregate_tunnel_state(state: &AppState) -> anyhow::Result<()> {
    let overview = build_overview(state)
        .await
        .map_err(|error| anyhow!(error.message))?;
    if overview.running_count > 0 {
        mark_tunnel_running(state).await;
    } else {
        mark_tunnel_stopped(state).await;
    }
    Ok(())
}

pub(super) async fn should_resume_tunnel(state: &AppState) -> bool {
    load_tunnel_state(state)
        .await
        .get("frp_enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

pub(super) async fn mark_tunnel_running(state: &AppState) {
    let mut object = load_tunnel_state(state).await;
    object.insert("frp_enabled".to_string(), Value::Bool(true));
    object.insert("last_tunnel".to_string(), Value::String("frp".to_string()));
    object.insert(
        "updated_at".to_string(),
        Value::String(time_utils::now_iso()),
    );
    let _ = state
        .redis
        .set_json_value(TUNNEL_RUNTIME_KEY, &Value::Object(object))
        .await;
}

pub(super) async fn mark_tunnel_stopped(state: &AppState) {
    let mut object = load_tunnel_state(state).await;
    if object
        .get("frp_enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        object.insert("frp_enabled".to_string(), Value::Bool(false));
        object.insert(
            "updated_at".to_string(),
            Value::String(time_utils::now_iso()),
        );
        let _ = state
            .redis
            .set_json_value(TUNNEL_RUNTIME_KEY, &Value::Object(object))
            .await;
    }
}

pub(super) async fn load_tunnel_state(state: &AppState) -> serde_json::Map<String, Value> {
    let raw = state
        .redis
        .get_json_value(TUNNEL_RUNTIME_KEY)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| json!({}));
    let mut object = if !raw.get("frp_enabled").is_some() && raw.get("tunnel").is_some() {
        let tunnel = raw.get("tunnel").and_then(Value::as_str).unwrap_or("frp");
        let enabled = raw.get("enabled").and_then(Value::as_bool).unwrap_or(false);
        json!({
            "frp_enabled": tunnel == "frp" && enabled,
            "cloudflared_enabled": tunnel == "cloudflared" && enabled,
            "last_tunnel": if tunnel == "cloudflared" { "cloudflared" } else { "frp" },
            "updated_at": raw.get("updated_at").and_then(Value::as_str).unwrap_or("1970-01-01T00:00:00Z")
        })
    } else {
        raw
    };
    let object = object.as_object_mut().cloned().unwrap_or_default();
    let mut normalized = serde_json::Map::new();
    normalized.insert(
        "frp_enabled".to_string(),
        Value::Bool(
            object
                .get("frp_enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        ),
    );
    normalized.insert(
        "cloudflared_enabled".to_string(),
        Value::Bool(
            object
                .get("cloudflared_enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        ),
    );
    normalized.insert(
        "last_tunnel".to_string(),
        Value::String(
            if object.get("last_tunnel").and_then(Value::as_str) == Some("cloudflared") {
                "cloudflared"
            } else {
                "frp"
            }
            .to_string(),
        ),
    );
    normalized.insert(
        "updated_at".to_string(),
        Value::String(
            object
                .get("updated_at")
                .and_then(Value::as_str)
                .unwrap_or("1970-01-01T00:00:00Z")
                .to_string(),
        ),
    );
    normalized
}
