use std::collections::BTreeMap;

use tokio_rusqlite::rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use super::{StorageResult, redis_compat::ConnectionManager, storage_error};

pub(crate) const LOGIN_BACKOFF_PREFIX: &str = "fn_knock:login_backoff:";

const SCHEMA_VERSION: i64 = 1;
const SCHEMA_NAME: &str = "typed_login_backoff_attempts";
const SCHEMA_SQL: &str = r#"
CREATE TABLE login_backoff_attempts (
  ip TEXT PRIMARY KEY CHECK (ip <> ''),
  state_json TEXT NOT NULL,
  expires_at_ms INTEGER NOT NULL CHECK (expires_at_ms >= 0),
  updated_at_ms INTEGER NOT NULL
);
CREATE INDEX idx_login_backoff_attempts_expiry
  ON login_backoff_attempts(expires_at_ms);
"#;
const MIGRATIONS_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS typed_login_backoff_schema_migrations (
  version INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  checksum TEXT NOT NULL,
  applied_at_ms INTEGER NOT NULL
);
"#;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TypedLoginBackoffAttempt {
    pub(crate) ip: String,
    pub(crate) state_json: String,
    pub(crate) expires_at_ms: i64,
}

#[derive(Clone)]
pub(crate) struct TypedLoginBackoffRepository {
    manager: ConnectionManager,
}

impl TypedLoginBackoffRepository {
    pub(crate) fn new(manager: ConnectionManager) -> Self {
        Self { manager }
    }

    pub(crate) async fn initialize(&self) -> StorageResult<()> {
        self.manager
            .call(|conn| {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                tx.execute_batch(MIGRATIONS_SQL)?;
                let checksum = crate::crypto_utils::sha256_hex_bytes(SCHEMA_SQL);
                let applied = tx
                    .query_row(
                        "SELECT name, checksum FROM typed_login_backoff_schema_migrations WHERE version = ?1",
                        [SCHEMA_VERSION],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                    )
                    .optional()?;
                match applied {
                    Some((name, stored)) if name == SCHEMA_NAME && stored == checksum => {
                        let exists = tx.query_row(
                            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'login_backoff_attempts')",
                            [],
                            |row| row.get::<_, bool>(0),
                        )?;
                        if !exists {
                            return Err(storage_error(
                                "typed login-backoff migration is recorded but its table is missing",
                            ));
                        }
                    }
                    Some((name, _)) if name != SCHEMA_NAME => {
                        return Err(storage_error("typed login-backoff migration name mismatch"));
                    }
                    Some(_) => {
                        return Err(storage_error(
                            "typed login-backoff migration checksum mismatch",
                        ));
                    }
                    None => {
                        tx.execute_batch(SCHEMA_SQL)?;
                        tx.execute(
                            "INSERT INTO typed_login_backoff_schema_migrations(version, name, checksum, applied_at_ms) VALUES (?1, ?2, ?3, ?4)",
                            params![
                                SCHEMA_VERSION,
                                SCHEMA_NAME,
                                checksum,
                                crate::time_utils::now_ms(),
                            ],
                        )?;
                    }
                }
                tx.commit()?;
                Ok(())
            })
            .await
    }

    pub(crate) async fn rebuild_from_legacy(&self) -> StorageResult<()> {
        self.manager
            .call(|conn| {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                Self::rebuild_from_legacy_tx(&tx)?;
                tx.commit()?;
                Ok(())
            })
            .await
    }

    pub(crate) fn rebuild_from_legacy_tx(tx: &Transaction<'_>) -> StorageResult<()> {
        let legacy = legacy_attempts_tx(tx)?;
        let mut stale = {
            let mut statement = tx.prepare("SELECT ip FROM login_backoff_attempts")?;
            statement
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?
        };
        for attempt in legacy.values() {
            stale.retain(|ip| ip != &attempt.ip);
            upsert_tx(tx, attempt)?;
        }
        for ip in stale {
            tx.execute("DELETE FROM login_backoff_attempts WHERE ip = ?1", [ip])?;
        }
        Ok(())
    }

    pub(crate) fn reconcile_legacy_keys_tx(
        tx: &Transaction<'_>,
        keys: &[String],
    ) -> StorageResult<()> {
        for key in keys {
            let Some(ip) = key.strip_prefix(LOGIN_BACKOFF_PREFIX) else {
                continue;
            };
            if ip.is_empty() {
                return Self::rebuild_from_legacy_tx(tx);
            }
            match live_legacy_attempt_tx(tx, ip)? {
                Some(attempt) => upsert_tx(tx, &attempt)?,
                None => {
                    tx.execute("DELETE FROM login_backoff_attempts WHERE ip = ?1", [ip])?;
                }
            }
        }
        Ok(())
    }

    pub(crate) async fn verify_and_repair(&self, ip: &str) -> StorageResult<bool> {
        let ip = ip.to_string();
        self.manager
            .call(move |conn| {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                let legacy = live_legacy_attempt_tx(&tx, &ip)?;
                let legacy_invalid = live_legacy_raw_tx(&tx, &ip)?
                    .is_some_and(|(state_json, _)| !valid_state_json(&state_json));
                let typed = typed_attempt_tx(&tx, &ip)?;
                let matched = !legacy_invalid && typed == legacy;
                if !matched {
                    match legacy {
                        Some(attempt) => upsert_tx(&tx, &attempt)?,
                        None => {
                            tx.execute("DELETE FROM login_backoff_attempts WHERE ip = ?1", [&ip])?;
                        }
                    }
                }
                tx.commit()?;
                Ok(matched)
            })
            .await
    }

    pub(crate) async fn verify_and_repair_all(&self) -> StorageResult<bool> {
        self.manager
            .call(|conn| {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                let legacy = legacy_attempts_tx(&tx)?;
                let typed = typed_attempts_tx(&tx)?;
                let matched = invalid_legacy_attempt_count_tx(&tx)? == 0 && typed == legacy;
                if !matched {
                    Self::rebuild_from_legacy_tx(&tx)?;
                }
                tx.commit()?;
                Ok(matched)
            })
            .await
    }

    #[cfg(test)]
    pub(crate) async fn load(&self, ip: &str) -> StorageResult<Option<TypedLoginBackoffAttempt>> {
        let ip = ip.to_string();
        self.manager
            .call(move |conn| typed_attempt_conn(conn, &ip))
            .await
    }

    #[cfg(test)]
    pub(crate) async fn count(&self) -> StorageResult<i64> {
        self.manager
            .call(|conn| {
                conn.query_row("SELECT COUNT(*) FROM login_backoff_attempts", [], |row| {
                    row.get::<_, i64>(0)
                })
                .map_err(Into::into)
            })
            .await
    }
}

