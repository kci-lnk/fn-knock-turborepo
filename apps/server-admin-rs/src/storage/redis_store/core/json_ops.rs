use super::*;

impl Store {
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
        // redis_compat executes every pipeline in one SQLite IMMEDIATE
        // transaction, so takeover readers cannot observe a partial snapshot.
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
}
