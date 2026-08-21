use super::*;

pub(super) const WAF_LOG_DATE_PREFIX: &str = "fn_knock:waf:logs:";
pub(super) const WAF_LOG_EVENT_PREFIX: &str = "fn_knock:waf:log:";
pub(super) const WAF_LOG_STATS_PREFIX: &str = "fn_knock:waf:stats:";
pub(super) const WAF_LOG_DATES_INDEX_KEY: &str = "fn_knock:waf:logs:dates";
pub(super) const WAF_LOG_DATES_INDEX_MIGRATED_KEY: &str = "fn_knock:waf:logs:dates:migrated";

pub(super) fn waf_log_date_key(date: &str) -> String {
    format!("{WAF_LOG_DATE_PREFIX}{date}")
}

pub(super) fn waf_log_event_key(trace_id: &str) -> String {
    format!("{WAF_LOG_EVENT_PREFIX}{trace_id}")
}

pub(super) fn waf_log_stats_key(date: &str) -> String {
    format!("{WAF_LOG_STATS_PREFIX}{date}")
}

pub(super) fn waf_log_event_score(event: &Value) -> i64 {
    event
        .get("time")
        .and_then(Value::as_str)
        .and_then(crate::time_utils::parse_iso_ms)
        .unwrap_or_else(crate::time_utils::now_ms)
}

pub(super) fn is_waf_log_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
}

pub(super) fn descending_strings(values: BTreeSet<String>) -> Vec<String> {
    values.into_iter().rev().collect()
}

pub(super) fn waf_log_date_score(date: &str) -> i64 {
    let mut parts = date.split('-');
    let year = parts.next().and_then(|value| value.parse::<i32>().ok());
    let month = parts.next().and_then(|value| value.parse::<u8>().ok());
    let day = parts.next().and_then(|value| value.parse::<u8>().ok());
    let (Some(year), Some(month), Some(day)) = (year, month, day) else {
        return 0;
    };
    let Ok(month) = time::Month::try_from(month) else {
        return 0;
    };
    let Ok(date) = time::Date::from_calendar_date(year, month, day) else {
        return 0;
    };
    date.with_time(time::Time::MIDNIGHT)
        .assume_utc()
        .unix_timestamp()
        * 1000
}

pub(super) fn waf_log_dates_for_range(from_ms: i64, to_ms: i64) -> Vec<String> {
    const DAY_MS: i64 = 86_400_000;
    let start_day = (from_ms.max(0).div_euclid(DAY_MS) - 1).max(0);
    let end_day = to_ms.max(from_ms).div_euclid(DAY_MS) + 1;
    let mut dates = BTreeSet::new();
    for day in start_day..=end_day {
        let timestamp = day.saturating_mul(DAY_MS).div_euclid(1000);
        if let Ok(date_time) = time::OffsetDateTime::from_unix_timestamp(timestamp) {
            let date = date_time.date();
            dates.insert(format!(
                "{:04}-{:02}-{:02}",
                date.year(),
                u8::from(date.month()),
                date.day()
            ));
        }
    }
    dates.into_iter().collect()
}

use tokio_rusqlite::rusqlite::{TransactionBehavior, params};

struct PersistableWafEvent {
    trace_id: String,
    event_key: String,
    date: String,
    score: i64,
    action: String,
    serialized: String,
}

