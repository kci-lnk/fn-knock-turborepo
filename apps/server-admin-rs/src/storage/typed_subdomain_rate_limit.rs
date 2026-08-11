use std::collections::{BTreeMap, BTreeSet};

use tokio_rusqlite::rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use super::{StorageResult, redis_compat::ConnectionManager, storage_error};

pub(crate) const RATE_LIMIT_PREFIX: &str = "fn_knock:auth:subdomain_rule_rate:";

const SCHEMA_VERSION: i64 = 1;
const SCHEMA_NAME: &str = "typed_subdomain_rule_rate_limits";
const SCHEMA_SQL: &str = r#"
CREATE TABLE subdomain_rule_rate_limit_counters (
  scope TEXT NOT NULL CHECK (scope IN ('host', 'client')),
  subject_hash TEXT NOT NULL CHECK (length(subject_hash) = 64),
  counter_value INTEGER NOT NULL CHECK (counter_value > 0),
  expires_at_ms INTEGER NOT NULL CHECK (expires_at_ms >= 0),
  updated_at_ms INTEGER NOT NULL,
  PRIMARY KEY(scope, subject_hash)
);
CREATE INDEX idx_subdomain_rule_rate_limit_expiry
  ON subdomain_rule_rate_limit_counters(expires_at_ms);
"#;
const MIGRATIONS_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS typed_subdomain_rule_rate_limit_schema_migrations (
  version INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  checksum TEXT NOT NULL,
  applied_at_ms INTEGER NOT NULL
);
"#;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TypedSubdomainRateLimitCounter {
    pub(crate) scope: String,
    pub(crate) subject_hash: String,
    pub(crate) counter_value: i64,
    pub(crate) expires_at_ms: i64,
}

#[derive(Clone)]
pub(crate) struct TypedSubdomainRateLimitRepository {
    manager: ConnectionManager,
}

