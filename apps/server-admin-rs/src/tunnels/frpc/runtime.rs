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
    let handle = ensure_frpc_supervisor(state, meta).await?;
    let supervisor = handle.snapshot();
    let runtime = read_runtime(&state.storage.store, &meta.id).await?;
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
        desired_running: supervisor.desired_running,
        running: supervisor.running,
        attached: supervisor.attached,
        pid: supervisor.pid,
        started_at: supervisor.started_at.clone(),
        stopped_at: supervisor.stopped_at.clone(),
        last_exit_code: if supervisor.running {
            None
        } else {
            supervisor
                .last_failure
                .as_ref()
                .and_then(|failure| failure.exit_code)
                .or(runtime.last_exit_code)
        },
        last_message: supervisor.last_message.clone().or(runtime.last_message),
        supervisor,
        summary: build_summary(&content),
    })
}

pub(super) async fn read_candidate_pid(
    meta: &FrpcInstanceMeta,
    runtime: &FrpcInstanceRuntime,
) -> Option<u32> {
    if let Some(pid) = runtime.pid
        && is_owned_frpc_pid(pid, &meta.config_path).await
    {
        return Some(pid);
    }
    if let Some(pid) = read_pid_file(&pid_path_for_meta(meta)).await
        && is_owned_frpc_pid(pid, &meta.config_path).await
    {
        return Some(pid);
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
    if normalized.chars().count() <= 4000 {
        normalized
    } else {
        format!("{}...", normalized.chars().take(4000).collect::<String>())
    }
}

pub(super) async fn start_instance_inner(state: &AppState, id: &str) -> FrpcResult<u32> {
    let meta = get_meta_or_error(state, id).await?;
    let handle = ensure_frpc_supervisor(state, &meta).await?;
    handle.start().await.map_err(frpc_internal)
}

pub(super) async fn stop_instance_inner(state: &AppState, id: &str) -> FrpcResult<()> {
    let meta = get_meta_or_error(state, id).await?;
    let handle = ensure_frpc_supervisor(state, &meta).await?;
    handle.stop().await.map_err(frpc_internal)
}

pub(super) async fn restart_instance_inner(state: &AppState, id: &str) -> FrpcResult<u32> {
    let meta = get_meta_or_error(state, id).await?;
    let handle = ensure_frpc_supervisor(state, &meta).await?;
    handle.restart().await.map_err(frpc_internal)
}

pub(super) async fn list_logs_inner(
    state: &AppState,
    id: &str,
    limit: usize,
) -> FrpcResult<Vec<String>> {
    let meta = get_meta_or_error(state, id).await?;
    Ok(state
        .storage
        .store
        .list_log_buffer(&log_key(&meta.id), limit, log_max_len(&meta.id))
        .await?)
}

pub(super) async fn clear_logs_inner(state: &AppState, id: &str) -> FrpcResult<()> {
    let meta = get_meta_or_error(state, id).await?;
    state
        .storage
        .store
        .clear_log_buffer(&log_key(&meta.id))
        .await?;
    Ok(())
}

pub(super) async fn poll_inner(
    state: &AppState,
    id: &str,
    cursor: Option<&str>,
) -> FrpcResult<Value> {
    let meta = get_meta_or_error(state, id).await?;
    let logs = state
        .storage
        .store
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
        .map(|line| crate::tunnels::supervisor::bounded_log_line(line.trim_end()))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    state
        .storage
        .store
        .append_log_buffer(
            &log_key(&meta.id),
            &normalized,
            LOG_TTL_SEC,
            log_max_len(&meta.id),
        )
        .await?;
    Ok(())
}

pub(super) fn normalize_frpc_tunnel_event_message(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let line = ["[ERR]", "[OUT]"]
        .into_iter()
        .find_map(|prefix| {
            trimmed
                .get(..prefix.len())
                .is_some_and(|candidate| candidate.eq_ignore_ascii_case(prefix))
                .then(|| trimmed[prefix.len()..].trim_start())
        })
        .unwrap_or(line);
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
        let mut runtime = read_runtime(&state.storage.store, FRPC_PRIMARY_INSTANCE_ID).await?;
        runtime.desired_running = true;
        write_runtime(&state.storage.store, FRPC_PRIMARY_INSTANCE_ID, &runtime).await?;
    }
    let metas = all_metas(state).await?;
    for meta in metas {
        let runtime = read_runtime(&state.storage.store, &meta.id).await?;
        let _ = ensure_frpc_supervisor(state, &meta).await?;
        if runtime.desired_running {
            append_logs(state, &meta, &[default_frpc_text("resumeOnBoot")]).await?;
        }
    }
    update_aggregate_tunnel_state(state).await?;
    Ok(())
}

