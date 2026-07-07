use super::*;

pub(super) async fn load_providers(state: &AppState) -> redis::RedisResult<Vec<Value>> {
    load_indexed_values(state, PROVIDERS_INDEX_KEY, PROVIDERS_DATA_PREFIX).await
}

pub(super) async fn load_provider(state: &AppState, id: &str) -> redis::RedisResult<Option<Value>> {
    state.redis.get_json_value(&provider_key(id)).await
}

pub(super) async fn save_provider_raw(
    state: &AppState,
    provider: &Value,
) -> redis::RedisResult<()> {
    let id = provider
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let score = iso_score_ms(
        provider
            .get("updated_at")
            .and_then(Value::as_str)
            .or_else(|| provider.get("created_at").and_then(Value::as_str)),
    );
    state
        .redis
        .set_json_value(&provider_key(id), provider)
        .await?;
    state
        .redis
        .zadd_string_member(PROVIDERS_INDEX_KEY, id, score)
        .await
}

pub(super) async fn load_rules(state: &AppState) -> redis::RedisResult<Vec<Value>> {
    load_indexed_values(state, RULES_INDEX_KEY, RULES_DATA_PREFIX).await
}

pub(super) async fn load_rule(state: &AppState, id: &str) -> redis::RedisResult<Option<Value>> {
    state.redis.get_json_value(&rule_key(id)).await
}

pub(super) async fn save_rule_raw(state: &AppState, rule: &Value) -> redis::RedisResult<()> {
    let id = rule.get("id").and_then(Value::as_str).unwrap_or_default();
    let score = iso_score_ms(
        rule.get("updated_at")
            .and_then(Value::as_str)
            .or_else(|| rule.get("created_at").and_then(Value::as_str)),
    );
    state.redis.set_json_value(&rule_key(id), rule).await?;
    state
        .redis
        .zadd_string_member(RULES_INDEX_KEY, id, score)
        .await
}

pub(super) async fn load_trigger(state: &AppState, id: &str) -> redis::RedisResult<Option<Value>> {
    state
        .redis
        .get_json_value(&format!("{TRIGGERS_DATA_PREFIX}{id}"))
        .await
}

fn history_cutoff_score_ms() -> i64 {
    time_utils::now_ms() - HISTORY_RETENTION_TTL_SECONDS * 1000
}

pub(super) async fn touch_trigger_index(
    state: &AppState,
    trigger: &Value,
) -> redis::RedisResult<()> {
    let id = trigger
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let score = iso_score_ms(trigger.get("created_at").and_then(Value::as_str));
    state
        .redis
        .zadd_string_member(TRIGGERS_INDEX_KEY, id, score)
        .await?;
    state
        .redis
        .zrem_range_by_score(TRIGGERS_INDEX_KEY, 0, history_cutoff_score_ms())
        .await
}

pub(super) async fn save_trigger_raw(state: &AppState, trigger: &Value) -> redis::RedisResult<()> {
    let id = trigger
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let created_at = trigger.get("created_at").and_then(Value::as_str);
    let ttl = history_ttl_seconds(created_at);
    state
        .redis
        .set_json_value_ex(&format!("{TRIGGERS_DATA_PREFIX}{id}"), trigger, ttl)
        .await?;
    touch_trigger_index(state, trigger).await
}

pub(super) async fn save_trigger_if_absent(
    state: &AppState,
    trigger: &Value,
) -> redis::RedisResult<bool> {
    let id = trigger
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let created_at = trigger.get("created_at").and_then(Value::as_str);
    let ttl = history_ttl_seconds(created_at);
    let saved = state
        .redis
        .set_json_value_nx_ex(&format!("{TRIGGERS_DATA_PREFIX}{id}"), trigger, ttl)
        .await?;
    if saved {
        touch_trigger_index(state, trigger).await?;
        return Ok(true);
    }
    if let Some(existing) = load_trigger(state, id).await? {
        touch_trigger_index(state, &existing).await?;
    }
    Ok(false)
}

