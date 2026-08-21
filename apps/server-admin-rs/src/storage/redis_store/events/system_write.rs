use super::*;

impl Store {
    #[cfg(test)]
    pub async fn append_system_event(
        &self,
        event: &Value,
        retention_days: i64,
        max_records: i64,
    ) -> crate::storage::StorageResult<()> {
        self.append_system_event_if_dedupe_available(event, retention_days, max_records, None, 0)
            .await
            .map(|_| ())
    }

    /// Atomically claims an optional dedupe window and persists the event.
    /// A failed event write cannot leave a live dedupe key that suppresses a
    /// retry, and a competing claimant observes `false` without writing.
    pub async fn append_system_event_if_dedupe_available(
        &self,
        event: &Value,
        retention_days: i64,
        max_records: i64,
        dedupe_key: Option<&str>,
        dedupe_ttl_seconds: i64,
    ) -> crate::storage::StorageResult<bool> {
        let event_id = event.get("id").and_then(Value::as_str).unwrap_or("");
        if event_id.trim().is_empty() {
            return Ok(false);
        }
        let dedupe_key = dedupe_key
            .filter(|key| !key.is_empty() && dedupe_ttl_seconds > 0)
            .map(str::to_string);
        if let Some(dedupe_key) = dedupe_key.as_deref() {
            let matched = self
                .typed
                .typed_event_dedupe
                .verify_and_repair(dedupe_key)
                .await?;
            self.observe_typed_event_dedupe_shadow(matched);
        }
        let now = crate::time_utils::now_ms();
        let retention_days = retention_days.clamp(1, MAX_EVENT_RETENTION_DAYS);
        let max_records = max_records.clamp(1_000, 50_000);
        let retention_ms = retention_days * 86_400 * 1000;
        let retention_seconds = retention_days * 86_400;
        let happened_at_ms = event
            .get("happened_at")
            .and_then(Value::as_str)
            .and_then(crate::time_utils::parse_iso_ms)
            .unwrap_or(now);
        let cutoff_timestamp = now - retention_ms;
        let expires_at_ms = happened_at_ms + retention_ms;
        let ttl_seconds =
            (((expires_at_ms - now).max(1000) + 999) / 1000).clamp(1, retention_seconds);
        // Match the actual compatibility-key TTL, including the future-date
        // cap above. A typed shadow must never retain an event longer than
        // the legacy data key that remains authoritative in 2.x.
        let typed_expires_at_ms = now.saturating_add(ttl_seconds.saturating_mul(1000));
        let serialized = serde_json::to_string(event).unwrap_or_default();

        let event_id = event_id.to_string();
        let dedupe_key = dedupe_key.map(|key| format!("{EVENTS_DEDUPE_PREFIX}{key}"));
        self.conn()
            .call(move |conn| {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                if let Some(dedupe_key) = dedupe_key {
                    let claimed = system_event_command_tx(
                        &tx,
                        "SET",
                        vec![
                            dedupe_key,
                            "1".to_string(),
                            "EX".to_string(),
                            dedupe_ttl_seconds.max(1).to_string(),
                            "NX".to_string(),
                        ],
                    )?;
                    if !matches!(
                        claimed,
                        redis::CmdOutput::OptionalString(Some(ref value)) if value == "OK"
                    ) {
                        return Ok(false);
                    }
                }
                let stream_id = match system_event_command_tx(
                    &tx,
                    "XADD",
                    vec![
                        EVENTS_STREAM_KEY.to_string(),
                        "*".to_string(),
                        "event".to_string(),
                        serialized.clone(),
                    ],
                )? {
                    redis::CmdOutput::String(stream_id) => stream_id,
                    _ => return Err(crate::storage::storage_error("unexpected event stream id")),
                };
                TypedEventRepository::upsert_tx(
                    &tx,
                    &event_id,
                    &serialized,
                    happened_at_ms,
                    typed_expires_at_ms,
                    &stream_id,
                )?;
                command_ok_tx(
                    &tx,
                    "SETEX",
                    vec![
                        system_event_data_key(&event_id),
                        ttl_seconds.to_string(),
                        serialized,
                    ],
                )?;
                command_ok_tx(
                    &tx,
                    "ZADD",
                    vec![
                        EVENTS_INDEX_KEY.to_string(),
                        happened_at_ms.to_string(),
                        event_id.clone(),
                    ],
                )?;
                command_ok_tx(
                    &tx,
                    "SETEX",
                    vec![
                        system_event_stream_id_key(&event_id),
                        ttl_seconds.to_string(),
                        stream_id,
                    ],
                )?;
                command_ok_tx(
                    &tx,
                    "ZREMRANGEBYSCORE",
                    vec![
                        EVENTS_INDEX_KEY.to_string(),
                        "0".to_string(),
                        cutoff_timestamp.to_string(),
                    ],
                )?;
                command_ok_tx(
                    &tx,
                    "EXPIRE",
                    vec![EVENTS_INDEX_KEY.to_string(), retention_seconds.to_string()],
                )?;
                command_ok_tx(
                    &tx,
                    "XTRIM",
                    vec![
                        EVENTS_STREAM_KEY.to_string(),
                        "MINID".to_string(),
                        format!("{cutoff_timestamp}-0"),
                    ],
                )?;
                command_ok_tx(
                    &tx,
                    "XTRIM",
                    vec![
                        EVENTS_STREAM_KEY.to_string(),
                        "MAXLEN".to_string(),
                        max_records.to_string(),
                    ],
                )?;
                command_ok_tx(
                    &tx,
                    "EXPIRE",
                    vec![EVENTS_STREAM_KEY.to_string(), retention_seconds.to_string()],
                )?;
                let record_count = match system_event_command_tx(
                    &tx,
                    "ZCARD",
                    vec![EVENTS_INDEX_KEY.to_string()],
                )? {
                    redis::CmdOutput::Int(count) => count,
                    _ => {
                        return Err(crate::storage::storage_error(
                            "unexpected event index count",
                        ));
                    }
                };
                let overflow = record_count - max_records;
                if overflow > 0 {
                    let stale_ids = match system_event_command_tx(
                        &tx,
                        "ZRANGE",
                        vec![
                            EVENTS_INDEX_KEY.to_string(),
                            "0".to_string(),
                            (overflow - 1).to_string(),
                        ],
                    )? {
                        redis::CmdOutput::Strings(ids) => ids,
                        _ => {
                            return Err(crate::storage::storage_error(
                                "unexpected stale event ids",
                            ));
                        }
                    };
                    command_ok_tx(
                        &tx,
                        "ZREM",
                        std::iter::once(EVENTS_INDEX_KEY.to_string())
                            .chain(stale_ids.iter().cloned())
                            .collect(),
                    )?;
                    let stale_keys = stale_ids
                        .iter()
                        .flat_map(|id| [system_event_data_key(id), system_event_stream_id_key(id)])
                        .collect::<Vec<_>>();
                    command_ok_tx(&tx, "DEL", stale_keys)?;
                }
                TypedEventRepository::trim_tx(&tx, cutoff_timestamp, max_records)?;
                tx.commit()?;
                Ok(true)
            })
            .await
    }

