use super::*;

impl Store {
    pub async fn get_passkeys(&self) -> crate::storage::StorageResult<Vec<Value>> {
        let mut conn = self.conn();
        let raw: Option<String> = conn.get("fn_knock:passkeys").await?;
        let Some(raw) = raw else {
            return Ok(Vec::new());
        };
        Ok(serde_json::from_str::<Vec<Value>>(&raw).unwrap_or_default())
    }

    pub async fn delete_passkey(&self, id: &str) -> crate::storage::StorageResult<bool> {
        let mut conn = self.conn();
        let key = "fn_knock:passkeys";
        loop {
            let Some(expected_raw) = conn.get::<_, Option<String>>(key).await? else {
                return Ok(false);
            };
            let mut passkeys = serde_json::from_str::<Vec<Value>>(&expected_raw)?;
            let original_len = passkeys.len();
            passkeys.retain(|passkey| passkey.get("id").and_then(Value::as_str) != Some(id));
            if passkeys.len() == original_len {
                return Ok(false);
            }
            let next_raw = serde_json::to_string(&passkeys)?;
            match compare_and_set_json(&mut conn, key, &expected_raw, &next_raw).await? {
                1 => return Ok(true),
                0 => continue,
                -1 => return Ok(false),
                _ => {
                    return Err(crate::storage::storage_error(
                        "unexpected passkey deletion CAS result",
                    ));
                }
            }
        }
    }

    pub async fn add_passkey(&self, passkey: &Value) -> crate::storage::StorageResult<bool> {
        let mut conn = self.conn();
        let key = "fn_knock:passkeys";
        loop {
            let expected_raw = match conn.get::<_, Option<String>>(key).await? {
                Some(value) => value,
                None => {
                    let _: Option<String> = redis::cmd("SET")
                        .arg(key)
                        .arg("[]")
                        .arg("NX")
                        .query_async(&mut conn)
                        .await?;
                    continue;
                }
            };
            let mut passkeys = serde_json::from_str::<Vec<Value>>(&expected_raw)?;
            let id = passkey.get("id").and_then(Value::as_str);
            if id.is_some_and(|id| {
                passkeys
                    .iter()
                    .any(|stored| stored.get("id").and_then(Value::as_str) == Some(id))
            }) {
                return Ok(false);
            }
            passkeys.push(passkey.clone());
            let next_raw = serde_json::to_string(&passkeys)?;
            match compare_and_set_json(&mut conn, key, &expected_raw, &next_raw).await? {
                1 => return Ok(true),
                0 | -1 => continue,
                _ => {
                    return Err(crate::storage::storage_error(
                        "unexpected passkey insertion CAS result",
                    ));
                }
            }
        }
    }

    pub async fn update_passkey_counter(
        &self,
        id: &str,
        counter: u32,
        last_used_at: &str,
        backup_eligible: Option<bool>,
        backup_state: Option<bool>,
    ) -> crate::storage::StorageResult<bool> {
        let mut conn = self.conn();
        let key = "fn_knock:passkeys";
        loop {
            let Some(expected_raw) = conn.get::<_, Option<String>>(key).await? else {
                return Ok(false);
            };
            let mut passkeys = serde_json::from_str::<Vec<Value>>(&expected_raw)?;
            let mut found = false;
            for passkey in &mut passkeys {
                if passkey.get("id").and_then(Value::as_str) != Some(id) {
                    continue;
                }
                let Some(object) = passkey.as_object_mut() else {
                    continue;
                };
                let stored_counter = object
                    .get("counter")
                    .and_then(Value::as_u64)
                    .and_then(|value| u32::try_from(value).ok())
                    .unwrap_or(0);
                let credential_counter = object
                    .get("webauthnCredential")
                    .and_then(Value::as_object)
                    .and_then(|credential| credential.get("counter"))
                    .and_then(Value::as_u64)
                    .and_then(|value| u32::try_from(value).ok())
                    .unwrap_or(0);
                let next_counter = counter.max(stored_counter).max(credential_counter);
                let existing_backup_eligible = object
                    .get("backupEligible")
                    .or_else(|| object.get("backup_eligible"))
                    .and_then(Value::as_bool)
                    .or_else(|| {
                        object
                            .get("webauthnCredential")
                            .and_then(Value::as_object)
                            .and_then(|credential| credential.get("backup_eligible"))
                            .and_then(Value::as_bool)
                    })
                    .unwrap_or(false);

                object.insert("counter".to_string(), json!(next_counter));
                object.insert("lastUsedAt".to_string(), json!(last_used_at));
                if let Some(value) = backup_eligible {
                    let value = value || existing_backup_eligible;
                    object.insert("backupEligible".to_string(), json!(value));
                    object.insert("backup_eligible".to_string(), json!(value));
                }
                if let Some(value) = backup_state {
                    object.insert("backupState".to_string(), json!(value));
                    object.insert("backup_state".to_string(), json!(value));
                }
                if let Some(credential) = object
                    .get_mut("webauthnCredential")
                    .and_then(Value::as_object_mut)
                {
                    credential.insert("counter".to_string(), json!(next_counter));
                    if let Some(value) = backup_eligible {
                        credential.insert(
                            "backup_eligible".to_string(),
                            json!(value || existing_backup_eligible),
                        );
                    }
                    if let Some(value) = backup_state {
                        credential.insert("backup_state".to_string(), json!(value));
                    }
                }
                found = true;
                break;
            }
            if !found {
                return Ok(false);
            }

            let next_raw = serde_json::to_string(&passkeys)?;
            let result = compare_and_set_json(&mut conn, key, &expected_raw, &next_raw).await?;
            match result {
                1 => return Ok(true),
                0 => continue,
                -1 => return Ok(false),
                _ => {
                    return Err(crate::storage::storage_error(
                        "unexpected passkey update CAS result",
                    ));
                }
            }
        }
    }