impl TypedSubdomainRateLimitRepository {
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
                        "SELECT name, checksum FROM typed_subdomain_rule_rate_limit_schema_migrations WHERE version = ?1",
                        [SCHEMA_VERSION],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                    )
                    .optional()?;
                match applied {
                    Some((name, stored)) if name == SCHEMA_NAME && stored == checksum => {
                        let exists = tx.query_row(
                            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'subdomain_rule_rate_limit_counters')",
                            [],
                            |row| row.get::<_, bool>(0),
                        )?;
                        if !exists {
                            return Err(storage_error(
                                "typed subdomain rate-limit migration is recorded but its table is missing",
                            ));
                        }
                    }
                    Some((name, _)) if name != SCHEMA_NAME => {
                        return Err(storage_error(
                            "typed subdomain rate-limit migration name mismatch",
                        ));
                    }
                    Some(_) => {
                        return Err(storage_error(
                            "typed subdomain rate-limit migration checksum mismatch",
                        ));
                    }
                    None => {
                        tx.execute_batch(SCHEMA_SQL)?;
                        tx.execute(
                            "INSERT INTO typed_subdomain_rule_rate_limit_schema_migrations(version, name, checksum, applied_at_ms) VALUES (?1, ?2, ?3, ?4)",
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
        let legacy = legacy_counters_tx(tx)?;
        let mut stale = typed_counter_ids_tx(tx)?;
        for counter in legacy.values() {
            stale.remove(&(counter.scope.clone(), counter.subject_hash.clone()));
            upsert_tx(tx, counter)?;
        }
        for (scope, subject_hash) in stale {
            delete_typed_tx(tx, &scope, &subject_hash)?;
        }
        Ok(())
    }

    pub(crate) fn reconcile_legacy_keys_tx(
        tx: &Transaction<'_>,
        keys: &[String],
    ) -> StorageResult<()> {
        for key in keys {
            if !key.starts_with(RATE_LIMIT_PREFIX) {
                continue;
            }
            let (scope, subject_hash) = parse_key(key).ok_or_else(|| {
                storage_error("invalid subdomain rule rate-limit compatibility key")
            })?;
            match live_legacy_counter_tx(tx, key)? {
                Some(counter) => upsert_tx(tx, &counter)?,
                None => delete_typed_tx(tx, scope, subject_hash)?,
            }
        }
        Ok(())
    }

    pub(crate) async fn verify_and_repair(&self, key: &str) -> StorageResult<bool> {
        let key = key.to_string();
        self.manager
            .call(move |conn| {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                let parsed = parse_key(&key)
                    .map(|(scope, subject_hash)| (scope.to_string(), subject_hash.to_string()));
                let raw = live_legacy_raw_tx(&tx, &key)?;
                let legacy = live_legacy_counter_tx(&tx, &key)?;
                let invalid = raw.is_some() && legacy.is_none();
                let typed = match &parsed {
                    Some((scope, subject_hash)) => typed_counter_tx(&tx, scope, subject_hash)?,
                    None => None,
                };
                let matched = !invalid && parsed.is_some() && typed == legacy;
                if !matched {
                    match legacy {
                        Some(counter) => upsert_tx(&tx, &counter)?,
                        None => {
                            if let Some((scope, subject_hash)) = parsed {
                                delete_typed_tx(&tx, &scope, &subject_hash)?;
                            }
                        }
                    }
                }
                tx.commit()?;
                Ok(matched)
            })
            .await
    }

    #[cfg(test)]
    pub(crate) async fn load(
        &self,
        key: &str,
    ) -> StorageResult<Option<TypedSubdomainRateLimitCounter>> {
        let Some((scope, subject_hash)) = parse_key(key) else {
            return Ok(None);
        };
        let scope = scope.to_string();
        let subject_hash = subject_hash.to_string();
        self.manager
            .call(move |conn| typed_counter_conn(conn, &scope, &subject_hash))
            .await
    }

    #[cfg(test)]
    pub(crate) async fn count(&self) -> StorageResult<i64> {
        self.manager
            .call(|conn| {
                conn.query_row(
                    "SELECT COUNT(*) FROM subdomain_rule_rate_limit_counters",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .map_err(Into::into)
            })
            .await
    }
}

fn parse_key(key: &str) -> Option<(&str, &str)> {
    let suffix = key.strip_prefix(RATE_LIMIT_PREFIX)?;
    let (scope, subject_hash) = suffix.split_once(':')?;
    if !matches!(scope, "host" | "client")
        || subject_hash.len() != 64
        || !subject_hash.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return None;
    }
    Some((scope, subject_hash))
}

