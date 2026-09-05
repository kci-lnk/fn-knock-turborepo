use super::*;

pub(super) const DOCKER_ADMIN_PASSWORD_KEY: &str = "fn_knock:docker_admin:password:v1";
pub(super) const DOCKER_ADMIN_SESSION_PREFIX: &str = "fn_knock:docker_admin:session:v1:";
pub(super) const DOCKER_ADMIN_LOGIN_BACKOFF_PREFIX: &str =
    "fn_knock:docker_admin:login_backoff:v1:";
pub(super) const DOCKER_ADMIN_LOGIN_BACKOFF_TTL_SECONDS: i64 = 3_600;
pub(super) const DOCKER_ADMIN_LOGIN_BACKOFF_BASE_DELAY_MS: i64 = 2_000;
pub(super) const DOCKER_ADMIN_LOGIN_BACKOFF_MAX_DELAY_MS: i64 = 15 * 60 * 1_000;
pub(super) const DOCKER_ADMIN_REGISTER_LOGIN_FAILURE_SCRIPT: &str = r#"
-- fn-knock:eval:docker-admin-login-backoff:v1
local key = KEYS[1]
local ip = ARGV[1]
local now = tonumber(ARGV[2])
local nowIso = ARGV[3]
local ttlSeconds = tonumber(ARGV[4])
local baseDelay = tonumber(ARGV[5])
local maxDelay = tonumber(ARGV[6])

local attempts = 0
local raw = redis.call('GET', key)
if raw then
  local ok, decoded = pcall(cjson.decode, raw)
  if ok and type(decoded) == 'table' and tonumber(decoded.attempts) then
    attempts = tonumber(decoded.attempts)
  end
end
attempts = attempts + 1
local exponent = math.min(math.max(attempts - 1, 0), 30)
local backoffMs = math.min(baseDelay * math.pow(2, exponent), maxDelay)
local blockedUntil = now + backoffMs
redis.call('SET', key, cjson.encode({
  ip = ip,
  attempts = attempts,
  last_attempt_at = nowIso,
  blocked_until = blockedUntil,
}), 'EX', ttlSeconds)
return { attempts, math.floor((backoffMs + 999) / 1000), blockedUntil }
"#;

impl Store {
    pub async fn docker_admin_password(
        &self,
    ) -> crate::storage::StorageResult<Option<DockerAdminPasswordRecord>> {
        let mut conn = self.conn();
        let raw: Option<String> = conn.get(DOCKER_ADMIN_PASSWORD_KEY).await?;
        Ok(raw.and_then(|value| serde_json::from_str(&value).ok()))
    }

