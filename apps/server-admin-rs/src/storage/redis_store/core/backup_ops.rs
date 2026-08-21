use super::*;

impl Store {
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

    pub async fn export_backup_entries_by_prefix_limited(
        &self,
        prefix: &str,
        max_serialized_bytes: usize,
        include_key: fn(&str) -> bool,
    ) -> crate::storage::StorageResult<Vec<Value>> {
        self.manager
            .export_backup_entries_by_prefix(prefix, max_serialized_bytes, include_key)
            .await
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
        self.typed.typed_event_dedupe.rebuild_from_legacy().await?;
        self.rebuild_typed_system_events_from_legacy().await?;
        self.typed.typed_fnos_share.rebuild_from_legacy().await?;
        self.typed.typed_hmac_nonce.rebuild_from_legacy().await?;
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
        self.rebuild_typed_whitelist_from_legacy().await?;
        self.typed
            .typed_whitelist_runtime
            .rebuild_from_legacy()
            .await?;
        self.typed.typed_wol_cooldown.rebuild_from_legacy().await?;
        self.refresh_config_snapshot().await?;
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
        if prefix == "fn_knock:" {
            self.typed.typed_event_dedupe.rebuild_from_legacy().await?;
            self.rebuild_typed_system_events_from_legacy().await?;
            self.typed.typed_fnos_share.rebuild_from_legacy().await?;
            self.typed.typed_hmac_nonce.rebuild_from_legacy().await?;
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
            self.rebuild_typed_whitelist_from_legacy().await?;
            self.typed
                .typed_whitelist_runtime
                .rebuild_from_legacy()
                .await?;
            self.typed.typed_wol_cooldown.rebuild_from_legacy().await?;
            self.refresh_config_snapshot().await?;
        }
        Ok(cleared_keys)
    }
}
