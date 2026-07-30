use super::*;

impl Store {
    pub async fn scanner_settings_raw(&self) -> crate::storage::StorageResult<Option<Value>> {
        self.get_json_value(SCANNER_SETTINGS_KEY).await
    }

    pub async fn save_scanner_settings(&self, value: &Value) -> crate::storage::StorageResult<()> {
        self.set_json_value(SCANNER_SETTINGS_KEY, value).await
    }

    pub async fn list_scanner_blacklist(
        &self,
        page: i64,
        limit: i64,
        search: &str,
    ) -> crate::storage::StorageResult<Value> {
        let safe_page = page.max(1);
        let safe_limit = limit.clamp(1, 200);
        let start = (safe_page - 1) * safe_limit;
        let end = start + safe_limit - 1;
        let search = search.trim();
        let total;
        let mut ips = Vec::<String>::new();

        if search.is_empty() {
            let mut conn = self.conn();
            total = conn.zcard(SCANNER_BLACKLIST_INDEX_KEY).await?;
            if total > 0 {
                ips = conn
                    .zrevrange(SCANNER_BLACKLIST_INDEX_KEY, start as isize, end as isize)
                    .await?;
            }
        } else {
            let chunk_size = 200_i64.max(safe_limit * 5);
            let mut matched_count = 0_i64;
            let mut offset = 0_i64;

            loop {
                let mut conn = self.conn();
                let chunk: Vec<String> = conn
                    .zrevrange(
                        SCANNER_BLACKLIST_INDEX_KEY,
                        offset as isize,
                        (offset + chunk_size - 1) as isize,
                    )
                    .await?;
                if chunk.is_empty() {
                    break;
                }
                offset += chunk.len() as i64;

                for ip in chunk {
                    if !ip.contains(search) {
                        continue;
                    }
                    if matched_count >= start && ips.len() < safe_limit as usize {
                        ips.push(ip);
                    }
                    matched_count += 1;
                }
            }

            total = matched_count;
        }

        let items = self.scanner_blacklist_records_by_ips(&ips).await?;
        Ok(json!({ "total": total, "items": items }))
    }

    pub async fn get_scanner_blacklist_record(
        &self,
        ip: &str,
    ) -> crate::storage::StorageResult<Option<Value>> {
        let mut conn = self.conn();
        let raw: Option<String> = conn.get(scanner_blacklist_data_key(ip)).await?;
        Ok(raw.and_then(|value| scanner_blacklist_record_from_raw(ip, &value)))
    }

    pub async fn scanner_blacklist_exists(&self, ip: &str) -> crate::storage::StorageResult<bool> {
        let mut conn = self.conn();
        let exists: i64 = conn.exists(scanner_blacklist_data_key(ip)).await?;
        Ok(exists == 1)
    }

    pub async fn record_scanner_suspicious_hit(
        &self,
        ip: &str,
        hit: &Value,
        now_ms: i64,
        min_score_ms: i64,
        window_min_score_ms: i64,
        ttl_seconds: i64,
    ) -> crate::storage::StorageResult<i64> {
        let key = scanner_suspicious_key(ip);
        let serialized = serde_json::to_string(hit).unwrap_or_else(|_| "{}".to_string());
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        pipe.zadd(&key, serialized, now_ms).ignore();
        pipe.zrembyscore(&key, 0, min_score_ms).ignore();
        pipe.expire(&key, ttl_seconds.max(1)).ignore();
        let _: () = pipe.query_async(&mut conn).await?;
        redis::cmd("ZCOUNT")
            .arg(&key)
            .arg(window_min_score_ms)
            .arg("+inf")
            .query_async(&mut conn)
            .await
    }

    pub async fn scanner_suspicious_hits_since(
        &self,
        ip: &str,
        min_score_ms: i64,
    ) -> crate::storage::StorageResult<Vec<Value>> {
        let key = scanner_suspicious_key(ip);
        let mut conn = self.conn();
        let raws: Vec<String> = redis::cmd("ZRANGEBYSCORE")
            .arg(&key)
            .arg(min_score_ms)
            .arg("+inf")
            .query_async(&mut conn)
            .await?;
        Ok(raws
            .into_iter()
            .filter_map(|raw| serde_json::from_str::<Value>(&raw).ok())
            .collect())
    }