    pub(crate) async fn rebuild_typed_system_events_from_legacy(
        &self,
    ) -> crate::storage::StorageResult<()> {
        self.typed
            .typed_events
            .rebuild_from_legacy(
                EVENTS_INDEX_KEY,
                EVENTS_DATA_PREFIX,
                EVENTS_STREAM_ID_PREFIX,
            )
            .await
    }

    pub(super) fn observe_typed_event_dedupe_shadow(&self, matched: bool) {
        if matched {
            if self.typed_event_dedupe_shadow.mark_healthy() {
                tracing::info!("typed system-event dedupe shadow comparison recovered");
            }
            return;
        }
        self.typed_event_dedupe_shadow.mark_mismatch();
        tracing::warn!(
            "typed system-event dedupe shadow differed from the compatibility lease and was repaired"
        );
    }

    pub(super) fn observe_typed_event_shadow_healthy(&self) {
        if self.typed_events_shadow.mark_healthy() {
            tracing::info!("typed system-event shadow comparison recovered");
        }
    }

    pub(super) fn observe_typed_event_shadow_failure(&self, reason: &str) {
        if self.typed_events_shadow.mark_mismatch() {
            tracing::warn!(%reason, "typed system-event shadow comparison failed");
        }
    }

    #[cfg(test)]
    pub(crate) fn typed_event_shadow_mismatch_count(&self) -> u64 {
        self.typed_events_shadow.mismatch_count()
    }

    pub async fn latest_system_event_stream_id(
        &self,
    ) -> crate::storage::StorageResult<Option<String>> {
        let mut conn = self.conn();
        let reply: StreamRangeReply = conn.xrevrange_count(EVENTS_STREAM_KEY, "+", "-", 1).await?;
        Ok(reply.ids.first().map(|entry| entry.id.clone()))
    }

    pub async fn read_system_event_stream_after(
        &self,
        last_id: &str,
        count: usize,
    ) -> crate::storage::StorageResult<Vec<(String, Value)>> {
        let mut conn = self.conn();
        let options = StreamReadOptions::default().count(count.max(1));
        let reply: Option<StreamReadReply> = conn
            .xread_options(&[EVENTS_STREAM_KEY], &[last_id], &options)
            .await?;
        let mut events = Vec::new();
        let Some(reply) = reply else {
            return Ok(events);
        };
        for key in reply.keys {
            for stream_id in key.ids {
                let Some(raw_event) = stream_id.get::<String>("event") else {
                    continue;
                };
                if let Ok(event) = serde_json::from_str::<Value>(&raw_event) {
                    events.push((stream_id.id, event));
                }
            }
        }
        Ok(events)
    }

    pub async fn get_notification_last_stream_id(
        &self,
    ) -> crate::storage::StorageResult<Option<String>> {
        self.get_string_value(NOTIFICATION_RUNTIME_LAST_STREAM_KEY)
            .await
    }

    pub async fn set_notification_last_stream_id(
        &self,
        id: &str,
    ) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        conn.set(NOTIFICATION_RUNTIME_LAST_STREAM_KEY, id).await
    }
}
