use super::*;

pub(super) fn frpc_dir(state: &AppState) -> PathBuf {
    state.settings.data_dir.join("frp")
}

pub(super) fn frpc_instances_dir(state: &AppState) -> PathBuf {
    frpc_dir(state).join("instances")
}

pub(super) fn primary_config_path(state: &AppState) -> PathBuf {
    frpc_dir(state).join("frpc.toml")
}

pub(super) fn extra_instance_paths(state: &AppState, id: &str) -> (PathBuf, PathBuf, PathBuf) {
    let work_dir = frpc_instances_dir(state).join(id);
    let config_path = work_dir.join("frpc.toml");
    let pid_path = work_dir.join("frpc.pid");
    (work_dir, config_path, pid_path)
}

pub(super) async fn ensure_layout(state: &AppState) -> anyhow::Result<()> {
    fs::create_dir_all(frpc_dir(state)).await?;
    fs::create_dir_all(frpc_instances_dir(state)).await?;
    Ok(())
}

pub(super) async fn ensure_primary_instance(state: &AppState) -> anyhow::Result<()> {
    ensure_layout(state).await?;
    let mut ids = read_instance_ids(&state.store).await?;
    if !ids.iter().any(|id| id == FRPC_PRIMARY_INSTANCE_ID) {
        ids.insert(0, FRPC_PRIMARY_INSTANCE_ID.to_string());
        write_instance_ids(&state.store, &ids).await?;
    }
    if read_meta(&state.store, state, FRPC_PRIMARY_INSTANCE_ID)
        .await?
        .is_none()
    {
        write_meta(&state.store, &primary_meta(state)).await?;
    }
    let config_path = primary_config_path(state);
    if !config_path.exists() {
        fs::write(config_path, default_frpc_template()).await?;
    }
    Ok(())
}

pub(super) fn default_frpc_template() -> String {
    let local_port = std::env::var("GO_REPROXY_PORT").unwrap_or_else(|_| "7999".to_string());
    [
        "serverAddr = \"\"".to_string(),
        "serverPort = 7000".to_string(),
        "".to_string(),
        "[auth]".to_string(),
        "method = \"token\"".to_string(),
        "token = \"\"".to_string(),
        "".to_string(),
        "[[proxies]]".to_string(),
        "name = \"reproxy\"".to_string(),
        "type = \"tcp\"".to_string(),
        "localIP = \"127.0.0.1\"".to_string(),
        format!("localPort = {local_port}"),
        "remotePort = 7999".to_string(),
        "transport.proxyProtocolVersion = \"v2\"".to_string(),
        "".to_string(),
    ]
    .join("\n")
}

pub(super) fn primary_meta(state: &AppState) -> FrpcInstanceMeta {
    let now = time_utils::now_iso();
    FrpcInstanceMeta {
        id: FRPC_PRIMARY_INSTANCE_ID.to_string(),
        name: default_frpc_primary_name(),
        is_primary: true,
        config_path: primary_config_path(state).to_string_lossy().to_string(),
        work_dir: frpc_dir(state).to_string_lossy().to_string(),
        created_at: now.clone(),
        updated_at: now,
        sort_order: 0,
    }
}

pub(super) fn fallback_meta(state: &AppState, id: &str) -> FrpcInstanceMeta {
    if id == FRPC_PRIMARY_INSTANCE_ID {
        return primary_meta(state);
    }
    let now = time_utils::now_iso();
    let (work_dir, config_path, _) = extra_instance_paths(state, id);
    FrpcInstanceMeta {
        id: id.to_string(),
        name: default_frpc_instance_name(),
        is_primary: false,
        config_path: config_path.to_string_lossy().to_string(),
        work_dir: work_dir.to_string_lossy().to_string(),
        created_at: now.clone(),
        updated_at: now,
        sort_order: 1000,
    }
}

pub(super) fn default_runtime() -> FrpcInstanceRuntime {
    FrpcInstanceRuntime {
        desired_running: false,
        pid: None,
        started_at: None,
        stopped_at: None,
        last_exit_code: None,
        last_message: None,
    }
}

pub(super) fn instance_key(id: &str, part: &str) -> String {
    format!("{KEY_PREFIX}:instance:{id}:{part}")
}