    pub async fn add_scanner_blacklist_record(
        &self,
        ip: &str,
        record: &Value,
        blocked_at_ms: i64,
        ttl_seconds: i64,
    ) -> crate::storage::StorageResult<()> {
        let ttl_seconds = ttl_seconds.max(1);
        let index_min_score = blocked_at_ms - ttl_seconds * 1000;
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        pipe.set_ex(
            scanner_blacklist_data_key(ip),
            serde_json::to_string(record).unwrap_or_else(|_| "{}".to_string()),
            ttl_seconds as u64,
        )
        .ignore();
        pipe.zadd(SCANNER_BLACKLIST_INDEX_KEY, ip, blocked_at_ms)
            .ignore();
        pipe.zrembyscore(SCANNER_BLACKLIST_INDEX_KEY, 0, index_min_score)
            .ignore();
        let _: () = pipe.query_async(&mut conn).await?;

        let current_ttl: i64 = conn.ttl(SCANNER_BLACKLIST_INDEX_KEY).await?;
        if current_ttl == -2 || current_ttl == -1 || current_ttl < ttl_seconds {
            let _: () = conn
                .expire(SCANNER_BLACKLIST_INDEX_KEY, ttl_seconds)
                .await?;
        }
        Ok(())
    }

    pub async fn remove_scanner_blacklist(
        &self,
        ips: &[String],
    ) -> crate::storage::StorageResult<()> {
        let clean_ips = sanitize_scanner_ips(ips);
        if clean_ips.is_empty() {
            return Ok(());
        }

        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        for ip in &clean_ips {
            pipe.del(scanner_blacklist_data_key(ip)).ignore();
            pipe.del(scanner_suspicious_key(ip)).ignore();
        }
        pipe.zrem(SCANNER_BLACKLIST_INDEX_KEY, clean_ips).ignore();
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(())
    }

    async fn scanner_blacklist_records_by_ips(
        &self,
        ips: &[String],
    ) -> crate::storage::StorageResult<Vec<Value>> {
        if ips.is_empty() {
            return Ok(Vec::new());
        }

        let keys = ips
            .iter()
            .map(|ip| scanner_blacklist_data_key(ip))
            .collect::<Vec<_>>();
        let mut conn = self.conn();
        let raws: Vec<Option<String>> = redis::cmd("MGET").arg(keys).query_async(&mut conn).await?;
        let mut records = Vec::new();
        let mut stale_ips = Vec::new();

        for (ip, raw) in ips.iter().zip(raws) {
            let Some(raw) = raw else {
                stale_ips.push(ip.clone());
                continue;
            };
            match scanner_blacklist_record_from_raw(ip, &raw) {
                Some(record) => records.push(record),
                None => stale_ips.push(ip.clone()),
            }
        }

        if !stale_ips.is_empty() {
            let mut pipe = redis::pipe();
            pipe.zrem(SCANNER_BLACKLIST_INDEX_KEY, stale_ips).ignore();
            let _: () = pipe.query_async(&mut conn).await?;
        }

        Ok(records)
    }

    pub async fn get_ip_location_cache(
        &self,
        ip: &str,
    ) -> crate::storage::StorageResult<Option<Value>> {
        self.get_json_value(&ip_location_cache_key(ip)).await
    }

    pub async fn get_ip_location_state(
        &self,
        ip: &str,
    ) -> crate::storage::StorageResult<Option<Value>> {
        self.get_json_value(&ip_location_state_key(ip)).await
    }

    pub async fn set_ip_location_state(
        &self,
        ip: &str,
        state: &Value,
        ttl_seconds: usize,
    ) -> crate::storage::StorageResult<()> {
        self.set_json_value_ex(&ip_location_state_key(ip), state, ttl_seconds)
            .await
    }

