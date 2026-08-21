use super::*;

pub(super) const NOTIFICATION_RUNTIME_LAST_STREAM_KEY: &str =
    "fn_knock:notifications:runtime:last-stream-id";
pub(super) const NOTIFICATION_RUNTIME_LOCK_PREFIX: &str = "fn_knock:notifications:runtime:lock:";
pub(super) const NOTIFICATION_RUNTIME_COOLDOWN_PREFIX: &str =
    "fn_knock:notifications:runtime:cooldown:";
pub(super) const NOTIFICATION_RUNTIME_WINDOW_PREFIX: &str =
    "fn_knock:notifications:runtime:window:";
pub(super) const NOTIFICATION_DELIVERIES_READY_KEY: &str =
    "fn_knock:notifications:deliveries:ready";
pub(super) const NOTIFICATION_DELIVERY_QUEUE_TTL_SECONDS: i64 = 30 * 24 * 60 * 60;

pub(super) fn notification_runtime_lock_key(name: &str) -> String {
    format!("{NOTIFICATION_RUNTIME_LOCK_PREFIX}{name}")
}

pub(super) fn notification_cooldown_key(rule_id: &str, group_key: &str) -> String {
    format!(
        "{NOTIFICATION_RUNTIME_COOLDOWN_PREFIX}{rule_id}:{}",
        encode_notification_key_part(group_key)
    )
}

pub(super) fn notification_window_key(rule_id: &str, group_key: &str) -> String {
    format!(
        "{NOTIFICATION_RUNTIME_WINDOW_PREFIX}{rule_id}:{}",
        encode_notification_key_part(group_key)
    )
}

pub(super) fn encode_notification_key_part(value: &str) -> String {
    URL_SAFE_NO_PAD.encode(if value.is_empty() { "empty" } else { value })
}

use tokio_rusqlite::rusqlite::{Transaction, TransactionBehavior};

const PROVIDER_KIND: &str = "provider";
const RULE_KIND: &str = "rule";
const PROVIDERS_INDEX_KEY: &str = "fn_knock:notifications:providers:index";
const PROVIDERS_DATA_PREFIX: &str = "fn_knock:notifications:providers:data:";
const RULES_INDEX_KEY: &str = "fn_knock:notifications:rules:index";
const RULES_DATA_PREFIX: &str = "fn_knock:notifications:rules:data:";
const TRIGGER_KIND: &str = "trigger";
const DELIVERY_KIND: &str = "delivery";
const TRIGGERS_INDEX_KEY: &str = "fn_knock:notifications:triggers:index";
const TRIGGERS_DATA_PREFIX: &str = "fn_knock:notifications:triggers:data:";
const DELIVERIES_INDEX_KEY: &str = "fn_knock:notifications:deliveries:index";
const DELIVERIES_DATA_PREFIX: &str = "fn_knock:notifications:deliveries:data:";
const DELIVERIES_READY_KEY: &str = "fn_knock:notifications:deliveries:ready";
const HISTORY_RETENTION_TTL_SECONDS: i64 = 30 * 24 * 60 * 60;
const HISTORY_MAX_RECORDS: i64 = 50_000;

fn notification_command_tx(
    tx: &Transaction<'_>,
    command: &str,
    args: Vec<String>,
) -> crate::storage::StorageResult<redis::CmdOutput> {
    redis::execute_command_in_transaction(tx, command, args)
}

fn notification_command_ok_tx(
    tx: &Transaction<'_>,
    command: &str,
    args: Vec<String>,
) -> crate::storage::StorageResult<()> {
    let _ = notification_command_tx(tx, command, args)?;
    Ok(())
}

