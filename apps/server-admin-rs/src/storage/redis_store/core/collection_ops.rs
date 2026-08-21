use super::*;

impl Store {
    pub async fn hgetall_string_map(
        &self,
        key: &str,
    ) -> crate::storage::StorageResult<HashMap<String, String>> {
        let mut conn = self.conn();
        conn.hgetall(key).await
    }

    pub async fn replace_hash_string_map(
        &self,
        key: &str,
        values: &HashMap<String, String>,
    ) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        if values.is_empty() {
            conn.del(key).await
        } else {
            let mut pipe = redis::pipe();
            pipe.del(key).ignore();
            pipe.hset_multiple(key, &values.iter().collect::<Vec<_>>())
                .ignore();
            let _: () = pipe.query_async(&mut conn).await?;
            Ok(())
        }
    }

    pub async fn smembers_strings(&self, key: &str) -> crate::storage::StorageResult<Vec<String>> {
        let mut conn = self.conn();
        conn.smembers(key).await
    }

    pub async fn sadd_string_member(
        &self,
        key: &str,
        member: &str,
    ) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        conn.sadd(key, member).await
    }

    pub async fn srem_string_member(
        &self,
        key: &str,
        member: &str,
    ) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        conn.srem(key, member).await
    }

    pub async fn srem_string_members(
        &self,
        key: &str,
        members: &[String],
    ) -> crate::storage::StorageResult<()> {
        if members.is_empty() {
            return Ok(());
        }
        let mut conn = self.conn();
        conn.srem(key, members).await
    }

    pub async fn zrevrange_strings(&self, key: &str) -> crate::storage::StorageResult<Vec<String>> {
        let mut conn = self.conn();
        conn.zrevrange(key, 0, -1).await
    }

    pub async fn zadd_string_member(
        &self,
        key: &str,
        member: &str,
        score: i64,
    ) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        conn.zadd(key, member, score).await
    }

    pub async fn zrem_string_member(
        &self,
        key: &str,
        member: &str,
    ) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        conn.zrem(key, member).await
    }

    pub async fn zadd_trim_count_expire(
        &self,
        key: &str,
        member: &str,
        score: i64,
        min_score: i64,
        ttl_seconds: usize,
    ) -> crate::storage::StorageResult<i64> {
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        pipe.zadd(key, member, score).ignore();
        pipe.zrembyscore(key, 0, min_score - 1).ignore();
        pipe.expire(key, ttl_seconds.max(1) as i64).ignore();
        pipe.zcard(key);
        let values: Vec<i64> = pipe.query_async(&mut conn).await?;
        Ok(values.into_iter().next().unwrap_or_default())
    }

    #[cfg(test)]
    pub async fn trim_oldest_zset_members(
        &self,
        key: &str,
        max_records: i64,
    ) -> crate::storage::StorageResult<Vec<String>> {
        let max_records = max_records.max(1);
        let mut conn = self.conn();
        let count: i64 = conn.zcard(key).await?;
        let overflow = count.saturating_sub(max_records);
        if overflow == 0 {
            return Ok(Vec::new());
        }
        let members: Vec<String> = conn.zrange(key, 0, (overflow - 1) as isize).await?;
        if !members.is_empty() {
            conn.zrem(key, members.clone()).await?;
        }
        Ok(members)
    }

    pub async fn set_string_and_zadd(
        &self,
        data_key: &str,
        value: &str,
        index_key: &str,
        member: &str,
        score: i64,
    ) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        pipe.set(data_key, value)
            .ignore()
            .zadd(index_key, member, score)
            .ignore();
        pipe.query_async(&mut conn).await
    }

    pub async fn delete_string_and_zrem(
        &self,
        data_key: &str,
        index_key: &str,
        member: &str,
    ) -> crate::storage::StorageResult<()> {
        self.verify_subdomain_grant_shadow_key(data_key).await?;
        self.verify_subdomain_grant_shadow_key(index_key).await?;
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        pipe.del(data_key).ignore().zrem(index_key, member).ignore();
        pipe.query_async(&mut conn).await
    }

    pub async fn save_expiring_string_and_sadd(
        &self,
        data_key: &str,
        value: &str,
        ttl_seconds: usize,
        set_key: &str,
        member: &str,
    ) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        let ttl = ttl_seconds.max(1);
        let mut pipe = redis::pipe();
        pipe.set_ex(data_key, value, ttl as u64)
            .ignore()
            .sadd(set_key, member)
            .ignore()
            .expire(set_key, ttl as i64)
            .ignore();
        pipe.query_async(&mut conn).await
    }

    pub async fn delete_string_and_srem(
        &self,
        data_key: &str,
        set_key: &str,
        member: &str,
    ) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        pipe.del(data_key).ignore().srem(set_key, member).ignore();
        pipe.query_async(&mut conn).await
    }

    pub async fn delete_keys(&self, keys: &[String]) -> crate::storage::StorageResult<()> {
        if keys.is_empty() {
            return Ok(());
        }
        let mut conn = self.conn();
        conn.del(keys).await
    }

    pub async fn scan_keys(
        &self,
        prefix: &str,
        count: usize,
    ) -> crate::storage::StorageResult<Vec<String>> {
        let mut conn = self.conn();
        let mut cursor = "0".to_string();
        let mut keys = BTreeSet::new();
        loop {
            let (next_cursor, batch): (String, Vec<String>) = redis::cmd("SCAN")
                .arg(&cursor)
                .arg("MATCH")
                .arg(format!("{prefix}*"))
                .arg("COUNT")
                .arg(count.max(1))
                .query_async(&mut conn)
                .await?;
            keys.extend(batch);
            if next_cursor == "0" {
                break;
            }
            cursor = next_cursor;
        }
        let mut keys = keys.into_iter().collect::<Vec<_>>();
        keys.sort_by(|left, right| node_locale_compare_ordering(left, right));
        Ok(keys)
    }

    pub async fn clear_all_keys(&self) -> crate::storage::StorageResult<usize> {
        let mut conn = self.conn();
        let (cleared_keys, _): (usize, ()) = redis::pipe()
            .query_async_replacing_prefix(&mut conn, "")
            .await?;
        self.typed.typed_docker_admin.rebuild_from_legacy().await?;
        self.typed.typed_event_dedupe.rebuild_from_legacy().await?;
        self.rebuild_typed_system_events_from_legacy().await?;
        self.typed.typed_fnos_share.rebuild_from_legacy().await?;
        self.typed.typed_hmac_nonce.rebuild_from_legacy().await?;
        self.typed.typed_login_backoff.rebuild_from_legacy().await?;
        self.typed.typed_mobility.rebuild_from_legacy().await?;
        self.typed
            .typed_notification_runtime
            .rebuild_from_legacy()
            .await?;
        self.rebuild_typed_notification_documents_from_legacy()
            .await?;
        self.rebuild_typed_notification_history_from_legacy()
            .await?;
        self.typed
            .typed_passkey_runtime
            .rebuild_from_legacy()
            .await?;
        self.typed
            .typed_subdomain_grant
            .rebuild_from_legacy()
            .await?;
        self.typed
            .typed_identity_runtime
            .rebuild_from_legacy()
            .await?;
        self.typed
            .typed_subdomain_rate_limit
            .rebuild_from_legacy()
            .await?;
        self.rebuild_typed_whitelist_from_legacy().await?;
        self.typed
            .typed_whitelist_runtime
            .rebuild_from_legacy()
            .await?;
        self.typed.typed_wol_cooldown.rebuild_from_legacy().await?;
        self.refresh_config_snapshot().await?;
        Ok(cleared_keys)
    }

    pub async fn storage_meta_value(
        &self,
        key: &str,
    ) -> crate::storage::StorageResult<Option<String>> {
        self.manager.meta_value(key).await
    }

    pub async fn set_storage_meta_value(
        &self,
        key: &str,
        value: &str,
    ) -> crate::storage::StorageResult<()> {
        self.manager.set_meta_value(key, value).await
    }

    pub async fn count_keys_by_prefix(&self, prefix: &str) -> crate::storage::StorageResult<i64> {
        self.manager.key_count_by_prefix(prefix).await
    }

    pub async fn append_log_buffer(
        &self,
        key: &str,
        lines: &[String],
        ttl_seconds: usize,
        max_len: usize,
    ) -> crate::storage::StorageResult<()> {
        if lines.is_empty() {
            return Ok(());
        }
        let seq_key = format!("{key}:seq");
        let mut conn = self.conn();
        let ttl_seconds = ttl_seconds.max(1) as u64;
        let max_len = max_len.max(1);
        let current_len = conn.llen(key).await?.max(0);
        let raw_seq: Option<String> = conn.get(&seq_key).await?;
        let current_seq = raw_seq
            .as_deref()
            .and_then(|value| value.parse::<i64>().ok())
            .filter(|value| *value >= 0);
        let repaired_seq = current_seq.unwrap_or(current_len).max(current_len);
        if current_seq != Some(repaired_seq) {
            conn.set_ex(&seq_key, repaired_seq, ttl_seconds).await?;
        }
        let mut pipe = redis::pipe();
        pipe.cmd("RPUSH")
            .arg(key)
            .arg(lines)
            .ignore()
            .cmd("LTRIM")
            .arg(key)
            .arg(-(max_len as i64))
            .arg(-1)
            .ignore()
            .cmd("INCRBY")
            .arg(&seq_key)
            .arg(lines.len() as i64)
            .ignore()
            .cmd("EXPIRE")
            .arg(key)
            .arg(ttl_seconds)
            .ignore()
            .cmd("EXPIRE")
            .arg(&seq_key)
            .arg(ttl_seconds)
            .ignore();
        pipe.query_async(&mut conn).await
    }

    pub async fn list_log_buffer(
        &self,
        key: &str,
        limit: usize,
        max_len: usize,
    ) -> crate::storage::StorageResult<Vec<String>> {
        let mut conn = self.conn();
        let safe_limit = limit.max(1).min(max_len.max(1)) as i64;
        conn.lrange(key, -(safe_limit as isize), -1).await
    }

    pub async fn clear_log_buffer(&self, key: &str) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        let seq_key = format!("{key}:seq");
        conn.del(&[key, seq_key.as_str()]).await
    }

    pub async fn poll_log_buffer(
        &self,
        key: &str,
        cursor: Option<&str>,
    ) -> crate::storage::StorageResult<Value> {
        let mut conn = self.conn();
        let seq_key = format!("{key}:seq");
        let total_len: i64 = conn.llen(key).await?;
        let raw_seq: Option<String> = conn.get(&seq_key).await?;
        let total_seq = raw_seq
            .and_then(|value| value.parse::<i64>().ok())
            .filter(|value| *value >= 0)
            .unwrap_or(total_len)
            .max(total_len);
        let retained_start_seq = (total_seq - total_len).max(0);
        let requested_cursor = cursor
            .and_then(|value| value.parse::<i64>().ok())
            .filter(|value| *value >= 0);
        let reset =
            requested_cursor.is_some_and(|value| value < retained_start_seq || value > total_seq);
        let from = if requested_cursor.is_none() || reset {
            0
        } else {
            (requested_cursor.unwrap_or(0) - retained_start_seq).max(0)
        };
        let items: Vec<String> = if total_len > 0 && from < total_len {
            conn.lrange(key, from as isize, -1).await?
        } else {
            Vec::new()
        };
        Ok(json!({
            "cursor": total_seq,
            "reset": reset,
            "items": items
        }))
    }
}