    pub async fn enqueue_ip_location(
        &self,
        ip: &str,
        state: &Value,
        next_attempt_at_ms: i64,
        ttl_seconds: usize,
    ) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        pipe.set_ex(
            ip_location_state_key(ip),
            serde_json::to_string(state).unwrap_or_else(|_| "{}".to_string()),
            ttl_seconds.max(1) as u64,
        )
        .ignore();
        pipe.zadd(IP_LOCATION_QUEUE_KEY, ip, next_attempt_at_ms)
            .ignore();
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(())
    }

    pub async fn due_ip_location_ips(
        &self,
        now_ms: i64,
        limit: usize,
    ) -> crate::storage::StorageResult<Vec<String>> {
        let mut conn = self.conn();
        redis::cmd("ZRANGEBYSCORE")
            .arg(IP_LOCATION_QUEUE_KEY)
            .arg(0)
            .arg(now_ms)
            .arg("LIMIT")
            .arg(0)
            .arg(limit.max(1))
            .query_async(&mut conn)
            .await
    }

    pub async fn acquire_ip_location_lock(
        &self,
        ip: &str,
        now_ms: i64,
        ttl_seconds: usize,
    ) -> crate::storage::StorageResult<bool> {
        let mut conn = self.conn();
        let result: Option<String> = redis::cmd("SET")
            .arg(ip_location_lock_key(ip))
            .arg(now_ms)
            .arg("EX")
            .arg(ttl_seconds.max(1))
            .arg("NX")
            .query_async(&mut conn)
            .await?;
        Ok(result.as_deref() == Some("OK"))
    }

    pub async fn release_ip_location_lock(&self, ip: &str) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        conn.del(ip_location_lock_key(ip)).await
    }

    pub async fn remove_ip_location_queue_entry(
        &self,
        ip: &str,
    ) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        conn.zrem(IP_LOCATION_QUEUE_KEY, ip).await
    }

    pub async fn complete_ip_location_lookup(
        &self,
        ip: &str,
        result: &Value,
        state: &Value,
        ttl_seconds: usize,
    ) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        pipe.set_ex(
            ip_location_cache_key(ip),
            serde_json::to_string(result).unwrap_or_else(|_| "{}".to_string()),
            ttl_seconds.max(1) as u64,
        )
        .ignore();
        pipe.set_ex(
            ip_location_state_key(ip),
            serde_json::to_string(state).unwrap_or_else(|_| "{}".to_string()),
            ttl_seconds.max(1) as u64,
        )
        .ignore();
        pipe.zrem(IP_LOCATION_QUEUE_KEY, ip).ignore();
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(())
    }

    pub async fn ip_location_references(
        &self,
        ip: &str,
    ) -> crate::storage::StorageResult<Vec<String>> {
        let mut conn = self.conn();
        conn.smembers(ip_location_refs_key(ip)).await
    }

    pub async fn add_ip_location_references(
        &self,
        ip: &str,
        refs: &[String],
        ttl_seconds: usize,
    ) -> crate::storage::StorageResult<()> {
        if refs.is_empty() {
            return Ok(());
        }
        let key = ip_location_refs_key(ip);
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        pipe.sadd(&key, refs).ignore();
        pipe.ttl(&key);
        let values: Vec<i64> = pipe.query_async(&mut conn).await?;
        let ttl = values.into_iter().next().unwrap_or_default();
        if ttl == -1 || ttl > ttl_seconds as i64 {
            let _: () = conn.expire(key, ttl_seconds.max(1) as i64).await?;
        }
        Ok(())
    }

    pub async fn remove_ip_location_references(
        &self,
        ip: &str,
        refs: &[String],
    ) -> crate::storage::StorageResult<()> {
        if refs.is_empty() {
            return Ok(());
        }
        let mut conn = self.conn();
        conn.srem(ip_location_refs_key(ip), refs).await
    }

    pub async fn record_recent_auth_ip(
        &self,
        ip: &str,
        now: i64,
    ) -> crate::storage::StorageResult<()> {
        let expire_at = now + RECENT_AUTH_IPS_TTL_SECONDS;
        let mut conn = self.conn();
        let expired_ips: Vec<String> = redis::cmd("ZRANGEBYSCORE")
            .arg(RECENT_AUTH_IPS_ZSET_KEY)
            .arg(0)
            .arg(now)
            .query_async(&mut conn)
            .await?;
        let raw_detail: Option<String> = conn.hget(RECENT_AUTH_IPS_DETAILS_KEY, ip).await?;
        let detail = raw_detail
            .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
            .unwrap_or_else(|| json!({}));
        let first_seen_at = detail
            .get("firstSeenAt")
            .and_then(Value::as_i64)
            .unwrap_or(now);
        let seen_count = detail
            .get("seenCount")
            .and_then(Value::as_i64)
            .unwrap_or_default()
            .max(0)
            + 1;
        let next_detail = json!({
            "firstSeenAt": first_seen_at,
            "lastSeenAt": now,
            "seenCount": seen_count.max(1),
        });
        let mut pipe = redis::pipe();
        pipe.zadd(RECENT_AUTH_IPS_ZSET_KEY, ip, expire_at).ignore();
        pipe.zrembyscore(RECENT_AUTH_IPS_ZSET_KEY, 0, now).ignore();
        pipe.hset(
            RECENT_AUTH_IPS_DETAILS_KEY,
            ip,
            serde_json::to_string(&next_detail).unwrap_or_else(|_| "{}".to_string()),
        )
        .ignore();
        let expired_ips = expired_ips
            .into_iter()
            .filter(|expired_ip| expired_ip != ip)
            .collect::<Vec<_>>();
        if !expired_ips.is_empty() {
            pipe.hdel(RECENT_AUTH_IPS_DETAILS_KEY, expired_ips).ignore();
        }
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(())
    }

    pub async fn is_recent_auth_ip_active(
        &self,
        ip: &str,
        now: i64,
    ) -> crate::storage::StorageResult<bool> {
        let mut conn = self.conn();
        let score: Option<i64> = conn.zscore(RECENT_AUTH_IPS_ZSET_KEY, ip).await.ok();
        Ok(score.is_some_and(|expires_at| expires_at > now))
    }

    pub async fn list_recent_auth_ips_with_scores(
        &self,
        now: i64,
        limit: usize,
    ) -> crate::storage::StorageResult<Vec<Value>> {
        let mut conn = self.conn();
        let raw: Vec<String> = redis::cmd("ZREVRANGEBYSCORE")
            .arg(RECENT_AUTH_IPS_ZSET_KEY)
            .arg("+inf")
            .arg(now + 1)
            .arg("WITHSCORES")
            .arg("LIMIT")
            .arg(0)
            .arg(limit.max(1))
            .query_async(&mut conn)
            .await?;
        let mut entries = Vec::new();
        let mut seen = BTreeSet::new();
        for pair in raw.chunks(2) {
            let Some(ip) = pair.first().map(String::as_str) else {
                continue;
            };
            let expires_at = pair
                .get(1)
                .and_then(|value| value.parse::<i64>().ok())
                .unwrap_or_default();
            if ip.trim().is_empty() || expires_at <= now || !seen.insert(ip.to_string()) {
                continue;
            }
            entries.push((ip.to_string(), expires_at));
        }
        if entries.is_empty() {
            return Ok(Vec::new());
        }
        let detail_values: Vec<Option<String>> = redis::cmd("HMGET")
            .arg(RECENT_AUTH_IPS_DETAILS_KEY)
            .arg(entries.iter().map(|(ip, _)| ip).collect::<Vec<_>>())
            .query_async(&mut conn)
            .await?;
        Ok(entries
            .into_iter()
            .zip(detail_values)
            .map(|((ip, expires_at), raw_detail)| {
                let detail = raw_detail
                    .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
                    .unwrap_or_else(|| json!({}));
                let fallback_last_seen_at = (expires_at - RECENT_AUTH_IPS_TTL_SECONDS).max(0);
                let last_seen_at = detail
                    .get("lastSeenAt")
                    .and_then(Value::as_i64)
                    .unwrap_or(fallback_last_seen_at);
                json!({
                    "ip": ip,
                    "expiresAt": expires_at,
                    "lastSeenAt": last_seen_at,
                    "firstSeenAt": detail
                        .get("firstSeenAt")
                        .and_then(Value::as_i64)
                        .unwrap_or(last_seen_at),
                    "seenCount": detail
                        .get("seenCount")
                        .and_then(Value::as_i64)
                        .unwrap_or(1)
                        .max(1),
                })
            })
            .collect())
    }

    pub async fn get_json_value_with_ttl(
        &self,
        key: &str,
    ) -> crate::storage::StorageResult<(Option<Value>, i64)> {
        let mut conn = self.conn();
        let raw: Option<String> = conn.get(key).await?;
        let ttl: i64 = conn.ttl(key).await?;
        Ok((raw.and_then(|value| serde_json::from_str(&value).ok()), ttl))
    }

    pub async fn set_json_value_preserve_ttl(
        &self,
        key: &str,
        value: &Value,
        ttl: i64,
    ) -> crate::storage::StorageResult<()> {
        let serialized = serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string());
        let mut conn = self.conn();
        if ttl > 0 {
            let _: () = conn.set_ex(key, serialized, ttl as u64).await?;
        } else if ttl == -1 {
            let _: () = conn.set(key, serialized).await?;
        } else {
            let _: () = conn.del(key).await?;
        }
        Ok(())
    }

    pub async fn hget_json_value(
        &self,
        key: &str,
        field: &str,
    ) -> crate::storage::StorageResult<Option<Value>> {
        let mut conn = self.conn();
        let raw: Option<String> = conn.hget(key, field).await?;
        Ok(raw.and_then(|value| serde_json::from_str(&value).ok()))
    }

    pub async fn hset_json_value(
        &self,
        key: &str,
        field: &str,
        value: &Value,
    ) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        conn.hset(
            key,
            field,
            serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string()),
        )
        .await
    }
}
