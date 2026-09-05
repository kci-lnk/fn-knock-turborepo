use super::*;

impl Store {
    pub(super) async fn verify_fnos_share_shadow_key(
        &self,
        key: &str,
    ) -> crate::storage::StorageResult<()> {
        if !crate::storage::typed_fnos_share::owns_key(key) {
            return Ok(());
        }
        let matched = self
            .typed
            .typed_fnos_share
            .verify_and_repair_key(key)
            .await?;
        if matched {
            if self.typed_fnos_share_shadow.mark_healthy() {
                tracing::info!("typed fnOS share runtime comparison recovered");
            }
            return Ok(());
        }
        self.typed_fnos_share_shadow.mark_mismatch();
        tracing::warn!(
            "typed fnOS share shadow differed from the compatibility capability and was repaired"
        );
        Ok(())
    }

    pub(super) async fn verify_subdomain_grant_shadow_key(
        &self,
        key: &str,
    ) -> crate::storage::StorageResult<()> {
        if !crate::storage::typed_subdomain_grant::owns_key(key) {
            return Ok(());
        }
        let matched = self
            .typed
            .typed_subdomain_grant
            .verify_and_repair_key(key)
            .await?;
        if matched {
            if self.typed_subdomain_grant_shadow.mark_healthy() {
                tracing::info!("typed subdomain grant aggregate comparison recovered");
            }
            return Ok(());
        }
        self.typed_subdomain_grant_shadow.mark_mismatch();
        tracing::warn!(
            "typed subdomain grant shadow differed from the compatibility aggregate and was repaired"
        );
        Ok(())
    }

    pub(super) async fn verify_whitelist_runtime_shadow_key(
        &self,
        key: &str,
    ) -> crate::storage::StorageResult<()> {
        if !crate::storage::typed_whitelist_runtime::owns_key(key) {
            return Ok(());
        }
        let matched = self
            .typed
            .typed_whitelist_runtime
            .verify_and_repair_key(key)
            .await?;
        if matched {
            if self.typed_whitelist_runtime_shadow.mark_healthy() {
                tracing::info!("typed whitelist owner runtime comparison recovered");
            }
            return Ok(());
        }
        self.typed_whitelist_runtime_shadow.mark_mismatch();
        tracing::warn!(
            "typed whitelist owner runtime differed from compatibility state and was repaired"
        );
        Ok(())
    }

    pub(crate) async fn verify_identity_runtime_shadow(
        &self,
        protocol: &str,
    ) -> crate::storage::StorageResult<()> {
        let matched = self
            .typed
            .typed_identity_runtime
            .verify_and_repair_protocol(protocol)
            .await?;
        if matched {
            if self.typed_identity_runtime_shadow.mark_healthy() {
                tracing::info!(protocol, "typed identity runtime comparison recovered");
            }
            return Ok(());
        }
        self.typed_identity_runtime_shadow.mark_mismatch();
        tracing::warn!(
            protocol,
            "typed identity runtime shadow differed from compatibility state and was repaired"
        );
        Ok(())
    }

    pub async fn ping(&self) -> crate::storage::StorageResult<()> {
        self.manager.ping().await
    }

    pub async fn get_json_value(&self, key: &str) -> crate::storage::StorageResult<Option<Value>> {
        self.verify_fnos_share_shadow_key(key).await?;
        let mut conn = self.conn();
        let raw: Option<String> = conn.get(key).await?;
        Ok(raw.and_then(|value| serde_json::from_str(&value).ok()))
    }

    pub(crate) async fn get_json_value_analytics(
        &self,
        key: &str,
    ) -> crate::storage::StorageResult<Option<Value>> {
        let values = self
            .manager
            .get_live_strings_analytics(vec![key.to_string()])
            .await?;
        Ok(values
            .into_iter()
            .next()
            .flatten()
            .and_then(|raw| serde_json::from_str(&raw).ok()))
    }

    pub async fn get_string_value(
        &self,
        key: &str,
    ) -> crate::storage::StorageResult<Option<String>> {
        self.verify_subdomain_grant_shadow_key(key).await?;
        self.verify_whitelist_runtime_shadow_key(key).await?;
        let mut conn = self.conn();
        conn.get(key).await
    }

    /// Authorization-only live read. Shadow validation and the authoritative
    /// compatibility value both use the isolated auth reader, so a normal
    /// credential lookup cannot queue behind unrelated primary writes.
    pub(crate) async fn get_string_value_auth(
        &self,
        key: &str,
    ) -> crate::storage::StorageResult<Option<String>> {
        self.verify_subdomain_grant_shadow_key(key).await?;
        self.verify_whitelist_runtime_shadow_key(key).await?;
        self.manager
            .get_live_string_auth(key.to_string(), crate::time_utils::now_ms())
            .await
    }

