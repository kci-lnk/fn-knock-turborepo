use super::*;

impl Store {
    pub async fn get_totps(&self) -> crate::storage::StorageResult<Vec<TotpCredential>> {
        let raw: Option<String> = {
            let mut conn = self.conn();
            conn.get("fn_knock:totps").await?
        };
        let Some(raw) = raw else {
            let old_secret: Option<String> = {
                let mut conn = self.conn();
                conn.get("fn_knock:totp_secret").await?
            };
            let Some(old_secret) = old_secret.filter(|value| !value.is_empty()) else {
                return Ok(Vec::new());
            };
            let legacy = TotpCredential {
                id: "legacy-totp-id".to_string(),
                secret: old_secret,
                comment: "默认凭据".to_string(),
                created_at: now_iso(),
                access_scopes: Value::Array(Vec::new()),
                subdomain_access: normalize_totp_subdomain_access(Value::Null),
            };
            self.set_totps(std::slice::from_ref(&legacy)).await?;
            {
                let mut conn = self.conn();
                let _: () = conn.del("fn_knock:totp_secret").await?;
            }
            let mut passkeys = self.get_passkeys().await?;
            let mut passkeys_modified = false;
            for passkey in &mut passkeys {
                let missing_totp_id = passkey
                    .get("totpId")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .is_none();
                if missing_totp_id && let Some(object) = passkey.as_object_mut() {
                    object.insert("totpId".to_string(), Value::String(legacy.id.clone()));
                    passkeys_modified = true;
                }
            }
            if passkeys_modified {
                self.set_json_value("fn_knock:passkeys", &Value::Array(passkeys))
                    .await?;
            }
            let normalized = normalize_totp_credentials(std::slice::from_ref(&legacy));
            return Ok(normalized);
        };
        if raw.trim().is_empty() {
            return Ok(Vec::new());
        }
        let value = serde_json::from_str::<Value>(&raw).unwrap_or(Value::Null);
        Ok(normalize_totp_credentials_value(&value))
    }