fn valid_state_json(raw: &str) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
        return false;
    };
    let Some(object) = value.as_object() else {
        return false;
    };
    object.get("ip").is_some_and(serde_json::Value::is_string)
        && object
            .get("attempts")
            .and_then(serde_json::Value::as_i64)
            .is_some_and(|attempts| attempts >= 0)
        && object
            .get("blockedUntil")
            .and_then(serde_json::Value::as_i64)
            .is_some()
}

fn live_legacy_attempt_tx(
    tx: &Transaction<'_>,
    ip: &str,
) -> StorageResult<Option<TypedLoginBackoffAttempt>> {
    let Some((state_json, expires_at_ms)) = live_legacy_raw_tx(tx, ip)? else {
        return Ok(None);
    };
    if !valid_state_json(&state_json) {
        return Ok(None);
    }
    Ok(Some(TypedLoginBackoffAttempt {
        ip: ip.to_string(),
        state_json,
        expires_at_ms,
    }))
}

fn live_legacy_raw_tx(tx: &Transaction<'_>, ip: &str) -> StorageResult<Option<(String, i64)>> {
    let key = format!("{LOGIN_BACKOFF_PREFIX}{ip}");
    let now = crate::time_utils::now_ms();
    tx.query_row(
        "SELECT strings.value, keys.expires_at_ms
             FROM kv_keys AS keys
             JOIN kv_strings AS strings ON strings.key = keys.key
             WHERE keys.key = ?1
               AND keys.kind = 'string'
               AND keys.expires_at_ms IS NOT NULL
               AND keys.expires_at_ms > ?2",
        params![key, now],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
    )
    .optional()
    .map_err(Into::into)
}