pub(super) async fn has_any_runtime_data(state: &AppState) -> FrpcResult<bool> {
    for id in read_instance_ids(&state.storage.store).await? {
        if state
            .storage
            .store
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
    let _guard = state.tunnel.runtime_update_lock.lock().await;
    let mut desired = false;
    for id in read_instance_ids(&state.storage.store).await? {
        if read_runtime(&state.storage.store, &id)
            .await?
            .desired_running
        {
            desired = true;
            break;
        }
    }
    if desired {
        mark_tunnel_running(state).await?;
    } else {
        mark_tunnel_stopped(state).await?;
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

async fn mark_tunnel_running(state: &AppState) -> anyhow::Result<()> {
    let mut object = load_tunnel_state(state).await;
    object.insert("frp_enabled".to_string(), Value::Bool(true));
    object.insert("last_tunnel".to_string(), Value::String("frp".to_string()));
    object.insert(
        "updated_at".to_string(),
        Value::String(time_utils::now_iso()),
    );
    state
        .storage
        .store
        .set_json_value(TUNNEL_RUNTIME_KEY, &Value::Object(object))
        .await?;
    Ok(())
}

async fn mark_tunnel_stopped(state: &AppState) -> anyhow::Result<()> {
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
        state
            .storage
            .store
            .set_json_value(TUNNEL_RUNTIME_KEY, &Value::Object(object))
            .await?;
    }
    Ok(())
}

pub(super) async fn load_tunnel_state(state: &AppState) -> serde_json::Map<String, Value> {
    let Some(raw) = state
        .storage
        .store
        .get_json_value(TUNNEL_RUNTIME_KEY)
        .await
        .ok()
        .flatten()
    else {
        return default_tunnel_state();
    };
    let Some(raw_object) = raw.as_object() else {
        return default_tunnel_state();
    };
    let updated_at = raw_object
        .get("updated_at")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(time_utils::now_iso);
    let mut object = if !raw_object.contains_key("frp_enabled")
        && !raw_object.contains_key("cloudflared_enabled")
        && raw_object.contains_key("tunnel")
        && raw_object.contains_key("enabled")
    {
        let tunnel = raw.get("tunnel").and_then(Value::as_str).unwrap_or("frp");
        let enabled = raw.get("enabled").and_then(Value::as_bool).unwrap_or(false);
        json!({
            "frp_enabled": tunnel == "frp" && enabled,
            "cloudflared_enabled": tunnel == "cloudflared" && enabled,
            "last_tunnel": if tunnel == "cloudflared" { "cloudflared" } else { "frp" },
            "updated_at": updated_at
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
                .map(str::to_string)
                .unwrap_or_else(time_utils::now_iso),
        ),
    );
    normalized
}

pub(super) fn default_tunnel_state() -> serde_json::Map<String, Value> {
    let mut normalized = serde_json::Map::new();
    normalized.insert("frp_enabled".to_string(), Value::Bool(false));
    normalized.insert("cloudflared_enabled".to_string(), Value::Bool(false));
    normalized.insert("last_tunnel".to_string(), Value::String("frp".to_string()));
    normalized.insert(
        "updated_at".to_string(),
        Value::String("1970-01-01T00:00:00.000Z".to_string()),
    );
    normalized
}