    pub async fn set_totps(&self, totps: &[TotpCredential]) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        let normalized = normalize_totp_credentials(totps);
        conn.set(
            "fn_knock:totps",
            serde_json::to_string(&normalized).unwrap_or_default(),
        )
        .await
    }

    pub async fn add_totp(&self, credential: TotpCredential) -> crate::storage::StorageResult<()> {
        let mut totps = self.get_totps().await?;
        if let Some(credential) = normalize_totp_credential_value(
            &serde_json::to_value(credential).unwrap_or(Value::Null),
        ) {
            totps.push(credential);
        }
        self.set_totps(&totps).await
    }

    pub async fn update_totp_comment(
        &self,
        id: &str,
        comment: String,
    ) -> crate::storage::StorageResult<Option<TotpCredential>> {
        let mut totps = self.get_totps().await?;
        let mut updated = None;
        for credential in &mut totps {
            if credential.id == id {
                credential.comment = comment.clone();
                updated = Some(credential.clone());
                break;
            }
        }
        if updated.is_some() {
            self.set_totps(&totps).await?;
        }
        Ok(updated)
    }

    pub async fn update_totp_access_scopes(
        &self,
        id: &str,
        access_scopes: Value,
    ) -> crate::storage::StorageResult<Option<TotpCredential>> {
        let mut totps = self.get_totps().await?;
        let normalized = normalize_totp_access_scopes(access_scopes);
        let mut updated = None;
        for credential in &mut totps {
            if credential.id == id {
                credential.access_scopes = normalized.clone();
                updated = Some(credential.clone());
                break;
            }
        }
        if updated.is_some() {
            self.set_totps(&totps).await?;
        }
        Ok(updated)
    }

    pub async fn update_totp_subdomain_access(
        &self,
        id: &str,
        subdomain_access: Value,
    ) -> crate::storage::StorageResult<Option<TotpCredential>> {
        let mut totps = self.get_totps().await?;
        let normalized = normalize_totp_subdomain_access(subdomain_access);
        let mut updated = None;
        for credential in &mut totps {
            if credential.id == id {
                credential.subdomain_access = normalized.clone();
                updated = Some(credential.clone());
                break;
            }
        }
        if updated.is_some() {
            self.set_totps(&totps).await?;
        }
        Ok(updated)
    }

    pub async fn delete_totp(&self, id: &str) -> crate::storage::StorageResult<bool> {
        let mut totps = self.get_totps().await?;
        let original_len = totps.len();
        totps.retain(|credential| credential.id != id);
        if totps.len() == original_len {
            return Ok(false);
        }
        self.set_totps(&totps).await?;
        let mut passkeys = self.get_passkeys().await?;
        let passkeys_original_len = passkeys.len();
        passkeys.retain(|passkey| passkey.get("totpId").and_then(Value::as_str) != Some(id));
        if passkeys.len() != passkeys_original_len {
            self.set_json_value("fn_knock:passkeys", &Value::Array(passkeys))
                .await?;
        }
        Ok(true)
    }

    pub async fn set_nonce_if_not_exists(
        &self,
        nonce: &str,
        ttl_seconds: usize,
    ) -> crate::storage::StorageResult<bool> {
        let mut conn = self.conn();
        let key = format!("fn_knock:nonce:{nonce}");
        let result: Option<String> = redis::cmd("SET")
            .arg(key)
            .arg("1")
            .arg("EX")
            .arg(ttl_seconds)
            .arg("NX")
            .query_async(&mut conn)
            .await?;
        Ok(result.as_deref() == Some("OK"))
    }

    pub async fn set_lock_if_not_exists(
        &self,
        lock_name: &str,
        ttl_seconds: usize,
    ) -> crate::storage::StorageResult<bool> {
        let mut conn = self.conn();
        let key = format!("fn_knock:lock:{lock_name}");
        let result: Option<String> = redis::cmd("SET")
            .arg(key)
            .arg("1")
            .arg("EX")
            .arg(ttl_seconds.max(1))
            .arg("NX")
            .query_async(&mut conn)
            .await?;
        Ok(result.as_deref() == Some("OK"))
    }

    pub async fn get_login_backoff_status(
        &self,
        ip: &str,
    ) -> crate::storage::StorageResult<LoginBackoffStatus> {
        let mut conn = self.conn();
        let raw: Option<String> = conn.get(login_backoff_key(ip)).await?;
        Ok(login_backoff_status_from_raw(
            ip,
            raw.as_deref(),
            crate::time_utils::now_ms(),
        ))
    }

    pub async fn register_login_backoff_failure(
        &self,
        ip: &str,
    ) -> crate::storage::StorageResult<LoginBackoffStatus> {
        let now = crate::time_utils::now_ms();
        let mut conn = self.conn();
        let result: Vec<i64> = redis::cmd("EVAL")
            .arg(LOGIN_BACKOFF_REGISTER_FAILURE_SCRIPT)
            .arg(1)
            .arg(login_backoff_key(ip))
            .arg(ip)
            .arg(now)
            .arg(LOGIN_BACKOFF_TTL_SECONDS)
            .arg(2000)
            .arg(3_600_000)
            .arg("0.4")
            .query_async(&mut conn)
            .await?;
        let attempts = result.first().copied().unwrap_or_default();
        let retry_after = result.get(1).copied().unwrap_or_default().max(0);
        let blocked_until = result.get(2).copied();
        Ok(LoginBackoffStatus {
            ip: ip.to_string(),
            attempts,
            blocked: blocked_until.is_some_and(|until| now <= until),
            retry_after: (retry_after > 0).then_some(retry_after),
            blocked_until,
        })
    }

    pub async fn reset_login_backoff(&self, ip: &str) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        conn.del(login_backoff_key(ip)).await
    }

    pub async fn list_blocked_login_backoffs(
        &self,
    ) -> crate::storage::StorageResult<Vec<LoginBackoffStatus>> {
        let mut conn = self.conn();
        let mut cursor = "0".to_string();
        let mut keys = Vec::<String>::new();
        loop {
            let (next_cursor, batch): (String, Vec<String>) = redis::cmd("SCAN")
                .arg(&cursor)
                .arg("MATCH")
                .arg(format!("{LOGIN_BACKOFF_PREFIX}*"))
                .arg("COUNT")
                .arg(100)
                .query_async(&mut conn)
                .await?;
            keys.extend(batch);
            if next_cursor == "0" {
                break;
            }
            cursor = next_cursor;
        }
        if keys.is_empty() {
            return Ok(Vec::new());
        }
        let values: Vec<Option<String>> = redis::cmd("MGET")
            .arg(keys.clone())
            .query_async(&mut conn)
            .await?;
        let now = crate::time_utils::now_ms();
        let mut items = Vec::new();
        for (key, raw) in keys.into_iter().zip(values) {
            let ip = key
                .strip_prefix(LOGIN_BACKOFF_PREFIX)
                .unwrap_or(&key)
                .to_string();
            let status = login_backoff_status_from_raw(&ip, raw.as_deref(), now);
            if status.blocked {
                items.push(status);
            }
        }
        items.sort_by(|left, right| {
            right
                .retry_after
                .unwrap_or_default()
                .cmp(&left.retry_after.unwrap_or_default())
        });
        Ok(items)
    }

    pub async fn add_session(
        &self,
        session_id: &str,
        session: &LoginSession,
        ttl_seconds: i64,
    ) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        let key = format!("fn_knock:session:{session_id}");
        conn.set_ex(
            key,
            serde_json::to_string(session).unwrap_or_default(),
            ttl_seconds as u64,
        )
        .await
    }

    pub async fn get_session(
        &self,
        session_id: &str,
    ) -> crate::storage::StorageResult<Option<LoginSession>> {
        let mut conn = self.conn();
        let key = format!("fn_knock:session:{session_id}");
        let raw: Option<String> = conn.get(key).await?;
        Ok(raw.and_then(|value| serde_json::from_str(&value).ok()))
    }

    pub async fn delete_session(&self, session_id: &str) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        let key = format!("fn_knock:session:{session_id}");
        conn.del(key).await
    }

    pub async fn list_login_sessions(
        &self,
    ) -> crate::storage::StorageResult<Vec<(String, LoginSession)>> {
        let values = self.list_session_values().await?;
        Ok(values
            .into_iter()
            .filter_map(|(id, value)| {
                serde_json::from_value::<LoginSession>(value)
                    .ok()
                    .map(|data| (id, data))
            })
            .collect())
    }

    pub async fn list_session_values(&self) -> crate::storage::StorageResult<Vec<(String, Value)>> {
        let mut conn = self.conn();
        let mut cursor = "0".to_string();
        let mut keys: Vec<String> = Vec::new();
        loop {
            let (next_cursor, batch): (String, Vec<String>) = redis::cmd("SCAN")
                .arg(&cursor)
                .arg("MATCH")
                .arg("fn_knock:session:*")
                .arg("COUNT")
                .arg(100)
                .query_async(&mut conn)
                .await?;
            keys.extend(batch);
            if next_cursor == "0" {
                break;
            }
            cursor = next_cursor;
        }
        if keys.is_empty() {
            return Ok(Vec::new());
        }
        let values: Vec<Option<String>> = redis::cmd("MGET")
            .arg(keys.clone())
            .query_async(&mut conn)
            .await?;
        let mut sessions = Vec::new();
        for (key, raw) in keys.into_iter().zip(values) {
            let Some(raw) = raw else {
                continue;
            };
            if let Ok(data) = serde_json::from_str::<Value>(&raw) {
                let id = key
                    .strip_prefix("fn_knock:session:")
                    .unwrap_or(&key)
                    .to_string();
                sessions.push((id, data));
            }
        }
        sessions.sort_by(|(_a_id, a), (_b_id, b)| {
            let at = a.get("loginTime").and_then(Value::as_str).unwrap_or("");
            let bt = b.get("loginTime").and_then(Value::as_str).unwrap_or("");
            bt.cmp(at)
        });
        Ok(sessions)
    }

    pub async fn get_session_value(
        &self,
        session_id: &str,
    ) -> crate::storage::StorageResult<Option<Value>> {
        let mut conn = self.conn();
        let key = format!("fn_knock:session:{session_id}");
        let raw: Option<String> = conn.get(key).await?;
        Ok(raw.and_then(|value| serde_json::from_str(&value).ok()))
    }

    pub async fn update_session_value(
        &self,
        session_id: &str,
        updates: Map<String, Value>,
    ) -> crate::storage::StorageResult<Option<Value>> {
        let mut conn = self.conn();
        let key = format!("fn_knock:session:{session_id}");
        let raw: Option<String> = conn.get(&key).await?;
        let ttl: i64 = conn.ttl(&key).await?;
        let Some(raw) = raw else {
            return Ok(None);
        };
        let mut current: Value = serde_json::from_str(&raw).unwrap_or_else(|_| json!({}));
        let Some(object) = current.as_object_mut() else {
            return Ok(None);
        };
        for (key, value) in updates {
            object.insert(key, value);
        }
        let serialized = serde_json::to_string(&current).unwrap_or_default();
        if ttl > 0 {
            let _: () = conn.set_ex(&key, serialized, ttl as u64).await?;
        } else {
            let _: () = conn.set(&key, serialized).await?;
        }
        Ok(Some(current))
    }

    pub async fn initialize_auth_mobility_login_session(
        &self,
        session_id: &str,
        subject_hash: &str,
        binding: &Value,
        login_event: &Value,
        summary: &Value,
        whitelist_record_id: &str,
        ttl_seconds: i64,
    ) -> crate::storage::StorageResult<()> {
        let ttl_seconds = ttl_seconds.max(1) as u64;
        let binding_key = auth_mobility_binding_key("proxy-session", subject_hash);
        let session_index_key = auth_mobility_session_index_key(session_id);
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        pipe.set_ex(
            &binding_key,
            serde_json::to_string(binding).unwrap_or_else(|_| "{}".to_string()),
            ttl_seconds,
        )
        .ignore();
        pipe.set_ex(
            auth_mobility_timeline_key(session_id),
            serde_json::to_string(&vec![login_event.clone()]).unwrap_or_else(|_| "[]".to_string()),
            ttl_seconds,
        )
        .ignore();
        pipe.set_ex(
            auth_mobility_summary_key(session_id),
            serde_json::to_string(summary).unwrap_or_else(|_| "{}".to_string()),
            ttl_seconds,
        )
        .ignore();
        pipe.sadd(&session_index_key, &binding_key).ignore();
        pipe.expire(&session_index_key, ttl_seconds as i64).ignore();
        pipe.set_ex(
            auth_mobility_whitelist_owner_key(whitelist_record_id),
            session_id,
            ttl_seconds,
        )
        .ignore();
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(())
    }

    pub async fn get_auth_mobility_binding(
        &self,
        subject_type: &str,
        subject_key: &str,
    ) -> crate::storage::StorageResult<Option<Value>> {
        let subject_hash = auth_mobility_subject_hash(subject_type, subject_key);
        self.get_json_value(&auth_mobility_binding_key(subject_type, &subject_hash))
            .await
    }

    pub async fn save_auth_mobility_binding_with_ttl(
        &self,
        subject_type: &str,
        subject_key: &str,
        binding: &Value,
        ttl_seconds: i64,
    ) -> crate::storage::StorageResult<()> {
        let subject_hash = auth_mobility_subject_hash(subject_type, subject_key);
        let binding_key = auth_mobility_binding_key(subject_type, &subject_hash);
        let mut conn = self.conn();
        conn.set_ex(
            binding_key,
            serde_json::to_string(binding).unwrap_or_else(|_| "{}".to_string()),
            ttl_seconds.max(1) as u64,
        )
        .await
    }

    pub async fn save_auth_mobility_owned_binding(
        &self,
        subject_type: &str,
        subject_key: &str,
        binding: &Value,
        owner_session_id: &str,
        binding_ttl_seconds: i64,
        session_index_ttl_seconds: Option<i64>,
    ) -> crate::storage::StorageResult<()> {
        let subject_hash = auth_mobility_subject_hash(subject_type, subject_key);
        let binding_key = auth_mobility_binding_key(subject_type, &subject_hash);
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        pipe.set_ex(
            &binding_key,
            serde_json::to_string(binding).unwrap_or_else(|_| "{}".to_string()),
            binding_ttl_seconds.max(1) as u64,
        )
        .ignore();
        pipe.sadd(
            auth_mobility_session_index_key(owner_session_id),
            &binding_key,
        )
        .ignore();
        if let Some(ttl) = session_index_ttl_seconds.filter(|ttl| *ttl > 0) {
            pipe.expire(auth_mobility_session_index_key(owner_session_id), ttl)
                .ignore();
        }
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(())
    }

    pub async fn save_auth_mobility_orphaned_binding(
        &self,
        subject_type: &str,
        subject_key: &str,
        binding: &Value,
        previous_owner_session_id: &str,
    ) -> crate::storage::StorageResult<()> {
        let subject_hash = auth_mobility_subject_hash(subject_type, subject_key);
        let binding_key = auth_mobility_binding_key(subject_type, &subject_hash);
        let ttl: i64 = {
            let mut conn = self.conn();
            conn.ttl(&binding_key).await?
        };
        self.set_json_value_preserve_ttl(&binding_key, binding, ttl)
            .await?;
        let mut conn = self.conn();
        conn.srem(
            auth_mobility_session_index_key(previous_owner_session_id),
            binding_key,
        )
        .await
    }

    pub async fn save_auth_mobility_binding_keep_ttl(
        &self,
        subject_type: &str,
        subject_key: &str,
        binding: &Value,
    ) -> crate::storage::StorageResult<()> {
        let subject_hash = auth_mobility_subject_hash(subject_type, subject_key);
        let binding_key = auth_mobility_binding_key(subject_type, &subject_hash);
        let ttl: i64 = {
            let mut conn = self.conn();
            conn.ttl(&binding_key).await?
        };
        self.set_json_value_preserve_ttl(&binding_key, binding, ttl)
            .await
    }

    pub async fn add_auth_mobility_session_binding(
        &self,
        owner_session_id: &str,
        subject_type: &str,
        subject_key: &str,
        session_index_ttl_seconds: Option<i64>,
    ) -> crate::storage::StorageResult<()> {
        let subject_hash = auth_mobility_subject_hash(subject_type, subject_key);
        let binding_key = auth_mobility_binding_key(subject_type, &subject_hash);
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        pipe.sadd(
            auth_mobility_session_index_key(owner_session_id),
            binding_key,
        )
        .ignore();
        if let Some(ttl) = session_index_ttl_seconds.filter(|ttl| *ttl > 0) {
            pipe.expire(auth_mobility_session_index_key(owner_session_id), ttl)
                .ignore();
        }
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(())
    }

    pub async fn list_auth_mobility_session_binding_keys(
        &self,
        session_id: &str,
    ) -> crate::storage::StorageResult<Vec<String>> {
        let mut conn = self.conn();
        conn.smembers(auth_mobility_session_index_key(session_id))
            .await
    }

    pub async fn remove_auth_mobility_session_bindings(
        &self,
        session_id: &str,
        binding_keys: &[String],
    ) -> crate::storage::StorageResult<()> {
        if binding_keys.is_empty() {
            return Ok(());
        }
        let mut conn = self.conn();
        conn.srem(auth_mobility_session_index_key(session_id), binding_keys)
            .await
    }

    pub async fn append_auth_mobility_timeline_event(
        &self,
        session_id: &str,
        event: &Value,
        seed_login_event: Option<&Value>,
        fallback_ttl_seconds: Option<i64>,
    ) -> crate::storage::StorageResult<()> {
        let timeline_key = auth_mobility_timeline_key(session_id);
        let summary_key = auth_mobility_summary_key(session_id);
        let (current_events, timeline_ttl) = self.get_json_value_with_ttl(&timeline_key).await?;
        let (stored_summary, summary_ttl) = self.get_json_value_with_ttl(&summary_key).await?;
        let events = current_events
            .and_then(|value| value.as_array().cloned())
            .unwrap_or_default();
        let mut next_events = if events.is_empty() {
            seed_login_event
                .cloned()
                .into_iter()
                .chain(std::iter::once(event.clone()))
                .collect::<Vec<_>>()
        } else {
            events
                .iter()
                .cloned()
                .chain(std::iter::once(event.clone()))
                .collect::<Vec<_>>()
        };
        limit_mobility_timeline_events(&mut next_events, 100);
        let next_summary =
            next_mobility_summary_from_event(&events, stored_summary, event, seed_login_event);
        let ttl = [
            timeline_ttl,
            summary_ttl,
            fallback_ttl_seconds.unwrap_or_default(),
        ]
        .into_iter()
        .filter(|value| *value > 0)
        .max()
        .unwrap_or_default();
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        let serialized_events =
            serde_json::to_string(&next_events).unwrap_or_else(|_| "[]".to_string());
        let serialized_summary =
            serde_json::to_string(&next_summary).unwrap_or_else(|_| "{}".to_string());
        if ttl > 0 {
            pipe.set_ex(&timeline_key, serialized_events, ttl as u64)
                .ignore();
            pipe.set_ex(&summary_key, serialized_summary, ttl as u64)
                .ignore();
        } else {
            pipe.set(&timeline_key, serialized_events).ignore();
            pipe.set(&summary_key, serialized_summary).ignore();
        }
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(())
    }

    pub async fn get_auth_mobility_active_ip_detail(
        &self,
        session_id: &str,
        ip: &str,
    ) -> crate::storage::StorageResult<Option<Value>> {
        self.hget_json_value(&auth_mobility_active_ip_details_key(session_id), ip)
            .await
    }

    pub async fn list_auth_mobility_active_ip_details(
        &self,
        session_id: &str,
    ) -> crate::storage::StorageResult<Vec<Value>> {
        let mut conn = self.conn();
        let raws: Vec<String> = conn
            .hvals(auth_mobility_active_ip_details_key(session_id))
            .await?;
        Ok(raws
            .into_iter()
            .filter_map(|raw| serde_json::from_str::<Value>(&raw).ok())
            .collect())
    }

    pub async fn clear_auth_mobility_active_ip_session(
        &self,
        session_id: &str,
    ) -> crate::storage::StorageResult<()> {
        let keys = vec![
            auth_mobility_active_ip_zset_key(session_id),
            auth_mobility_active_ip_details_key(session_id),
        ];
        self.delete_keys(&keys).await
    }

    pub async fn save_auth_mobility_active_ip_detail(
        &self,
        session_id: &str,
        ip: &str,
        score: i64,
        detail: &Value,
        ttl_seconds: i64,
    ) -> crate::storage::StorageResult<()> {
        let ttl_seconds = ttl_seconds.max(1);
        let zset_key = auth_mobility_active_ip_zset_key(session_id);
        let detail_key = auth_mobility_active_ip_details_key(session_id);
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        pipe.zadd(&zset_key, ip, score).ignore();
        pipe.hset(
            &detail_key,
            ip,
            serde_json::to_string(detail).unwrap_or_else(|_| "{}".to_string()),
        )
        .ignore();
        pipe.expire(&zset_key, ttl_seconds).ignore();
        pipe.expire(&detail_key, ttl_seconds).ignore();
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(())
    }

    pub async fn list_auth_mobility_recent_active_ip_details(
        &self,
        session_id: &str,
        since: i64,
    ) -> crate::storage::StorageResult<Vec<Value>> {
        let mut conn = self.conn();
        let ips: Vec<String> = redis::cmd("ZRANGEBYSCORE")
            .arg(auth_mobility_active_ip_zset_key(session_id))
            .arg(since)
            .arg("+inf")
            .query_async(&mut conn)
            .await?;
        if ips.is_empty() {
            return Ok(Vec::new());
        }
        let raws: Vec<Option<String>> = redis::cmd("HMGET")
            .arg(auth_mobility_active_ip_details_key(session_id))
            .arg(ips)
            .query_async(&mut conn)
            .await?;
        Ok(raws
            .into_iter()
            .filter_map(|raw| raw.and_then(|value| serde_json::from_str::<Value>(&value).ok()))
            .collect())
    }

    pub async fn collect_auth_mobility_prune_targets(
        &self,
        session_id: &str,
        cutoff: i64,
        keep_ip: Option<&str>,
        max_entries: usize,
    ) -> crate::storage::StorageResult<Vec<String>> {
        let zset_key = auth_mobility_active_ip_zset_key(session_id);
        let mut conn = self.conn();
        let expired_ips: Vec<String> = redis::cmd("ZRANGEBYSCORE")
            .arg(&zset_key)
            .arg(0)
            .arg(cutoff)
            .query_async(&mut conn)
            .await?;
        let all_ips: Vec<String> = conn.zrange(&zset_key, 0, -1).await?;
        let mut remove_ips = expired_ips.into_iter().collect::<BTreeSet<_>>();
        let remaining_ips = all_ips
            .into_iter()
            .filter(|ip| !remove_ips.contains(ip))
            .collect::<Vec<_>>();
        let overflow_count = remaining_ips.len().saturating_sub(max_entries);
        if overflow_count > 0 {
            let keep_ip = keep_ip.unwrap_or("");
            for ip in remaining_ips
                .into_iter()
                .filter(|ip| ip != keep_ip)
                .take(overflow_count)
            {
                remove_ips.insert(ip);
            }
        }
        Ok(remove_ips.into_iter().collect())
    }

    pub async fn remove_auth_mobility_active_ips(
        &self,
        session_id: &str,
        ips: &[String],
    ) -> crate::storage::StorageResult<Vec<Value>> {
        if ips.is_empty() {
            return Ok(Vec::new());
        }
        let detail_key = auth_mobility_active_ip_details_key(session_id);
        let mut conn = self.conn();
        let raws: Vec<Option<String>> = redis::cmd("HMGET")
            .arg(&detail_key)
            .arg(ips)
            .query_async(&mut conn)
            .await?;
        let details = raws
            .into_iter()
            .filter_map(|raw| raw.and_then(|value| serde_json::from_str::<Value>(&value).ok()))
            .collect::<Vec<_>>();
        let mut pipe = redis::pipe();
        pipe.zrem(auth_mobility_active_ip_zset_key(session_id), ips)
            .ignore();
        pipe.hdel(detail_key, ips).ignore();
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(details)
    }

    pub async fn expire_auth_mobility_active_ip_keys(
        &self,
        session_id: &str,
        ttl_seconds: i64,
    ) -> crate::storage::StorageResult<()> {
        let ttl_seconds = ttl_seconds.max(1);
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        pipe.expire(auth_mobility_active_ip_zset_key(session_id), ttl_seconds)
            .ignore();
        pipe.expire(auth_mobility_active_ip_details_key(session_id), ttl_seconds)
            .ignore();
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(())
    }

    pub async fn set_auth_mobility_whitelist_owner(
        &self,
        whitelist_record_id: &str,
        session_id: &str,
        ttl_seconds: i64,
    ) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        conn.set_ex(
            auth_mobility_whitelist_owner_key(whitelist_record_id),
            session_id,
            ttl_seconds.max(1) as u64,
        )
        .await
    }

    pub async fn destroy_auth_mobility_session(
        &self,
        session_id: &str,
    ) -> crate::storage::StorageResult<Vec<String>> {
        let session_index_key = auth_mobility_session_index_key(session_id);
        let active_details_key = auth_mobility_active_ip_details_key(session_id);
        let active_zset_key = auth_mobility_active_ip_zset_key(session_id);
        let timeline_key = auth_mobility_timeline_key(session_id);
        let summary_key = auth_mobility_summary_key(session_id);
        let proxy_hash = auth_mobility_subject_hash("proxy-session", session_id);
        let proxy_binding_key = auth_mobility_binding_key("proxy-session", &proxy_hash);

        let mut conn = self.conn();
        let mut binding_keys: Vec<String> = conn.smembers(&session_index_key).await?;
        if !binding_keys.iter().any(|key| key == &proxy_binding_key) {
            binding_keys.push(proxy_binding_key.clone());
        }
        let binding_raws: Vec<Option<String>> = if binding_keys.is_empty() {
            Vec::new()
        } else {
            redis::cmd("MGET")
                .arg(binding_keys.clone())
                .query_async(&mut conn)
                .await?
        };
        let active_details: HashMap<String, String> = conn.hgetall(&active_details_key).await?;
        let mut whitelist_ids = BTreeSet::new();
        for raw in binding_raws.into_iter().flatten() {
            if let Ok(value) = serde_json::from_str::<Value>(&raw)
                && let Some(id) = value.get("whitelistRecordId").and_then(Value::as_str)
                && !id.trim().is_empty()
            {
                whitelist_ids.insert(id.to_string());
            }
        }
        for raw in active_details.into_values() {
            if let Ok(value) = serde_json::from_str::<Value>(&raw)
                && let Some(id) = value.get("whitelistRecordId").and_then(Value::as_str)
                && !id.trim().is_empty()
            {
                whitelist_ids.insert(id.to_string());
            }
        }

        let mut delete_keys = vec![
            active_details_key,
            active_zset_key,
            timeline_key,
            summary_key,
            session_index_key,
        ];
        delete_keys.extend(binding_keys);
        delete_keys.extend(
            whitelist_ids
                .iter()
                .map(|id| auth_mobility_whitelist_owner_key(id)),
        );
        let mut pipe = redis::pipe();
        pipe.del(delete_keys).ignore();
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(whitelist_ids.into_iter().collect())
    }

    pub async fn get_passkeys(&self) -> crate::storage::StorageResult<Vec<Value>> {
        let mut conn = self.conn();
        let raw: Option<String> = conn.get("fn_knock:passkeys").await?;
        let Some(raw) = raw else {
            return Ok(Vec::new());
        };
        Ok(serde_json::from_str::<Vec<Value>>(&raw).unwrap_or_default())
    }

    pub async fn delete_passkey(&self, id: &str) -> crate::storage::StorageResult<bool> {
        let mut passkeys = self.get_passkeys().await?;
        let original_len = passkeys.len();
        passkeys.retain(|passkey| passkey.get("id").and_then(Value::as_str) != Some(id));
        if passkeys.len() == original_len {
            return Ok(false);
        }
        self.set_json_value("fn_knock:passkeys", &Value::Array(passkeys))
            .await?;
        Ok(true)
    }

    pub async fn add_passkey(&self, passkey: &Value) -> crate::storage::StorageResult<()> {
        let mut passkeys = self.get_passkeys().await?;
        passkeys.push(passkey.clone());
        self.set_json_value("fn_knock:passkeys", &Value::Array(passkeys))
            .await
    }

    pub async fn update_passkey_counter(
        &self,
        id: &str,
        counter: u32,
        last_used_at: &str,
        backup_eligible: Option<bool>,
        backup_state: Option<bool>,
    ) -> crate::storage::StorageResult<bool> {
        let mut passkeys = self.get_passkeys().await?;
        let mut found = false;
        for passkey in &mut passkeys {
            if passkey.get("id").and_then(Value::as_str) != Some(id) {
                continue;
            }
            if let Some(object) = passkey.as_object_mut() {
                object.insert("counter".to_string(), json!(counter));
                object.insert("lastUsedAt".to_string(), json!(last_used_at));
                if let Some(value) = backup_eligible {
                    object.insert("backupEligible".to_string(), json!(value));
                    object.insert("backup_eligible".to_string(), json!(value));
                }
                if let Some(value) = backup_state {
                    object.insert("backupState".to_string(), json!(value));
                    object.insert("backup_state".to_string(), json!(value));
                }
                found = true;
            }
        }
        if !found {
            return Ok(false);
        }
        self.set_json_value("fn_knock:passkeys", &Value::Array(passkeys))
            .await?;
        Ok(true)
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
        let mut conn = self.conn();
        let result: i64 = redis::cmd("EVAL")
            .arg(
                r#"
local value = redis.call("GET", KEYS[1])
if value == ARGV[1] then
  redis.call("DEL", KEYS[1])
  return 1
end
return 0
"#,
            )
            .arg(1)
            .arg(format!("fn_knock:passkey:challenge:{challenge}"))
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

    pub async fn is_passkey_bind_token_valid(
        &self,
        token: &str,
    ) -> crate::storage::StorageResult<bool> {
        let mut conn = self.conn();
        let value: Option<String> = conn.get(format!("fn_knock:passkey:bind:{token}")).await?;
        Ok(value.is_some())
    }

    pub async fn consume_passkey_bind_token(
        &self,
        token: &str,
    ) -> crate::storage::StorageResult<Option<String>> {
        let mut conn = self.conn();
        redis::cmd("EVAL")
            .arg(
                r#"
local value = redis.call("GET", KEYS[1])
if not value then
  return nil
end
redis.call("DEL", KEYS[1])
return value
"#,
            )
            .arg(1)
            .arg(format!("fn_knock:passkey:bind:{token}"))
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
        self.consume_json_value(&format!("fn_knock:passkey:state:{challenge}"))
            .await
    }
}
