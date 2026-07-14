use super::*;

struct ConfigFenceSnapshot {
    config_raw: Option<String>,
    generation_raw: Option<String>,
    config: Value,
    generation: u64,
}

struct ConfigGenerationMarker {
    generation: u64,
    host_fingerprint: String,
}

async fn load_config_fence_snapshot(
    conn: &mut ConnectionManager,
) -> crate::storage::StorageResult<ConfigFenceSnapshot> {
    let values: Vec<Option<String>> = redis::cmd("MGET")
        .arg(vec![
            CONFIG_KEY.to_string(),
            HOST_MAPPINGS_GENERATION_KEY.to_string(),
        ])
        .query_async(conn)
        .await?;
    let config_raw = values.first().cloned().flatten();
    let generation_raw = values.get(1).cloned().flatten();
    let config = match config_raw.as_deref() {
        Some(raw) => serde_json::from_str(raw)?,
        None => default_config(),
    };
    let generation = generation_raw
        .as_deref()
        .unwrap_or("0")
        .parse::<u64>()
        .map_err(|_| crate::storage::storage_error("host mappings generation is invalid"))?;
    Ok(ConfigFenceSnapshot {
        config_raw,
        generation_raw,
        config,
        generation,
    })
}

async fn compare_and_set_config_fence_snapshot(
    conn: &mut ConnectionManager,
    snapshot: &ConfigFenceSnapshot,
    replacement_raw: &str,
    replacement_generation: u64,
) -> crate::storage::StorageResult<bool> {
    let applied: i64 = redis::cmd("EVAL")
        .arg(
            r#"
-- fn-knock:eval:cas-config-host-generation-raw:v1
local current_config = redis.call("GET", KEYS[1])
local current_generation = redis.call("GET", KEYS[2])
local function raw_matches(current, expected_exists, expected)
  if expected_exists == "0" then
    return not current
  end
  return current and current == expected
end
if not raw_matches(current_config, ARGV[1], ARGV[2])
    or not raw_matches(current_generation, ARGV[3], ARGV[4]) then
  return 0
end
redis.call("SET", KEYS[1], ARGV[5])
redis.call("SET", KEYS[2], ARGV[6])
return 1
"#,
        )
        .arg(2)
        .arg(CONFIG_KEY)
        .arg(HOST_MAPPINGS_GENERATION_KEY)
        .arg(if snapshot.config_raw.is_some() {
            "1"
        } else {
            "0"
        })
        .arg(snapshot.config_raw.as_deref().unwrap_or(""))
        .arg(if snapshot.generation_raw.is_some() {
            "1"
        } else {
            "0"
        })
        .arg(snapshot.generation_raw.as_deref().unwrap_or(""))
        .arg(replacement_raw)
        .arg(replacement_generation.to_string())
        .query_async(conn)
        .await?;
    Ok(applied == 1)
}

fn config_host_mappings(config: &Value) -> Value {
    config
        .get("host_mappings")
        .cloned()
        .unwrap_or_else(|| Value::Array(Vec::new()))
}

fn config_host_mappings_fingerprint(config: &Value) -> crate::storage::StorageResult<String> {
    Ok(crate::crypto_utils::sha256_hex_bytes(serde_json::to_vec(
        &config_host_mappings(config),
    )?))
}

fn take_config_generation_marker(
    config: &mut Value,
) -> crate::storage::StorageResult<Option<ConfigGenerationMarker>> {
    let Some(object) = config.as_object_mut() else {
        return Ok(None);
    };
    let Some(marker) = object.remove(CONFIG_GENERATION_MARKER) else {
        return Ok(None);
    };
    let generation = marker
        .get("generation")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            crate::storage::storage_error("host mappings generation marker is invalid")
        })?;
    let host_fingerprint = marker
        .get("host_fingerprint")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            crate::storage::storage_error("host mappings generation fingerprint is invalid")
        })?
        .to_string();
    Ok(Some(ConfigGenerationMarker {
        generation,
        host_fingerprint,
    }))
}

