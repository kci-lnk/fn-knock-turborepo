use super::*;

impl Store {
    pub async fn list_system_events(
        &self,
        page: i64,
        limit: i64,
        search: &str,
        event_type: Option<&str>,
        level: Option<&str>,
        source: Option<&str>,
    ) -> crate::storage::StorageResult<Value> {
        let typed = self.typed.typed_events.load_active().await;
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

    pub(super) async fn list_system_events_legacy(
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
        let typed = self.typed.typed_events.load_active().await;
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

    pub(super) async fn list_system_events_by_range_legacy(
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

    pub(super) async fn system_events_by_ids(
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

    pub(super) async fn remove_stale_system_event_ids(
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
