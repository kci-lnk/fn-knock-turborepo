use super::*;

pub(super) async fn ssh_security_maintenance_tick(state: &AppState) -> anyhow::Result<()> {
    let config = load_config(state).await?;
    let runtime = load_runtime(state).await?;
    apply_ssh_security_config_once(state, &config, &runtime).await
}

pub(super) async fn apply_ssh_security_config_once(
    state: &AppState,
    config: &Value,
    runtime: &Value,
) -> anyhow::Result<()> {
    if config.get("enabled").and_then(Value::as_bool) != Some(true)
        || runtime.get("enabled").and_then(Value::as_bool) != Some(true)
    {
        disable_ssh_security(state, Some(runtime)).await?;
        return Ok(());
    }

    let translator = Translator::from_state(state).await;
    let availability = ssh_security_availability(state, &translator);
    if !availability.available {
        tracing::warn!(reason = %availability.reason, "skipped SSH security sync");
        disable_ssh_security(state, Some(runtime)).await?;
        return Ok(());
    }

    reconcile_expired_blocks(state).await?;
    let _ = sync_firewall_policy(state, Some(runtime), None, Vec::new(), &translator).await?;
    process_recent_ssh_entries(state, config, STARTUP_BACKFILL_LOG_LIMIT).await?;
    Ok(())
}

pub(super) async fn disable_ssh_security(
    state: &AppState,
    runtime: Option<&Value>,
) -> anyhow::Result<()> {
    if host_firewall_available(state) {
        let payload = json!({
            "chain_name": SSH_FIREWALL_CHAIN,
            "parent_chain": ["INPUT", "DOCKER-USER"]
        });
        if let Err(error) = state.go_backend.clear_ssh_firewall(&payload).await {
            tracing::debug!(%error, "failed to clear disabled SSH firewall policy");
        }
    }
    for record in active_blocks(state).await? {
        if let Some(ip) = record.get("ip").and_then(Value::as_str)
            && let Err(error) = mark_block_removed(state, ip, "disabled").await
        {
            tracing::warn!(%error, ip, "failed to mark SSH block disabled");
        }
    }
    if let Some(runtime) = runtime {
        let next = json!({
            "enabled": false,
            "allowed_cidrs": [],
            "updated_at": time_utils::now_iso(),
        });
        if runtime != &next {
            state.store.set_json_value(RUNTIME_KEY, &next).await?;
        }
    }
    Ok(())
}

pub(super) async fn sync_firewall_blocks_now(
    state: &AppState,
    translator: &Translator,
) -> anyhow::Result<Value> {
    let availability = ssh_security_availability(state, translator);
    if !availability.available {
        anyhow::bail!(availability.reason);
    }
    reconcile_expired_blocks(state).await?;
    let active = active_blocks(state).await?;
    let policy =
        sync_firewall_policy(state, None, Some(active.clone()), Vec::new(), translator).await?;
    let ports = policy.ports;
    let mut synced = 0usize;
    for record in active {
        let mut next = record.as_object().cloned().unwrap_or_default();
        next.insert("ports".to_string(), json!(ports));
        next.insert("applied".to_string(), Value::Bool(true));
        next.insert("removed_at".to_string(), Value::Null);
        next.insert("remove_reason".to_string(), Value::Null);
        save_block(state, &Value::Object(next)).await?;
        synced += 1;
    }
    Ok(json!({
        "cleared": synced,
        "synced": synced,
        "active_blocks": policy.blocked_ips,
        "allowed_cidrs": policy.allowed_cidrs,
        "ports": ports
    }))
}

pub(super) async fn reconcile_expired_blocks(
    state: &AppState,
) -> crate::storage::StorageResult<()> {
    for record in expired_active_blocks(state).await? {
        if let Some(ip) = record.get("ip").and_then(Value::as_str) {
            let _ = mark_block_removed(state, ip, "expired").await?;
        }
    }
    Ok(())
}

pub(super) async fn expired_active_blocks(
    state: &AppState,
) -> crate::storage::StorageResult<Vec<Value>> {
    let keys = state.store.scan_keys(BLOCK_DATA_PREFIX, 100).await?;
    let mut records = Vec::new();
    let now = time_utils::now_ms();
    for key in keys {
        if let Some(record) = state
            .store
            .get_json_value(&key)
            .await?
            .and_then(normalize_block_record)
            && record
                .get("applied")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            && iso_score(record.get("expires_at").and_then(Value::as_str)) <= now
        {
            records.push(record);
        }
    }
    Ok(records)
}