    pub async fn set_string_value_with_optional_ttl(
        &self,
        key: &str,
        value: &str,
        ttl_seconds: Option<i64>,
    ) -> crate::storage::StorageResult<()> {
        self.verify_whitelist_runtime_shadow_key(key).await?;
        let mut conn = self.conn();
        if let Some(ttl_seconds) = ttl_seconds.filter(|value| *value > 0) {
            let _: () = conn.set_ex(key, value, ttl_seconds as u64).await?;
        } else {
            let _: () = conn.set(key, value).await?;
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) async fn ttl_seconds(&self, key: &str) -> crate::storage::StorageResult<i64> {
        let mut conn = self.conn();
        conn.ttl(key).await
    }

    pub async fn purge_expired_keys(&self) -> crate::storage::StorageResult<usize> {
        self.manager.purge_expired_keys().await
    }

    /// Atomically increment a short-lived counter and attach its window TTL.
    ///
    /// This is intentionally kept in the storage layer so the production
    /// Redis backend and the SQLite compatibility backend share the exact same
    /// rate-limit semantics.  The first increment owns the expiry; subsequent
    /// increments only update the value.
    pub async fn increment_counter_with_ttl(
        &self,
        key: &str,
        ttl_seconds: i64,
    ) -> crate::storage::StorageResult<i64> {
        if key.starts_with(crate::storage::typed_subdomain_rate_limit::RATE_LIMIT_PREFIX) {
            let matched = self
                .typed
                .typed_subdomain_rate_limit
                .verify_and_repair(key)
                .await?;
            self.observe_subdomain_rate_limit_shadow_comparison(matched);
        }
        let mut conn = self.conn();
        let script = r#"
-- fn-knock:eval:increment-counter-with-ttl:v1
local count = redis.call("INCR", KEYS[1])
if count == 1 then
  redis.call("EXPIRE", KEYS[1], ARGV[1])
end
return count
"#;
        redis::cmd("EVAL")
            .arg(script)
            .arg(1)
            .arg(key)
            .arg(ttl_seconds.max(1))
            .query_async(&mut conn)
            .await
    }

    pub(super) fn observe_subdomain_rate_limit_shadow_comparison(&self, matched: bool) {
        if matched {
            if self.typed_subdomain_rate_limit_shadow.mark_healthy() {
                tracing::info!("typed subdomain rate-limit shadow comparison recovered");
            }
            return;
        }
        self.typed_subdomain_rate_limit_shadow.mark_mismatch();
        tracing::warn!(
            "typed subdomain rate-limit shadow differed from the compatibility counter and was repaired"
        );
    }

    /// Store an expiring string and track it in a sorted-set expiry index,
    /// refusing new members once the live index reaches `limit`.
    ///
    /// Existing data keys may always be refreshed so reaching the cap does not
    /// invalidate grants that have already been issued.
    #[allow(clippy::too_many_arguments)]
    pub async fn set_expiring_string_with_zset_limit(
        &self,
        data_key: &str,
        value: &str,
        ttl_seconds: i64,
        index_key: &str,
        now_score: i64,
        expires_at_score: i64,
        limit: i64,
    ) -> crate::storage::StorageResult<bool> {
        self.verify_subdomain_grant_shadow_key(data_key).await?;
        self.verify_subdomain_grant_shadow_key(index_key).await?;
        let mut conn = self.conn();
        let script = r#"
-- fn-knock:eval:set-expiring-string-with-zset-limit:v1
redis.call("ZREMRANGEBYSCORE", KEYS[2], "-inf", ARGV[3])
local tracked = redis.call("ZSCORE", KEYS[2], KEYS[1])
local existing = redis.call("EXISTS", KEYS[1])
if not tracked and existing == 0 and redis.call("ZCARD", KEYS[2]) >= tonumber(ARGV[5]) then
  return 0
end
redis.call("SET", KEYS[1], ARGV[1], "EX", ARGV[2])
redis.call("ZADD", KEYS[2], ARGV[4], KEYS[1])
return 1
"#;
        let stored: i64 = redis::cmd("EVAL")
            .arg(script)
            .arg(2)
            .arg(data_key)
            .arg(index_key)
            .arg(value)
            .arg(ttl_seconds.max(1))
            .arg(now_score)
            .arg(expires_at_score)
            .arg(limit.max(1))
            .query_async(&mut conn)
            .await?;
        Ok(stored == 1)
    }

    pub async fn set_string_value(
        &self,
        key: &str,
        value: &str,
    ) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        conn.set(key, value).await?;
        self.refresh_config_snapshot_after_key_change(key).await
    }

    pub(super) async fn refresh_config_snapshot_after_key_change(
        &self,
        key: &str,
    ) -> crate::storage::StorageResult<()> {
        if matches!(key, CONFIG_KEY | HOST_MAPPINGS_GENERATION_KEY) {
            self.refresh_config_snapshot().await?;
        }
        Ok(())
    }

    pub async fn set_key_if_not_exists_with_ttl(
        &self,
        key: &str,
        value: &str,
        ttl_seconds: usize,
    ) -> crate::storage::StorageResult<bool> {
        self.verify_fnos_share_shadow_key(key).await?;
        if let Some(target_id) = key
            .strip_prefix(crate::storage::typed_wol_cooldown::COOLDOWN_PREFIX)
            .filter(|target_id| !target_id.is_empty())
        {
            let matched = self
                .typed
                .typed_wol_cooldown
                .verify_and_repair(target_id)
                .await?;
            self.observe_wol_cooldown_shadow_comparison(matched);
        }
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

    pub(super) fn observe_wol_cooldown_shadow_comparison(&self, matched: bool) {
        if matched {
            if self.typed_wol_cooldown_shadow.mark_healthy() {
                tracing::info!("typed WOL cooldown shadow comparison recovered");
            }
            return;
        }
        self.typed_wol_cooldown_shadow.mark_mismatch();
        tracing::warn!(
            "typed WOL cooldown shadow differed from the compatibility guard and was repaired"
        );
    }

    pub async fn delete_key_if_value(
        &self,
        key: &str,
        value: &str,
    ) -> crate::storage::StorageResult<()> {
        self.verify_fnos_share_shadow_key(key).await?;
        self.verify_whitelist_runtime_shadow_key(key).await?;
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
        self.verify_fnos_share_shadow_key(key).await?;
        self.verify_subdomain_grant_shadow_key(key).await?;
        self.verify_whitelist_runtime_shadow_key(key).await?;
        let mut conn = self.conn();
        let _: () = conn.del(key).await?;
        self.refresh_config_snapshot_after_key_change(key).await
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
}
