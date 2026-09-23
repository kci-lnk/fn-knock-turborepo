use super::*;

impl Store {
    /// Standalone presentation metadata only; session/grant authority must use
    /// its dedicated shadow-validation APIs instead.
    pub(crate) async fn get_auth_presentation_values(
        &self,
        keys: &[&str],
    ) -> crate::storage::StorageResult<Vec<Option<Value>>> {
        Ok(self
            .manager
            .get_auth_live_strings(keys.iter().map(|key| (*key).to_string()).collect())
            .await?
            .into_iter()
            .map(|raw| raw.and_then(|raw| serde_json::from_str(&raw).ok()))
            .collect())
    }

    pub(crate) async fn get_passkeys_for_authorization(
        &self,
    ) -> crate::storage::StorageResult<Vec<Value>> {
        Ok(self
            .manager
            .get_auth_live_strings(vec!["fn_knock:passkeys".into()])
            .await?
            .pop()
            .flatten()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default())
    }

    pub(crate) async fn get_auth_login_mode_for_authorization(
        &self,
    ) -> crate::storage::StorageResult<crate::auth::mode::AuthLoginMode> {
        let raw = self
            .manager
            .get_auth_live_strings(vec!["fn_knock:auth:login_mode:v1".into()])
            .await?
            .pop()
            .flatten();
        Ok(normalize_auth_login_mode(raw.as_deref()))
    }

    pub(crate) async fn get_ip_location_cache_for_authorization(
        &self,
        ip: &str,
    ) -> crate::storage::StorageResult<Option<Value>> {
        let key = super::super::discovery::ip_location_cache_key(ip);
        Ok(self
            .manager
            .get_auth_live_strings(vec![key])
            .await?
            .pop()
            .flatten()
            .and_then(|raw| serde_json::from_str(&raw).ok()))
    }

