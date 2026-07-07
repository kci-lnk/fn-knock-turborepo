use super::*;

pub(super) async fn load_block(state: &AppState, ip: &str) -> redis::RedisResult<Option<Value>> {
    let normalized = normalize_ip(ip);
    if normalized.is_empty() {
        return Ok(None);
    }
    Ok(state
        .redis
        .get_json_value(&format!("{BLOCK_DATA_PREFIX}{normalized}"))
        .await?
        .and_then(normalize_block_record))
}

pub(super) async fn save_block(state: &AppState, record: &Value) -> redis::RedisResult<()> {
    let Some(record) = normalize_block_record(record.clone()) else {
        return Ok(());
    };
    let ip = record.get("ip").and_then(Value::as_str).unwrap_or_default();
    let ttl = block_ttl_seconds(&record);
    state
        .redis
        .set_json_value_ex(&format!("{BLOCK_DATA_PREFIX}{ip}"), &record, ttl)
        .await?;
    if record
        .get("applied")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        let score = iso_score(record.get("expires_at").and_then(Value::as_str));
        state
            .redis
            .zadd_string_member(BLOCKS_INDEX_KEY, ip, score)
            .await?;
    } else {
        state.redis.zrem_string_member(BLOCKS_INDEX_KEY, ip).await?;
    }
    Ok(())
}

pub(super) async fn active_blocks(state: &AppState) -> redis::RedisResult<Vec<Value>> {
    let keys = state.redis.scan_keys(BLOCK_DATA_PREFIX, 100).await?;
    let mut records = Vec::new();
    let now = time_utils::now_ms();
    for key in keys {
        if let Some(record) = state
            .redis
            .get_json_value(&key)
            .await?
            .and_then(normalize_block_record)
        {
            if is_active_block(&record, now) {
                records.push(record);
            } else if let Some(ip) = record.get("ip").and_then(Value::as_str) {
                state.redis.zrem_string_member(BLOCKS_INDEX_KEY, ip).await?;
            }
        }
    }
    records.sort_by(|left, right| {
        iso_score(right.get("blocked_at").and_then(Value::as_str))
            .cmp(&iso_score(left.get("blocked_at").and_then(Value::as_str)))
    });
    Ok(records)
}

pub(super) async fn list_active_blocks(
    state: &AppState,
    page: i64,
    limit: i64,
    search: &str,
) -> redis::RedisResult<(Vec<Value>, usize)> {
    let mut records = active_blocks(state).await?;
    if !search.is_empty() {
        records.retain(|record| {
            ["ip", "ipLocation", "sample_user"].iter().any(|key| {
                record
                    .get(*key)
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_ascii_lowercase()
                    .contains(search)
            })
        });
    }
    let total = records.len();
    let start = ((page - 1) * limit) as usize;
    Ok((
        records
            .into_iter()
            .skip(start)
            .take(limit as usize)
            .collect(),
        total,
    ))
}

pub(super) async fn remove_block(
    state: &AppState,
    ip: &str,
    reason: &str,
    translator: &Translator,
) -> anyhow::Result<bool> {
    let Some(record) = load_block(state, ip).await? else {
        return Ok(false);
    };
    let record_ip = record
        .get("ip")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if record_ip.is_empty() {
        return Ok(false);
    }
    if record
        .get("applied")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        let active = active_blocks(state)
            .await?
            .into_iter()
            .filter(|item| item.get("ip").and_then(Value::as_str) != Some(record_ip.as_str()))
            .collect::<Vec<_>>();
        let _ = sync_firewall_policy(state, None, Some(active), Vec::new(), translator).await?;
    }
    let removed = mark_block_removed(state, &record_ip, reason).await?;
    if removed && reason == "manual" {
        clear_failures(state, &record_ip).await?;
    }
    Ok(removed)
}

