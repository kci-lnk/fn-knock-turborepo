use super::*;

pub(crate) struct LdapBindingClaim<'a> {
    pub invite_key: &'a str,
    pub subject_key: &'a str,
    pub binding_key: &'a str,
    pub bindings_index_key: &'a str,
    pub binding_id: &'a str,
    pub binding: &'a Value,
    pub provider_id: &'a str,
    pub totp_id: &'a str,
    pub score: i64,
}

pub(crate) struct OwnedBindingUpdate<'a> {
    pub subject_key: &'a str,
    pub binding_key: &'a str,
    pub bindings_index_key: &'a str,
    pub binding_id: &'a str,
    pub binding: &'a Value,
    pub score: i64,
}

pub(crate) struct OwnedBindingDelete<'a> {
    pub subject_key: &'a str,
    pub binding_key: &'a str,
    pub bindings_index_key: &'a str,
    pub binding_id: &'a str,
}

pub(crate) struct OidcBindingClaim<'a> {
    pub invite_key: &'a str,
    pub subject_key: &'a str,
    pub binding_key: &'a str,
    pub bindings_index_key: &'a str,
    pub binding_id: &'a str,
    pub binding: &'a Value,
    pub provider_id: &'a str,
    pub totp_id: &'a str,
    pub score: i64,
}

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
    config_fence_snapshot_from_raw(
        values.first().cloned().flatten(),
        values.get(1).cloned().flatten(),
    )
}