fn legacy_attempts_tx(
    tx: &Transaction<'_>,
) -> StorageResult<BTreeMap<String, TypedLoginBackoffAttempt>> {
    let pattern = format!("{LOGIN_BACKOFF_PREFIX}%");
    let now = crate::time_utils::now_ms();
    let mut statement = tx.prepare(
        "SELECT keys.key, strings.value, keys.expires_at_ms
         FROM kv_keys AS keys
         JOIN kv_strings AS strings ON strings.key = keys.key
         WHERE keys.key LIKE ?1
           AND keys.kind = 'string'
           AND keys.expires_at_ms IS NOT NULL
           AND keys.expires_at_ms > ?2
         ORDER BY keys.key",
    )?;
    let rows = statement.query_map(params![pattern, now], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;
    let mut attempts = BTreeMap::new();
    for row in rows {
        let (key, state_json, expires_at_ms) = row?;
        let Some(ip) = key.strip_prefix(LOGIN_BACKOFF_PREFIX) else {
            continue;
        };
        if ip.is_empty() || !valid_state_json(&state_json) {
            continue;
        }
        attempts.insert(
            ip.to_string(),
            TypedLoginBackoffAttempt {
                ip: ip.to_string(),
                state_json,
                expires_at_ms,
            },
        );
    }
    Ok(attempts)
}

fn typed_attempt_tx(
    tx: &Transaction<'_>,
    ip: &str,
) -> StorageResult<Option<TypedLoginBackoffAttempt>> {
    tx.query_row(
        "SELECT state_json, expires_at_ms FROM login_backoff_attempts WHERE ip = ?1",
        [ip],
        |row| {
            Ok(TypedLoginBackoffAttempt {
                ip: ip.to_string(),
                state_json: row.get(0)?,
                expires_at_ms: row.get(1)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

#[cfg(test)]
fn typed_attempt_conn(
    conn: &tokio_rusqlite::rusqlite::Connection,
    ip: &str,
) -> StorageResult<Option<TypedLoginBackoffAttempt>> {
    conn.query_row(
        "SELECT state_json, expires_at_ms FROM login_backoff_attempts WHERE ip = ?1",
        [ip],
        |row| {
            Ok(TypedLoginBackoffAttempt {
                ip: ip.to_string(),
                state_json: row.get(0)?,
                expires_at_ms: row.get(1)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

fn typed_attempts_tx(
    tx: &Transaction<'_>,
) -> StorageResult<BTreeMap<String, TypedLoginBackoffAttempt>> {
    let mut statement =
        tx.prepare("SELECT ip, state_json, expires_at_ms FROM login_backoff_attempts ORDER BY ip")?;
    let rows = statement.query_map([], |row| {
        let ip = row.get::<_, String>(0)?;
        Ok(TypedLoginBackoffAttempt {
            ip,
            state_json: row.get(1)?,
            expires_at_ms: row.get(2)?,
        })
    })?;
    let mut attempts = BTreeMap::new();
    for attempt in rows {
        let attempt = attempt?;
        attempts.insert(attempt.ip.clone(), attempt);
    }
    Ok(attempts)
}

fn invalid_legacy_attempt_count_tx(tx: &Transaction<'_>) -> StorageResult<i64> {
    let pattern = format!("{LOGIN_BACKOFF_PREFIX}%");
    let now = crate::time_utils::now_ms();
    let mut statement = tx.prepare(
        "SELECT strings.value
         FROM kv_keys AS keys
         JOIN kv_strings AS strings ON strings.key = keys.key
         WHERE keys.key LIKE ?1
           AND keys.kind = 'string'
           AND keys.expires_at_ms IS NOT NULL
           AND keys.expires_at_ms > ?2",
    )?;
    let rows = statement.query_map(params![pattern, now], |row| row.get::<_, String>(0))?;
    let mut invalid = 0_i64;
    for raw in rows {
        if !valid_state_json(&raw?) {
            invalid += 1;
        }
    }
    Ok(invalid)
}

fn upsert_tx(tx: &Transaction<'_>, attempt: &TypedLoginBackoffAttempt) -> StorageResult<()> {
    if attempt.ip.is_empty() || !valid_state_json(&attempt.state_json) {
        return Err(storage_error("invalid typed login-backoff attempt"));
    }
    tx.execute(
        "INSERT INTO login_backoff_attempts(ip, state_json, expires_at_ms, updated_at_ms)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(ip) DO UPDATE SET
           state_json = excluded.state_json,
           expires_at_ms = excluded.expires_at_ms,
           updated_at_ms = excluded.updated_at_ms
         WHERE login_backoff_attempts.state_json <> excluded.state_json
            OR login_backoff_attempts.expires_at_ms <> excluded.expires_at_ms",
        params![
            attempt.ip,
            attempt.state_json,
            attempt.expires_at_ms,
            crate::time_utils::now_ms(),
        ],
    )?;
    Ok(())
}
