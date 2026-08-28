use super::*;

pub(super) async fn load_providers(state: &AppState) -> crate::storage::StorageResult<Vec<Value>> {
    state.storage.store.load_notification_providers().await
}

pub(super) async fn load_provider(
    state: &AppState,
    id: &str,
) -> crate::storage::StorageResult<Option<Value>> {
    state.storage.store.load_notification_provider(id).await
}

pub(super) async fn save_provider_raw(
    state: &AppState,
    provider: &Value,
) -> crate::storage::StorageResult<()> {
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
        .storage
        .store
        .save_notification_provider(id, provider, score)
        .await
}

pub(super) async fn load_rules(state: &AppState) -> crate::storage::StorageResult<Vec<Value>> {
    state.storage.store.load_notification_rules().await
}

pub(super) async fn load_rule(
    state: &AppState,
    id: &str,
) -> crate::storage::StorageResult<Option<Value>> {
    state.storage.store.load_notification_rule(id).await
}

pub(super) async fn save_rule_raw(
    state: &AppState,
    rule: &Value,
) -> crate::storage::StorageResult<()> {
    let id = rule.get("id").and_then(Value::as_str).unwrap_or_default();
    let score = iso_score_ms(
        rule.get("updated_at")
            .and_then(Value::as_str)
            .or_else(|| rule.get("created_at").and_then(Value::as_str)),
    );
    state
        .storage
        .store
        .save_notification_rule(id, rule, score)
        .await
}

pub(super) async fn load_trigger(
    state: &AppState,
    id: &str,
) -> crate::storage::StorageResult<Option<Value>> {
    state.storage.store.load_notification_trigger(id).await
}

pub(super) async fn save_trigger_raw(
    state: &AppState,
    trigger: &Value,
) -> crate::storage::StorageResult<()> {
    let id = trigger
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let created_at = trigger.get("created_at").and_then(Value::as_str);
    let ttl = history_ttl_seconds(created_at);
    let _ = state
        .storage
        .store
        .save_notification_trigger(id, trigger, iso_score_ms(created_at), ttl, false)
        .await?;
    Ok(())
}

pub(super) async fn save_trigger_if_absent(
    state: &AppState,
    trigger: &Value,
) -> crate::storage::StorageResult<bool> {
    let id = trigger
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let created_at = trigger.get("created_at").and_then(Value::as_str);
    let ttl = history_ttl_seconds(created_at);
    state
        .storage
        .store
        .save_notification_trigger(id, trigger, iso_score_ms(created_at), ttl, true)
        .await
}

pub(super) async fn load_delivery(
    state: &AppState,
    id: &str,
) -> crate::storage::StorageResult<Option<Value>> {
    state.storage.store.load_notification_delivery(id).await
}

pub(super) async fn save_delivery_raw(
    state: &AppState,
    delivery: &Value,
) -> crate::storage::StorageResult<()> {
    let id = delivery
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let triggered_at = delivery.get("triggered_at").and_then(Value::as_str);
    let ttl = history_ttl_seconds(triggered_at);
    let _ = state
        .storage
        .store
        .save_notification_delivery(id, delivery, iso_score_ms(triggered_at), ttl, false)
        .await?;
    Ok(())
}

pub(super) async fn save_delivery_if_absent(
    state: &AppState,
    delivery: &Value,
) -> crate::storage::StorageResult<bool> {
    let id = delivery
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let triggered_at = delivery.get("triggered_at").and_then(Value::as_str);
    let ttl = history_ttl_seconds(triggered_at);
    state
        .storage
        .store
        .save_notification_delivery(id, delivery, iso_score_ms(triggered_at), ttl, true)
        .await
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

pub(super) async fn list_history<F>(
    state: &AppState,
    index_key: &str,
    _data_prefix: &str,
    page: i64,
    limit: i64,
    matches: F,
) -> crate::storage::StorageResult<(Vec<Value>, i64)>
where
    F: Fn(&Value) -> bool,
{
    let kind = match index_key {
        TRIGGERS_INDEX_KEY => "trigger",
        DELIVERIES_INDEX_KEY => "delivery",
        _ => {
            return Err(crate::storage::storage_error(
                "invalid notification history index",
            ));
        }
    };
    let values = state.storage.store.load_notification_history(kind).await?;
    let page_start = (page.saturating_sub(1)).saturating_mul(limit);
    let mut matched_total = 0_i64;
    let mut items = Vec::new();

    for value in values {
        if !matches(&value) {
            continue;
        }
        if matched_total >= page_start && (items.len() as i64) < limit {
            items.push(sanitize_notification_record(value));
        }
        matched_total += 1;
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
) -> crate::storage::StorageResult<usize> {
    let deliveries = state
        .storage
        .store
        .load_notification_history("delivery")
        .await?;
    let mut matched_ids = Vec::new();
    for value in deliveries {
        if matches_optional_string(&value, "rule_id", filter.rule_id.as_deref())
            && matches_optional_string(&value, "provider_id", filter.provider_id.as_deref())
            && matches_optional_string(&value, "trigger_id", filter.trigger_id.as_deref())
            && matches_optional_string(&value, "status", filter.status.as_deref())
            && let Some(id) = value.get("id").and_then(Value::as_str)
        {
            matched_ids.push(id.to_string());
        }
    }
    state
        .storage
        .store
        .delete_notification_deliveries(&matched_ids)
        .await?;
    Ok(matched_ids.len())
}