pub(super) fn log_key(id: &str) -> String {
    format!("{KEY_PREFIX}:instance:{id}:logs")
}

pub(super) async fn read_instance_ids(store: &Store) -> anyhow::Result<Vec<String>> {
    let raw = store.get_string_value(INSTANCE_IDS_KEY).await?;
    let parsed = raw
        .as_deref()
        .and_then(|value| serde_json::from_str::<Value>(value).ok())
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default();
    let mut seen = Vec::new();
    for value in parsed {
        let Some(id) = value.as_str().and_then(sanitize_instance_id) else {
            continue;
        };
        if !seen.iter().any(|existing| existing == &id) {
            seen.push(id);
        }
    }
    Ok(seen)
}

pub(super) async fn write_instance_ids(store: &Store, ids: &[String]) -> anyhow::Result<()> {
    let mut unique = Vec::new();
    for id in ids {
        if !unique.iter().any(|existing| existing == id) {
            unique.push(id.clone());
        }
    }
    store
        .set_string_value(INSTANCE_IDS_KEY, &serde_json::to_string(&unique)?)
        .await?;
    store
        .set_string_value(PRIMARY_INSTANCE_ID_KEY, FRPC_PRIMARY_INSTANCE_ID)
        .await?;
    Ok(())
}

pub(super) async fn read_meta(
    store: &Store,
    state: &AppState,
    id: &str,
) -> anyhow::Result<Option<FrpcInstanceMeta>> {
    let Some(value) = store.get_json_value(&instance_key(id, "meta")).await? else {
        return Ok(None);
    };
    let fallback = fallback_meta(state, id);
    Ok(Some(normalize_meta(value, fallback)))
}

pub(super) async fn write_meta(store: &Store, meta: &FrpcInstanceMeta) -> anyhow::Result<()> {
    store
        .set_json_value(
            &instance_key(&meta.id, "meta"),
            &serde_json::to_value(meta)?,
        )
        .await?;
    Ok(())
}

pub(super) async fn read_runtime(store: &Store, id: &str) -> anyhow::Result<FrpcInstanceRuntime> {
    let raw = store.get_json_value(&instance_key(id, "runtime")).await?;
    Ok(raw.map(normalize_runtime).unwrap_or_else(default_runtime))
}

pub(super) async fn write_runtime(
    store: &Store,
    id: &str,
    runtime: &FrpcInstanceRuntime,
) -> anyhow::Result<()> {
    store
        .set_json_value(
            &instance_key(id, "runtime"),
            &serde_json::to_value(runtime)?,
        )
        .await?;
    Ok(())
}

pub(super) fn normalize_meta(value: Value, fallback: FrpcInstanceMeta) -> FrpcInstanceMeta {
    FrpcInstanceMeta {
        id: value
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or(fallback.id),
        name: value
            .get("name")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or(fallback.name),
        is_primary: value
            .get("isPrimary")
            .and_then(Value::as_bool)
            .unwrap_or(fallback.is_primary),
        config_path: value
            .get("configPath")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or(fallback.config_path),
        work_dir: value
            .get("workDir")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or(fallback.work_dir),
        created_at: value
            .get("createdAt")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or(fallback.created_at),
        updated_at: value
            .get("updatedAt")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or(fallback.updated_at),
        sort_order: value
            .get("sortOrder")
            .and_then(Value::as_i64)
            .unwrap_or(fallback.sort_order),
    }
}

