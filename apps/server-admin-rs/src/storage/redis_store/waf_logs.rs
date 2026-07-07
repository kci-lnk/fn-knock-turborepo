use super::*;

impl RedisStore {
    pub async fn count_waf_logs_for_buckets(
        &self,
        bucket_starts: &[i64],
        to_ms: i64,
    ) -> redis::RedisResult<(i64, Vec<i64>)> {
        if bucket_starts.is_empty() {
            return Ok((0, Vec::new()));
        }
        let step = if bucket_starts.len() > 1 {
            (bucket_starts[1] - bucket_starts[0]).max(1)
        } else {
            1
        };
        let mut total = 0_i64;
        let mut counts = vec![0_i64; bucket_starts.len()];
        let mut conn = self.conn();

        for (bucket_index, bucket_start) in bucket_starts.iter().enumerate() {
            let bucket_end = if bucket_index == bucket_starts.len() - 1 {
                to_ms
            } else {
                to_ms.min(bucket_start + step)
            };
            let end_arg = if bucket_index == bucket_starts.len() - 1 {
                bucket_end.to_string()
            } else {
                format!("({bucket_end}")
            };
            for date in waf_log_dates_for_range(*bucket_start, bucket_end) {
                let count: i64 = redis::cmd("ZCOUNT")
                    .arg(waf_log_date_key(&date))
                    .arg(*bucket_start)
                    .arg(&end_arg)
                    .query_async(&mut conn)
                    .await?;
                counts[bucket_index] += count;
                total += count;
            }
        }

        Ok((total, counts))
    }

    pub async fn persist_waf_events(
        &self,
        events: &[Value],
        retention_days: i64,
    ) -> redis::RedisResult<()> {
        if events.is_empty() {
            return Ok(());
        }
        let ttl_seconds = retention_days.clamp(1, 365) * 86_400;
        let cutoff_date =
            crate::time_utils::local_date_from_ms(crate::time_utils::now_ms() - ttl_seconds * 1000);
        let cutoff_date_score = waf_log_date_score(&cutoff_date);
        let mut touched_dates = BTreeSet::new();
        let mut pipe = redis::pipe();
        let mut operations = 0_usize;

        for event in events {
            let Some(trace_id) = event.get("trace_id").and_then(Value::as_str) else {
                continue;
            };
            if trace_id.trim().is_empty() {
                continue;
            }
            let score = waf_log_event_score(event);
            let date = crate::time_utils::local_date_from_ms(score);
            let action = event.get("action").and_then(Value::as_str).unwrap_or("log");
            let serialized = serde_json::to_string(event).unwrap_or_default();

            touched_dates.insert(date.clone());
            pipe.set_ex(waf_log_event_key(trace_id), serialized, ttl_seconds as u64)
                .ignore();
            pipe.zadd(waf_log_date_key(&date), trace_id, score).ignore();
            pipe.expire(waf_log_date_key(&date), ttl_seconds).ignore();
            pipe.cmd("HINCRBY")
                .arg(waf_log_stats_key(&date))
                .arg("events")
                .arg(1)
                .ignore();
            pipe.cmd("HINCRBY")
                .arg(waf_log_stats_key(&date))
                .arg(format!("action:{action}"))
                .arg(1)
                .ignore();
            pipe.expire(waf_log_stats_key(&date), ttl_seconds).ignore();
            operations += 6;
        }

        for date in touched_dates {
            pipe.zadd(WAF_LOG_DATES_INDEX_KEY, &date, waf_log_date_score(&date))
                .ignore();
            operations += 1;
        }
        pipe.cmd("ZREMRANGEBYSCORE")
            .arg(WAF_LOG_DATES_INDEX_KEY)
            .arg(0)
            .arg(format!("({cutoff_date_score}"))
            .ignore();
        operations += 1;

        if operations > 0 {
            let _: () = pipe.query_async(&mut self.conn()).await?;
        }
        Ok(())
    }

    pub async fn list_waf_log_dates(&self, today: &str) -> redis::RedisResult<Vec<String>> {
        let migrated = self
            .get_string_value(WAF_LOG_DATES_INDEX_MIGRATED_KEY)
            .await?;
        if migrated.is_none() {
            return self.scan_waf_log_dates_and_backfill_index(today).await;
        }

        let mut conn = self.conn();
        let indexed_dates: Vec<String> = conn.zrevrange(WAF_LOG_DATES_INDEX_KEY, 0, -1).await?;
        if indexed_dates.is_empty() {
            return Ok(vec![today.to_string()]);
        }

        let mut dates = BTreeSet::new();
        dates.insert(today.to_string());
        let mut stale_dates = Vec::new();
        for date in indexed_dates
            .into_iter()
            .filter(|date| is_waf_log_date(date))
        {
            let count: i64 = conn.zcard(waf_log_date_key(&date)).await?;
            if count > 0 {
                dates.insert(date);
            } else {
                stale_dates.push(date);
            }
        }
        if !stale_dates.is_empty() {
            let mut pipe = redis::pipe();
            pipe.zrem(WAF_LOG_DATES_INDEX_KEY, stale_dates).ignore();
            let _: () = pipe.query_async(&mut conn).await?;
        }

        Ok(descending_strings(dates))
    }