    /// Project candidate discovery on the SQLite worker before returning to the
    /// async caller, so unmatched sessions need not survive later awaits.
    /// Callers must reload session authority before authorizing; this snapshot
    /// never replaces the compatibility session key.
    pub(crate) async fn map_auth_session_ip_candidates<T: Send + 'static>(
        &self,
        mobility_window_seconds: Option<i64>,
        project: impl FnOnce(Vec<(String, LoginSession, Vec<Value>)>) -> T + Send + 'static,
    ) -> crate::storage::StorageResult<T> {
        use tokio_rusqlite::rusqlite::params;
        self.manager.call_auth_read(move |conn| {
            let tx = conn.transaction()?;
            let now = crate::time_utils::now_ms();
            let mut sessions = {
                let mut statement = tx.prepare_cached(
                    "SELECT substr(strings.key, length('fn_knock:session:') + 1), strings.value
                     FROM kv_strings AS strings JOIN kv_keys AS keys ON keys.key = strings.key
                     WHERE strings.key GLOB 'fn_knock:session:*' AND keys.kind = 'string'
                       AND (keys.expires_at_ms IS NULL OR keys.expires_at_ms > ?1)
                     ORDER BY strings.key",
                )?;
                let rows = statement.query_map([now], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?;
                let mut sessions = Vec::new();
                for row in rows {
                    let (id, raw) = row?;
                    if let Ok(session) = serde_json::from_str::<LoginSession>(&raw) {
                        sessions.push((id, session, Vec::new()));
                    }
                }
                sessions
            };
            if let Some(window) = mobility_window_seconds {
                let indices = sessions.iter().enumerate()
                    .map(|(index, (id, _, _))| (id.clone(), index)).collect::<HashMap<_, _>>();
                let mut statement = tx.prepare_cached(
                    "SELECT substr(sessions.key, length('fn_knock:session:') + 1), details.value
                     FROM kv_keys AS sessions
                     JOIN kv_keys AS scores_key
                       ON scores_key.key = 'fn_knock:auth_mobility:active_ips:' || substr(sessions.key, length('fn_knock:session:') + 1)
                     JOIN kv_zset AS scores ON scores.key = scores_key.key
                     JOIN kv_keys AS details_key
                       ON details_key.key = 'fn_knock:auth_mobility:active_ip_details:' || substr(sessions.key, length('fn_knock:session:') + 1)
                     JOIN kv_hash AS details ON details.key = details_key.key AND details.field = scores.member
                     WHERE sessions.key GLOB 'fn_knock:session:*' AND sessions.kind = 'string'
                       AND scores_key.kind = 'zset' AND details_key.kind = 'hash'
                       AND scores.score >= ?1
                       AND (sessions.expires_at_ms IS NULL OR sessions.expires_at_ms > ?2)
                       AND (scores_key.expires_at_ms IS NULL OR scores_key.expires_at_ms > ?2)
                       AND (details_key.expires_at_ms IS NULL OR details_key.expires_at_ms > ?2)",
                )?;
                let rows = statement.query_map(params![now.div_euclid(1000) - window + 1, now], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?;
                for row in rows {
                    let (id, raw) = row?;
                    if let Some(&index) = indices.get(&id)
                        && let Ok(value) = serde_json::from_str(&raw)
                    {
                        sessions[index].2.push(value);
                    }
                }
            }
            tx.commit()?;
            // Match list_login_sessions' stable newest-first owner order.
            sessions.sort_by(|(_, a, _), (_, b, _)| b.login_time.cmp(&a.login_time));
            Ok(project(sessions))
        }).await
    }

    pub(crate) async fn get_auth_mobility_recent_ip_details_for_authorization(
        &self,
        session_id: &str,
        window_seconds: i64,
    ) -> crate::storage::StorageResult<Vec<Value>> {
        use tokio_rusqlite::rusqlite::params;
        let scores_key = auth_mobility_active_ip_zset_key(session_id);
        let details_key = auth_mobility_active_ip_details_key(session_id);
        self.manager
            .call_auth_read(move |conn| {
                let now = crate::time_utils::now_ms();
                let mut statement = conn.prepare_cached(
                    "SELECT details.value FROM kv_zset AS scores
                 JOIN kv_keys AS scores_key ON scores_key.key = scores.key
                 JOIN kv_hash AS details ON details.key = ?2 AND details.field = scores.member
                 JOIN kv_keys AS details_key ON details_key.key = details.key
                 WHERE scores.key = ?1 AND scores.score >= ?3
                   AND scores_key.kind = 'zset' AND details_key.kind = 'hash'
                   AND (scores_key.expires_at_ms IS NULL OR scores_key.expires_at_ms > ?4)
                   AND (details_key.expires_at_ms IS NULL OR details_key.expires_at_ms > ?4)",
                )?;
                let rows = statement.query_map(
                    params![
                        scores_key,
                        details_key,
                        now.div_euclid(1000) - window_seconds + 1,
                        now
                    ],
                    |row| row.get::<_, String>(0),
                )?;
                let mut values = Vec::new();
                for raw in rows {
                    if let Ok(value) = serde_json::from_str(&raw?) {
                        values.push(value);
                    }
                }
                Ok(values)
            })
            .await
    }

    pub(crate) async fn get_auth_accounts_raw_for_authorization(
        &self,
    ) -> crate::storage::StorageResult<Option<String>> {
        Ok(self
            .manager
            .get_auth_live_strings(vec!["fn_knock:auth:accounts:v1".into()])
            .await?
            .pop()
            .flatten()
            .filter(|raw| !raw.trim().is_empty()))
    }

    pub(crate) async fn get_totps_raw_for_authorization(
        &self,
    ) -> crate::storage::StorageResult<Option<String>> {
        let mut values = self
            .manager
            .get_auth_live_strings(vec!["fn_knock:totps".into(), "fn_knock:totp_secret".into()])
            .await?
            .into_iter();
        if let Some(raw) = values.next().flatten() {
            return Ok(Some(raw));
        }
        if values
            .next()
            .flatten()
            .is_some_and(|secret| !secret.is_empty())
        {
            // Keep the existing authoritative migration and passkey association
            // writes. Serialize only this rare migration result, not normal reads.
            return Ok(Some(serde_json::to_string(&self.get_totps().await?)?));
        }
        Ok(None)
    }

    pub(crate) fn authorization_totp_id(value: &Value) -> Option<String> {
        let object = value.as_object()?;
        let id = object
            .get("id")
            .map(js_string)
            .unwrap_or_default()
            .trim()
            .to_string();
        let secret = object.get("secret").map(js_string).unwrap_or_default();
        (!id.is_empty() && !secret.trim().is_empty()).then_some(id)
    }

    pub(crate) fn authorization_totp_from_value_at(
        value: Value,
        id: Option<&str>,
        normalized_at: &str,
    ) -> Option<TotpCredential> {
        if id.is_some_and(|id| value.get("id").map(js_string).unwrap_or_default().trim() != id) {
            return None;
        }
        let mut credential = normalize_totp_credential_value(&value)?;
        if value
            .get("createdAt")
            .map(js_string)
            .unwrap_or_default()
            .trim()
            .is_empty()
        {
            credential.created_at = normalized_at.to_string();
        }
        Some(credential)
    }

    pub(crate) fn authorization_account_from_value_at(
        value: Value,
        id: Option<&str>,
        normalized_at: &str,
    ) -> Option<AuthAccount> {
        if id.is_some_and(|id| value.get("id").and_then(Value::as_str).map(str::trim) != Some(id)) {
            return None;
        }
        let mut account = serde_json::from_value::<AuthAccount>(value).ok()?;
        if account.created_at.trim().is_empty() {
            account.created_at = normalized_at.to_string();
        }
        let account = normalize_auth_account(account);
        (!account.id.is_empty() && !account.username.is_empty()).then_some(account)
    }

    #[cfg(test)]
    pub(crate) async fn get_auth_accounts_for_authorization(
        &self,
    ) -> crate::storage::StorageResult<Vec<AuthAccount>> {
        let Some(raw) = self.get_auth_accounts_raw_for_authorization().await? else {
            return Ok(Vec::new());
        };
        Ok(normalize_auth_accounts_value(&serde_json::from_str(&raw)?))
    }

    #[cfg(test)]
    pub(crate) async fn get_totps_for_authorization(
        &self,
    ) -> crate::storage::StorageResult<Vec<TotpCredential>> {
        let Some(raw) = self.get_totps_raw_for_authorization().await? else {
            return Ok(Vec::new());
        };
        Ok(normalize_totp_credentials_value(
            &serde_json::from_str(&raw).unwrap_or(Value::Null),
        ))
    }

    pub(crate) async fn get_auth_mobility_binding_for_authorization(
        &self,
        subject_type: &str,
        subject_key: &str,
    ) -> crate::storage::StorageResult<Option<Value>> {
        let hash = auth_mobility_subject_hash(subject_type, subject_key);
        let key = auth_mobility_binding_key(subject_type, &hash);
        Ok(self
            .manager
            .get_auth_live_strings(vec![key])
            .await?
            .pop()
            .flatten()
            .and_then(|raw| serde_json::from_str(&raw).ok()))
    }

    pub(crate) async fn get_auth_mobility_active_ip_detail_for_authorization(
        &self,
        session_id: &str,
        ip: &str,
    ) -> crate::storage::StorageResult<Option<Value>> {
        Ok(self
            .manager
            .get_auth_live_hash_field(
                auth_mobility_active_ip_details_key(session_id),
                ip.to_string(),
            )
            .await?
            .and_then(|raw| serde_json::from_str(&raw).ok()))
    }
}