pub(super) async fn touch_delivery_index(
    state: &AppState,
    delivery: &Value,
) -> redis::RedisResult<()> {
    let id = delivery
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let score = iso_score_ms(delivery.get("triggered_at").and_then(Value::as_str));
    state
        .redis
        .zadd_string_member(DELIVERIES_INDEX_KEY, id, score)
        .await?;
    state
        .redis
        .zrem_range_by_score(DELIVERIES_INDEX_KEY, 0, history_cutoff_score_ms())
        .await
}

pub(super) async fn load_delivery(state: &AppState, id: &str) -> redis::RedisResult<Option<Value>> {
    state.redis.get_json_value(&delivery_key(id)).await
}

pub(super) async fn save_delivery_raw(
    state: &AppState,
    delivery: &Value,
) -> redis::RedisResult<()> {
    let id = delivery
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let triggered_at = delivery.get("triggered_at").and_then(Value::as_str);
    let ttl = history_ttl_seconds(triggered_at);
    state
        .redis
        .set_json_value_ex(&delivery_key(id), delivery, ttl)
        .await?;
    touch_delivery_index(state, delivery).await
}

pub(super) async fn save_delivery_if_absent(
    state: &AppState,
    delivery: &Value,
) -> redis::RedisResult<bool> {
    let id = delivery
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let triggered_at = delivery.get("triggered_at").and_then(Value::as_str);
    let ttl = history_ttl_seconds(triggered_at);
    let saved = state
        .redis
        .set_json_value_nx_ex(&delivery_key(id), delivery, ttl)
        .await?;
    if saved {
        touch_delivery_index(state, delivery).await?;
        return Ok(true);
    }
    if let Some(existing) = load_delivery(state, id).await? {
        touch_delivery_index(state, &existing).await?;
    }
    Ok(false)
}

pub(super) fn history_ttl_seconds(happened_at: Option<&str>) -> usize {
    let expires_at = happened_at
        .and_then(time_utils::parse_iso_ms)
        .unwrap_or_else(time_utils::now_ms)
        + HISTORY_RETENTION_TTL_SECONDS * 1000;
    (((expires_at - time_utils::now_ms()).max(1000) + 999) / 1000) as usize
}

pub(super) async fn refresh_trigger_status(
    state: &AppState,
    trigger_id: &str,
) -> anyhow::Result<()> {
    if trigger_id.is_empty() {
        return Ok(());
    }
    let Some(trigger) = load_trigger(state, trigger_id).await? else {
        return Ok(());
    };
    let (deliveries, _) = list_history(
        state,
        DELIVERIES_INDEX_KEY,
        DELIVERIES_DATA_PREFIX,
        1,
        i64::MAX,
        |delivery| delivery.get("trigger_id").and_then(Value::as_str) == Some(trigger_id),
    )
    .await?;
    if deliveries.is_empty() {
        return Ok(());
    }
    if deliveries.iter().any(|delivery| {
        !is_terminal_delivery_status(delivery.get("status").and_then(Value::as_str))
    }) {
        return Ok(());
    }
    let all_succeeded = deliveries.iter().all(|delivery| {
        matches!(
            delivery.get("status").and_then(Value::as_str),
            Some("success" | "skipped")
        )
    });
    let mut updated = trigger.as_object().cloned().unwrap_or_default();
    updated.insert(
        "status".to_string(),
        Value::String(if all_succeeded {
            "completed".to_string()
        } else {
            "partially_failed".to_string()
        }),
    );
    save_trigger_raw(state, &Value::Object(updated)).await?;
    Ok(())
}

pub(super) fn find_rule_target(rule: &Value, target_id: &str) -> Option<Value> {
    rule.get("targets")
        .and_then(Value::as_array)?
        .iter()
        .find(|target| target.get("id").and_then(Value::as_str) == Some(target_id))
        .cloned()
}

pub(super) struct DeliveryPolicy {
    pub(super) timeout_seconds: i64,
    pub(super) max_attempts: i64,
    pub(super) backoff_seconds: i64,
}

pub(super) fn resolve_delivery_policy(value: Option<&Value>) -> DeliveryPolicy {
    let object = value.and_then(Value::as_object);
    DeliveryPolicy {
        timeout_seconds: object
            .and_then(|value| value.get("timeout_seconds"))
            .map(|value| value_to_i64(value, 5))
            .unwrap_or(5)
            .clamp(1, 30),
        max_attempts: object
            .and_then(|value| value.get("max_attempts"))
            .map(|value| value_to_i64(value, 3))
            .unwrap_or(3)
            .clamp(1, 10),
        backoff_seconds: object
            .and_then(|value| value.get("backoff_seconds"))
            .map(|value| value_to_i64(value, 30))
            .unwrap_or(30)
            .clamp(5, 3600),
    }
}