pub(super) async fn mark_block_removed(
    state: &AppState,
    ip: &str,
    reason: &str,
) -> redis::RedisResult<bool> {
    let Some(record) = load_block(state, ip).await? else {
        return Ok(false);
    };
    let mut next = record.as_object().cloned().unwrap_or_default();
    next.insert("applied".to_string(), Value::Bool(false));
    next.insert(
        "removed_at".to_string(),
        Value::String(time_utils::now_iso()),
    );
    next.insert(
        "remove_reason".to_string(),
        Value::String(reason.to_string()),
    );
    save_block(state, &Value::Object(next)).await?;
    Ok(true)
}

pub(super) fn normalize_block_record(value: Value) -> Option<Value> {
    let raw = value.as_object()?;
    let ip = normalize_ip(raw.get("ip")?.as_str()?);
    if ip.is_empty() {
        return None;
    }
    let blocked_at = normalize_timestamp(raw.get("blocked_at"))?
        .as_str()?
        .to_string();
    let expires_at = normalize_timestamp(raw.get("expires_at"))?
        .as_str()?
        .to_string();
    let reason = match raw.get("reason").and_then(Value::as_str) {
        Some("cidr_not_allowed") => "cidr_not_allowed",
        _ => "failed_login_threshold",
    };
    let ports = raw
        .get("ports")
        .and_then(Value::as_array)
        .map(|items| merge_port_values(items.iter()))
        .unwrap_or_default();
    let mut record = Map::new();
    record.insert("ip".to_string(), Value::String(ip));
    if let Some(location) = raw
        .get("ipLocation")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        record.insert(
            "ipLocation".to_string(),
            Value::String(location.to_string()),
        );
    }
    if !ports.is_empty() {
        record.insert("ports".to_string(), json!(ports));
    }
    record.insert("blocked_at".to_string(), Value::String(blocked_at));
    record.insert("expires_at".to_string(), Value::String(expires_at));
    record.insert("reason".to_string(), Value::String(reason.to_string()));
    record.insert(
        "failed_count".to_string(),
        json!(
            raw.get("failed_count")
                .and_then(Value::as_i64)
                .unwrap_or_default()
                .max(0)
        ),
    );
    record.insert(
        "window_minutes".to_string(),
        json!(
            raw.get("window_minutes")
                .and_then(Value::as_i64)
                .unwrap_or_default()
                .max(0)
        ),
    );
    record.insert(
        "threshold".to_string(),
        json!(
            raw.get("threshold")
                .and_then(Value::as_i64)
                .unwrap_or_default()
                .max(0)
        ),
    );
    for key in ["sample_user", "sample_auth_method", "sample_log_time"] {
        if let Some(value) = raw
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            record.insert(key.to_string(), Value::String(value.to_string()));
        }
    }
    record.insert(
        "applied".to_string(),
        Value::Bool(raw.get("applied").and_then(Value::as_bool).unwrap_or(false)),
    );
    record.insert(
        "removed_at".to_string(),
        normalize_timestamp(raw.get("removed_at")).unwrap_or(Value::Null),
    );
    record.insert(
        "remove_reason".to_string(),
        match raw.get("remove_reason").and_then(Value::as_str) {
            Some("manual" | "expired" | "disabled") => Value::String(
                raw.get("remove_reason")
                    .and_then(Value::as_str)
                    .unwrap()
                    .to_string(),
            ),
            _ => Value::Null,
        },
    );
    Some(Value::Object(record))
}

pub(super) fn is_active_block(record: &Value, now_ms: i64) -> bool {
    record
        .get("applied")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && iso_score(record.get("expires_at").and_then(Value::as_str)) > now_ms
}

pub(super) fn block_ttl_seconds(record: &Value) -> usize {
    if !record
        .get("applied")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return 90 * 24 * 3600;
    }
    let expires_at = iso_score(record.get("expires_at").and_then(Value::as_str));
    let seconds_until_expiry = ((expires_at - time_utils::now_ms()).max(0) + 999) / 1000;
    (seconds_until_expiry + 90 * 24 * 3600).clamp(90 * 24 * 3600, (365 + 90) * 24 * 3600) as usize
}
