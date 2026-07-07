use super::*;

impl RedisStore {
    pub async fn docker_admin_password(
        &self,
    ) -> redis::RedisResult<Option<DockerAdminPasswordRecord>> {
        let mut conn = self.conn();
        let raw: Option<String> = conn.get(DOCKER_ADMIN_PASSWORD_KEY).await?;
        Ok(raw.and_then(|value| serde_json::from_str(&value).ok()))
    }

    pub async fn set_docker_admin_password(
        &self,
        record: &DockerAdminPasswordRecord,
    ) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        conn.set(
            DOCKER_ADMIN_PASSWORD_KEY,
            serde_json::to_string(record).unwrap_or_default(),
        )
        .await
    }

    pub async fn docker_admin_session(
        &self,
        session_id: &str,
    ) -> redis::RedisResult<Option<DockerAdminSessionRecord>> {
        let mut conn = self.conn();
        let raw: Option<String> = conn
            .get(format!("{DOCKER_ADMIN_SESSION_PREFIX}{session_id}"))
            .await?;
        Ok(raw.and_then(|value| serde_json::from_str(&value).ok()))
    }

    pub async fn set_docker_admin_session(
        &self,
        record: &DockerAdminSessionRecord,
    ) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        let ttl = crate::time_utils::parse_iso_ms(&record.expires_at)
            .map(|expires_ms| ((expires_ms - crate::time_utils::now_ms()).max(1000) / 1000) as u64)
            .unwrap_or(record.ttl_seconds.max(1) as u64);
        conn.set_ex(
            format!("{DOCKER_ADMIN_SESSION_PREFIX}{}", record.id),
            serde_json::to_string(record).unwrap_or_default(),
            ttl,
        )
        .await
    }

    pub async fn delete_docker_admin_session(&self, session_id: &str) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        conn.del(format!("{DOCKER_ADMIN_SESSION_PREFIX}{session_id}"))
            .await
    }

    pub async fn docker_admin_login_attempt(
        &self,
        ip: &str,
    ) -> redis::RedisResult<Option<LoginAttemptRecord>> {
        let mut conn = self.conn();
        let raw: Option<String> = conn
            .get(format!("{DOCKER_ADMIN_LOGIN_BACKOFF_PREFIX}{ip}"))
            .await?;
        Ok(raw.and_then(|value| serde_json::from_str(&value).ok()))
    }

    pub async fn set_docker_admin_login_attempt(
        &self,
        record: &LoginAttemptRecord,
    ) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        conn.set_ex(
            format!("{DOCKER_ADMIN_LOGIN_BACKOFF_PREFIX}{}", record.ip),
            serde_json::to_string(record).unwrap_or_default(),
            3600,
        )
        .await
    }

    pub async fn reset_docker_admin_login_attempt(&self, ip: &str) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        conn.del(format!("{DOCKER_ADMIN_LOGIN_BACKOFF_PREFIX}{ip}"))
            .await
    }

    pub async fn clear_docker_admin_sessions(&self) -> redis::RedisResult<usize> {
        self.clear_keys_by_prefix(DOCKER_ADMIN_SESSION_PREFIX, 200)
            .await
    }

    pub async fn clear_docker_admin_login_failures(&self) -> redis::RedisResult<usize> {
        self.clear_keys_by_prefix(DOCKER_ADMIN_LOGIN_BACKOFF_PREFIX, 200)
            .await
    }

    pub async fn reset_docker_admin_password_state(
        &self,
    ) -> redis::RedisResult<DockerAdminResetSummary> {
        let password_deleted = self.delete_key_count(DOCKER_ADMIN_PASSWORD_KEY).await?;
        let sessions_cleared = self.clear_docker_admin_sessions().await?;
        let login_failures_cleared = self.clear_docker_admin_login_failures().await?;

        Ok(DockerAdminResetSummary {
            password_cleared: password_deleted > 0,
            sessions_cleared,
            login_failures_cleared,
        })
    }
}