    pub async fn set_passkey_challenge(
        &self,
        challenge: &str,
        challenge_type: &str,
        ttl_seconds: usize,
    ) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        conn.set_ex(
            format!("fn_knock:passkey:challenge:{challenge}"),
            challenge_type,
            ttl_seconds.max(1) as u64,
        )
        .await
    }

    pub async fn consume_passkey_challenge(
        &self,
        challenge: &str,
        challenge_type: &str,
    ) -> crate::storage::StorageResult<bool> {
        let key = format!("fn_knock:passkey:challenge:{challenge}");
        self.verify_passkey_runtime_shadow_key(&key).await?;
        let mut conn = self.conn();
        let result: i64 = redis::cmd("EVAL")
            .arg(
                r#"
-- fn-knock:eval:delete-if-value:v1
local value = redis.call("GET", KEYS[1])
if value == ARGV[1] then
  redis.call("DEL", KEYS[1])
  return 1
end
return 0
"#,
            )
            .arg(1)
            .arg(key)
            .arg(challenge_type)
            .query_async(&mut conn)
            .await?;
        Ok(result == 1)
    }

    pub async fn create_passkey_bind_token(
        &self,
        totp_id: &str,
        ttl_seconds: usize,
    ) -> crate::storage::StorageResult<String> {
        let token = hex::encode(rand::random::<[u8; 24]>());
        let mut conn = self.conn();
        let _: () = conn
            .set_ex(
                format!("fn_knock:passkey:bind:{token}"),
                totp_id,
                ttl_seconds.max(1) as u64,
            )
            .await?;
        Ok(token)
    }

    pub async fn get_passkey_bind_token_totp_id(
        &self,
        token: &str,
    ) -> crate::storage::StorageResult<Option<String>> {
        let key = format!("fn_knock:passkey:bind:{token}");
        self.verify_passkey_runtime_shadow_key(&key).await?;
        let mut conn = self.conn();
        conn.get(key).await
    }

    pub async fn consume_passkey_bind_token(
        &self,
        token: &str,
    ) -> crate::storage::StorageResult<Option<String>> {
        let key = format!("fn_knock:passkey:bind:{token}");
        self.verify_passkey_runtime_shadow_key(&key).await?;
        let mut conn = self.conn();
        redis::cmd("EVAL")
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
            .await
    }

    pub async fn set_passkey_state(
        &self,
        challenge: &str,
        state: &Value,
        ttl_seconds: usize,
    ) -> crate::storage::StorageResult<()> {
        self.set_json_value_ex(
            &format!("fn_knock:passkey:state:{challenge}"),
            state,
            ttl_seconds,
        )
        .await
    }

    pub async fn consume_passkey_state(
        &self,
        challenge: &str,
    ) -> crate::storage::StorageResult<Option<Value>> {
        let key = format!("fn_knock:passkey:state:{challenge}");
        self.verify_passkey_runtime_shadow_key(&key).await?;
        self.consume_json_value(&key).await
    }
}