pub(super) fn resolve_delivery_ready_at_ms(delivery: &Value) -> i64 {
    delivery
        .get("next_retry_at")
        .and_then(Value::as_str)
        .and_then(time_utils::parse_iso_ms)
        .or_else(|| {
            delivery
                .get("triggered_at")
                .and_then(Value::as_str)
                .and_then(time_utils::parse_iso_ms)
        })
        .unwrap_or_else(time_utils::now_ms)
}

pub(super) async fn load_indexed_values(
    state: &AppState,
    index_key: &str,
    data_prefix: &str,
) -> redis::RedisResult<Vec<Value>> {
    let ids = state.redis.zrevrange_strings(index_key).await?;
    let mut values = Vec::new();
    let mut stale_ids = Vec::new();
    for id in ids {
        match state
            .redis
            .get_json_value(&format!("{data_prefix}{id}"))
            .await?
        {
            Some(value) => values.push(value),
            None => stale_ids.push(id),
        }
    }
    for id in stale_ids {
        state.redis.zrem_string_member(index_key, &id).await?;
    }
    Ok(values)
}

pub(super) async fn list_history<F>(
    state: &AppState,
    index_key: &str,
    data_prefix: &str,
    page: i64,
    limit: i64,
    matches: F,
) -> redis::RedisResult<(Vec<Value>, i64)>
where
    F: Fn(&Value) -> bool,
{
    let ids = state.redis.zrevrange_strings(index_key).await?;
    let page_start = (page.saturating_sub(1)).saturating_mul(limit);
    let mut matched_total = 0_i64;
    let mut items = Vec::new();
    let mut stale_ids = Vec::new();

    for id in ids {
        let value = state
            .redis
            .get_json_value(&format!("{data_prefix}{id}"))
            .await?;
        let Some(value) = value else {
            stale_ids.push(id);
            continue;
        };
        if !matches(&value) {
            continue;
        }
        if matched_total >= page_start && (items.len() as i64) < limit {
            items.push(value);
        }
        matched_total += 1;
    }
    for id in stale_ids {
        state.redis.zrem_string_member(index_key, &id).await?;
    }
    Ok((items, matched_total))
}

pub(super) struct ClearDeliveryFilter {
    pub(super) rule_id: Option<String>,
    pub(super) provider_id: Option<String>,
    pub(super) trigger_id: Option<String>,
    pub(super) status: Option<String>,
}

pub(super) async fn clear_delivery_values(
    state: &AppState,
    filter: ClearDeliveryFilter,
) -> redis::RedisResult<usize> {
    let ids = state.redis.zrevrange_strings(DELIVERIES_INDEX_KEY).await?;
    let mut matched_ids = Vec::new();
    let mut stale_ids = Vec::new();
    for id in ids {
        match state.redis.get_json_value(&delivery_key(&id)).await? {
            Some(value) => {
                if matches_optional_string(&value, "rule_id", filter.rule_id.as_deref())
                    && matches_optional_string(&value, "provider_id", filter.provider_id.as_deref())
                    && matches_optional_string(&value, "trigger_id", filter.trigger_id.as_deref())
                    && matches_optional_string(&value, "status", filter.status.as_deref())
                {
                    matched_ids.push(id);
                }
            }
            None => stale_ids.push(id),
        }
    }

    let delete_keys = matched_ids
        .iter()
        .map(|id| delivery_key(id))
        .collect::<Vec<_>>();
    state.redis.delete_keys(&delete_keys).await?;
    for id in stale_ids.iter().chain(matched_ids.iter()) {
        state
            .redis
            .zrem_string_member(DELIVERIES_INDEX_KEY, id)
            .await?;
    }
    for id in &matched_ids {
        state
            .redis
            .zrem_string_member(DELIVERIES_READY_KEY, id)
            .await?;
    }
    Ok(matched_ids.len())
}
