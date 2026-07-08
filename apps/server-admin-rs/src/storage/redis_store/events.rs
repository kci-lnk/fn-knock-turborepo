use super::*;

impl Store {
    pub async fn append_system_event(
        &self,
        event: &Value,
        retention_days: i64,
    ) -> crate::storage::StorageResult<()> {
        let event_id = event.get("id").and_then(Value::as_str).unwrap_or("");
        if event_id.trim().is_empty() {
            return Ok(());
        }
        let now = crate::time_utils::now_ms();
        let retention_days = retention_days.clamp(1, MAX_EVENT_RETENTION_DAYS);
        let retention_ms = retention_days * 86_400 * 1000;
        let happened_at_ms = event
            .get("happened_at")
            .and_then(Value::as_str)
            .and_then(crate::time_utils::parse_iso_ms)
            .unwrap_or(now);
        let cutoff_timestamp = now - retention_ms;
        let expires_at_ms = happened_at_ms + retention_ms;
        let ttl_seconds = ((expires_at_ms - now).max(1000) + 999) / 1000;
        let serialized = serde_json::to_string(event).unwrap_or_default();

        let mut conn = self.conn();
        let stream_id: String = redis::cmd("XADD")
            .arg(EVENTS_STREAM_KEY)
            .arg("*")
            .arg("event")
            .arg(&serialized)
            .query_async(&mut conn)
            .await?;

        let mut pipe = redis::pipe();
        pipe.set_ex(
            system_event_data_key(event_id),
            &serialized,
            ttl_seconds as u64,
        )
        .ignore();
        pipe.zadd(EVENTS_INDEX_KEY, event_id, happened_at_ms)
            .ignore();
        pipe.set_ex(
            system_event_stream_id_key(event_id),
            stream_id,
            ttl_seconds as u64,
        )
        .ignore();
        pipe.zrembyscore(EVENTS_INDEX_KEY, 0, cutoff_timestamp)
            .ignore();
        pipe.cmd("XTRIM")
            .arg(EVENTS_STREAM_KEY)
            .arg("MINID")
            .arg("~")
            .arg(format!("{cutoff_timestamp}-0"))
            .ignore();
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(())
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
        let mut conn = self.conn();
        let result: Option<String> = redis::cmd("SET")
            .arg(notification_runtime_lock_key(name))
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
        let mut conn = self.conn();
        let _: i64 = redis::cmd("EVAL")
            .arg(
                r#"
                if redis.call('GET', KEYS[1]) == ARGV[1] then
                    return redis.call('DEL', KEYS[1])
                end
                return 0
                "#,
            )
            .arg(1)
            .arg(notification_runtime_lock_key(name))
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
        let window_ms = window_seconds.max(1) * 1000;
        let start_score = (happened_at_ms - window_ms).max(0);
        let mut conn = self.conn();
        let _: () = conn.zadd(&key, event_id, happened_at_ms).await?;
        let _: () = conn
            .zrembyscore(&key, 0, start_score.saturating_sub(1))
            .await?;
        let _: () = conn.expire(&key, (window_seconds * 2).max(60)).await?;
        conn.zcount(&key, start_score, happened_at_ms).await
    }

    pub async fn get_notification_cooldown_until(
        &self,
        rule_id: &str,
        group_key: &str,
    ) -> crate::storage::StorageResult<Option<String>> {
        self.get_string_value(&notification_cooldown_key(rule_id, group_key))
            .await
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
        let mut conn = self.conn();
        conn.set_ex(
            notification_cooldown_key(rule_id, group_key),
            until,
            cooldown_seconds as u64,
        )
        .await
    }

    pub async fn enqueue_notification_delivery(
        &self,
        id: &str,
        ready_at_ms: i64,
    ) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        conn.zadd(NOTIFICATION_DELIVERIES_READY_KEY, id, ready_at_ms)
            .await
    }

    pub async fn pull_ready_notification_delivery_ids(
        &self,
        limit: usize,
        now_ms: i64,
    ) -> crate::storage::StorageResult<Vec<String>> {
        let mut conn = self.conn();
        let ids: Vec<String> = redis::cmd("EVAL")
            .arg(
                r#"
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

    pub async fn acquire_system_event_dedupe(
        &self,
        key: &str,
        ttl_seconds: i64,
    ) -> crate::storage::StorageResult<bool> {
        let mut conn = self.conn();
        let result: Option<String> = redis::cmd("SET")
            .arg(format!("{EVENTS_DEDUPE_PREFIX}{key}"))
            .arg("1")
            .arg("EX")
            .arg(ttl_seconds.max(1))
            .arg("NX")
            .query_async(&mut conn)
            .await?;
        Ok(result.as_deref() == Some("OK"))
    }

    pub async fn release_system_event_dedupe(
        &self,
        key: &str,
    ) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        conn.del(format!("{EVENTS_DEDUPE_PREFIX}{key}")).await
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
        let mut conn = self.conn();
        let stream_ids: Vec<Option<String>> = redis::cmd("MGET")
            .arg(
                unique_ids
                    .iter()
                    .map(|id| system_event_stream_id_key(id))
                    .collect::<Vec<_>>(),
            )
            .query_async(&mut conn)
            .await?;
        let valid_stream_ids = stream_ids.into_iter().flatten().collect::<Vec<_>>();
        let mut pipe = redis::pipe();
        pipe.del(
            unique_ids
                .iter()
                .map(|id| system_event_data_key(id))
                .collect::<Vec<_>>(),
        )
        .ignore();
        pipe.del(
            unique_ids
                .iter()
                .map(|id| system_event_stream_id_key(id))
                .collect::<Vec<_>>(),
        )
        .ignore();
        pipe.zrem(EVENTS_INDEX_KEY, unique_ids.clone()).ignore();
        if !valid_stream_ids.is_empty() {
            pipe.cmd("XDEL")
                .arg(EVENTS_STREAM_KEY)
                .arg(valid_stream_ids)
                .ignore();
        }
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(())
    }

    pub async fn clear_system_events(&self) -> crate::storage::StorageResult<usize> {
        let mut conn = self.conn();
        let ids: Vec<String> = conn.zrange(EVENTS_INDEX_KEY, 0, -1).await?;
        let mut pipe = redis::pipe();
        for batch in ids.chunks(EVENT_CLEAR_CHUNK_SIZE) {
            pipe.del(
                batch
                    .iter()
                    .map(|id| system_event_data_key(id))
                    .collect::<Vec<_>>(),
            )
            .ignore();
            pipe.del(
                batch
                    .iter()
                    .map(|id| system_event_stream_id_key(id))
                    .collect::<Vec<_>>(),
            )
            .ignore();
        }
        pipe.del(EVENTS_INDEX_KEY).ignore();
        pipe.del(EVENTS_STREAM_KEY).ignore();
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(ids.len())
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