pub(super) async fn sync_firewall_policy(
    state: &AppState,
    runtime: Option<&Value>,
    active_records: Option<Vec<Value>>,
    extra_blocked_ips: Vec<String>,
    translator: &Translator,
) -> anyhow::Result<FirewallPolicyResult> {
    let loaded_runtime;
    let runtime = match runtime {
        Some(runtime) => runtime,
        None => {
            loaded_runtime = load_runtime(state).await?;
            &loaded_runtime
        }
    };
    let active = match active_records {
        Some(records) => records,
        None => active_blocks(state).await?,
    };
    let mut blocked_ips = active
        .iter()
        .filter_map(|record| record.get("ip").and_then(Value::as_str))
        .map(str::to_string)
        .collect::<Vec<_>>();
    blocked_ips.extend(extra_blocked_ips);
    blocked_ips = normalize_ip_strings(blocked_ips);

    let allowed_cidrs = if runtime.get("enabled").and_then(Value::as_bool) == Some(true) {
        runtime
            .get("allowed_cidrs")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let allowed_cidrs = normalize_cidr_strings(allowed_cidrs);
    let ports = resolve_ssh_ports();
    if allowed_cidrs.is_empty() && blocked_ips.is_empty() {
        let payload = json!({
            "chain_name": SSH_FIREWALL_CHAIN,
            "parent_chain": ["INPUT", "DOCKER-USER"]
        });
        let value = state.go_backend.clear_ssh_firewall(&payload).await?;
        ensure_go_success(value, translator, "clearSshPolicyFailed")?;
        return Ok(FirewallPolicyResult {
            allowed_cidrs: 0,
            blocked_ips: 0,
            ports,
        });
    }

    let allowed_count = allowed_cidrs.len();
    let blocked_count = blocked_ips.len();
    let payload = json!({
        "chain_name": SSH_FIREWALL_CHAIN,
        "parent_chain": ["INPUT", "DOCKER-USER"],
        "ports": ports.clone(),
        "allowed_cidrs": allowed_cidrs,
        "blocked_ips": blocked_ips,
        "include_local_cidrs": true
    });
    let value = state.go_backend.sync_ssh_firewall(&payload).await?;
    ensure_go_success(value, translator, "syncSshPolicyFailed")?;
    Ok(FirewallPolicyResult {
        allowed_cidrs: allowed_count,
        blocked_ips: blocked_count,
        ports,
    })
}

pub(super) async fn process_recent_ssh_entries(
    state: &AppState,
    config: &Value,
    limit: usize,
) -> anyhow::Result<()> {
    let window_ms = config
        .get("window_minutes")
        .and_then(Value::as_i64)
        .unwrap_or(10)
        .max(1)
        * 60
        * 1000;
    let cutoff = time_utils::now_ms() - window_ms;
    let mut entries = query_recent_ssh_logs(limit)
        .into_iter()
        .filter(|entry| iso_score(entry.get("happened_at").and_then(Value::as_str)) >= cutoff)
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        iso_score(left.get("happened_at").and_then(Value::as_str))
            .cmp(&iso_score(right.get("happened_at").and_then(Value::as_str)))
    });
    for entry in entries {
        if let Err(error) = handle_ssh_entry(state, config, &entry).await {
            tracing::warn!(%error, entry = %entry, "failed to handle SSH log entry");
        }
    }
    Ok(())
}

pub(super) async fn handle_ssh_entry(
    state: &AppState,
    config: &Value,
    entry: &Value,
) -> anyhow::Result<()> {
    let id = entry.get("id").and_then(Value::as_str).unwrap_or("");
    if id.is_empty() || is_processed(state, id).await? {
        return Ok(());
    }
    if config.get("enabled").and_then(Value::as_bool) != Some(true) {
        mark_processed(state, id).await?;
        return Ok(());
    }
    let ip = normalize_ip(entry.get("ip").and_then(Value::as_str).unwrap_or(""));
    if ip.is_empty() || is_private_or_local_ip(&ip) {
        mark_processed(state, id).await?;
        return Ok(());
    }
    let ip_location = ip_location::register_usage(state, &ip, vec![format!("ssh-login-log|{id}")])
        .await
        .unwrap_or_default();
    let mut entry = entry.clone();
    if !ip_location.trim().is_empty()
        && let Some(object) = entry.as_object_mut()
    {
        object.insert("ipLocation".to_string(), Value::String(ip_location));
    }

    match entry.get("outcome").and_then(Value::as_str) {
        Some("failure") => handle_ssh_failure(state, config, &entry, id, &ip).await?,
        Some("success") => handle_ssh_success(state, config, &entry, id, &ip).await?,
        _ => {}
    }
    mark_processed(state, id).await?;
    Ok(())
}

