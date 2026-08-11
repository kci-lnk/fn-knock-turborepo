use super::*;
use tokio_rusqlite::rusqlite::{Transaction, TransactionBehavior};

fn system_event_command_tx(
    tx: &Transaction<'_>,
    command: &str,
    args: Vec<String>,
) -> crate::storage::StorageResult<redis::CmdOutput> {
    redis::execute_command_in_transaction(tx, command, args)
}

fn command_ok_tx(
    tx: &Transaction<'_>,
    command: &str,
    args: Vec<String>,
) -> crate::storage::StorageResult<()> {
    let _ = system_event_command_tx(tx, command, args)?;
    Ok(())
}

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
        self.typed_events
            .rebuild_from_legacy(
                EVENTS_INDEX_KEY,
                EVENTS_DATA_PREFIX,
                EVENTS_STREAM_ID_PREFIX,
            )
            .await
    }

    fn observe_typed_event_dedupe_shadow(&self, matched: bool) {
        if matched {
            if !self
                .typed_event_dedupe_shadow_healthy
                .swap(true, AtomicOrdering::AcqRel)
            {
                tracing::info!("typed system-event dedupe shadow comparison recovered");
            }
            return;
        }
        self.typed_event_dedupe_shadow_mismatches
            .fetch_add(1, AtomicOrdering::Relaxed);
        self.typed_event_dedupe_shadow_healthy
            .store(false, AtomicOrdering::Release);
        tracing::warn!(
            "typed system-event dedupe shadow differed from the compatibility lease and was repaired"
        );
    }

    fn observe_typed_event_shadow_healthy(&self) {
        if !self
            .typed_events_shadow_healthy
            .swap(true, AtomicOrdering::AcqRel)
        {
            tracing::info!("typed system-event shadow comparison recovered");
        }
    }

    fn observe_typed_event_shadow_failure(&self, reason: &str) {
        self.typed_events_shadow_mismatches
            .fetch_add(1, AtomicOrdering::Relaxed);
        if self
            .typed_events_shadow_healthy
            .swap(false, AtomicOrdering::AcqRel)
        {
            tracing::warn!(%reason, "typed system-event shadow comparison failed");
        }
    }

    #[cfg(test)]
    pub(crate) fn typed_event_shadow_mismatch_count(&self) -> u64 {
        self.typed_events_shadow_mismatches
            .load(AtomicOrdering::Acquire)
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

    pub async fn acquire_notification_runtime_lease(
        &self,
        name: &str,
        token: &str,
        ttl_seconds: usize,
    ) -> crate::storage::StorageResult<bool> {
        let key = notification_runtime_lock_key(name);
        self.verify_notification_runtime_shadow(&key).await?;
        let mut conn = self.conn();
        let result: Option<String> = redis::cmd("SET")
            .arg(key)
            .arg(token)
            .arg("EX")
            .arg(ttl_seconds.max(1))
            .arg("NX")
            .query_async(&mut conn)
            .await?;
        Ok(result.as_deref() == Some("OK"))
    }

    pub async fn release_notification_runtime_lease(
        &self,
        name: &str,
        token: &str,
    ) -> crate::storage::StorageResult<()> {
        let key = notification_runtime_lock_key(name);
        self.verify_notification_runtime_shadow(&key).await?;
        let mut conn = self.conn();
        let _: i64 = redis::cmd("EVAL")
            .arg(
                r#"
                -- fn-knock:eval:delete-if-value:v1
                if redis.call('GET', KEYS[1]) == ARGV[1] then
                    return redis.call('DEL', KEYS[1])
                end
                return 0
                "#,
            )
            .arg(1)
            .arg(key)
            .arg(token)
            .query_async(&mut conn)
            .await?;
        Ok(())
    }

    pub async fn append_notification_window_hit(
        &self,
        rule_id: &str,
        group_key: &str,
        event_id: &str,
        happened_at_ms: i64,
        window_seconds: i64,
    ) -> crate::storage::StorageResult<i64> {
        let key = notification_window_key(rule_id, group_key);
        self.verify_notification_runtime_shadow(&key).await?;
        let window_ms = window_seconds.max(1) * 1000;
        let start_score = (happened_at_ms - window_ms).max(0);
        let event_id = event_id.to_string();
        let ttl_seconds = (window_seconds * 2).max(60);
        self.conn()
            .call(move |conn| {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                command_ok_tx(
                    &tx,
                    "ZADD",
                    vec![key.clone(), happened_at_ms.to_string(), event_id],
                )?;
                command_ok_tx(
                    &tx,
                    "ZREMRANGEBYSCORE",
                    vec![
                        key.clone(),
                        "0".to_string(),
                        start_score.saturating_sub(1).to_string(),
                    ],
                )?;
                command_ok_tx(&tx, "EXPIRE", vec![key.clone(), ttl_seconds.to_string()])?;
                let count = match system_event_command_tx(
                    &tx,
                    "ZCOUNT",
                    vec![key, start_score.to_string(), happened_at_ms.to_string()],
                )? {
                    redis::CmdOutput::Int(count) => count,
                    _ => {
                        return Err(crate::storage::storage_error(
                            "unexpected notification window count",
                        ));
                    }
                };
                tx.commit()?;
                Ok(count)
            })
            .await
    }

    pub async fn get_notification_cooldown_until(
        &self,
        rule_id: &str,
        group_key: &str,
    ) -> crate::storage::StorageResult<Option<String>> {
        let key = notification_cooldown_key(rule_id, group_key);
        self.verify_notification_runtime_shadow(&key).await?;
        self.get_string_value(&key).await
    }

    pub async fn set_notification_cooldown_until(
        &self,
        rule_id: &str,
        group_key: &str,
        until: &str,
        cooldown_seconds: i64,
    ) -> crate::storage::StorageResult<()> {
        if cooldown_seconds <= 0 {
            return Ok(());
        }
        let key = notification_cooldown_key(rule_id, group_key);
        self.verify_notification_runtime_shadow(&key).await?;
        let mut conn = self.conn();
        conn.set_ex(key, until, cooldown_seconds as u64).await
    }

    pub async fn enqueue_notification_delivery(
        &self,
        id: &str,
        ready_at_ms: i64,
    ) -> crate::storage::StorageResult<()> {
        self.verify_notification_runtime_shadow(NOTIFICATION_DELIVERIES_READY_KEY)
            .await?;
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        pipe.zadd(NOTIFICATION_DELIVERIES_READY_KEY, id, ready_at_ms)
            .ignore();
        pipe.expire(
            NOTIFICATION_DELIVERIES_READY_KEY,
            NOTIFICATION_DELIVERY_QUEUE_TTL_SECONDS,
        )
        .ignore();
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(())
    }

    pub async fn pull_ready_notification_delivery_ids(
        &self,
        limit: usize,
        now_ms: i64,
    ) -> crate::storage::StorageResult<Vec<String>> {
        self.verify_notification_runtime_shadow(NOTIFICATION_DELIVERIES_READY_KEY)
            .await?;
        let mut conn = self.conn();
        let ids: Vec<String> = redis::cmd("EVAL")
            .arg(
                r#"
                -- fn-knock:eval:zset-claim:v1
                local ids = redis.call(
                    'ZRANGEBYSCORE',
                    KEYS[1],
                    '-inf',
                    ARGV[1],
                    'LIMIT',
                    0,
                    tonumber(ARGV[2])
                )
                if #ids == 0 then
                    return ids
                end
                redis.call('ZREM', KEYS[1], unpack(ids))
                return ids
                "#,
            )
            .arg(1)
            .arg(NOTIFICATION_DELIVERIES_READY_KEY)
            .arg(now_ms)
            .arg(limit.max(1))
            .query_async(&mut conn)
            .await?;
        Ok(ids.into_iter().filter(|id| !id.trim().is_empty()).collect())
    }

    async fn verify_notification_runtime_shadow(
        &self,
        key: &str,
    ) -> crate::storage::StorageResult<()> {
        let matched = self
            .typed_notification_runtime
            .verify_and_repair_key(key)
            .await?;
        if matched {
            if !self
                .typed_notification_runtime_shadow_healthy
                .swap(true, AtomicOrdering::AcqRel)
            {
                tracing::info!("typed notification runtime shadow comparison recovered");
            }
        } else {
            self.typed_notification_runtime_shadow_mismatches
                .fetch_add(1, AtomicOrdering::AcqRel);
            self.typed_notification_runtime_shadow_healthy
                .store(false, AtomicOrdering::Release);
            let runtime_kind = if key.starts_with(NOTIFICATION_RUNTIME_LOCK_PREFIX) {
                "lease"
            } else if key.starts_with(NOTIFICATION_RUNTIME_COOLDOWN_PREFIX) {
                "cooldown"
            } else if key.starts_with(NOTIFICATION_RUNTIME_WINDOW_PREFIX) {
                "window"
            } else {
                "ready_queue"
            };
            tracing::warn!(
                runtime_kind,
                "typed notification runtime shadow differed from the compatibility keyspace and was repaired"
            );
        }
        Ok(())
    }

    pub async fn list_system_events(
        &self,
        page: i64,
        limit: i64,
        search: &str,
        event_type: Option<&str>,
        level: Option<&str>,
        source: Option<&str>,
    ) -> crate::storage::StorageResult<Value> {
        let typed = self.typed_events.load_active().await;
        let typed_page = typed.as_ref().map(|events| {
            system_event_page(
                events.iter().map(|document| document.event.clone()),
                page,
                limit,
                search,
                event_type,
                level,
                source,
            )
        });
        match (
            typed_page,
            self.list_system_events_legacy(page, limit, search, event_type, level, source)
                .await,
        ) {
            (Ok(typed_page), Ok(legacy_page)) if typed_page == legacy_page => {
                self.observe_typed_event_shadow_healthy();
                Ok(typed_page)
            }
            (Ok(_), Ok(legacy_page)) => {
                self.observe_typed_event_shadow_failure(
                    "typed system-event page differs from legacy keyspace",
                );
                self.rebuild_typed_system_events_from_legacy().await?;
                Ok(legacy_page)
            }
            (Ok(typed_page), Err(error)) => {
                self.observe_typed_event_shadow_failure(&format!(
                    "legacy system-event comparison failed while typed primary remained available: {error}"
                ));
                Ok(typed_page)
            }
            (Err(error), Ok(legacy_page)) => {
                self.observe_typed_event_shadow_failure(&format!(
                    "typed system-event read failed; using legacy fallback: {error}"
                ));
                self.rebuild_typed_system_events_from_legacy().await?;
                Ok(legacy_page)
            }
            (Err(typed_error), Err(legacy_error)) => Err(crate::storage::storage_error(format!(
                "typed and legacy system-event reads both failed: typed={typed_error}; legacy={legacy_error}"
            ))),
        }
    }

    async fn list_system_events_legacy(
        &self,
        page: i64,
        limit: i64,
        search: &str,
        event_type: Option<&str>,
        level: Option<&str>,
        source: Option<&str>,
    ) -> crate::storage::StorageResult<Value> {
        let safe_page = page.max(1);
        let safe_limit = limit.clamp(1, 100);
        let has_filter = !search.trim().is_empty()
            || event_type.is_some()
            || level.is_some()
            || source.is_some();

        if !has_filter {
            let start = (safe_page - 1) * safe_limit;
            loop {
                let mut conn = self.conn();
                let total: i64 = conn.zcard(EVENTS_INDEX_KEY).await?;
                if total == 0 {
                    return Ok(json!({ "events": [], "total": 0 }));
                }
                let ids: Vec<String> = conn
                    .zrevrange(
                        EVENTS_INDEX_KEY,
                        start as isize,
                        (start + safe_limit - 1) as isize,
                    )
                    .await?;
                if ids.is_empty() {
                    return Ok(json!({ "events": [], "total": total }));
                }
                let (events, stale_ids) = self.system_events_by_ids(&ids).await?;
                if !stale_ids.is_empty() {
                    self.remove_stale_system_event_ids(&stale_ids).await?;
                    continue;
                }
                return Ok(json!({ "events": events, "total": total }));
            }
        }

        let page_start = (safe_page - 1) * safe_limit;
        let mut matched_total = 0_i64;
        let mut offset = 0_isize;
        let mut events = Vec::new();
        let mut all_stale_ids = Vec::new();

        loop {
            let mut conn = self.conn();
            let ids: Vec<String> = conn
                .zrevrange(
                    EVENTS_INDEX_KEY,
                    offset,
                    offset + EVENT_LIST_SCAN_CHUNK_SIZE - 1,
                )
                .await?;
            if ids.is_empty() {
                break;
            }
            offset += ids.len() as isize;

            let (batch_events, stale_ids) = self.system_events_by_ids(&ids).await?;
            all_stale_ids.extend(stale_ids);
            for event in batch_events {
                if !system_event_matches_filters(&event, search, event_type, level, source) {
                    continue;
                }
                if matched_total >= page_start && events.len() < safe_limit as usize {
                    events.push(event);
                }
                matched_total += 1;
            }
        }

        if !all_stale_ids.is_empty() {
            self.remove_stale_system_event_ids(&all_stale_ids).await?;
        }
        Ok(json!({ "events": events, "total": matched_total }))
    }

    pub async fn list_system_events_by_range(
        &self,
        from_ms: i64,
        to_ms: i64,
        types: &[&str],
    ) -> crate::storage::StorageResult<Vec<(Value, i64)>> {
        let typed = self.typed_events.load_active().await;
        let allowed_types = types
            .iter()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .collect::<HashSet<_>>();
        let typed_events = typed.as_ref().map(|events| {
            events
                .iter()
                .filter(|document| {
                    document.happened_at_ms >= from_ms.max(0)
                        && document.happened_at_ms <= to_ms.max(from_ms)
                        && (allowed_types.is_empty()
                            || document
                                .event
                                .get("type")
                                .and_then(Value::as_str)
                                .is_some_and(|event_type| allowed_types.contains(event_type)))
                })
                .map(|document| (document.event.clone(), document.happened_at_ms))
                .collect::<Vec<_>>()
        });
        match (
            typed_events,
            self.list_system_events_by_range_legacy(from_ms, to_ms, types)
                .await,
        ) {
            (Ok(typed_events), Ok(legacy_events)) if typed_events == legacy_events => {
                self.observe_typed_event_shadow_healthy();
                Ok(typed_events)
            }
            (Ok(_), Ok(legacy_events)) => {
                self.observe_typed_event_shadow_failure(
                    "typed system-event range differs from legacy keyspace",
                );
                self.rebuild_typed_system_events_from_legacy().await?;
                Ok(legacy_events)
            }
            (Ok(typed_events), Err(error)) => {
                self.observe_typed_event_shadow_failure(&format!(
                    "legacy system-event range comparison failed while typed primary remained available: {error}"
                ));
                Ok(typed_events)
            }
            (Err(error), Ok(legacy_events)) => {
                self.observe_typed_event_shadow_failure(&format!(
                    "typed system-event range read failed; using legacy fallback: {error}"
                ));
                self.rebuild_typed_system_events_from_legacy().await?;
                Ok(legacy_events)
            }
            (Err(typed_error), Err(legacy_error)) => Err(crate::storage::storage_error(format!(
                "typed and legacy system-event range reads both failed: typed={typed_error}; legacy={legacy_error}"
            ))),
        }
    }

    async fn list_system_events_by_range_legacy(
        &self,
        from_ms: i64,
        to_ms: i64,
        types: &[&str],
    ) -> crate::storage::StorageResult<Vec<(Value, i64)>> {
        let mut conn = self.conn();
        let pairs: Vec<String> = redis::cmd("ZRANGEBYSCORE")
            .arg(EVENTS_INDEX_KEY)
            .arg(from_ms.max(0))
            .arg(to_ms.max(from_ms))
            .arg("WITHSCORES")
            .query_async(&mut conn)
            .await?;
        if pairs.is_empty() {
            return Ok(Vec::new());
        }

        let mut ids = Vec::new();
        let mut scores = Vec::new();
        for pair in pairs.chunks(2) {
            let Some(id) = pair.first() else {
                continue;
            };
            let score = pair
                .get(1)
                .and_then(|value| value.parse::<f64>().ok())
                .filter(|value| value.is_finite())
                .map(|value| value as i64)
                .unwrap_or_default();
            ids.push(id.clone());
            scores.push(score);
        }
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let allowed_types = types
            .iter()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .collect::<HashSet<_>>();
        let raws: Vec<Option<String>> = redis::cmd("MGET")
            .arg(
                ids.iter()
                    .map(|id| system_event_data_key(id))
                    .collect::<Vec<_>>(),
            )
            .query_async(&mut conn)
            .await?;
        let mut events = Vec::new();
        let mut stale_ids = Vec::new();
        for ((id, score), raw) in ids.into_iter().zip(scores).zip(raws) {
            let Some(raw) = raw else {
                stale_ids.push(id);
                continue;
            };
            match serde_json::from_str::<Value>(&raw) {
                Ok(event) => {
                    let event_type = event.get("type").and_then(Value::as_str).unwrap_or("");
                    if allowed_types.is_empty() || allowed_types.contains(event_type) {
                        events.push((event, score));
                    }
                }
                Err(_) => stale_ids.push(id),
            }
        }
        if !stale_ids.is_empty() {
            let mut pipe = redis::pipe();
            pipe.zrem(EVENTS_INDEX_KEY, stale_ids).ignore();
            let _: () = pipe.query_async(&mut conn).await?;
        }
        Ok(events)
    }

    pub async fn delete_system_events(&self, ids: &[String]) -> crate::storage::StorageResult<()> {
        let unique_ids = unique_non_empty_strings(ids);
        if unique_ids.is_empty() {
            return Ok(());
        }
        self.conn()
            .call(move |conn| {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                let stream_ids = match system_event_command_tx(
                    &tx,
                    "MGET",
                    unique_ids
                        .iter()
                        .map(|id| system_event_stream_id_key(id))
                        .collect(),
                )? {
                    redis::CmdOutput::OptionalStrings(ids) => ids,
                    _ => {
                        return Err(crate::storage::storage_error(
                            "unexpected system-event stream-id response",
                        ));
                    }
                };
                let valid_stream_ids = stream_ids.into_iter().flatten().collect::<Vec<_>>();
                command_ok_tx(
                    &tx,
                    "DEL",
                    unique_ids
                        .iter()
                        .map(|id| system_event_data_key(id))
                        .collect(),
                )?;
                command_ok_tx(
                    &tx,
                    "DEL",
                    unique_ids
                        .iter()
                        .map(|id| system_event_stream_id_key(id))
                        .collect(),
                )?;
                command_ok_tx(
                    &tx,
                    "ZREM",
                    std::iter::once(EVENTS_INDEX_KEY.to_string())
                        .chain(unique_ids.iter().cloned())
                        .collect(),
                )?;
                if !valid_stream_ids.is_empty() {
                    command_ok_tx(
                        &tx,
                        "XDEL",
                        std::iter::once(EVENTS_STREAM_KEY.to_string())
                            .chain(valid_stream_ids)
                            .collect(),
                    )?;
                }
                TypedEventRepository::delete_tx(&tx, &unique_ids)?;
                tx.commit()?;
                Ok(())
            })
            .await
    }

    pub async fn clear_system_events(&self) -> crate::storage::StorageResult<usize> {
        self.conn()
            .call(move |conn| {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                let ids = match system_event_command_tx(
                    &tx,
                    "ZRANGE",
                    vec![
                        EVENTS_INDEX_KEY.to_string(),
                        "0".to_string(),
                        "-1".to_string(),
                    ],
                )? {
                    redis::CmdOutput::Strings(ids) => ids,
                    _ => {
                        return Err(crate::storage::storage_error(
                            "unexpected system-event index response",
                        ));
                    }
                };
                for batch in ids.chunks(EVENT_CLEAR_CHUNK_SIZE) {
                    command_ok_tx(
                        &tx,
                        "DEL",
                        batch.iter().map(|id| system_event_data_key(id)).collect(),
                    )?;
                    command_ok_tx(
                        &tx,
                        "DEL",
                        batch
                            .iter()
                            .map(|id| system_event_stream_id_key(id))
                            .collect(),
                    )?;
                }
                command_ok_tx(&tx, "DEL", vec![EVENTS_INDEX_KEY.to_string()])?;
                command_ok_tx(&tx, "DEL", vec![EVENTS_STREAM_KEY.to_string()])?;
                TypedEventRepository::clear_tx(&tx)?;
                let count = ids.len();
                tx.commit()?;
                Ok(count)
            })
            .await
    }

    async fn system_events_by_ids(
        &self,
        ids: &[String],
    ) -> crate::storage::StorageResult<(Vec<Value>, Vec<String>)> {
        if ids.is_empty() {
            return Ok((Vec::new(), Vec::new()));
        }
        let mut conn = self.conn();
        let raws: Vec<Option<String>> = redis::cmd("MGET")
            .arg(
                ids.iter()
                    .map(|id| system_event_data_key(id))
                    .collect::<Vec<_>>(),
            )
            .query_async(&mut conn)
            .await?;
        let mut events = Vec::new();
        let mut stale_ids = Vec::new();
        for (id, raw) in ids.iter().zip(raws) {
            let Some(raw) = raw else {
                stale_ids.push(id.clone());
                continue;
            };
            match serde_json::from_str::<Value>(&raw) {
                Ok(event) => events.push(event),
                Err(_) => stale_ids.push(id.clone()),
            }
        }
        Ok((events, stale_ids))
    }

    async fn remove_stale_system_event_ids(
        &self,
        ids: &[String],
    ) -> crate::storage::StorageResult<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let mut conn = self.conn();
        let _: () = redis::cmd("ZREM")
            .arg(EVENTS_INDEX_KEY)
            .arg(ids)
            .query_async(&mut conn)
            .await?;
        Ok(())
    }
}

fn system_event_page(
    events: impl IntoIterator<Item = Value>,
    page: i64,
    limit: i64,
    search: &str,
    event_type: Option<&str>,
    level: Option<&str>,
    source: Option<&str>,
) -> Value {
    let safe_page = page.max(1);
    let safe_limit = limit.clamp(1, 100);
    let page_start = (safe_page - 1) * safe_limit;
    let mut total = 0_i64;
    let mut page_events = Vec::new();
    for event in events {
        if !system_event_matches_filters(&event, search, event_type, level, source) {
            continue;
        }
        if total >= page_start && page_events.len() < safe_limit as usize {
            page_events.push(event);
        }
        total += 1;
    }
    json!({ "events": page_events, "total": total })
}
