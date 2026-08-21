use super::*;

impl Store {
    pub async fn set_nonce_if_not_exists(
        &self,
        nonce: &str,
        ttl_seconds: usize,
    ) -> crate::storage::StorageResult<bool> {
        let matched = self.typed.typed_hmac_nonce.verify_and_repair(nonce).await?;
        if matched {
            if self.typed_hmac_nonce_shadow.mark_healthy() {
                tracing::info!("typed HMAC nonce shadow comparison recovered");
            }
        } else {
            self.typed_hmac_nonce_shadow.mark_mismatch();
            tracing::warn!(
                "typed HMAC nonce shadow differed from the compatibility replay guard and was repaired"
            );
        }
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
        self.verify_login_backoff_shadow(ip).await?;
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
            self.verify_all_login_backoff_shadows().await?;
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
        self.verify_all_login_backoff_shadows().await?;
        items.sort_by(|left, right| {
            right
                .retry_after
                .unwrap_or_default()
                .cmp(&left.retry_after.unwrap_or_default())
        });
        Ok(items)
    }

    pub(super) async fn verify_login_backoff_shadow(
        &self,
        ip: &str,
    ) -> crate::storage::StorageResult<()> {
        let matched = self.typed.typed_login_backoff.verify_and_repair(ip).await?;
        self.observe_login_backoff_shadow_comparison(matched, Some(ip));
        Ok(())
    }

    pub(super) async fn verify_all_login_backoff_shadows(
        &self,
    ) -> crate::storage::StorageResult<()> {
        let matched = self
            .typed
            .typed_login_backoff
            .verify_and_repair_all()
            .await?;
        self.observe_login_backoff_shadow_comparison(matched, None);
        Ok(())
    }

    pub(super) fn observe_login_backoff_shadow_comparison(&self, matched: bool, ip: Option<&str>) {
        if matched {
            if self.typed_login_backoff_shadow.mark_healthy() {
                tracing::info!("typed login-backoff shadow comparison recovered");
            }
            return;
        }
        self.typed_login_backoff_shadow.mark_mismatch();
        if let Some(ip) = ip {
            tracing::warn!(
                ip,
                "typed login-backoff shadow differed from the compatibility record and was repaired"
            );
        } else {
            tracing::warn!(
                "typed login-backoff shadow set differed from the compatibility keyspace and was repaired"
            );
        }
    }
}