pub(super) async fn handle_ssh_failure(
    state: &AppState,
    config: &Value,
    entry: &Value,
    id: &str,
    ip: &str,
) -> anyhow::Result<()> {
    let window_minutes = config
        .get("window_minutes")
        .and_then(Value::as_i64)
        .unwrap_or(10)
        .max(1);
    let threshold = config
        .get("failed_login_threshold")
        .and_then(Value::as_i64)
        .unwrap_or(5)
        .max(1);
    let attempts = add_failure(state, ip, id, entry, window_minutes).await?;
    let mut event_payload = json!({
        "ip": ip,
        "username": entry.get("username").cloned().unwrap_or_else(|| json!("-")),
        "invalid_user": entry.get("invalid_user").and_then(Value::as_bool).unwrap_or(false),
        "auth_method": entry.get("auth_method").cloned().unwrap_or(Value::Null),
        "port": entry.get("port").cloned().unwrap_or(Value::Null),
        "attempts": attempts,
        "window_minutes": window_minutes,
        "threshold": threshold,
        "log_time": entry.get("happened_at").cloned().unwrap_or_else(|| json!(time_utils::now_iso()))
    });
    if let Some(location) = entry
        .get("ipLocation")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        && let Some(object) = event_payload.as_object_mut()
    {
        object.insert(
            "ip_location".to_string(),
            Value::String(location.to_string()),
        );
    }
    if let Err(error) = system_events::publish_ssh_login_failure_event(state, event_payload).await {
        tracing::debug!(%error, "failed to publish SSH login failure event");
    }
    if attempts < threshold || is_active_blocked(state, ip).await? {
        return Ok(());
    }
    create_ssh_block(state, config, entry, "failed_login_threshold", attempts).await
}

pub(super) async fn handle_ssh_success(
    state: &AppState,
    config: &Value,
    entry: &Value,
    id: &str,
    ip: &str,
) -> anyhow::Result<()> {
    clear_failures(state, ip).await?;
    let mut event_payload = json!({
        "ip": ip,
        "username": entry.get("username").cloned().unwrap_or_else(|| json!("-")),
        "auth_method": entry.get("auth_method").cloned().unwrap_or(Value::Null),
        "port": entry.get("port").cloned().unwrap_or(Value::Null),
        "log_time": entry.get("happened_at").cloned().unwrap_or_else(|| json!(time_utils::now_iso()))
    });
    if let Some(location) = entry
        .get("ipLocation")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        && let Some(object) = event_payload.as_object_mut()
    {
        object.insert(
            "ip_location".to_string(),
            Value::String(location.to_string()),
        );
    }
    if let Err(error) = system_events::publish_ssh_login_success_event(state, event_payload).await {
        tracing::debug!(%error, "failed to publish SSH login success event");
    }
    let runtime = load_runtime(state).await?;
    if ip_allowed_by_runtime(&runtime, ip) || is_active_blocked(state, ip).await? {
        return Ok(());
    }
    create_ssh_block(state, config, entry, "cidr_not_allowed", 0).await?;
    mark_processed(state, id).await?;
    Ok(())
}