    pub async fn set_docker_admin_password(
        &self,
        record: &DockerAdminPasswordRecord,
    ) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        conn.set(
            DOCKER_ADMIN_PASSWORD_KEY,
            serde_json::to_string(record).unwrap_or_default(),
        )
        .await
    }

    pub async fn install_docker_admin_password_if_absent_and_clear_security_state(
        &self,
        record: &DockerAdminPasswordRecord,
    ) -> crate::storage::StorageResult<bool> {
        let password_json = serde_json::to_string(record)?;
        self.manager
            .install_password_and_delete_security_state_atomically(
                DOCKER_ADMIN_PASSWORD_KEY,
                &password_json,
                DOCKER_ADMIN_SESSION_PREFIX,
                DOCKER_ADMIN_LOGIN_BACKOFF_PREFIX,
            )
            .await
    }

    pub async fn docker_admin_session(
        &self,
        session_id: &str,
    ) -> crate::storage::StorageResult<Option<DockerAdminSessionRecord>> {
        let mut conn = self.conn();
        let raw: Option<String> = conn
            .get(format!("{DOCKER_ADMIN_SESSION_PREFIX}{session_id}"))
            .await?;
        let matched = self
            .typed
            .typed_docker_admin
            .verify_and_repair_session(session_id)
            .await?;
        self.observe_docker_admin_shadow_comparison(matched, "session");
        Ok(raw.and_then(|value| serde_json::from_str(&value).ok()))
    }

    pub async fn set_docker_admin_session(
        &self,
        record: &DockerAdminSessionRecord,
    ) -> crate::storage::StorageResult<()> {
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

    pub async fn refresh_docker_admin_session_if_exists(
        &self,
        record: &DockerAdminSessionRecord,
    ) -> crate::storage::StorageResult<bool> {
        let mut conn = self.conn();
        let ttl = crate::time_utils::parse_iso_ms(&record.expires_at)
            .map(|expires_ms| ((expires_ms - crate::time_utils::now_ms()).max(1000) / 1000) as u64)
            .unwrap_or(record.ttl_seconds.max(1) as u64);
        let result: Option<String> = redis::cmd("SET")
            .arg(format!("{DOCKER_ADMIN_SESSION_PREFIX}{}", record.id))
            .arg(serde_json::to_string(record).unwrap_or_default())
            .arg("EX")
            .arg(ttl)
            .arg("XX")
            .query_async(&mut conn)
            .await?;
        Ok(result.is_some())
    }

    pub async fn delete_docker_admin_session(
        &self,
        session_id: &str,
    ) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        conn.del(format!("{DOCKER_ADMIN_SESSION_PREFIX}{session_id}"))
            .await
    }

    pub async fn docker_admin_login_attempt(
        &self,
        ip: &str,
    ) -> crate::storage::StorageResult<Option<LoginAttemptRecord>> {
        let mut conn = self.conn();
        let raw: Option<String> = conn
            .get(format!("{DOCKER_ADMIN_LOGIN_BACKOFF_PREFIX}{ip}"))
            .await?;
        let matched = self
            .typed
            .typed_docker_admin
            .verify_and_repair_login_backoff(ip)
            .await?;
        self.observe_docker_admin_shadow_comparison(matched, "login_backoff");
        Ok(raw.and_then(|value| serde_json::from_str(&value).ok()))
    }

    pub async fn register_docker_admin_login_failure(
        &self,
        ip: &str,
    ) -> crate::storage::StorageResult<(i64, i64)> {
        let now_ms = crate::time_utils::now_ms();
        let now_iso = crate::time_utils::now_iso();
        let mut conn = self.conn();
        let result: Vec<i64> = redis::cmd("EVAL")
            .arg(DOCKER_ADMIN_REGISTER_LOGIN_FAILURE_SCRIPT)
            .arg(1)
            .arg(format!("{DOCKER_ADMIN_LOGIN_BACKOFF_PREFIX}{ip}"))
            .arg(ip)
            .arg(now_ms)
            .arg(now_iso)
            .arg(DOCKER_ADMIN_LOGIN_BACKOFF_TTL_SECONDS)
            .arg(DOCKER_ADMIN_LOGIN_BACKOFF_BASE_DELAY_MS)
            .arg(DOCKER_ADMIN_LOGIN_BACKOFF_MAX_DELAY_MS)
            .query_async(&mut conn)
            .await?;
        let retry_after = result.get(1).copied().unwrap_or(1).max(1);
        let blocked_until = result
            .get(2)
            .copied()
            .unwrap_or_else(|| now_ms.saturating_add(2_000));
        Ok((retry_after, blocked_until))
    }

    pub async fn reset_docker_admin_login_attempt(
        &self,
        ip: &str,
    ) -> crate::storage::StorageResult<()> {
        let mut conn = self.conn();
        conn.del(format!("{DOCKER_ADMIN_LOGIN_BACKOFF_PREFIX}{ip}"))
            .await
    }

    pub async fn reset_docker_admin_password_state(
        &self,
    ) -> crate::storage::StorageResult<DockerAdminResetSummary> {
        let (password_cleared, sessions_cleared, login_failures_cleared) = self
            .manager
            .delete_security_state_atomically(
                DOCKER_ADMIN_PASSWORD_KEY,
                DOCKER_ADMIN_SESSION_PREFIX,
                DOCKER_ADMIN_LOGIN_BACKOFF_PREFIX,
            )
            .await?;

        Ok(DockerAdminResetSummary {
            password_cleared,
            sessions_cleared,
            login_failures_cleared,
        })
    }

    fn observe_docker_admin_shadow_comparison(&self, matched: bool, kind: &'static str) {
        if matched {
            if self.typed_docker_admin_shadow.mark_healthy() {
                tracing::info!("typed Docker-admin security shadow comparison recovered");
            }
            return;
        }
        self.typed_docker_admin_shadow.mark_mismatch();
        tracing::warn!(
            kind,
            "typed Docker-admin security shadow differed from the compatibility record and was repaired"
        );
    }
}