fn live_legacy_raw_tx(tx: &Transaction<'_>, key: &str) -> StorageResult<Option<(String, i64)>> {
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

fn live_legacy_counter_tx(
    tx: &Transaction<'_>,
    key: &str,
) -> StorageResult<Option<TypedSubdomainRateLimitCounter>> {
    let Some((scope, subject_hash)) = parse_key(key) else {
        return Ok(None);
    };
    let Some((raw, expires_at_ms)) = live_legacy_raw_tx(tx, key)? else {
        return Ok(None);
    };
    let Ok(counter_value) = raw.parse::<i64>() else {
        return Ok(None);
    };
    if counter_value <= 0 {
        return Ok(None);
    }
    Ok(Some(TypedSubdomainRateLimitCounter {
        scope: scope.to_string(),
        subject_hash: subject_hash.to_string(),
        counter_value,
        expires_at_ms,
    }))
}

fn legacy_counters_tx(
    tx: &Transaction<'_>,
) -> StorageResult<BTreeMap<(String, String), TypedSubdomainRateLimitCounter>> {
    let now = crate::time_utils::now_ms();
    let mut statement = tx.prepare(
        "SELECT keys.key
         FROM kv_keys AS keys
         JOIN kv_strings AS strings ON strings.key = keys.key
         WHERE substr(keys.key, 1, ?1) = ?2
           AND keys.kind = 'string'
           AND keys.expires_at_ms IS NOT NULL
           AND keys.expires_at_ms > ?3
         ORDER BY keys.key",
    )?;
    let rows = statement.query_map(
        params![RATE_LIMIT_PREFIX.len() as i64, RATE_LIMIT_PREFIX, now],
        |row| row.get::<_, String>(0),
    )?;
    let mut counters = BTreeMap::new();
    for key in rows {
        if let Some(counter) = live_legacy_counter_tx(tx, &key?)? {
            counters.insert(
                (counter.scope.clone(), counter.subject_hash.clone()),
                counter,
            );
        }
    }
    Ok(counters)
}

fn typed_counter_tx(
    tx: &Transaction<'_>,
    scope: &str,
    subject_hash: &str,
) -> StorageResult<Option<TypedSubdomainRateLimitCounter>> {
    tx.query_row(
        "SELECT counter_value, expires_at_ms
         FROM subdomain_rule_rate_limit_counters
         WHERE scope = ?1 AND subject_hash = ?2",
        params![scope, subject_hash],
        |row| {
            Ok(TypedSubdomainRateLimitCounter {
                scope: scope.to_string(),
                subject_hash: subject_hash.to_string(),
                counter_value: row.get(0)?,
                expires_at_ms: row.get(1)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

#[cfg(test)]
fn typed_counter_conn(
    conn: &tokio_rusqlite::rusqlite::Connection,
    scope: &str,
    subject_hash: &str,
) -> StorageResult<Option<TypedSubdomainRateLimitCounter>> {
    conn.query_row(
        "SELECT counter_value, expires_at_ms
         FROM subdomain_rule_rate_limit_counters
         WHERE scope = ?1 AND subject_hash = ?2",
        params![scope, subject_hash],
        |row| {
            Ok(TypedSubdomainRateLimitCounter {
                scope: scope.to_string(),
                subject_hash: subject_hash.to_string(),
                counter_value: row.get(0)?,
                expires_at_ms: row.get(1)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

fn typed_counter_ids_tx(tx: &Transaction<'_>) -> StorageResult<BTreeSet<(String, String)>> {
    let mut statement = tx.prepare(
        "SELECT scope, subject_hash FROM subdomain_rule_rate_limit_counters ORDER BY scope, subject_hash",
    )?;
    statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<BTreeSet<_>, _>>()
        .map_err(Into::into)
}

fn upsert_tx(tx: &Transaction<'_>, counter: &TypedSubdomainRateLimitCounter) -> StorageResult<()> {
    let key = format!(
        "{RATE_LIMIT_PREFIX}{}:{}",
        counter.scope, counter.subject_hash
    );
    if parse_key(&key).is_none() || counter.counter_value <= 0 {
        return Err(storage_error("invalid typed subdomain rate-limit counter"));
    }
    tx.execute(
        "INSERT INTO subdomain_rule_rate_limit_counters(
           scope, subject_hash, counter_value, expires_at_ms, updated_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(scope, subject_hash) DO UPDATE SET
           counter_value = excluded.counter_value,
           expires_at_ms = excluded.expires_at_ms,
           updated_at_ms = excluded.updated_at_ms
         WHERE subdomain_rule_rate_limit_counters.counter_value <> excluded.counter_value
            OR subdomain_rule_rate_limit_counters.expires_at_ms <> excluded.expires_at_ms",
        params![
            counter.scope,
            counter.subject_hash,
            counter.counter_value,
            counter.expires_at_ms,
            crate::time_utils::now_ms(),
        ],
    )?;
    Ok(())
}

fn delete_typed_tx(tx: &Transaction<'_>, scope: &str, subject_hash: &str) -> StorageResult<()> {
    tx.execute(
        "DELETE FROM subdomain_rule_rate_limit_counters WHERE scope = ?1 AND subject_hash = ?2",
        params![scope, subject_hash],
    )?;
    Ok(())
}
