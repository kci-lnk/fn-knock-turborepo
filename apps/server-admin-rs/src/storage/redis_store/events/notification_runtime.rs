use super::*;

impl Store {
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

    pub(super) async fn verify_notification_runtime_shadow(
        &self,
        key: &str,
    ) -> crate::storage::StorageResult<()> {
        let matched = self
            .typed
            .typed_notification_runtime
            .verify_and_repair_key(key)
            .await?;
        if matched {
            if self.typed_notification_runtime_shadow.mark_healthy() {
                tracing::info!("typed notification runtime shadow comparison recovered");
            }
        } else {
            self.typed_notification_runtime_shadow.mark_mismatch();
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
}
