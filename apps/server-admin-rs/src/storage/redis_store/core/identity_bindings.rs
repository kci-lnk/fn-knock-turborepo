use super::*;

impl Store {
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
}