pub(super) fn normalize_runtime(value: Value) -> FrpcInstanceRuntime {
    FrpcInstanceRuntime {
        desired_running: value
            .get("desiredRunning")
            .or_else(|| value.get("desired_running"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        pid: value
            .get("pid")
            .and_then(Value::as_u64)
            .filter(|pid| *pid > 0)
            .and_then(|pid| u32::try_from(pid).ok()),
        started_at: value
            .get("startedAt")
            .or_else(|| value.get("started_at"))
            .and_then(Value::as_str)
            .map(str::to_string),
        stopped_at: value
            .get("stoppedAt")
            .or_else(|| value.get("stopped_at"))
            .and_then(Value::as_str)
            .map(str::to_string),
        last_exit_code: value
            .get("lastExitCode")
            .or_else(|| value.get("last_exit_code"))
            .and_then(Value::as_i64)
            .and_then(|value| i32::try_from(value).ok()),
        last_message: value
            .get("lastMessage")
            .or_else(|| value.get("last_message"))
            .and_then(Value::as_str)
            .map(str::to_string),
    }
}

pub(super) async fn all_metas(state: &AppState) -> FrpcResult<Vec<FrpcInstanceMeta>> {
    ensure_primary_instance(state).await?;
    let ids = read_instance_ids(&state.store).await?;
    let mut metas = Vec::new();
    for id in ids {
        if let Some(meta) = read_meta(&state.store, state, &id).await? {
            metas.push(meta);
        }
    }
    metas.sort_by(|left, right| {
        left.sort_order
            .cmp(&right.sort_order)
            .then_with(|| left.created_at.cmp(&right.created_at))
    });
    Ok(metas)
}

pub(super) async fn get_meta_or_error(state: &AppState, id: &str) -> FrpcResult<FrpcInstanceMeta> {
    let Some(safe_id) = sanitize_instance_id(id) else {
        return Err(frpc_not_found(id));
    };
    ensure_primary_instance(state).await?;
    read_meta(&state.store, state, &safe_id)
        .await?
        .ok_or_else(|| frpc_not_found(id))
}

pub(super) fn sanitize_instance_id(id: &str) -> Option<String> {
    let trimmed = id.trim();
    if trimmed.is_empty()
        || trimmed.len() > 80
        || !trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
    {
        return None;
    }
    Some(trimmed.to_string())
}

pub(super) async fn read_config(state: &AppState, id: &str) -> FrpcResult<String> {
    let meta = get_meta_or_error(state, id).await?;
    Ok(read_config_for_meta(&meta).await?)
}

pub(super) async fn read_config_for_meta(meta: &FrpcInstanceMeta) -> anyhow::Result<String> {
    fs::create_dir_all(&meta.work_dir).await?;
    let config_path = PathBuf::from(&meta.config_path);
    if fs::metadata(&config_path).await.is_err() {
        let content = default_frpc_template_for_port();
        fs::write(&config_path, &content).await?;
        return Ok(content);
    }
    Ok(fs::read_to_string(config_path).await?)
}

pub(super) fn default_frpc_template_for_port() -> String {
    let local_port = std::env::var("GO_REPROXY_PORT").unwrap_or_else(|_| "7999".to_string());
    [
        "serverAddr = \"\"".to_string(),
        "serverPort = 7000".to_string(),
        "".to_string(),
        "[auth]".to_string(),
        "method = \"token\"".to_string(),
        "token = \"\"".to_string(),
        "".to_string(),
        "[[proxies]]".to_string(),
        "name = \"reproxy\"".to_string(),
        "type = \"tcp\"".to_string(),
        "localIP = \"127.0.0.1\"".to_string(),
        format!("localPort = {local_port}"),
        "remotePort = 7999".to_string(),
        "transport.proxyProtocolVersion = \"v2\"".to_string(),
        "".to_string(),
    ]
    .join("\n")
}

pub(super) async fn write_config_for_meta(
    meta: &FrpcInstanceMeta,
    content: &str,
) -> anyhow::Result<()> {
    fs::create_dir_all(&meta.work_dir).await?;
    fs::write(&meta.config_path, content).await?;
    Ok(())
}

pub(super) async fn save_config_inner(
    state: &AppState,
    id: &str,
    content: String,
) -> FrpcResult<()> {
    let mut meta = get_meta_or_error(state, id).await?;
    verify_frpc_config(state, &meta, &content).await?;
    write_config_for_meta(&meta, &content).await?;
    meta.updated_at = time_utils::now_iso();
    write_meta(&state.store, &meta).await?;
    Ok(())
}

pub(super) async fn update_instance_inner(
    state: &AppState,
    id: &str,
    body: InstanceBody,
) -> FrpcResult<FrpcInstanceStatus> {
    let mut meta = get_meta_or_error(state, id).await?;
    if let Some(name) = body.name {
        let name = name.trim();
        meta.name = if name.is_empty() {
            if meta.is_primary {
                default_frpc_primary_name()
            } else {
                default_frpc_instance_name()
            }
        } else {
            name.to_string()
        };
    }
    if let Some(content) = body.content {
        verify_frpc_config(state, &meta, &content).await?;
        write_config_for_meta(&meta, &content).await?;
    }
    meta.updated_at = time_utils::now_iso();
    write_meta(&state.store, &meta).await?;
    build_status(state, &meta).await
}

pub(super) async fn create_instance_inner(
    state: &AppState,
    body: InstanceBody,
) -> FrpcResult<FrpcInstanceStatus> {
    ensure_primary_instance(state).await?;
    let metas = all_metas(state).await?;
    if metas.iter().filter(|meta| !meta.is_primary).count() >= EXTRA_INSTANCE_LIMIT {
        return Err(frpc_validation(format!(
            "FRPC instance limit exceeded ({EXTRA_INSTANCE_LIMIT})"
        )));
    }
    let id = Uuid::new_v4().to_string();
    let (work_dir, config_path, _) = extra_instance_paths(state, &id);
    let now = time_utils::now_iso();
    let meta = FrpcInstanceMeta {
        id: id.clone(),
        name: body
            .name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(default_frpc_instance_name),
        is_primary: false,
        config_path: config_path.to_string_lossy().to_string(),
        work_dir: work_dir.to_string_lossy().to_string(),
        created_at: now.clone(),
        updated_at: now,
        sort_order: metas.iter().map(|meta| meta.sort_order).max().unwrap_or(0) + 1,
    };
    let content = body.content.unwrap_or_else(default_frpc_template_for_port);
    let result = async {
        verify_frpc_config(state, &meta, &content).await?;
        fs::create_dir_all(&meta.work_dir).await?;
        write_config_for_meta(&meta, &content).await?;
        write_meta(&state.store, &meta).await?;
        write_runtime(&state.store, &meta.id, &default_runtime()).await?;
        let mut ids = metas.iter().map(|meta| meta.id.clone()).collect::<Vec<_>>();
        ids.push(meta.id.clone());
        write_instance_ids(&state.store, &ids).await?;
        append_logs(state, &meta, &["frpc instance created".to_string()]).await?;
        build_status(state, &meta).await
    }
    .await;
    if result.is_err() {
        cleanup_created_instance(state, &meta, &metas).await;
    }
    result
}

pub(super) async fn delete_instance_inner(state: &AppState, id: &str) -> FrpcResult<()> {
    let meta = get_meta_or_error(state, id).await?;
    if meta.is_primary {
        return Err(frpc_validation("Primary FRPC instance cannot be deleted"));
    }
    let status = build_status(state, &meta).await?;
    if status.running {
        stop_instance_inner(state, &meta.id).await?;
    }
    state
        .store
        .delete_keys(&[
            instance_key(&meta.id, "meta"),
            instance_key(&meta.id, "runtime"),
            log_key(&meta.id),
            format!("{}:seq", log_key(&meta.id)),
        ])
        .await?;
    let ids = read_instance_ids(&state.store).await?;
    write_instance_ids(
        &state.store,
        &ids.into_iter()
            .filter(|item| item != &meta.id)
            .collect::<Vec<_>>(),
    )
    .await?;
    let _ = fs::remove_dir_all(&meta.work_dir).await;
    ATTACHED_PIDS.lock().await.remove(&meta.id);
    Ok(())
}

pub(super) async fn cleanup_created_instance(
    state: &AppState,
    meta: &FrpcInstanceMeta,
    previous_metas: &[FrpcInstanceMeta],
) {
    let _ = state
        .store
        .delete_keys(&[
            instance_key(&meta.id, "meta"),
            instance_key(&meta.id, "runtime"),
            log_key(&meta.id),
            format!("{}:seq", log_key(&meta.id)),
        ])
        .await;
    let ids = previous_metas
        .iter()
        .map(|item| item.id.clone())
        .collect::<Vec<_>>();
    let _ = write_instance_ids(&state.store, &ids).await;
    let _ = fs::remove_dir_all(&meta.work_dir).await;
    ATTACHED_PIDS.lock().await.remove(&meta.id);
}