fn config_fence_snapshot_from_raw(
    config_raw: Option<String>,
    generation_raw: Option<String>,
) -> crate::storage::StorageResult<ConfigFenceSnapshot> {
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
) -> crate::storage::StorageResult<Option<u64>> {
    let applied: i64 = redis::cmd("EVAL")
        .arg(
            r#"
-- fn-knock:eval:cas-config-host-generation-raw:v3
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
    if applied == 0 {
        return Ok(None);
    }
    let revision = u64::try_from(applied)
        .ok()
        .filter(|revision| *revision > 0)
        .ok_or_else(|| crate::storage::storage_error("typed config revision is invalid"))?;
    Ok(Some(revision))
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

fn replace_visibility_policies_for_host_mappings(
    config: &mut Value,
    replacement_mappings: &[Value],
    supplied_policies: &Map<String, Value>,
) -> crate::storage::StorageResult<()> {
    let existing_policies = config
        .get("visibility_policies")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut referenced = referenced_host_ipset_policy_ids(replacement_mappings);
    if let Some(id) = config
        .pointer("/gateway_visibility/policy_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        referenced.insert(id.to_string());
    }
    let mut next = Map::new();
    for id in referenced {
        let policy = supplied_policies
            .get(&id)
            .or_else(|| existing_policies.get(&id))
            .cloned()
            .ok_or_else(|| {
                crate::storage::storage_error(format!(
                    "visibility policy {id} is missing from the host mapping transaction"
                ))
            })?;
        next.insert(id, policy);
    }
    let object = config
        .as_object_mut()
        .ok_or_else(|| crate::storage::storage_error("stored config must be a JSON object"))?;
    object.insert("visibility_policies".to_string(), Value::Object(next));
    Ok(())
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
    async fn verify_fnos_share_shadow_key(&self, key: &str) -> crate::storage::StorageResult<()> {
        if !crate::storage::typed_fnos_share::owns_key(key) {
            return Ok(());
        }
        let matched = self.typed_fnos_share.verify_and_repair_key(key).await?;
        if matched {
            if !self
                .typed_fnos_share_shadow_healthy
                .swap(true, AtomicOrdering::AcqRel)
            {
                tracing::info!("typed fnOS share runtime comparison recovered");
            }
            return Ok(());
        }
        self.typed_fnos_share_shadow_mismatches
            .fetch_add(1, AtomicOrdering::Relaxed);
        self.typed_fnos_share_shadow_healthy
            .store(false, AtomicOrdering::Release);
        tracing::warn!(
            "typed fnOS share shadow differed from the compatibility capability and was repaired"
        );
        Ok(())
    }

    async fn verify_subdomain_grant_shadow_key(
        &self,
        key: &str,
    ) -> crate::storage::StorageResult<()> {
        if !crate::storage::typed_subdomain_grant::owns_key(key) {
            return Ok(());
        }
        let matched = self
            .typed_subdomain_grant
            .verify_and_repair_key(key)
            .await?;
        if matched {
            if !self
                .typed_subdomain_grant_shadow_healthy
                .swap(true, AtomicOrdering::AcqRel)
            {
                tracing::info!("typed subdomain grant aggregate comparison recovered");
            }
            return Ok(());
        }
        self.typed_subdomain_grant_shadow_mismatches
            .fetch_add(1, AtomicOrdering::Relaxed);
        self.typed_subdomain_grant_shadow_healthy
            .store(false, AtomicOrdering::Release);
        tracing::warn!(
            "typed subdomain grant shadow differed from the compatibility aggregate and was repaired"
        );
        Ok(())
    }

    async fn verify_whitelist_runtime_shadow_key(
        &self,
        key: &str,
    ) -> crate::storage::StorageResult<()> {
        if !crate::storage::typed_whitelist_runtime::owns_key(key) {
            return Ok(());
        }
        let matched = self
            .typed_whitelist_runtime
            .verify_and_repair_key(key)
            .await?;
        if matched {
            if !self
                .typed_whitelist_runtime_shadow_healthy
                .swap(true, AtomicOrdering::AcqRel)
            {
                tracing::info!("typed whitelist owner runtime comparison recovered");
            }
            return Ok(());
        }
        self.typed_whitelist_runtime_shadow_mismatches
            .fetch_add(1, AtomicOrdering::Relaxed);
        self.typed_whitelist_runtime_shadow_healthy
            .store(false, AtomicOrdering::Release);
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
            .typed_identity_runtime
            .verify_and_repair_protocol(protocol)
            .await?;
        if matched {
            if !self
                .typed_identity_runtime_shadow_healthy
                .swap(true, AtomicOrdering::AcqRel)
            {
                tracing::info!(protocol, "typed identity runtime comparison recovered");
            }
            return Ok(());
        }
        self.typed_identity_runtime_shadow_mismatches
            .fetch_add(1, AtomicOrdering::Relaxed);
        self.typed_identity_runtime_shadow_healthy
            .store(false, AtomicOrdering::Release);
        tracing::warn!(
            protocol,
            "typed identity runtime shadow differed from compatibility state and was repaired"
        );
        Ok(())
    }

    pub async fn ping(&self) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        redis::cmd("PING").query_async(&mut conn).await
    }

    pub async fn get_json_value(&self, key: &str) -> crate::storage::StorageResult<Option<Value>> {
        self.verify_fnos_share_shadow_key(key).await?;
        let mut conn = self.conn();
        let raw: Option<String> = conn.get(key).await?;
        Ok(raw.and_then(|value| serde_json::from_str(&value).ok()))
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

    fn observe_subdomain_rate_limit_shadow_comparison(&self, matched: bool) {
        if matched {
            if !self
                .typed_subdomain_rate_limit_shadow_healthy
                .swap(true, AtomicOrdering::AcqRel)
            {
                tracing::info!("typed subdomain rate-limit shadow comparison recovered");
            }
            return;
        }
        self.typed_subdomain_rate_limit_shadow_mismatches
            .fetch_add(1, AtomicOrdering::Relaxed);
        self.typed_subdomain_rate_limit_shadow_healthy
            .store(false, AtomicOrdering::Release);
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
        conn.set(key, value).await
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
            let matched = self.typed_wol_cooldown.verify_and_repair(target_id).await?;
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

    fn observe_wol_cooldown_shadow_comparison(&self, matched: bool) {
        if matched {
            if !self
                .typed_wol_cooldown_shadow_healthy
                .swap(true, AtomicOrdering::AcqRel)
            {
                tracing::info!("typed WOL cooldown shadow comparison recovered");
            }
            return;
        }
        self.typed_wol_cooldown_shadow_mismatches
            .fetch_add(1, AtomicOrdering::Relaxed);
        self.typed_wol_cooldown_shadow_healthy
            .store(false, AtomicOrdering::Release);
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

    /// Atomically consumes an LDAP invitation and reserves/persists its
    /// provider-scoped directory identity. Returns false when the invitation
    /// is gone or the identity has already been claimed.
    pub(crate) async fn claim_ldap_binding_and_consume_invite(
        &self,
        claim: LdapBindingClaim<'_>,
    ) -> crate::storage::StorageResult<bool> {
        let binding_raw = serde_json::to_string(claim.binding)?;
        let mut conn = self.conn();
        let claimed: i64 = redis::cmd("EVAL")
            .arg(
                r#"
-- fn-knock:eval:claim-ldap-binding:v2
local invite_raw = redis.call("GET", KEYS[1])
if not invite_raw then
  return 0
end
local decoded, invite = pcall(cjson.decode, invite_raw)
if not decoded or type(invite) ~= "table" then
  return 0
end
if tostring(invite["provider_id"] or "") ~= ARGV[4]
  or tostring(invite["totp_id"] or "") ~= ARGV[5] then
  return 0
end
if redis.call("EXISTS", KEYS[2]) == 1 or redis.call("EXISTS", KEYS[3]) == 1 then
  return 0
end
redis.call("SET", KEYS[2], ARGV[1])
redis.call("SET", KEYS[3], ARGV[2])
redis.call("ZADD", KEYS[4], ARGV[3], ARGV[1])
redis.call("DEL", KEYS[1])
return 1
"#,
            )
            .arg(4)
            .arg(claim.invite_key)
            .arg(claim.subject_key)
            .arg(claim.binding_key)
            .arg(claim.bindings_index_key)
            .arg(claim.binding_id)
            .arg(binding_raw)
            .arg(claim.score)
            .arg(claim.provider_id)
            .arg(claim.totp_id)
            .query_async(&mut conn)
            .await?;
        Ok(claimed == 1)
    }

    /// Updates binding metadata only while the provider-scoped subject still
    /// points at the same binding. This prevents a concurrent admin
    /// revocation from being resurrected by an in-flight login.
    pub(crate) async fn update_binding_if_owned(
        &self,
        update: OwnedBindingUpdate<'_>,
    ) -> crate::storage::StorageResult<bool> {
        if update.binding_id.is_empty()
            || update.subject_key.is_empty()
            || update.binding_key.is_empty()
            || update.bindings_index_key.is_empty()
        {
            return Ok(false);
        }
        let binding_raw = serde_json::to_string(update.binding)?;
        let mut conn = self.conn();
        let updated: i64 = redis::cmd("EVAL")
            .arg(
                r#"
-- fn-knock:eval:update-owned-binding:v1
if redis.call("GET", KEYS[1]) ~= ARGV[1]
  or redis.call("EXISTS", KEYS[2]) == 0 then
  return 0
end
redis.call("SET", KEYS[2], ARGV[2])
redis.call("ZADD", KEYS[3], ARGV[3], ARGV[1])
return 1
"#,
            )
            .arg(3)
            .arg(update.subject_key)
            .arg(update.binding_key)
            .arg(update.bindings_index_key)
            .arg(update.binding_id)
            .arg(binding_raw)
            .arg(update.score)
            .query_async(&mut conn)
            .await?;
        Ok(updated == 1)
    }

    /// Removes a binding document, its subject owner, and its list index in
    /// one transaction. A stale caller can never delete a subject owner that
    /// has already moved to a different binding.
    pub(crate) async fn delete_binding_if_owned(
        &self,
        deletion: OwnedBindingDelete<'_>,
    ) -> crate::storage::StorageResult<bool> {
        if deletion.binding_id.is_empty()
            || deletion.subject_key.is_empty()
            || deletion.binding_key.is_empty()
            || deletion.bindings_index_key.is_empty()
        {
            return Ok(false);
        }
        let mut conn = self.conn();
        let deleted: i64 = redis::cmd("EVAL")
            .arg(
                r#"
-- fn-knock:eval:delete-owned-binding:v1
if redis.call("EXISTS", KEYS[2]) == 0 then
  return 0
end
redis.call("DEL", KEYS[2])
if redis.call("GET", KEYS[1]) == ARGV[1] then
  redis.call("DEL", KEYS[1])
end
redis.call("ZREM", KEYS[3], ARGV[1])
return 1
"#,
            )
            .arg(3)
            .arg(deletion.subject_key)
            .arg(deletion.binding_key)
            .arg(deletion.bindings_index_key)
            .arg(deletion.binding_id)
            .query_async(&mut conn)
            .await?;
        Ok(deleted == 1)
    }

    /// Consumes an OIDC invitation only in the same transaction that claims
    /// (or refreshes) its provider-scoped subject binding.
    pub(crate) async fn claim_oidc_binding_and_consume_invite(
        &self,
        claim: OidcBindingClaim<'_>,
    ) -> crate::storage::StorageResult<bool> {
        if claim.binding_id.is_empty()
            || claim.provider_id.is_empty()
            || claim.totp_id.is_empty()
            || claim.subject_key.is_empty()
            || claim.binding_key.is_empty()
        {
            return Ok(false);
        }
        let binding_raw = serde_json::to_string(claim.binding)?;
        let mut conn = self.conn();
        let claimed: i64 = redis::cmd("EVAL")
            .arg(
                r#"
-- fn-knock:eval:claim-oidc-binding:v1
local invite_raw = redis.call("GET", KEYS[1])
if not invite_raw then return 0 end
local decoded, invite = pcall(cjson.decode, invite_raw)
if not decoded or type(invite) ~= "table" then return 0 end
if invite["used_at"] ~= nil
  or tostring(invite["provider_id"] or "") ~= ARGV[4]
  or tostring(invite["totp_id"] or "") ~= ARGV[5] then return 0 end
local current_binding = redis.call("GET", KEYS[2])
if current_binding and current_binding ~= ARGV[1] then return 0 end
redis.call("SET", KEYS[2], ARGV[1])
redis.call("SET", KEYS[3], ARGV[2])
redis.call("ZADD", KEYS[4], ARGV[3], ARGV[1])
redis.call("DEL", KEYS[1])
return 1
"#,
            )
            .arg(4)
            .arg(claim.invite_key)
            .arg(claim.subject_key)
            .arg(claim.binding_key)
            .arg(claim.bindings_index_key)
            .arg(claim.binding_id)
            .arg(binding_raw)
            .arg(claim.score)
            .arg(claim.provider_id)
            .arg(claim.totp_id)
            .query_async(&mut conn)
            .await?;
        Ok(claimed == 1)
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
        self.typed_docker_admin.rebuild_from_legacy().await?;
        self.typed_event_dedupe.rebuild_from_legacy().await?;
        self.rebuild_typed_system_events_from_legacy().await?;
        self.typed_fnos_share.rebuild_from_legacy().await?;
        self.typed_hmac_nonce.rebuild_from_legacy().await?;
        self.typed_login_backoff.rebuild_from_legacy().await?;
        self.typed_mobility.rebuild_from_legacy().await?;
        self.typed_notification_runtime
            .rebuild_from_legacy()
            .await?;
        self.rebuild_typed_notification_documents_from_legacy()
            .await?;
        self.rebuild_typed_notification_history_from_legacy()
            .await?;
        self.typed_passkey_runtime.rebuild_from_legacy().await?;
        self.typed_subdomain_grant.rebuild_from_legacy().await?;
        self.typed_identity_runtime.rebuild_from_legacy().await?;
        self.typed_subdomain_rate_limit
            .rebuild_from_legacy()
            .await?;
        self.rebuild_typed_whitelist_from_legacy().await?;
        self.typed_whitelist_runtime.rebuild_from_legacy().await?;
        self.typed_wol_cooldown.rebuild_from_legacy().await?;
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
        self.typed_event_dedupe.rebuild_from_legacy().await?;
        self.rebuild_typed_system_events_from_legacy().await?;
        self.typed_fnos_share.rebuild_from_legacy().await?;
        self.typed_hmac_nonce.rebuild_from_legacy().await?;
        self.typed_mobility.rebuild_from_legacy().await?;
        self.typed_notification_runtime
            .rebuild_from_legacy()
            .await?;
        self.rebuild_typed_notification_documents_from_legacy()
            .await?;
        self.rebuild_typed_notification_history_from_legacy()
            .await?;
        self.typed_passkey_runtime.rebuild_from_legacy().await?;
        self.typed_subdomain_grant.rebuild_from_legacy().await?;
        self.typed_identity_runtime.rebuild_from_legacy().await?;
        self.rebuild_typed_whitelist_from_legacy().await?;
        self.typed_whitelist_runtime.rebuild_from_legacy().await?;
        self.typed_wol_cooldown.rebuild_from_legacy().await?;
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
            self.typed_event_dedupe.rebuild_from_legacy().await?;
            self.rebuild_typed_system_events_from_legacy().await?;
            self.typed_fnos_share.rebuild_from_legacy().await?;
            self.typed_hmac_nonce.rebuild_from_legacy().await?;
            self.typed_mobility.rebuild_from_legacy().await?;
            self.typed_notification_runtime
                .rebuild_from_legacy()
                .await?;
            self.rebuild_typed_notification_documents_from_legacy()
                .await?;
            self.rebuild_typed_notification_history_from_legacy()
                .await?;
            self.typed_passkey_runtime.rebuild_from_legacy().await?;
            self.typed_subdomain_grant.rebuild_from_legacy().await?;
            self.typed_identity_runtime.rebuild_from_legacy().await?;
            self.rebuild_typed_whitelist_from_legacy().await?;
            self.typed_whitelist_runtime.rebuild_from_legacy().await?;
            self.typed_wol_cooldown.rebuild_from_legacy().await?;
            self.refresh_config_snapshot().await?;
        }
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

    pub async fn set_json_values_atomically(
        &self,
        values: &[(&str, &Value)],
    ) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        for (key, value) in values {
            pipe.set(
                *key,
                serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string()),
            )
            .ignore();
        }
        pipe.query_async::<()>(&mut conn).await
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
        self.verify_whitelist_runtime_shadow_key(key).await?;
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
        self.verify_whitelist_runtime_shadow_key(key).await?;
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
        self.verify_whitelist_runtime_shadow_key(key).await?;
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
        let (config, _) = self.load_typed_config_primary().await?;
        Ok(config)
    }

    pub(super) async fn reconcile_typed_config_from_legacy(
        &self,
    ) -> crate::storage::StorageResult<(Value, u64)> {
        self.load_typed_config_primary().await
    }

    async fn load_typed_config_primary(&self) -> crate::storage::StorageResult<(Value, u64)> {
        let shadow = self
            .typed_config
            .load_shadow(CONFIG_KEY, HOST_MAPPINGS_GENERATION_KEY)
            .await?;
        let legacy_snapshot =
            config_fence_snapshot_from_raw(shadow.legacy.config_raw, shadow.legacy.generation_raw)?;
        match shadow.typed {
            Ok(Some(typed))
                if typed.document == legacy_snapshot.config
                    && typed.host_mappings_generation == legacy_snapshot.generation =>
            {
                self.observe_typed_config_shadow(&legacy_snapshot, Ok(Some(typed.clone())));
                self.typed_config_primary_bootstrapped
                    .store(true, AtomicOrdering::Release);
                let mut config = typed.document;
                inject_config_generation_marker(&mut config, typed.host_mappings_generation)?;
                Ok((config, typed.revision))
            }
            typed => {
                // A newly-created database has no typed document until this
                // first read seeds it from the 2.x-compatible keyspace. That
                // expected bootstrap is not a shadow inconsistency. Once a
                // primary document has been established, missing data,
                // corruption, and content divergence are counted and
                // surfaced as fallbacks.
                let initial_bootstrap = matches!(&typed, Ok(None))
                    && !self
                        .typed_config_primary_bootstrapped
                        .load(AtomicOrdering::Acquire);
                if !initial_bootstrap {
                    self.observe_typed_config_shadow(&legacy_snapshot, typed);
                }
                let reconciled = self
                    .typed_config
                    .reconcile_from_legacy(
                        CONFIG_KEY,
                        HOST_MAPPINGS_GENERATION_KEY,
                        &default_config(),
                    )
                    .await?;
                let repaired_snapshot = config_fence_snapshot_from_raw(
                    reconciled.legacy.config_raw,
                    reconciled.legacy.generation_raw,
                )?;
                self.typed_config_shadow_healthy
                    .store(true, AtomicOrdering::Release);
                self.typed_config_primary_bootstrapped
                    .store(true, AtomicOrdering::Release);
                tracing::info!(
                    typed_revision = reconciled.typed_revision,
                    host_mappings_generation = repaired_snapshot.generation,
                    "repaired typed config from legacy fallback"
                );
                let mut config = repaired_snapshot.config;
                inject_config_generation_marker(&mut config, repaired_snapshot.generation)?;
                Ok((config, reconciled.typed_revision))
            }
        }
    }

    fn observe_typed_config_shadow(
        &self,
        snapshot: &ConfigFenceSnapshot,
        typed: crate::storage::StorageResult<
            Option<crate::storage::typed_config::TypedConfigDocument>,
        >,
    ) {
        match typed {
            Ok(Some(typed))
                if typed.document == snapshot.config
                    && typed.host_mappings_generation == snapshot.generation =>
            {
                if !self
                    .typed_config_shadow_healthy
                    .swap(true, AtomicOrdering::AcqRel)
                {
                    tracing::info!(
                        typed_revision = typed.revision,
                        host_mappings_generation = snapshot.generation,
                        "typed config shadow recovered"
                    );
                }
            }
            Ok(typed) => {
                self.typed_config_shadow_mismatches
                    .fetch_add(1, AtomicOrdering::Relaxed);
                if self
                    .typed_config_shadow_healthy
                    .swap(false, AtomicOrdering::AcqRel)
                {
                    tracing::warn!(
                        legacy_generation = snapshot.generation,
                        typed_generation = typed
                            .as_ref()
                            .map(|document| document.host_mappings_generation),
                        typed_revision = typed.as_ref().map(|document| document.revision),
                        typed_present = typed.is_some(),
                        "typed config mismatch; falling back to legacy keyspace and repairing typed primary"
                    );
                }
            }
            Err(error) => {
                self.typed_config_shadow_mismatches
                    .fetch_add(1, AtomicOrdering::Relaxed);
                if self
                    .typed_config_shadow_healthy
                    .swap(false, AtomicOrdering::AcqRel)
                {
                    tracing::warn!(
                        %error,
                        legacy_generation = snapshot.generation,
                        "typed config read failed; falling back to legacy keyspace and repairing typed primary"
                    );
                }
            }
        }
    }

    /// Atomically replaces the complete config only when the persisted
    /// generation and value still match `expected`. This is reserved for
    /// idempotent format migrations which must update host mappings and their
    /// shared policy table in one commit.
    pub async fn compare_and_set_config_migration(
        &self,
        expected: &Value,
        replacement: &Value,
    ) -> crate::storage::StorageResult<Option<Value>> {
        let mut expected_config = expected.clone();
        take_config_generation_marker(&mut expected_config)?;
        strip_internal_config_metadata(&mut expected_config);
        let mut replacement_config = replacement.clone();
        strip_internal_config_metadata(&mut replacement_config);
        let mut conn = self.conn();
        for _ in 0..32 {
            let snapshot = load_config_fence_snapshot(&mut conn).await?;
            let mut current_config = snapshot.config.clone();
            strip_internal_config_metadata(&mut current_config);
            if current_config != expected_config {
                return Ok(None);
            }
            let host_mappings_changed =
                config_host_mappings(&current_config) != config_host_mappings(&replacement_config);
            let replacement_generation = if host_mappings_changed {
                snapshot.generation.checked_add(1).ok_or_else(|| {
                    crate::storage::storage_error("host mappings generation overflow")
                })?
            } else {
                snapshot.generation
            };
            let replacement_raw = serde_json::to_string(&replacement_config)?;
            if let Some(revision) = compare_and_set_config_fence_snapshot(
                &mut conn,
                &snapshot,
                &replacement_raw,
                replacement_generation,
            )
            .await?
            {
                let mut published = replacement_config;
                inject_config_generation_marker(&mut published, replacement_generation)?;
                self.publish_config_snapshot(published.clone(), revision);
                return Ok(Some(published));
            }
        }
        Ok(None)
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
        self.compare_and_set_host_mappings_inner(expected, replacement, None)
            .await
    }

    pub async fn compare_and_set_host_mappings_with_visibility_policies(
        &self,
        expected: &[Value],
        replacement: &[Value],
        visibility_policies: &Map<String, Value>,
    ) -> crate::storage::StorageResult<Option<Value>> {
        self.compare_and_set_host_mappings_inner(expected, replacement, Some(visibility_policies))
            .await
    }

    async fn compare_and_set_host_mappings_inner(
        &self,
        expected: &[Value],
        replacement: &[Value],
        visibility_policies: Option<&Map<String, Value>>,
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
            if let Some(visibility_policies) = visibility_policies {
                replace_visibility_policies_for_host_mappings(
                    &mut current_config,
                    replacement,
                    visibility_policies,
                )?;
            }
            let replacement_raw = serde_json::to_string(&current_config)?;
            let replacement_generation = if mappings_changed {
                snapshot.generation.checked_add(1).ok_or_else(|| {
                    crate::storage::storage_error("host mappings generation overflow")
                })?
            } else {
                snapshot.generation
            };
            if let Some(revision) = compare_and_set_config_fence_snapshot(
                &mut conn,
                &snapshot,
                &replacement_raw,
                replacement_generation,
            )
            .await?
            {
                inject_config_generation_marker(&mut current_config, replacement_generation)?;
                self.publish_config_snapshot(current_config.clone(), revision);
                return Ok(Some(current_config));
            }
        }
        Ok(None)
    }

    /// Atomically replaces the Host mapping list and its UI grouping catalog.
    /// The shared generation advances when either section changes so a stale
    /// full-config writer cannot overwrite a concurrent organization update.
    #[cfg(test)]
    pub async fn compare_and_set_host_mapping_catalog(
        &self,
        expected_mappings: &[Value],
        expected_groups: &[Value],
        expected_grouped_view: bool,
        replacement_mappings: &[Value],
        replacement_groups: &[Value],
        replacement_grouped_view: bool,
    ) -> crate::storage::StorageResult<Option<Value>> {
        self.compare_and_set_host_mapping_catalog_inner(
            expected_mappings,
            expected_groups,
            expected_grouped_view,
            replacement_mappings,
            replacement_groups,
            replacement_grouped_view,
            None,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn compare_and_set_host_mapping_catalog_with_visibility_policies(
        &self,
        expected_mappings: &[Value],
        expected_groups: &[Value],
        expected_grouped_view: bool,
        replacement_mappings: &[Value],
        replacement_groups: &[Value],
        replacement_grouped_view: bool,
        visibility_policies: &Map<String, Value>,
    ) -> crate::storage::StorageResult<Option<Value>> {
        self.compare_and_set_host_mapping_catalog_inner(
            expected_mappings,
            expected_groups,
            expected_grouped_view,
            replacement_mappings,
            replacement_groups,
            replacement_grouped_view,
            Some(visibility_policies),
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn compare_and_set_host_mapping_catalog_inner(
        &self,
        expected_mappings: &[Value],
        expected_groups: &[Value],
        expected_grouped_view: bool,
        replacement_mappings: &[Value],
        replacement_groups: &[Value],
        replacement_grouped_view: bool,
        visibility_policies: Option<&Map<String, Value>>,
    ) -> crate::storage::StorageResult<Option<Value>> {
        let mut conn = self.conn();
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
                Some(Value::Array(items)) => items.as_slice(),
                Some(_) => return Ok(None),
            };
            let current_groups = match current_object.get("host_mapping_groups") {
                None => &[][..],
                Some(Value::Array(items)) => items.as_slice(),
                Some(_) => return Ok(None),
            };
            let current_grouped_view = current_object
                .get("host_mapping_grouped_view")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if current_mappings != expected_mappings
                || current_groups != expected_groups
                || current_grouped_view != expected_grouped_view
            {
                return Ok(None);
            }

            let changed = current_mappings != replacement_mappings
                || current_groups != replacement_groups
                || current_grouped_view != replacement_grouped_view;
            current_object.insert(
                "host_mappings".to_string(),
                Value::Array(replacement_mappings.to_vec()),
            );
            current_object.insert(
                "host_mapping_groups".to_string(),
                Value::Array(replacement_groups.to_vec()),
            );
            current_object.insert(
                "host_mapping_grouped_view".to_string(),
                Value::Bool(replacement_grouped_view),
            );
            if let Some(visibility_policies) = visibility_policies {
                replace_visibility_policies_for_host_mappings(
                    &mut current_config,
                    replacement_mappings,
                    visibility_policies,
                )?;
            }
            let replacement_raw = serde_json::to_string(&current_config)?;
            let replacement_generation = if changed {
                snapshot.generation.checked_add(1).ok_or_else(|| {
                    crate::storage::storage_error("host mapping catalog generation overflow")
                })?
            } else {
                snapshot.generation
            };
            if let Some(revision) = compare_and_set_config_fence_snapshot(
                &mut conn,
                &snapshot,
                &replacement_raw,
                replacement_generation,
            )
            .await?
            {
                inject_config_generation_marker(&mut current_config, replacement_generation)?;
                self.publish_config_snapshot(current_config.clone(), revision);
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
            if let Some(revision) = compare_and_set_config_fence_snapshot(
                &mut conn,
                &snapshot,
                &replacement_raw,
                snapshot.generation,
            )
            .await?
            {
                inject_config_generation_marker(&mut current_config, snapshot.generation)?;
                self.publish_config_snapshot(current_config.clone(), revision);
                return Ok(current_config);
            }
        }
        Err(crate::storage::storage_error(
            "config changed too frequently while merging gateway target sections",
        ))
    }

    /// Atomically replaces one ordinary top-level config value while retaining
    /// every unrelated field from the latest stored snapshot. Host mapping
    /// catalog fields have dedicated generation-aware APIs and must not use
    /// this helper.
    pub async fn set_config_top_level_value(
        &self,
        key: &str,
        value: Value,
    ) -> crate::storage::StorageResult<Value> {
        if matches!(
            key,
            "host_mappings"
                | "host_mapping_groups"
                | "host_mapping_grouped_view"
                | "visibility_policies"
        ) {
            return Err(crate::storage::storage_error(
                "host mapping catalog fields require a generation-aware config API",
            ));
        }

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
            object.insert(key.to_string(), value.clone());

            let replacement_raw = serde_json::to_string(&current_config)?;
            if let Some(revision) = compare_and_set_config_fence_snapshot(
                &mut conn,
                &snapshot,
                &replacement_raw,
                snapshot.generation,
            )
            .await?
            {
                inject_config_generation_marker(&mut current_config, snapshot.generation)?;
                self.publish_config_snapshot(current_config.clone(), revision);
                return Ok(current_config);
            }
        }
        Err(crate::storage::storage_error(
            "config changed too frequently while setting a top-level value",
        ))
    }

    /// Atomically merges fields into an object-valued top-level config
    /// section. Each CAS retry starts from the latest stored config, so
    /// independent writers cannot replace one another with stale snapshots.
    pub async fn merge_config_object_fields(
        &self,
        section: &str,
        fields: Map<String, Value>,
    ) -> crate::storage::StorageResult<Value> {
        let mut conn = self.conn();
        for _ in 0..32 {
            let snapshot = load_config_fence_snapshot(&mut conn).await?;
            let mut current_config = snapshot.config.clone();
            strip_internal_config_metadata(&mut current_config);
            let Some(root) = current_config.as_object_mut() else {
                return Err(crate::storage::storage_error(
                    "stored config must be a JSON object",
                ));
            };
            let section_value = root
                .entry(section.to_string())
                .or_insert_with(|| Value::Object(Map::new()));
            if !section_value.is_object() {
                *section_value = Value::Object(Map::new());
            }
            let section_object = section_value
                .as_object_mut()
                .expect("object value was initialized above");
            section_object.extend(fields.clone());

            let replacement_raw = serde_json::to_string(&current_config)?;
            if let Some(revision) = compare_and_set_config_fence_snapshot(
                &mut conn,
                &snapshot,
                &replacement_raw,
                snapshot.generation,
            )
            .await?
            {
                inject_config_generation_marker(&mut current_config, snapshot.generation)?;
                self.publish_config_snapshot(current_config.clone(), revision);
                return Ok(current_config);
            }
        }
        Err(crate::storage::storage_error(
            "config changed too frequently while merging object fields",
        ))
    }

    /// Atomically replaces the SSL section only while it still exactly
    /// matches the caller's expected value. Unrelated top-level configuration
    /// writes are merged from the latest snapshot, while a concurrent SSL
    /// writer produces a conflict instead of being overwritten.
    pub async fn compare_and_set_ssl_config(
        &self,
        expected: Option<&Value>,
        replacement: Option<&Value>,
    ) -> crate::storage::StorageResult<Option<Value>> {
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
            let ssl_unchanged = match (object.get("ssl"), expected) {
                (None, None) => true,
                (Some(current), Some(expected)) => current == expected,
                _ => false,
            };
            if !ssl_unchanged {
                return Ok(None);
            }
            match replacement {
                Some(replacement) => {
                    object.insert("ssl".to_string(), replacement.clone());
                }
                None => {
                    object.remove("ssl");
                }
            }
            let replacement_raw = serde_json::to_string(&current_config)?;
            if let Some(revision) = compare_and_set_config_fence_snapshot(
                &mut conn,
                &snapshot,
                &replacement_raw,
                snapshot.generation,
            )
            .await?
            {
                inject_config_generation_marker(&mut current_config, snapshot.generation)?;
                self.publish_config_snapshot(current_config.clone(), revision);
                return Ok(Some(current_config));
            }
        }
        Err(crate::storage::storage_error(
            "config changed too frequently while replacing SSL configuration",
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
            if let Some(revision) = compare_and_set_config_fence_snapshot(
                &mut conn,
                &snapshot,
                &replacement_raw,
                replacement_generation,
            )
            .await?
            {
                inject_config_generation_marker(&mut replacement_config, replacement_generation)?;
                self.publish_config_snapshot(replacement_config, revision);
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
            if let Some(revision) = compare_and_set_config_fence_snapshot(
                &mut conn,
                &snapshot,
                &replacement_raw,
                replacement_generation,
            )
            .await?
            {
                let mut published_config = replacement_config.clone();
                inject_config_generation_marker(&mut published_config, replacement_generation)?;
                self.publish_config_snapshot(published_config, revision);
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