fn inject_config_generation_marker(
    config: &mut Value,
    generation: u64,
) -> crate::storage::StorageResult<()> {
    let host_fingerprint = config_host_mappings_fingerprint(config)?;
    if let Some(object) = config.as_object_mut() {
        object.insert(
            CONFIG_GENERATION_MARKER.to_string(),
            json!({
                "generation": generation,
                "host_fingerprint": host_fingerprint,
            }),
        );
    }
    Ok(())
}

impl Store {
    pub async fn ping(&self) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        redis::cmd("PING").query_async(&mut conn).await
    }

    pub async fn get_json_value(&self, key: &str) -> crate::storage::StorageResult<Option<Value>> {
        let mut conn = self.conn();
        let raw: Option<String> = conn.get(key).await?;
        Ok(raw.and_then(|value| serde_json::from_str(&value).ok()))
    }

    pub async fn get_string_value(
        &self,
        key: &str,
    ) -> crate::storage::StorageResult<Option<String>> {
        let mut conn = self.conn();
        conn.get(key).await
    }

    pub async fn set_string_value_with_optional_ttl(
        &self,
        key: &str,
        value: &str,
        ttl_seconds: Option<i64>,
    ) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        if let Some(ttl_seconds) = ttl_seconds.filter(|value| *value > 0) {
            let _: () = conn.set_ex(key, value, ttl_seconds as u64).await?;
        } else {
            let _: () = conn.set(key, value).await?;
        }
        Ok(())
    }

    pub async fn set_string_value(
        &self,
        key: &str,
        value: &str,
    ) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        conn.set(key, value).await
    }

    pub async fn set_key_if_not_exists_with_ttl(
        &self,
        key: &str,
        value: &str,
        ttl_seconds: usize,
    ) -> crate::storage::StorageResult<bool> {
        let mut conn = self.conn();
        let result: Option<String> = redis::cmd("SET")
            .arg(key)
            .arg(value)
            .arg("EX")
            .arg(ttl_seconds.max(1))
            .arg("NX")
            .query_async(&mut conn)
            .await?;
        Ok(result.as_deref() == Some("OK"))
    }

    pub async fn delete_key_if_value(
        &self,
        key: &str,
        value: &str,
    ) -> crate::storage::StorageResult<()> {
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
            .arg(value)
            .query_async(&mut conn)
            .await?;
        Ok(())
    }

    pub async fn delete_key(&self, key: &str) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        conn.del(key).await
    }

    pub async fn delete_key_count(&self, key: &str) -> crate::storage::StorageResult<usize> {
        let mut conn = self.conn();
        conn.del(key).await
    }

    pub async fn mget_string_values(
        &self,
        keys: &[String],
    ) -> crate::storage::StorageResult<Vec<Option<String>>> {
        if keys.is_empty() {
            return Ok(Vec::new());
        }
        let mut conn = self.conn();
        redis::cmd("MGET").arg(keys).query_async(&mut conn).await
    }

    pub async fn consume_json_value(
        &self,
        key: &str,
    ) -> crate::storage::StorageResult<Option<Value>> {
        let mut conn = self.conn();
        let raw: Option<String> = redis::cmd("EVAL")
            .arg(
                r#"
-- fn-knock:eval:consume-value:v1
local value = redis.call("GET", KEYS[1])
if not value then
  return nil
end
redis.call("DEL", KEYS[1])
return value
"#,
            )
            .arg(1)
            .arg(key)
            .query_async(&mut conn)
            .await?;
        Ok(raw.and_then(|value| serde_json::from_str(&value).ok()))
    }

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

    pub async fn zrem_range_by_score(
        &self,
        key: &str,
        min_score: i64,
        max_score: i64,
    ) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        let _: () = conn.zrembyscore(key, min_score, max_score).await?;
        Ok(())
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

    pub async fn delete_keys_count(&self, keys: &[String]) -> crate::storage::StorageResult<usize> {
        if keys.is_empty() {
            return Ok(0);
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

    pub async fn clear_keys_by_prefix(
        &self,
        prefix: &str,
        count: usize,
    ) -> crate::storage::StorageResult<usize> {
        let keys = self.scan_keys(prefix, count).await?;
        let mut deleted = 0;
        for chunk in keys.chunks(200) {
            deleted += self.delete_keys_count(chunk).await?;
        }
        Ok(deleted)
    }

    pub async fn clear_all_keys(&self) -> crate::storage::StorageResult<usize> {
        let mut conn = self.conn();
        let (cleared_keys, _): (usize, ()) = redis::pipe()
            .query_async_replacing_prefix(&mut conn, "")
            .await?;
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

    pub async fn export_backup_entry(
        &self,
        key: &str,
    ) -> crate::storage::StorageResult<Option<Value>> {
        let mut conn = self.conn();
        let value_type: String = redis::cmd("TYPE").arg(key).query_async(&mut conn).await?;
        if value_type == "none" {
            return Ok(None);
        }
        let ttl_ms: i64 = redis::cmd("PTTL").arg(key).query_async(&mut conn).await?;
        let ttl = if ttl_ms > 0 {
            Value::Number(ttl_ms.into())
        } else {
            Value::Null
        };

        match value_type.as_str() {
            "string" => {
                let value: Option<String> = conn.get(key).await?;
                Ok(value.map(|value| {
                    json!({
                        "key": key,
                        "type": "string",
                        "ttl_ms": ttl,
                        "value": value,
                    })
                }))
            }
            "hash" => {
                let value: HashMap<String, String> = conn.hgetall(key).await?;
                Ok(Some(json!({
                    "key": key,
                    "type": "hash",
                    "ttl_ms": ttl,
                    "value": value,
                })))
            }
            "list" => {
                let value: Vec<String> = conn.lrange(key, 0, -1).await?;
                Ok(Some(json!({
                    "key": key,
                    "type": "list",
                    "ttl_ms": ttl,
                    "value": value,
                })))
            }
            "set" => {
                let mut value: Vec<String> = conn.smembers(key).await?;
                value.sort_by(|left, right| node_locale_compare_ordering(left, right));
                Ok(Some(json!({
                    "key": key,
                    "type": "set",
                    "ttl_ms": ttl,
                    "value": value,
                })))
            }
            "zset" => {
                let pairs: Vec<(String, f64)> = redis::cmd("ZRANGE")
                    .arg(key)
                    .arg(0)
                    .arg(-1)
                    .arg("WITHSCORES")
                    .query_async(&mut conn)
                    .await?;
                let value = pairs
                    .into_iter()
                    .map(|(member, score)| json!({ "member": member, "score": score }))
                    .collect::<Vec<_>>();
                Ok(Some(json!({
                    "key": key,
                    "type": "zset",
                    "ttl_ms": ttl,
                    "value": value,
                })))
            }
            "stream" => {
                let response: Vec<(String, Vec<String>)> = redis::cmd("XRANGE")
                    .arg(key)
                    .arg("-")
                    .arg("+")
                    .query_async(&mut conn)
                    .await?;
                let value = response
                    .into_iter()
                    .filter(|(_, fields)| !fields.is_empty() && fields.len() % 2 == 0)
                    .map(|(id, fields)| json!({ "id": id, "fields": fields }))
                    .collect::<Vec<_>>();
                Ok(Some(json!({
                    "key": key,
                    "type": "stream",
                    "ttl_ms": ttl,
                    "value": value,
                })))
            }
            _ => Ok(Some(json!({
                "key": key,
                "type": value_type,
                "ttl_ms": ttl,
                "value": Value::Null,
            }))),
        }
    }

    #[allow(dead_code)]
    pub async fn restore_backup_entries(
        &self,
        entries: &[Value],
    ) -> crate::storage::StorageResult<()> {
        const PIPELINE_BATCH_SIZE: usize = 100;

        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        let mut batched_commands = 0usize;

        for entry in entries {
            if entry.get("key").and_then(Value::as_str) == Some(HOST_MAPPINGS_GENERATION_KEY) {
                continue;
            }
            batched_commands += append_backup_restore_commands(&mut pipe, entry);

            if batched_commands >= PIPELINE_BATCH_SIZE {
                pipe.query_async::<()>(&mut conn).await?;
                pipe = redis::pipe();
                batched_commands = 0;
            }
        }

        if batched_commands > 0 {
            pipe.query_async::<()>(&mut conn).await?;
        }
        Ok(())
    }

    pub async fn replace_backup_entries_by_prefix(
        &self,
        prefix: &str,
        entries: &[Value],
        _count: usize,
    ) -> crate::storage::StorageResult<usize> {
        let mut conn = self.conn();
        let trusted_generation = if prefix == "fn_knock:" {
            Some(
                load_config_fence_snapshot(&mut conn)
                    .await?
                    .generation
                    .checked_add(1)
                    .ok_or_else(|| {
                        crate::storage::storage_error("host mappings generation overflow")
                    })?,
            )
        } else {
            None
        };
        let mut pipe = redis::pipe();
        for entry in entries {
            if entry.get("key").and_then(Value::as_str) == Some(HOST_MAPPINGS_GENERATION_KEY) {
                continue;
            }
            append_backup_restore_commands(&mut pipe, entry);
        }
        if let Some(generation) = trusted_generation {
            pipe.set(HOST_MAPPINGS_GENERATION_KEY, generation.to_string())
                .ignore();
        }
        let (cleared_keys, _): (usize, ()) =
            pipe.query_async_replacing_prefix(&mut conn, prefix).await?;
        Ok(cleared_keys)
    }

    #[allow(dead_code)]
    pub async fn set_json_value(
        &self,
        key: &str,
        value: &Value,
    ) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        conn.set(
            key,
            serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string()),
        )
        .await
    }

    pub async fn set_json_value_ex(
        &self,
        key: &str,
        value: &Value,
        ttl_seconds: usize,
    ) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        conn.set_ex(
            key,
            serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string()),
            ttl_seconds.max(1) as u64,
        )
        .await
    }

    pub async fn set_json_value_nx_ex(
        &self,
        key: &str,
        value: &Value,
        ttl_seconds: usize,
    ) -> crate::storage::StorageResult<bool> {
        let mut conn = self.conn();
        let serialized = serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string());
        let result: Option<String> = redis::cmd("SET")
            .arg(key)
            .arg(serialized)
            .arg("EX")
            .arg(ttl_seconds.max(1))
            .arg("NX")
            .query_async(&mut conn)
            .await?;
        Ok(result.as_deref() == Some("OK"))
    }

    pub async fn set_json_lock_if_owned_ex(
        &self,
        key: &str,
        lock_id: &str,
        value: &Value,
        ttl_seconds: usize,
    ) -> crate::storage::StorageResult<bool> {
        let mut conn = self.conn();
        let serialized = serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string());
        let result: i64 = redis::cmd("EVAL")
            .arg(
                r#"
-- fn-knock:eval:json-lock-refresh:v1
local raw = redis.call("GET", KEYS[1])
if not raw then
  return 0
end
local ok, decoded = pcall(cjson.decode, raw)
if not ok or type(decoded) ~= "table" or decoded["lockId"] ~= ARGV[1] then
  return 0
end
redis.call("SET", KEYS[1], ARGV[2], "EX", tonumber(ARGV[3]))
return 1
"#,
            )
            .arg(1)
            .arg(key)
            .arg(lock_id)
            .arg(serialized)
            .arg(ttl_seconds.max(1).to_string())
            .query_async(&mut conn)
            .await?;
        Ok(result == 1)
    }

    pub async fn delete_lock_if_owned(
        &self,
        key: &str,
        lock_id: &str,
    ) -> crate::storage::StorageResult<bool> {
        let mut conn = self.conn();
        let result: i64 = redis::cmd("EVAL")
            .arg(
                r#"
-- fn-knock:eval:json-lock-release:v1
local raw = redis.call("GET", KEYS[1])
if not raw then
  return 0
end
local ok, decoded = pcall(cjson.decode, raw)
if not ok or type(decoded) ~= "table" or decoded["lockId"] ~= ARGV[1] then
  return 0
end
redis.call("DEL", KEYS[1])
return 1
"#,
            )
            .arg(1)
            .arg(key)
            .arg(lock_id)
            .query_async(&mut conn)
            .await?;
        Ok(result == 1)
    }

    pub async fn get_config(&self) -> crate::storage::StorageResult<Value> {
        let mut conn = self.conn();
        let snapshot = load_config_fence_snapshot(&mut conn).await?;
        let mut config = snapshot.config;
        inject_config_generation_marker(&mut config, snapshot.generation)?;
        Ok(config)
    }

    /// Atomically replaces only the `host_mappings` section when its current
    /// value still exactly matches `expected`.
    ///
    /// The returned value is the complete config that was persisted. This is
    /// intentionally produced inside the storage transaction so callers do
    /// not have to reconstruct a full config from a stale read and therefore
    /// cannot overwrite unrelated top-level sections.
    pub async fn compare_and_set_host_mappings(
        &self,
        expected: &[Value],
        replacement: &[Value],
    ) -> crate::storage::StorageResult<Option<Value>> {
        let mut conn = self.conn();
        // An unrelated top-level section may change between our read and the
        // raw-string CAS. Re-read and merge in that case; if host_mappings
        // itself changed, the exact structural comparison below returns a
        // conflict instead of overwriting the newer value.
        for _ in 0..32 {
            let snapshot = load_config_fence_snapshot(&mut conn).await?;
            let mut current_config = snapshot.config.clone();
            strip_internal_config_metadata(&mut current_config);
            let Some(current_object) = current_config.as_object_mut() else {
                return Err(crate::storage::storage_error(
                    "stored config must be a JSON object",
                ));
            };
            let current_mappings = match current_object.get("host_mappings") {
                None => &[][..],
                Some(Value::Array(mappings)) => mappings.as_slice(),
                Some(_) => return Ok(None),
            };
            if current_mappings != expected {
                return Ok(None);
            }
            let mappings_changed = current_mappings != replacement;
            current_object.insert(
                "host_mappings".to_string(),
                Value::Array(replacement.to_vec()),
            );
            let replacement_raw = serde_json::to_string(&current_config)?;
            let replacement_generation = if mappings_changed {
                snapshot.generation.checked_add(1).ok_or_else(|| {
                    crate::storage::storage_error("host mappings generation overflow")
                })?
            } else {
                snapshot.generation
            };
            if compare_and_set_config_fence_snapshot(
                &mut conn,
                &snapshot,
                &replacement_raw,
                replacement_generation,
            )
            .await?
            {
                inject_config_generation_marker(&mut current_config, replacement_generation)?;
                return Ok(Some(current_config));
            }
        }
        Ok(None)
    }

    /// Atomically merges the two gateway target configuration sections into
    /// the latest full config. Host runtime synchronization may overlap both
    /// a non-Host writer (for example a run_type update) and a newer writer of
    /// either target section. A section is replaced only while its exact
    /// original value, including absence, still matches; otherwise the newer
    /// stored section is retained.
    pub async fn merge_gateway_target_config_sections(
        &self,
        expected_gateway_proxy_headers: Option<&Value>,
        gateway_proxy_headers: &Value,
        expected_gateway_host_response: Option<&Value>,
        gateway_host_response: &Value,
    ) -> crate::storage::StorageResult<Value> {
        let mut conn = self.conn();
        for _ in 0..32 {
            let snapshot = load_config_fence_snapshot(&mut conn).await?;
            let mut current_config = snapshot.config.clone();
            strip_internal_config_metadata(&mut current_config);
            let Some(object) = current_config.as_object_mut() else {
                return Err(crate::storage::storage_error(
                    "stored config must be a JSON object",
                ));
            };
            let proxy_headers_unchanged = match (
                object.get("gateway_proxy_headers"),
                expected_gateway_proxy_headers,
            ) {
                (None, None) => true,
                (Some(current), Some(expected)) => current == expected,
                _ => false,
            };
            if proxy_headers_unchanged {
                object.insert(
                    "gateway_proxy_headers".to_string(),
                    gateway_proxy_headers.clone(),
                );
            }
            let host_response_unchanged = match (
                object.get("gateway_host_response"),
                expected_gateway_host_response,
            ) {
                (None, None) => true,
                (Some(current), Some(expected)) => current == expected,
                _ => false,
            };
            if host_response_unchanged {
                object.insert(
                    "gateway_host_response".to_string(),
                    gateway_host_response.clone(),
                );
            }
            let replacement_raw = serde_json::to_string(&current_config)?;
            if compare_and_set_config_fence_snapshot(
                &mut conn,
                &snapshot,
                &replacement_raw,
                snapshot.generation,
            )
            .await?
            {
                inject_config_generation_marker(&mut current_config, snapshot.generation)?;
                return Ok(current_config);
            }
        }
        Err(crate::storage::storage_error(
            "config changed too frequently while merging gateway target sections",
        ))
    }

    pub async fn save_config(&self, value: &Value) -> crate::storage::StorageResult<()> {
        let mut requested_config = value.clone();
        let requested_generation = take_config_generation_marker(&mut requested_config)?;
        strip_internal_config_metadata(&mut requested_config);
        let requested_host_mappings = config_host_mappings(&requested_config);
        let requested_host_fingerprint = config_host_mappings_fingerprint(&requested_config)?;
        if let Some(marker) = requested_generation.as_ref()
            && marker.host_fingerprint != requested_host_fingerprint
        {
            return Err(crate::storage::storage_error(
                "host mappings must be updated through compare_and_set_host_mappings",
            ));
        }
        let mut conn = self.conn();

        for _ in 0..32 {
            let snapshot = load_config_fence_snapshot(&mut conn).await?;
            let current_host_mappings = config_host_mappings(&snapshot.config);
            let current_host_fingerprint = config_host_mappings_fingerprint(&snapshot.config)?;
            let mut replacement_config = requested_config.clone();
            let replacement_generation = match requested_generation.as_ref() {
                Some(marker)
                    if marker.host_fingerprint != current_host_fingerprint
                        || marker.generation != snapshot.generation =>
                {
                    return Err(crate::storage::storage_error(
                        "host mappings changed after this config snapshot was read",
                    ));
                }
                Some(_) => snapshot.generation,
                None => {
                    if snapshot.config_raw.is_some() {
                        return Err(crate::storage::storage_error(
                            "config generation marker is required for an existing config",
                        ));
                    }
                    if requested_host_mappings == current_host_mappings {
                        snapshot.generation
                    } else {
                        snapshot.generation.checked_add(1).ok_or_else(|| {
                            crate::storage::storage_error("host mappings generation overflow")
                        })?
                    }
                }
            };
            strip_internal_config_metadata(&mut replacement_config);
            let replacement_raw = serde_json::to_string(&replacement_config)?;
            if compare_and_set_config_fence_snapshot(
                &mut conn,
                &snapshot,
                &replacement_raw,
                replacement_generation,
            )
            .await?
            {
                return Ok(());
            }
        }
        Err(crate::storage::storage_error(
            "config changed too frequently while saving",
        ))
    }

    /// Explicitly replaces the complete persisted config. Normal application
    /// updates must use `get_config` followed by `save_config`; this test-only
    /// method sets up explicit full replacements.
    #[cfg(test)]
    pub async fn replace_config(&self, value: &Value) -> crate::storage::StorageResult<()> {
        let mut replacement_config = value.clone();
        strip_internal_config_metadata(&mut replacement_config);
        let replacement_host_mappings = config_host_mappings(&replacement_config);
        let replacement_raw = serde_json::to_string(&replacement_config)?;
        let mut conn = self.conn();
        for _ in 0..32 {
            let snapshot = load_config_fence_snapshot(&mut conn).await?;
            let replacement_generation =
                if replacement_host_mappings == config_host_mappings(&snapshot.config) {
                    snapshot.generation
                } else {
                    snapshot.generation.checked_add(1).ok_or_else(|| {
                        crate::storage::storage_error("host mappings generation overflow")
                    })?
                };
            if compare_and_set_config_fence_snapshot(
                &mut conn,
                &snapshot,
                &replacement_raw,
                replacement_generation,
            )
            .await?
            {
                return Ok(());
            }
        }
        Err(crate::storage::storage_error(
            "config changed too frequently while replacing",
        ))
    }

    pub async fn locale(&self) -> crate::storage::StorageResult<Value> {
        let config = self.get_config().await?;
        Ok(config
            .get("locale")
            .cloned()
            .unwrap_or_else(|| json!({ "default_locale": "zh-CN" })))
    }

    pub async fn appearance(&self) -> crate::storage::StorageResult<Value> {
        let config = self.get_config().await?;
        Ok(config
            .get("appearance")
            .cloned()
            .unwrap_or_else(|| json!({ "theme_color_preset": "default" })))
    }

    #[allow(dead_code)]
    pub async fn captcha_public_settings(&self) -> crate::storage::StorageResult<Value> {
        let config = self.get_config().await?;
        let settings = config.get("captcha").cloned().unwrap_or_else(|| {
            json!({
                "provider": "pow",
                "widget_mode": "normal",
                "pow": {},
                "turnstile": { "site_key": "", "secret_key": "" }
            })
        });
        let provider = settings
            .get("provider")
            .and_then(Value::as_str)
            .unwrap_or("pow");
        let site_key = settings
            .pointer("/turnstile/site_key")
            .and_then(Value::as_str)
            .unwrap_or("");
        Ok(json!({
            "provider": provider,
            "widget_mode": "normal",
            "available": true,
            "unavailable_reason": null,
            "pow": {},
            "turnstile": { "site_key": site_key }
        }))
    }
}

