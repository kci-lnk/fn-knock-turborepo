use super::*;

impl ConnectionManager {
    pub(crate) async fn get<K: IntoKey, T: FromOptionalString>(
        &mut self,
        key: K,
    ) -> RedisResult<T> {
        let key = key.into_key();
        let value = self
            .call(move |conn| {
                purge_expired(conn, &key)?;
                if key_kind(conn, &key)? != Some("string".to_string()) {
                    return Ok(None);
                }
                conn.query_row(
                    "SELECT value FROM kv_strings WHERE key = ?1",
                    params![key],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(Into::into)
            })
            .await?;
        T::from_optional_string(value)
    }

    pub(crate) async fn set<K: IntoKey, V: Display>(
        &mut self,
        key: K,
        value: V,
    ) -> RedisResult<()> {
        self.set_internal(key.into_key(), value.to_string(), None)
            .await
    }

    pub(crate) async fn set_ex<K: IntoKey, V: Display>(
        &mut self,
        key: K,
        value: V,
        ttl_seconds: u64,
    ) -> RedisResult<()> {
        self.set_internal(
            key.into_key(),
            value.to_string(),
            Some(ttl_seconds.max(1) as i64 * 1000),
        )
        .await
    }

    pub(crate) async fn del<K: IntoKeys, T: FromDeleteCount>(&mut self, keys: K) -> RedisResult<T> {
        let keys = keys.into_keys();
        let deleted = self
            .call(move |conn| {
                let tx = immediate_transaction(conn)?;
                let sync_mobility = TypedMobilitySyncScope::from_keys(keys.iter().cloned());
                let mut deleted = 0usize;
                for key in keys {
                    purge_expired_tx(&tx, &key)?;
                    deleted += delete_key_tx(&tx, &key)?;
                }
                sync_typed_mobility_tx(&tx, sync_mobility)?;
                tx.commit()?;
                Ok(deleted)
            })
            .await?;
        Ok(T::from_delete_count(deleted))
    }

    pub(crate) async fn exists<K: IntoKey>(&mut self, key: K) -> RedisResult<i64> {
        let key = key.into_key();
        self.call(move |conn| {
            purge_expired(conn, &key)?;
            Ok((key_kind(conn, &key)?.is_some()) as i64)
        })
        .await
    }

    pub(crate) async fn ttl<K: IntoKey>(&mut self, key: K) -> RedisResult<i64> {
        let key = key.into_key();
        self.call(move |conn| {
            purge_expired(conn, &key)?;
            let expires_at: Option<Option<i64>> = conn
                .query_row(
                    "SELECT expires_at_ms FROM kv_keys WHERE key = ?1",
                    params![key],
                    |row| row.get(0),
                )
                .optional()?;
            Ok(match expires_at {
                None => -2,
                Some(None) => -1,
                Some(Some(value)) => ((value - now_ms()).max(0) + 999) / 1000,
            })
        })
        .await
    }

    pub(crate) async fn expire<K: IntoKey>(&mut self, key: K, ttl_seconds: i64) -> RedisResult<()> {
        let key = key.into_key();
        let expires_at = now_ms() + ttl_seconds.max(1) * 1000;
        self.call(move |conn| {
            let sync_mobility = TypedMobilitySyncScope::from_key(&key);
            purge_expired(conn, &key)?;
            let tx = immediate_transaction(conn)?;
            tx.execute(
                "UPDATE kv_keys SET expires_at_ms = ?2 WHERE key = ?1",
                params![key, expires_at],
            )?;
            sync_typed_mobility_tx(&tx, sync_mobility)?;
            tx.commit()?;
            Ok(())
        })
        .await
    }

    pub(crate) async fn hget<K: IntoKey, F: Display, T: FromOptionalString>(
        &mut self,
        key: K,
        field: F,
    ) -> RedisResult<T> {
        let key = key.into_key();
        let field = field.to_string();
        let value = self
            .call(move |conn| {
                purge_expired(conn, &key)?;
                if key_kind(conn, &key)? != Some("hash".to_string()) {
                    return Ok(None);
                }
                conn.query_row(
                    "SELECT value FROM kv_hash WHERE key = ?1 AND field = ?2",
                    params![key, field],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(Into::into)
            })
            .await?;
        T::from_optional_string(value)
    }

    pub(crate) async fn hgetall<K: IntoKey>(
        &mut self,
        key: K,
    ) -> RedisResult<HashMap<String, String>> {
        let key = key.into_key();
        self.call(move |conn| {
            purge_expired(conn, &key)?;
            if key_kind(conn, &key)? != Some("hash".to_string()) {
                return Ok(HashMap::new());
            }
            let mut stmt =
                conn.prepare("SELECT field, value FROM kv_hash WHERE key = ?1 ORDER BY field")?;
            let rows = stmt.query_map(params![key], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            rows.collect::<Result<HashMap<_, _>, _>>()
                .map_err(Into::into)
        })
        .await
    }

    pub(crate) async fn hvals<K: IntoKey>(&mut self, key: K) -> RedisResult<Vec<String>> {
        let key = key.into_key();
        self.call(move |conn| {
            purge_expired(conn, &key)?;
            if key_kind(conn, &key)? != Some("hash".to_string()) {
                return Ok(Vec::new());
            }
            query_strings(
                conn,
                "SELECT value FROM kv_hash WHERE key = ?1 ORDER BY field",
                &[&key],
            )
        })
        .await
    }

    pub(crate) async fn hset<K: IntoKey, F: Display, V: Display>(
        &mut self,
        key: K,
        field: F,
        value: V,
    ) -> RedisResult<()> {
        let key = key.into_key();
        let field = field.to_string();
        let value = value.to_string();
        self.call(move |conn| {
            let tx = immediate_transaction(conn)?;
            let sync_mobility = TypedMobilitySyncScope::from_key(&key);
            ensure_key_tx(&tx, &key, "hash", None)?;
            tx.execute(
                "INSERT OR REPLACE INTO kv_hash(key, field, value) VALUES (?1, ?2, ?3)",
                params![key, field, value],
            )?;
            sync_typed_mobility_tx(&tx, sync_mobility)?;
            tx.commit()?;
            Ok(())
        })
        .await
    }

    #[allow(dead_code)]
    pub(crate) async fn hdel<K: IntoKey, F: IntoMembers>(
        &mut self,
        key: K,
        fields: F,
    ) -> RedisResult<()> {
        let key = key.into_key();
        let fields = fields.into_members();
        self.call(move |conn| {
            let tx = immediate_transaction(conn)?;
            let sync_mobility = TypedMobilitySyncScope::from_key(&key);
            purge_expired_tx(&tx, &key)?;
            for field in fields {
                tx.execute(
                    "DELETE FROM kv_hash WHERE key = ?1 AND field = ?2",
                    params![key, field],
                )?;
            }
            delete_collection_key_if_empty_tx(&tx, &key, "hash")?;
            sync_typed_mobility_tx(&tx, sync_mobility)?;
            tx.commit()?;
            Ok(())
        })
        .await
    }

    pub(crate) async fn smembers<K: IntoKey>(&mut self, key: K) -> RedisResult<Vec<String>> {
        let key = key.into_key();
        self.call(move |conn| {
            purge_expired(conn, &key)?;
            if key_kind(conn, &key)? != Some("set".to_string()) {
                return Ok(Vec::new());
            }
            query_strings(
                conn,
                "SELECT member FROM kv_set WHERE key = ?1 ORDER BY member",
                &[&key],
            )
        })
        .await
    }

    pub(crate) async fn sadd<K: IntoKey, M: IntoMembers>(
        &mut self,
        key: K,
        members: M,
    ) -> RedisResult<()> {
        let key = key.into_key();
        let members = members.into_members();
        self.call(move |conn| {
            let tx = immediate_transaction(conn)?;
            let sync_mobility = TypedMobilitySyncScope::from_key(&key);
            ensure_key_tx(&tx, &key, "set", None)?;
            for member in members {
                tx.execute(
                    "INSERT OR IGNORE INTO kv_set(key, member) VALUES (?1, ?2)",
                    params![key, member],
                )?;
            }
            sync_typed_mobility_tx(&tx, sync_mobility)?;
            tx.commit()?;
            Ok(())
        })
        .await
    }

    pub(crate) async fn srem<K: IntoKey, M: IntoMembers>(
        &mut self,
        key: K,
        members: M,
    ) -> RedisResult<()> {
        let key = key.into_key();
        let members = members.into_members();
        self.call(move |conn| {
            let tx = immediate_transaction(conn)?;
            let sync_mobility = TypedMobilitySyncScope::from_key(&key);
            purge_expired_tx(&tx, &key)?;
            for member in members {
                tx.execute(
                    "DELETE FROM kv_set WHERE key = ?1 AND member = ?2",
                    params![key, member],
                )?;
            }
            delete_collection_key_if_empty_tx(&tx, &key, "set")?;
            sync_typed_mobility_tx(&tx, sync_mobility)?;
            tx.commit()?;
            Ok(())
        })
        .await
    }

    pub(crate) async fn zadd<K: IntoKey, M: Display, S: Display + Send>(
        &mut self,
        key: K,
        member: M,
        score: S,
    ) -> RedisResult<()> {
        let key = key.into_key();
        let member = member.to_string();
        let score = parse_f64(&score.to_string())?;
        self.call(move |conn| {
            let tx = immediate_transaction(conn)?;
            let sync_mobility = TypedMobilitySyncScope::from_key(&key);
            ensure_key_tx(&tx, &key, "zset", None)?;
            tx.execute(
                "INSERT OR REPLACE INTO kv_zset(key, member, score) VALUES (?1, ?2, ?3)",
                params![key, member, score],
            )?;
            sync_typed_mobility_tx(&tx, sync_mobility)?;
            tx.commit()?;
            Ok(())
        })
        .await
    }

    pub(crate) async fn zrem<K: IntoKey, M: IntoMembers>(
        &mut self,
        key: K,
        members: M,
    ) -> RedisResult<()> {
        let key = key.into_key();
        let members = members.into_members();
        self.call(move |conn| {
            let tx = immediate_transaction(conn)?;
            let sync_mobility = TypedMobilitySyncScope::from_key(&key);
            purge_expired_tx(&tx, &key)?;
            for member in members {
                tx.execute(
                    "DELETE FROM kv_zset WHERE key = ?1 AND member = ?2",
                    params![key, member],
                )?;
            }
            delete_collection_key_if_empty_tx(&tx, &key, "zset")?;
            sync_typed_mobility_tx(&tx, sync_mobility)?;
            tx.commit()?;
            Ok(())
        })
        .await
    }

    pub(crate) async fn zcard<K: IntoKey>(&mut self, key: K) -> RedisResult<i64> {
        let key = key.into_key();
        self.call(move |conn| {
            purge_expired(conn, &key)?;
            count_rows(conn, "SELECT COUNT(*) FROM kv_zset WHERE key = ?1", &[&key])
        })
        .await
    }

    pub(crate) async fn zscore<K: IntoKey, M: Display>(
        &mut self,
        key: K,
        member: M,
    ) -> RedisResult<i64> {
        let key = key.into_key();
        let member = member.to_string();
        self.call(move |conn| {
            purge_expired(conn, &key)?;
            let score = conn
                .query_row(
                    "SELECT score FROM kv_zset WHERE key = ?1 AND member = ?2",
                    params![key, member],
                    |row| row.get::<_, f64>(0),
                )
                .optional()?
                .ok_or_else(|| storage_error("zscore member not found"))?;
            Ok(score.trunc() as i64)
        })
        .await
    }

    pub(crate) async fn zrange<K: IntoKey>(
        &mut self,
        key: K,
        start: isize,
        end: isize,
    ) -> RedisResult<Vec<String>> {
        self.zrange_ordered(key.into_key(), start, end, false).await
    }

    pub(crate) async fn zrevrange<K: IntoKey>(
        &mut self,
        key: K,
        start: isize,
        end: isize,
    ) -> RedisResult<Vec<String>> {
        self.zrange_ordered(key.into_key(), start, end, true).await
    }

    /// Read a persistent analytics sorted set without scheduling compatibility
    /// TTL cleanup on the primary SQLite executor.
    ///
    /// Traffic metric keys manage retention through `ZREMRANGEBYSCORE` and do
    /// not carry key-level TTLs. Keeping this path read-only prevents dashboard
    /// history scans from sitting ahead of authentication work in the primary
    /// executor queue or acquiring an unnecessary `IMMEDIATE` transaction.
    pub(crate) async fn zrangebyscore_analytics<K: IntoKey>(
        &mut self,
        key: K,
        min_score: i64,
        max_score: i64,
    ) -> RedisResult<Vec<String>> {
        let key = key.into_key();
        self.call_analytics(move |conn| {
            query_strings(
                conn,
                "SELECT member FROM kv_zset
                 WHERE key = ?1 AND score >= ?2 AND score <= ?3
                 ORDER BY score ASC, member ASC",
                &[&key, &(min_score as f64), &(max_score as f64)],
            )
        })
        .await
    }

    pub(crate) async fn lrange<K: IntoKey>(
        &mut self,
        key: K,
        start: isize,
        end: isize,
    ) -> RedisResult<Vec<String>> {
        let key = key.into_key();
        self.call(move |conn| {
            purge_expired(conn, &key)?;
            let len = count_rows(conn, "SELECT COUNT(*) FROM kv_list WHERE key = ?1", &[&key])?;
            let Some((offset, limit)) = normalize_range(len, start, end) else {
                return Ok(Vec::new());
            };
            let mut stmt = conn.prepare(
                "SELECT value FROM kv_list WHERE key = ?1 ORDER BY idx ASC LIMIT ?2 OFFSET ?3",
            )?;
            let rows = stmt.query_map(params![key, limit, offset], |row| row.get(0))?;
            rows.collect::<Result<Vec<String>, _>>().map_err(Into::into)
        })
        .await
    }

    pub(crate) async fn llen<K: IntoKey>(&mut self, key: K) -> RedisResult<i64> {
        let key = key.into_key();
        self.call(move |conn| {
            purge_expired(conn, &key)?;
            count_rows(conn, "SELECT COUNT(*) FROM kv_list WHERE key = ?1", &[&key])
        })
        .await
    }

    pub(crate) async fn xrevrange_count<K: IntoKey>(
        &mut self,
        key: K,
        max: &str,
        min: &str,
        count: usize,
    ) -> RedisResult<streams::StreamRangeReply> {
        let key = key.into_key();
        let max = max.to_string();
        let min = min.to_string();
        self.call(move |conn| {
            purge_expired(conn, &key)?;
            let min = parse_stream_bound(&min, true)?;
            let max = parse_stream_bound(&max, false)?;
            let ids = query_stream_rows(conn, &key, min, false, max, true, count)?
                .into_iter()
                .map(|(id, fields_json)| stream_id_from_row(id, fields_json))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(streams::StreamRangeReply { ids })
        })
        .await
    }

    pub(crate) async fn xread_options(
        &mut self,
        keys: &[&str],
        last_ids: &[&str],
        options: &streams::StreamReadOptions,
    ) -> RedisResult<Option<streams::StreamReadReply>> {
        let key = keys.first().copied().unwrap_or("").to_string();
        let last_id = last_ids.first().copied().unwrap_or("0-0").to_string();
        let count = options.count.unwrap_or(100).max(1);
        self.call(move |conn| {
            purge_expired(conn, &key)?;
            let last_stream_id = parse_stream_id(&last_id).unwrap_or_default();
            let ids =
                query_stream_rows(conn, &key, Some(last_stream_id), true, None, false, count)?
                    .into_iter()
                    .map(|(id, fields_json)| stream_id_from_row(id, fields_json))
                    .collect::<Result<Vec<_>, _>>()?;
            if ids.is_empty() {
                Ok(None)
            } else {
                Ok(Some(streams::StreamReadReply {
                    keys: vec![streams::StreamKey { key, ids }],
                }))
            }
        })
        .await
    }

    async fn set_internal(
        &mut self,
        key: String,
        value: String,
        ttl_ms_from_now: Option<i64>,
    ) -> RedisResult<()> {
        self.call(move |conn| {
            let tx = immediate_transaction(conn)?;
            let sync_mobility = TypedMobilitySyncScope::from_key(&key);
            set_string_tx(&tx, &key, &value, ttl_ms_from_now.map(|ttl| now_ms() + ttl))?;
            sync_typed_mobility_tx(&tx, sync_mobility)?;
            tx.commit()?;
            Ok(())
        })
        .await
    }

    async fn zrange_ordered(
        &mut self,
        key: String,
        start: isize,
        end: isize,
        reverse: bool,
    ) -> RedisResult<Vec<String>> {
        self.call(move |conn| {
            purge_expired(conn, &key)?;
            let len = count_rows(conn, "SELECT COUNT(*) FROM kv_zset WHERE key = ?1", &[&key])?;
            let Some((offset, limit)) = normalize_range(len, start, end) else {
                return Ok(Vec::new());
            };
            let order = if reverse {
                "ORDER BY score DESC, member DESC"
            } else {
                "ORDER BY score ASC, member ASC"
            };
            let sql =
                format!("SELECT member FROM kv_zset WHERE key = ?1 {order} LIMIT ?2 OFFSET ?3");
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(params![key, limit, offset], |row| row.get(0))?;
            rows.collect::<Result<Vec<String>, _>>().map_err(Into::into)
        })
        .await
    }

    pub(super) async fn execute_pipeline(
        &mut self,
        commands: Vec<CommandSpec>,
    ) -> RedisResult<Vec<CmdOutput>> {
        self.call(move |conn| {
            let tx = immediate_transaction(conn)?;
            let outputs = execute_pipeline_commands_tx(&tx, commands)?;
            tx.commit()?;
            Ok(outputs)
        })
        .await
    }

    pub(crate) async fn export_backup_entries_by_prefix(
        &self,
        prefix: &str,
        max_serialized_bytes: usize,
        include_key: fn(&str) -> bool,
    ) -> RedisResult<Vec<Value>> {
        let prefix = prefix.to_string();
        self.call(move |conn| {
            let tx = immediate_transaction(conn)?;
            let snapshot_ms = now_ms();
            purge_expired_all_tx(&tx)?;
            let keys = scan_keys_tx(&tx, &prefix)?;
            let mut entries = Vec::with_capacity(keys.len());
            let mut serialized_bytes = 0usize;

            for key in keys.into_iter().filter(|key| include_key(key)) {
                let Some(value_type) = key_kind_tx(&tx, &key)? else {
                    continue;
                };
                let expires_at_ms = tx
                    .query_row(
                        "SELECT expires_at_ms FROM kv_keys WHERE key = ?1",
                        params![key],
                        |row| row.get::<_, Option<i64>>(0),
                    )
                    .optional()?
                    .flatten();
                let ttl_ms = expires_at_ms
                    .and_then(|expires| expires.checked_sub(snapshot_ms))
                    .filter(|ttl| *ttl > 0)
                    .map(Value::from)
                    .unwrap_or(Value::Null);

                let value = match value_type.as_str() {
                    "string" => string_get_tx(&tx, &key)?.map(Value::String),
                    "hash" => {
                        let mut stmt = tx.prepare(
                            "SELECT field, value FROM kv_hash WHERE key = ?1 ORDER BY field ASC",
                        )?;
                        let rows = stmt.query_map(params![key], |row| {
                            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                        })?;
                        let mut object = serde_json::Map::new();
                        for row in rows {
                            let (field, value) = row?;
                            object.insert(field, Value::String(value));
                        }
                        Some(Value::Object(object))
                    }
                    "list" => Some(json!(list_range_tx(&tx, &key, 0, -1)?)),
                    "set" => {
                        let mut stmt = tx.prepare("SELECT member FROM kv_set WHERE key = ?1")?;
                        let rows = stmt.query_map(params![key], |row| row.get::<_, String>(0))?;
                        let mut members = rows.collect::<Result<Vec<_>, _>>()?;
                        members.sort_by(|left, right| node_locale_compare_ordering(left, right));
                        Some(json!(members))
                    }
                    "zset" => {
                        let mut stmt = tx.prepare(
                            "SELECT member, score FROM kv_zset WHERE key = ?1 ORDER BY score ASC, member ASC",
                        )?;
                        let rows = stmt.query_map(params![key], |row| {
                            Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
                        })?;
                        Some(Value::Array(
                            rows.map(|row| {
                                row.map(|(member, score)| json!({
                                    "member": member,
                                    "score": score,
                                }))
                            })
                            .collect::<Result<Vec<_>, _>>()?,
                        ))
                    }
                    "stream" => Some(Value::Array(
                        query_stream_rows(&tx, &key, None, false, None, false, usize::MAX)?
                            .into_iter()
                            .map(|(id, fields_json)| {
                                stream_fields_vec(&fields_json)
                                    .map(|fields| json!({ "id": id, "fields": fields }))
                            })
                            .collect::<Result<Vec<_>, _>>()?,
                    )),
                    _ => Some(Value::Null),
                };
                let Some(value) = value else {
                    continue;
                };
                let entry = json!({
                    "key": key,
                    "type": value_type,
                    "ttl_ms": ttl_ms,
                    "value": value,
                });
                serialized_bytes =
                    serialized_bytes.saturating_add(serde_json::to_vec(&entry)?.len());
                if serialized_bytes > max_serialized_bytes {
                    return Err(storage_error("Backup export is too large"));
                }
                entries.push(entry);
            }

            tx.commit()?;
            Ok(entries)
        })
        .await
    }

    pub(super) async fn execute_pipeline_replacing_prefix(
        &mut self,
        prefix: &str,
        commands: Vec<CommandSpec>,
    ) -> RedisResult<(usize, Vec<CmdOutput>)> {
        let prefix = prefix.to_string();
        self.call(move |conn| {
            let tx = immediate_transaction(conn)?;
            purge_expired_all_tx(&tx)?;
            let pattern = format!("{}%", escape_like_pattern(&prefix));
            let removed_typed_shadow_keys = {
                let mut statement = tx.prepare(
                    "SELECT key FROM kv_keys
                     WHERE key LIKE ?1 ESCAPE '\\'",
                )?;
                statement
                    .query_map(params![pattern], |row| row.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()?
            };
            let deleted = tx.execute(
                "DELETE FROM kv_keys WHERE key LIKE ?1 ESCAPE '\\'",
                params![pattern],
            )?;
            let sync_mobility = commands.iter().fold(
                TypedMobilitySyncScope::from_keys(removed_typed_shadow_keys),
                |scope, command| scope.merge(command_typed_mobility_scope(command)),
            );
            let mut outputs = Vec::new();
            for command in commands {
                let ignore = command.ignore;
                let output = execute_command_tx(&tx, command)?;
                if !ignore {
                    outputs.push(output);
                }
            }
            sync_typed_mobility_tx(&tx, sync_mobility)?;
            tx.commit()?;
            Ok((deleted, outputs))
        })
        .await
    }

    pub(super) async fn execute_command(&mut self, command: CommandSpec) -> RedisResult<CmdOutput> {
        self.call(move |conn| {
            let tx = immediate_transaction(conn)?;
            let sync_mobility = command_typed_mobility_scope(&command);
            let output = execute_command_tx(&tx, command)?;
            sync_typed_mobility_tx(&tx, sync_mobility)?;
            tx.commit()?;
            Ok(output)
        })
        .await
    }
}