impl Store {
    pub async fn count_waf_logs_for_buckets(
        &self,
        bucket_starts: &[i64],
        to_ms: i64,
    ) -> crate::storage::StorageResult<(i64, Vec<i64>)> {
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
    ) -> crate::storage::StorageResult<()> {
        if events.is_empty() {
            return Ok(());
        }
        let ttl_seconds = retention_days.clamp(1, 365) * 86_400;
        let cutoff_date =
            crate::time_utils::local_date_from_ms(crate::time_utils::now_ms() - ttl_seconds * 1000);
        let cutoff_date_score = waf_log_date_score(&cutoff_date);
        let prepared = events
            .iter()
            .filter_map(|event| {
                let trace_id = event.get("trace_id").and_then(Value::as_str)?.trim();
                if trace_id.is_empty() {
                    return None;
                }
                let score = waf_log_event_score(event);
                Some(PersistableWafEvent {
                    trace_id: trace_id.to_string(),
                    event_key: waf_log_event_key(trace_id),
                    date: crate::time_utils::local_date_from_ms(score),
                    score,
                    action: event
                        .get("action")
                        .and_then(Value::as_str)
                        .unwrap_or("log")
                        .to_string(),
                    serialized: serde_json::to_string(event).unwrap_or_default(),
                })
            })
            .collect::<Vec<_>>();
        if prepared.is_empty() {
            return Ok(());
        }

        self.conn()
            .call(move |connection| {
                let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
                let now_ms = crate::time_utils::now_ms();
                let mut touched_dates = BTreeSet::new();

                for event in prepared {
                    let already_persisted: bool = tx.query_row(
                        "SELECT EXISTS(SELECT 1 FROM kv_keys WHERE key = ?1 AND kind = 'string' AND (expires_at_ms IS NULL OR expires_at_ms > ?2))",
                        params![event.event_key, now_ms],
                        |row| row.get(0),
                    )?;
                    redis::execute_command_in_transaction(
                        &tx,
                        "SETEX",
                        vec![
                            event.event_key,
                            ttl_seconds.to_string(),
                            event.serialized,
                        ],
                    )?;
                    redis::execute_command_in_transaction(
                        &tx,
                        "ZADD",
                        vec![
                            waf_log_date_key(&event.date),
                            event.score.to_string(),
                            event.trace_id,
                        ],
                    )?;
                    redis::execute_command_in_transaction(
                        &tx,
                        "EXPIRE",
                        vec![waf_log_date_key(&event.date), ttl_seconds.to_string()],
                    )?;
                    if !already_persisted {
                        redis::execute_command_in_transaction(
                            &tx,
                            "HINCRBY",
                            vec![waf_log_stats_key(&event.date), "events".to_string(), "1".to_string()],
                        )?;
                        redis::execute_command_in_transaction(
                            &tx,
                            "HINCRBY",
                            vec![
                                waf_log_stats_key(&event.date),
                                format!("action:{}", event.action),
                                "1".to_string(),
                            ],
                        )?;
                    }
                    redis::execute_command_in_transaction(
                        &tx,
                        "EXPIRE",
                        vec![waf_log_stats_key(&event.date), ttl_seconds.to_string()],
                    )?;
                    touched_dates.insert(event.date);
                }

                for date in touched_dates {
                    redis::execute_command_in_transaction(
                        &tx,
                        "ZADD",
                        vec![
                            WAF_LOG_DATES_INDEX_KEY.to_string(),
                            waf_log_date_score(&date).to_string(),
                            date,
                        ],
                    )?;
                }
                redis::execute_command_in_transaction(
                    &tx,
                    "ZREMRANGEBYSCORE",
                    vec![
                        WAF_LOG_DATES_INDEX_KEY.to_string(),
                        "0".to_string(),
                        format!("({cutoff_date_score}"),
                    ],
                )?;
                tx.commit()?;
                Ok(())
            })
            .await
    }

    pub async fn list_waf_log_dates(
        &self,
        today: &str,
    ) -> crate::storage::StorageResult<Vec<String>> {
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
    ) -> crate::storage::StorageResult<Vec<String>> {
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

    pub async fn waf_log_date_total(&self, date: &str) -> crate::storage::StorageResult<i64> {
        let mut conn = self.conn();
        conn.zcard(waf_log_date_key(date)).await
    }

    pub async fn waf_log_ids_desc(
        &self,
        date: &str,
        start: isize,
        end: isize,
    ) -> crate::storage::StorageResult<Vec<String>> {
        let mut conn = self.conn();
        conn.zrevrange(waf_log_date_key(date), start, end).await
    }

    pub async fn waf_log_events_by_ids(
        &self,
        ids: &[String],
    ) -> crate::storage::StorageResult<Vec<Option<Value>>> {
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

    pub async fn get_waf_log_event(
        &self,
        trace_id: &str,
    ) -> crate::storage::StorageResult<Option<Value>> {
        self.get_json_value(&waf_log_event_key(trace_id)).await
    }

    pub async fn remove_waf_log_stale_ids(
        &self,
        date: &str,
        ids: &[String],
    ) -> crate::storage::StorageResult<()> {
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

    pub async fn delete_waf_log_date(&self, date: &str) -> crate::storage::StorageResult<bool> {
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