fn append_backup_restore_commands(pipe: &mut redis::Pipeline, entry: &Value) -> usize {
    let key = entry.get("key").and_then(Value::as_str).unwrap_or("");
    let value_type = entry.get("type").and_then(Value::as_str).unwrap_or("");
    let ttl_ms = entry
        .get("ttl_ms")
        .and_then(Value::as_i64)
        .filter(|value| *value > 0);
    if key.is_empty() {
        return 0;
    }

    let mut command_count = 0usize;
    match value_type {
        "string" => {
            let command = pipe
                .cmd("SET")
                .arg(key)
                .arg(entry.get("value").and_then(Value::as_str).unwrap_or(""));
            if let Some(ttl_ms) = ttl_ms {
                command.arg("PX").arg(ttl_ms);
            }
            command.ignore();
            command_count += 1;
        }
        "hash" => {
            if let Some(object) = entry.get("value").and_then(Value::as_object)
                && !object.is_empty()
            {
                let pairs = object
                    .iter()
                    .filter_map(|(field, value)| value.as_str().map(|text| (field.as_str(), text)))
                    .collect::<Vec<_>>();
                if pairs.is_empty() {
                    return command_count;
                }
                pipe.cmd("HSET").arg(key);
                for (field, value) in pairs {
                    pipe.arg(field).arg(value);
                }
                pipe.ignore();
                command_count += 1;
            }
        }
        "list" => {
            if let Some(items) = entry.get("value").and_then(Value::as_array)
                && !items.is_empty()
            {
                pipe.cmd("RPUSH").arg(key);
                for item in items {
                    pipe.arg(item.as_str().unwrap_or(""));
                }
                pipe.ignore();
                command_count += 1;
            }
        }
        "set" => {
            if let Some(items) = entry.get("value").and_then(Value::as_array)
                && !items.is_empty()
            {
                pipe.cmd("SADD").arg(key);
                for item in items {
                    pipe.arg(item.as_str().unwrap_or(""));
                }
                pipe.ignore();
                command_count += 1;
            }
        }
        "zset" => {
            if let Some(items) = entry.get("value").and_then(Value::as_array)
                && !items.is_empty()
            {
                pipe.cmd("ZADD").arg(key);
                for item in items {
                    pipe.arg(item.get("score").and_then(Value::as_f64).unwrap_or(0.0))
                        .arg(item.get("member").and_then(Value::as_str).unwrap_or(""));
                }
                pipe.ignore();
                command_count += 1;
            }
        }
        "stream" => {
            if let Some(items) = entry.get("value").and_then(Value::as_array) {
                for item in items {
                    let id = item.get("id").and_then(Value::as_str).unwrap_or("*");
                    let Some(fields) = item.get("fields").and_then(Value::as_array) else {
                        continue;
                    };
                    if fields.is_empty() || fields.len() % 2 != 0 {
                        continue;
                    }
                    pipe.cmd("XADD").arg(key).arg(id);
                    for field in fields {
                        pipe.arg(field.as_str().unwrap_or(""));
                    }
                    pipe.ignore();
                    command_count += 1;
                }
            }
        }
        _ => {}
    }

    if let Some(ttl_ms) = ttl_ms.filter(|_| !matches!(value_type, "none" | "string")) {
        pipe.cmd("PEXPIRE").arg(key).arg(ttl_ms).ignore();
        command_count += 1;
    }
    command_count
}