    async fn scan_waf_log_dates_and_backfill_index(
        &self,
        today: &str,
    ) -> redis::RedisResult<Vec<String>> {
        let mut conn = self.conn();
        let mut cursor = "0".to_string();
        let mut dates = BTreeSet::new();
        loop {
            let (next_cursor, batch): (String, Vec<String>) = redis::cmd("SCAN")
                .arg(&cursor)
                .arg("MATCH")
                .arg(format!("{WAF_LOG_DATE_PREFIX}*"))
                .arg("COUNT")
                .arg(100)
                .query_async(&mut conn)
                .await?;
            for key in batch {
                let date = key.strip_prefix(WAF_LOG_DATE_PREFIX).unwrap_or("");
                if is_waf_log_date(date) {
                    dates.insert(date.to_string());
                }
            }
            if next_cursor == "0" {
                break;
            }
            cursor = next_cursor;
        }

        let mut pipe = redis::pipe();
        for date in &dates {
            pipe.zadd(WAF_LOG_DATES_INDEX_KEY, date, waf_log_date_score(date))
                .ignore();
        }
        pipe.set(WAF_LOG_DATES_INDEX_MIGRATED_KEY, "1").ignore();
        let _: () = pipe.query_async(&mut conn).await?;
        dates.insert(today.to_string());
        Ok(descending_strings(dates))
    }

    pub async fn waf_log_date_total(&self, date: &str) -> redis::RedisResult<i64> {
        let mut conn = self.conn();
        conn.zcard(waf_log_date_key(date)).await
    }

    pub async fn waf_log_ids_desc(
        &self,
        date: &str,
        start: isize,
        end: isize,
    ) -> redis::RedisResult<Vec<String>> {
        let mut conn = self.conn();
        conn.zrevrange(waf_log_date_key(date), start, end).await
    }

    pub async fn waf_log_events_by_ids(
        &self,
        ids: &[String],
    ) -> redis::RedisResult<Vec<Option<Value>>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut conn = self.conn();
        let raws: Vec<Option<String>> = redis::cmd("MGET")
            .arg(
                ids.iter()
                    .map(|id| waf_log_event_key(id))
                    .collect::<Vec<_>>(),
            )
            .query_async(&mut conn)
            .await?;
        Ok(raws
            .into_iter()
            .map(|raw| raw.and_then(|value| serde_json::from_str::<Value>(&value).ok()))
            .collect())
    }

    pub async fn get_waf_log_event(&self, trace_id: &str) -> redis::RedisResult<Option<Value>> {
        self.get_json_value(&waf_log_event_key(trace_id)).await
    }

    pub async fn remove_waf_log_stale_ids(
        &self,
        date: &str,
        ids: &[String],
    ) -> redis::RedisResult<()> {
        let unique_ids = unique_non_empty_strings(ids);
        if unique_ids.is_empty() {
            return Ok(());
        }
        let mut conn = self.conn();
        for chunk in unique_ids.chunks(500) {
            let mut pipe = redis::pipe();
            pipe.zrem(waf_log_date_key(date), chunk).ignore();
            let _: () = pipe.query_async(&mut conn).await?;
        }
        Ok(())
    }

    pub async fn delete_waf_log_date(&self, date: &str) -> redis::RedisResult<bool> {
        let mut conn = self.conn();
        let mut deleted_count = 0usize;
        loop {
            let ids: Vec<String> = conn.zrange(waf_log_date_key(date), 0, 499).await?;
            if ids.is_empty() {
                break;
            }
            let mut pipe = redis::pipe();
            for id in &ids {
                pipe.del(waf_log_event_key(id)).ignore();
            }
            pipe.zrem(waf_log_date_key(date), ids.clone()).ignore();
            let _: () = pipe.query_async(&mut conn).await?;
            deleted_count += ids.len();
        }

        let mut pipe = redis::pipe();
        pipe.del(waf_log_date_key(date)).ignore();
        pipe.del(waf_log_stats_key(date)).ignore();
        pipe.zrem(WAF_LOG_DATES_INDEX_KEY, date).ignore();
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(deleted_count > 0)
    }
}