impl Store {
    /// Rebuild the transient ready queue from durable, non-terminal delivery
    /// history. This closes both crash windows around queue claim and enqueue:
    /// a delivery left in `sending`, `queued`, or retryable `failed` state is
    /// made ready again when the runtime starts.
    pub(crate) async fn rebuild_notification_delivery_ready_queue(
        &self,
    ) -> crate::storage::StorageResult<usize> {
        // Runtime scheduling remains legacy-primary in 2.x. Recovery must not
        // consume a typed-only history row if the compatibility index is
        // missing or unreadable.
        let deliveries = self
            .load_notification_records_legacy(DELIVERIES_INDEX_KEY, DELIVERIES_DATA_PREFIX)
            .await?;
        let mut ready = deliveries
            .into_iter()
            .filter_map(|delivery| {
                let status = delivery.get("status").and_then(Value::as_str);
                if matches!(status, Some("success" | "gave_up" | "skipped")) {
                    return None;
                }
                let id = delivery
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|id| !id.is_empty())?;
                let ready_at_ms = delivery
                    .get("next_retry_at")
                    .and_then(Value::as_str)
                    .and_then(crate::time_utils::parse_iso_ms)
                    .or_else(|| {
                        delivery
                            .get("triggered_at")
                            .and_then(Value::as_str)
                            .and_then(crate::time_utils::parse_iso_ms)
                    })
                    .unwrap_or_else(crate::time_utils::now_ms);
                Some((id.to_string(), ready_at_ms))
            })
            .collect::<Vec<_>>();
        ready.sort_by(|left, right| left.0.cmp(&right.0));
        ready.dedup_by(|left, right| left.0 == right.0);
        let count = ready.len();
        self.conn()
            .call(move |conn| {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                notification_command_ok_tx(&tx, "DEL", vec![DELIVERIES_READY_KEY.to_string()])?;
                for (id, ready_at_ms) in ready {
                    notification_command_ok_tx(
                        &tx,
                        "ZADD",
                        vec![
                            DELIVERIES_READY_KEY.to_string(),
                            ready_at_ms.to_string(),
                            id,
                        ],
                    )?;
                }
                if count > 0 {
                    notification_command_ok_tx(
                        &tx,
                        "EXPIRE",
                        vec![
                            DELIVERIES_READY_KEY.to_string(),
                            HISTORY_RETENTION_TTL_SECONDS.to_string(),
                        ],
                    )?;
                }
                tx.commit()?;
                Ok(count)
            })
            .await
    }

    pub(crate) async fn rebuild_typed_notification_documents_from_legacy(
        &self,
    ) -> crate::storage::StorageResult<()> {
        self.typed
            .typed_notifications
            .rebuild_from_legacy(vec![
                (
                    PROVIDER_KIND.to_string(),
                    PROVIDERS_INDEX_KEY.to_string(),
                    PROVIDERS_DATA_PREFIX.to_string(),
                ),
                (
                    RULE_KIND.to_string(),
                    RULES_INDEX_KEY.to_string(),
                    RULES_DATA_PREFIX.to_string(),
                ),
            ])
            .await
    }

    pub(crate) async fn rebuild_typed_notification_history_from_legacy(
        &self,
    ) -> crate::storage::StorageResult<()> {
        self.typed
            .typed_notifications
            .rebuild_history_from_legacy(vec![
                (
                    TRIGGER_KIND.to_string(),
                    TRIGGERS_INDEX_KEY.to_string(),
                    TRIGGERS_DATA_PREFIX.to_string(),
                ),
                (
                    DELIVERY_KIND.to_string(),
                    DELIVERIES_INDEX_KEY.to_string(),
                    DELIVERIES_DATA_PREFIX.to_string(),
                ),
            ])
            .await
    }

    pub(crate) async fn save_notification_trigger(
        &self,
        id: &str,
        value: &Value,
        score: i64,
        ttl_seconds: usize,
        only_if_absent: bool,
    ) -> crate::storage::StorageResult<bool> {
        self.save_typed_notification_history_record(
            TRIGGER_KIND,
            TRIGGERS_INDEX_KEY,
            TRIGGERS_DATA_PREFIX,
            None,
            id,
            value,
            score,
            ttl_seconds,
            only_if_absent,
        )
        .await
    }

    pub(crate) async fn save_notification_delivery(
        &self,
        id: &str,
        value: &Value,
        score: i64,
        ttl_seconds: usize,
        only_if_absent: bool,
    ) -> crate::storage::StorageResult<bool> {
        self.save_typed_notification_history_record(
            DELIVERY_KIND,
            DELIVERIES_INDEX_KEY,
            DELIVERIES_DATA_PREFIX,
            Some(DELIVERIES_READY_KEY),
            id,
            value,
            score,
            ttl_seconds,
            only_if_absent,
        )
        .await
    }

    pub(crate) async fn load_notification_trigger(
        &self,
        id: &str,
    ) -> crate::storage::StorageResult<Option<Value>> {
        self.load_typed_notification_history_record(
            TRIGGER_KIND,
            &format!("{TRIGGERS_DATA_PREFIX}{id}"),
            id,
        )
        .await
    }

    pub(crate) async fn load_notification_delivery(
        &self,
        id: &str,
    ) -> crate::storage::StorageResult<Option<Value>> {
        self.load_typed_notification_history_record(
            DELIVERY_KIND,
            &format!("{DELIVERIES_DATA_PREFIX}{id}"),
            id,
        )
        .await
    }

    pub(crate) async fn load_notification_history(
        &self,
        kind: &str,
    ) -> crate::storage::StorageResult<Vec<Value>> {
        let (index_key, data_prefix) = match kind {
            TRIGGER_KIND => (TRIGGERS_INDEX_KEY, TRIGGERS_DATA_PREFIX),
            DELIVERY_KIND => (DELIVERIES_INDEX_KEY, DELIVERIES_DATA_PREFIX),
            _ => {
                return Err(crate::storage::storage_error(
                    "invalid notification history kind",
                ));
            }
        };
        let typed = self.typed.typed_notifications.load_history(kind).await;
        let legacy = self
            .load_notification_records_legacy(index_key, data_prefix)
            .await;
        match (typed, legacy) {
            (Ok(typed), Ok(legacy)) if typed == legacy => Ok(typed),
            (Ok(_), Ok(legacy)) | (Err(_), Ok(legacy)) => {
                self.rebuild_typed_notification_history_from_legacy()
                    .await?;
                Ok(legacy)
            }
            (Ok(typed), Err(_)) => Ok(typed),
            (Err(typed_error), Err(legacy_error)) => Err(crate::storage::storage_error(format!(
                "typed and legacy notification history reads both failed: typed={typed_error}; legacy={legacy_error}"
            ))),
        }
    }

    pub(crate) async fn delete_notification_deliveries(
        &self,
        ids: &[String],
    ) -> crate::storage::StorageResult<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let ids = ids.to_vec();
        self.conn()
            .call(move |conn| {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                TypedNotificationRepository::delete_history_tx(&tx, DELIVERY_KIND, &ids)?;
                notification_command_ok_tx(
                    &tx,
                    "DEL",
                    ids.iter()
                        .map(|id| format!("{DELIVERIES_DATA_PREFIX}{id}"))
                        .collect(),
                )?;
                notification_command_ok_tx(
                    &tx,
                    "ZREM",
                    std::iter::once(DELIVERIES_INDEX_KEY.to_string())
                        .chain(ids.iter().cloned())
                        .collect(),
                )?;
                notification_command_ok_tx(
                    &tx,
                    "ZREM",
                    std::iter::once(DELIVERIES_READY_KEY.to_string())
                        .chain(ids.iter().cloned())
                        .collect(),
                )?;
                tx.commit()?;
                Ok(())
            })
            .await
    }

    pub(crate) async fn save_notification_provider(
        &self,
        id: &str,
        value: &Value,
        sort_score: i64,
    ) -> crate::storage::StorageResult<()> {
        self.save_typed_notification_record(
            PROVIDER_KIND,
            PROVIDERS_INDEX_KEY,
            &format!("{PROVIDERS_DATA_PREFIX}{id}"),
            id,
            value,
            sort_score,
        )
        .await
    }

    pub(crate) async fn load_notification_providers(
        &self,
    ) -> crate::storage::StorageResult<Vec<Value>> {
        self.load_typed_notification_records(
            PROVIDER_KIND,
            PROVIDERS_INDEX_KEY,
            PROVIDERS_DATA_PREFIX,
        )
        .await
    }

    pub(crate) async fn load_notification_rules(
        &self,
    ) -> crate::storage::StorageResult<Vec<Value>> {
        self.load_typed_notification_records(RULE_KIND, RULES_INDEX_KEY, RULES_DATA_PREFIX)
            .await
    }

    pub(crate) async fn load_notification_provider(
        &self,
        id: &str,
    ) -> crate::storage::StorageResult<Option<Value>> {
        self.load_typed_notification_record(
            PROVIDER_KIND,
            &format!("{PROVIDERS_DATA_PREFIX}{id}"),
            id,
        )
        .await
    }

    pub(crate) async fn load_notification_rule(
        &self,
        id: &str,
    ) -> crate::storage::StorageResult<Option<Value>> {
        self.load_typed_notification_record(RULE_KIND, &format!("{RULES_DATA_PREFIX}{id}"), id)
            .await
    }

    pub(crate) async fn save_notification_rule(
        &self,
        id: &str,
        value: &Value,
        sort_score: i64,
    ) -> crate::storage::StorageResult<()> {
        self.save_typed_notification_record(
            RULE_KIND,
            RULES_INDEX_KEY,
            &format!("{RULES_DATA_PREFIX}{id}"),
            id,
            value,
            sort_score,
        )
        .await
    }

    pub(crate) async fn delete_notification_provider(
        &self,
        id: &str,
    ) -> crate::storage::StorageResult<()> {
        self.delete_typed_notification_record(
            PROVIDER_KIND,
            PROVIDERS_INDEX_KEY,
            &format!("{PROVIDERS_DATA_PREFIX}{id}"),
            id,
        )
        .await
    }

    pub(crate) async fn delete_notification_rule(
        &self,
        id: &str,
    ) -> crate::storage::StorageResult<()> {
        self.delete_typed_notification_record(
            RULE_KIND,
            RULES_INDEX_KEY,
            &format!("{RULES_DATA_PREFIX}{id}"),
            id,
        )
        .await
    }

    async fn save_typed_notification_record(
        &self,
        kind: &str,
        index_key: &str,
        data_key: &str,
        id: &str,
        value: &Value,
        sort_score: i64,
    ) -> crate::storage::StorageResult<()> {
        let kind = kind.to_string();
        let index_key = index_key.to_string();
        let data_key = data_key.to_string();
        let id = id.to_string();
        let document_json = serde_json::to_string(value)?;
        self.conn()
            .call(move |conn| {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                TypedNotificationRepository::upsert_tx(
                    &tx,
                    &kind,
                    &id,
                    &document_json,
                    sort_score,
                )?;
                notification_command_ok_tx(&tx, "SET", vec![data_key, document_json])?;
                notification_command_ok_tx(
                    &tx,
                    "ZADD",
                    vec![index_key, sort_score.to_string(), id],
                )?;
                tx.commit()?;
                Ok(())
            })
            .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn save_typed_notification_history_record(
        &self,
        kind: &str,
        index_key: &str,
        data_prefix: &str,
        ready_key: Option<&str>,
        id: &str,
        value: &Value,
        score: i64,
        ttl_seconds: usize,
        only_if_absent: bool,
    ) -> crate::storage::StorageResult<bool> {
        if id.trim().is_empty() {
            return Err(crate::storage::storage_error(
                "notification history id cannot be empty",
            ));
        }
        let kind = kind.to_string();
        let index_key = index_key.to_string();
        let data_prefix = data_prefix.to_string();
        let ready_key = ready_key.map(str::to_string);
        let id = id.to_string();
        let data_key = format!("{data_prefix}{id}");
        let document_json = serde_json::to_string(value)?;
        let ttl_seconds = ttl_seconds.max(1) as i64;
        self.conn()
            .call(move |conn| {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                let (saved, effective_json, effective_score, effective_expires_at_ms) =
                    if only_if_absent {
                        let saved = matches!(
                            notification_command_tx(
                                &tx,
                                "SET",
                                vec![
                                    data_key.clone(),
                                    document_json.clone(),
                                    "EX".to_string(),
                                    ttl_seconds.to_string(),
                                    "NX".to_string(),
                                ],
                            )?,
                            redis::CmdOutput::OptionalString(Some(_))
                        );
                        if saved {
                            (
                                true,
                                document_json.clone(),
                                score,
                                crate::time_utils::now_ms()
                                    .saturating_add(ttl_seconds.saturating_mul(1000)),
                            )
                        } else {
                            let existing = match notification_command_tx(
                                &tx,
                                "GET",
                                vec![data_key.clone()],
                            )? {
                                redis::CmdOutput::OptionalString(Some(raw)) => raw,
                                _ => return Ok(false),
                            };
                            let existing_value: Value = serde_json::from_str(&existing)?;
                            let effective_score =
                                notification_history_score(&kind, &existing_value);
                            let remaining_ttl = match notification_command_tx(
                                &tx,
                                "TTL",
                                vec![data_key.clone()],
                            )? {
                                redis::CmdOutput::Int(ttl) => ttl.max(1),
                                _ => 1,
                            };
                            (
                                false,
                                existing,
                                effective_score,
                                crate::time_utils::now_ms()
                                    .saturating_add(remaining_ttl.saturating_mul(1000)),
                            )
                        }
                    } else {
                        notification_command_ok_tx(
                            &tx,
                            "SETEX",
                            vec![
                                data_key.clone(),
                                ttl_seconds.to_string(),
                                document_json.clone(),
                            ],
                        )?;
                        (
                            true,
                            document_json,
                            score,
                            crate::time_utils::now_ms()
                                .saturating_add(ttl_seconds.saturating_mul(1000)),
                        )
                    };
                TypedNotificationRepository::upsert_history_tx(
                    &tx,
                    &kind,
                    &id,
                    &effective_json,
                    effective_score,
                    effective_expires_at_ms,
                )?;
                notification_command_ok_tx(
                    &tx,
                    "ZADD",
                    vec![index_key.clone(), effective_score.to_string(), id],
                )?;
                let cutoff_score = crate::time_utils::now_ms()
                    .saturating_sub(HISTORY_RETENTION_TTL_SECONDS.saturating_mul(1000))
                    .saturating_add(1);
                notification_command_ok_tx(
                    &tx,
                    "ZREMRANGEBYSCORE",
                    vec![
                        index_key.clone(),
                        "0".to_string(),
                        cutoff_score.saturating_sub(1).to_string(),
                    ],
                )?;
                notification_command_ok_tx(
                    &tx,
                    "EXPIRE",
                    vec![index_key.clone(), HISTORY_RETENTION_TTL_SECONDS.to_string()],
                )?;
                let count = match notification_command_tx(&tx, "ZCARD", vec![index_key.clone()])? {
                    redis::CmdOutput::Int(count) => count,
                    _ => return Err(crate::storage::storage_error("unexpected history count")),
                };
                let overflow = count.saturating_sub(HISTORY_MAX_RECORDS);
                if overflow > 0 {
                    let stale_ids = match notification_command_tx(
                        &tx,
                        "ZRANGE",
                        vec![
                            index_key.clone(),
                            "0".to_string(),
                            (overflow - 1).to_string(),
                        ],
                    )? {
                        redis::CmdOutput::Strings(ids) => ids,
                        _ => {
                            return Err(crate::storage::storage_error(
                                "unexpected stale notification history ids",
                            ));
                        }
                    };
                    if !stale_ids.is_empty() {
                        notification_command_ok_tx(
                            &tx,
                            "ZREM",
                            std::iter::once(index_key.clone())
                                .chain(stale_ids.iter().cloned())
                                .collect(),
                        )?;
                        notification_command_ok_tx(
                            &tx,
                            "DEL",
                            stale_ids
                                .iter()
                                .map(|id| format!("{data_prefix}{id}"))
                                .collect(),
                        )?;
                        if let Some(ready_key) = &ready_key {
                            notification_command_ok_tx(
                                &tx,
                                "ZREM",
                                std::iter::once(ready_key.clone())
                                    .chain(stale_ids.iter().cloned())
                                    .collect(),
                            )?;
                        }
                    }
                }
                TypedNotificationRepository::trim_history_tx(
                    &tx,
                    &kind,
                    cutoff_score,
                    HISTORY_MAX_RECORDS,
                )?;
                tx.commit()?;
                Ok(saved)
            })
            .await
    }

    async fn load_typed_notification_records(
        &self,
        kind: &str,
        index_key: &str,
        data_prefix: &str,
    ) -> crate::storage::StorageResult<Vec<Value>> {
        let typed = self.typed.typed_notifications.load_kind(kind).await;
        let legacy = self
            .load_notification_records_legacy(index_key, data_prefix)
            .await;
        match (typed, legacy) {
            (Ok(typed), Ok(legacy)) if typed == legacy => Ok(typed),
            (Ok(_), Ok(legacy)) | (Err(_), Ok(legacy)) => {
                self.rebuild_typed_notification_documents_from_legacy()
                    .await?;
                Ok(legacy)
            }
            (Ok(typed), Err(_)) => Ok(typed),
            (Err(typed_error), Err(legacy_error)) => Err(crate::storage::storage_error(format!(
                "typed and legacy notification reads both failed: typed={typed_error}; legacy={legacy_error}"
            ))),
        }
    }

    async fn load_typed_notification_record(
        &self,
        kind: &str,
        data_key: &str,
        id: &str,
    ) -> crate::storage::StorageResult<Option<Value>> {
        let typed = self.typed.typed_notifications.load_one(kind, id).await;
        let legacy = self.get_json_value(data_key).await;
        match (typed, legacy) {
            (Ok(typed), Ok(legacy)) if typed == legacy => Ok(typed),
            (Ok(_), Ok(legacy)) | (Err(_), Ok(legacy)) => {
                self.rebuild_typed_notification_documents_from_legacy()
                    .await?;
                Ok(legacy)
            }
            (Ok(typed), Err(_)) => Ok(typed),
            (Err(typed_error), Err(legacy_error)) => Err(crate::storage::storage_error(format!(
                "typed and legacy notification record reads both failed: typed={typed_error}; legacy={legacy_error}"
            ))),
        }
    }

    async fn load_typed_notification_history_record(
        &self,
        kind: &str,
        data_key: &str,
        id: &str,
    ) -> crate::storage::StorageResult<Option<Value>> {
        let typed = self
            .typed
            .typed_notifications
            .load_history_one(kind, id)
            .await;
        let legacy = self.get_json_value(data_key).await;
        match (typed, legacy) {
            (Ok(typed), Ok(legacy)) if typed == legacy => Ok(typed),
            (Ok(_), Ok(legacy)) | (Err(_), Ok(legacy)) => {
                self.rebuild_typed_notification_history_from_legacy()
                    .await?;
                Ok(legacy)
            }
            (Ok(typed), Err(_)) => Ok(typed),
            (Err(typed_error), Err(legacy_error)) => Err(crate::storage::storage_error(format!(
                "typed and legacy notification history record reads both failed: typed={typed_error}; legacy={legacy_error}"
            ))),
        }
    }

    async fn load_notification_records_legacy(
        &self,
        index_key: &str,
        data_prefix: &str,
    ) -> crate::storage::StorageResult<Vec<Value>> {
        let ids = self.zrevrange_strings(index_key).await?;
        let mut values = Vec::new();
        let mut stale = Vec::new();
        for id in ids {
            match self.get_json_value(&format!("{data_prefix}{id}")).await? {
                Some(value) => values.push(value),
                None => stale.push(id),
            }
        }
        for id in stale {
            self.zrem_string_member(index_key, &id).await?;
        }
        Ok(values)
    }

    async fn delete_typed_notification_record(
        &self,
        kind: &str,
        index_key: &str,
        data_key: &str,
        id: &str,
    ) -> crate::storage::StorageResult<()> {
        let kind = kind.to_string();
        let index_key = index_key.to_string();
        let data_key = data_key.to_string();
        let id = id.to_string();
        self.conn()
            .call(move |conn| {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                TypedNotificationRepository::delete_tx(&tx, &kind, &id)?;
                notification_command_ok_tx(&tx, "DEL", vec![data_key])?;
                notification_command_ok_tx(&tx, "ZREM", vec![index_key, id])?;
                tx.commit()?;
                Ok(())
            })
            .await
    }
}

fn notification_history_score(kind: &str, value: &Value) -> i64 {
    let timestamp_field = if kind == TRIGGER_KIND {
        "created_at"
    } else {
        "triggered_at"
    };
    value
        .get(timestamp_field)
        .and_then(Value::as_str)
        .and_then(crate::time_utils::parse_iso_ms)
        .unwrap_or_else(crate::time_utils::now_ms)
}