pub(super) async fn create_ssh_block(
    state: &AppState,
    config: &Value,
    entry: &Value,
    reason: &str,
    failed_count: i64,
) -> anyhow::Result<()> {
    let ip = normalize_ip(entry.get("ip").and_then(Value::as_str).unwrap_or(""));
    if ip.is_empty() {
        return Ok(());
    }
    let block_seconds = ssh_block_duration_seconds(config);
    let blocked_at = time_utils::now_iso();
    let expires_at = millis_to_iso(time_utils::now_ms() + block_seconds * 1000);
    let translator = Translator::from_state(state).await;
    let ip_location = if let Some(location) = entry
        .get("ipLocation")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        location.to_string()
    } else {
        ip_location::register_usage(state, &ip, vec![format!("ssh-blocklist|{ip}")])
            .await
            .unwrap_or_default()
    };
    let policy = sync_firewall_policy(state, None, None, vec![ip.clone()], &translator).await?;
    let mut record = json!({
        "ip": ip,
        "ports": policy.ports,
        "blocked_at": blocked_at,
        "expires_at": expires_at,
        "reason": reason,
        "failed_count": failed_count,
        "window_minutes": config.get("window_minutes").and_then(Value::as_i64).unwrap_or(10),
        "threshold": config.get("failed_login_threshold").and_then(Value::as_i64).unwrap_or(5),
        "sample_user": entry.get("username").cloned().unwrap_or_else(|| json!("-")),
        "sample_auth_method": entry.get("auth_method").cloned().unwrap_or(Value::Null),
        "sample_log_time": entry.get("happened_at").cloned().unwrap_or_else(|| json!(time_utils::now_iso())),
        "applied": true,
        "removed_at": Value::Null,
        "remove_reason": Value::Null,
    });
    if !ip_location.trim().is_empty()
        && let Some(object) = record.as_object_mut()
    {
        object.insert("ipLocation".to_string(), Value::String(ip_location.clone()));
    }
    save_block(state, &record).await?;
    let mut payload = json!({
        "ip": record.get("ip").cloned().unwrap_or(Value::Null),
        "blocked_at": record.get("blocked_at").cloned().unwrap_or(Value::Null),
        "blocked_until": record.get("expires_at").cloned().unwrap_or(Value::Null),
        "block_seconds": block_seconds,
        "reason": reason,
        "failed_count": failed_count,
        "window_minutes": record.get("window_minutes").cloned().unwrap_or(Value::Null),
        "threshold": record.get("threshold").cloned().unwrap_or(Value::Null),
        "username": record.get("sample_user").cloned().unwrap_or(Value::Null),
    });
    if !ip_location.trim().is_empty()
        && let Some(object) = payload.as_object_mut()
    {
        object.insert("ip_location".to_string(), Value::String(ip_location));
    }
    if let Err(error) = system_events::publish_ssh_ip_blocked_event(state, payload).await {
        tracing::debug!(%error, "failed to publish SSH block event");
    }
    Ok(())
}

pub(super) async fn is_processed(
    state: &AppState,
    id: &str,
) -> crate::storage::StorageResult<bool> {
    Ok(state
        .store
        .get_string_value(&format!("{PROCESSED_PREFIX}{id}"))
        .await?
        .is_some())
}

pub(super) async fn mark_processed(
    state: &AppState,
    id: &str,
) -> crate::storage::StorageResult<()> {
    state
        .store
        .set_string_value_with_optional_ttl(
            &format!("{PROCESSED_PREFIX}{id}"),
            "1",
            Some(PROCESSED_TTL_SECONDS),
        )
        .await
}

pub(super) async fn add_failure(
    state: &AppState,
    ip: &str,
    id: &str,
    entry: &Value,
    window_minutes: i64,
) -> crate::storage::StorageResult<i64> {
    let score = iso_score(entry.get("happened_at").and_then(Value::as_str));
    let score = if score > 0 {
        score
    } else {
        time_utils::now_ms()
    };
    let window_ms = window_minutes.max(1) * 60 * 1000;
    state
        .store
        .zadd_trim_count_expire(
            &format!("{FAILURES_PREFIX}{ip}"),
            id,
            score,
            score - window_ms,
            ((window_ms / 1000) + 3600) as usize,
        )
        .await
}

pub(super) async fn clear_failures(
    state: &AppState,
    ip: &str,
) -> crate::storage::StorageResult<()> {
    state
        .store
        .delete_key(&format!("{FAILURES_PREFIX}{ip}"))
        .await
}

pub(super) async fn is_active_blocked(
    state: &AppState,
    ip: &str,
) -> crate::storage::StorageResult<bool> {
    Ok(load_block(state, ip)
        .await?
        .is_some_and(|record| is_active_block(&record, time_utils::now_ms())))
}

pub(super) fn ip_allowed_by_runtime(runtime: &Value, ip: &str) -> bool {
    let cidrs = runtime
        .get("allowed_cidrs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter_map(|value| value.parse::<IpNet>().ok())
        .collect::<Vec<_>>();
    if cidrs.is_empty() {
        return true;
    }
    let Ok(ip) = ip.parse::<IpAddr>() else {
        return true;
    };
    cidrs.iter().any(|cidr| cidr.contains(&ip))
}

pub(super) fn ssh_block_duration_seconds(config: &Value) -> i64 {
    let value = config
        .get("block_duration_value")
        .and_then(Value::as_i64)
        .unwrap_or(1)
        .clamp(1, 365);
    match config
        .get("block_duration_unit")
        .and_then(Value::as_str)
        .unwrap_or("day")
    {
        "minute" => value * 60,
        "hour" => value * 3600,
        _ => value * 24 * 3600,
    }
}

pub(super) fn normalize_ip_strings(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = HashSet::new();
    values
        .into_iter()
        .filter_map(|value| {
            let ip = normalize_ip(&value);
            (!ip.is_empty() && seen.insert(ip.clone())).then_some(ip)
        })
        .collect()
}
