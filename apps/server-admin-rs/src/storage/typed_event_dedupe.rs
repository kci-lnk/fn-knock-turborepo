use std::collections::{BTreeMap, BTreeSet};

use tokio_rusqlite::rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use super::{StorageResult, redis_compat::ConnectionManager, storage_error};

pub(crate) const DEDUPE_PREFIX: &str = "fn_knock:events:dedupe:";

const SCHEMA_VERSION: i64 = 1;
const SCHEMA_NAME: &str = "typed_system_event_dedupe_leases";
const SCHEMA_SQL: &str = r#"
CREATE TABLE system_event_dedupe_leases (
  dedupe_key TEXT PRIMARY KEY CHECK (dedupe_key <> ''),
  lease_value TEXT NOT NULL CHECK (lease_value = '1'),
  expires_at_ms INTEGER NOT NULL CHECK (expires_at_ms >= 0),
  updated_at_ms INTEGER NOT NULL
);
CREATE INDEX idx_system_event_dedupe_leases_expiry
  ON system_event_dedupe_leases(expires_at_ms);
"#;
const MIGRATIONS_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS typed_system_event_dedupe_schema_migrations (
  version INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  checksum TEXT NOT NULL,
  applied_at_ms INTEGER NOT NULL
);
"#;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TypedEventDedupeLease {
    pub(crate) dedupe_key: String,
    pub(crate) expires_at_ms: i64,
}

#[derive(Clone)]
pub(crate) struct TypedEventDedupeRepository {
    manager: ConnectionManager,
}

impl TypedEventDedupeRepository {
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
                        "SELECT name, checksum FROM typed_system_event_dedupe_schema_migrations WHERE version = ?1",
                        [SCHEMA_VERSION],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                    )
                    .optional()?;
                match applied {
                    Some((name, stored)) if name == SCHEMA_NAME && stored == checksum => {
                        let exists = tx.query_row(
                            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'system_event_dedupe_leases')",
                            [],
                            |row| row.get::<_, bool>(0),
                        )?;
                        if !exists {
                            return Err(storage_error(
                                "typed event-dedupe migration is recorded but its table is missing",
                            ));
                        }
                    }
                    Some((name, _)) if name != SCHEMA_NAME => {
                        return Err(storage_error("typed event-dedupe migration name mismatch"));
                    }
                    Some(_) => {
                        return Err(storage_error(
                            "typed event-dedupe migration checksum mismatch",
                        ));
                    }
                    None => {
                        tx.execute_batch(SCHEMA_SQL)?;
                        tx.execute(
                            "INSERT INTO typed_system_event_dedupe_schema_migrations(version, name, checksum, applied_at_ms) VALUES (?1, ?2, ?3, ?4)",
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
        let legacy = legacy_leases_tx(tx)?;
        let mut stale = typed_ids_tx(tx)?;
        for lease in legacy.values() {
            stale.remove(&lease.dedupe_key);
            upsert_tx(tx, lease)?;
        }
        for dedupe_key in stale {
            delete_tx(tx, &dedupe_key)?;
        }
        Ok(())
    }

    pub(crate) fn reconcile_legacy_keys_tx(
        tx: &Transaction<'_>,
        keys: &[String],
    ) -> StorageResult<()> {
        for key in keys {
            let Some(dedupe_key) = key.strip_prefix(DEDUPE_PREFIX) else {
                continue;
            };
            if dedupe_key.is_empty() {
                return Err(storage_error("empty system-event dedupe key"));
            }
            match live_legacy_lease_tx(tx, key)? {
                Some(lease) => upsert_tx(tx, &lease)?,
                None => delete_tx(tx, dedupe_key)?,
            }
        }
        Ok(())
    }

    pub(crate) async fn verify_and_repair(&self, dedupe_key: &str) -> StorageResult<bool> {
        let dedupe_key = dedupe_key.to_string();
        self.manager
            .call(move |conn| {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                let key = format!("{DEDUPE_PREFIX}{dedupe_key}");
                let raw = live_legacy_raw_tx(&tx, &key)?;
                let legacy = live_legacy_lease_tx(&tx, &key)?;
                let invalid = raw.is_some() && legacy.is_none();
                let typed = typed_lease_tx(&tx, &dedupe_key)?;
                let matched = !invalid && typed == legacy;
                if !matched {
                    match legacy {
                        Some(lease) => upsert_tx(&tx, &lease)?,
                        None => delete_tx(&tx, &dedupe_key)?,
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
        dedupe_key: &str,
    ) -> StorageResult<Option<TypedEventDedupeLease>> {
        let dedupe_key = dedupe_key.to_string();
        self.manager
            .call(move |conn| typed_lease_conn(conn, &dedupe_key))
            .await
    }

    #[cfg(test)]
    pub(crate) async fn count(&self) -> StorageResult<i64> {
        self.manager
            .call(|conn| {
                conn.query_row(
                    "SELECT COUNT(*) FROM system_event_dedupe_leases",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .map_err(Into::into)
            })
            .await
    }
}

fn live_legacy_raw_tx(tx: &Transaction<'_>, key: &str) -> StorageResult<Option<(String, i64)>> {
    tx.query_row(
        "SELECT strings.value, keys.expires_at_ms
         FROM kv_keys AS keys
         JOIN kv_strings AS strings ON strings.key = keys.key
         WHERE keys.key = ?1
           AND keys.kind = 'string'
           AND keys.expires_at_ms IS NOT NULL
           AND keys.expires_at_ms > ?2",
        params![key, crate::time_utils::now_ms()],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
    )
    .optional()
    .map_err(Into::into)
}

fn live_legacy_lease_tx(
    tx: &Transaction<'_>,
    key: &str,
) -> StorageResult<Option<TypedEventDedupeLease>> {
    let Some(dedupe_key) = key
        .strip_prefix(DEDUPE_PREFIX)
        .filter(|key| !key.is_empty())
    else {
        return Ok(None);
    };
    let Some((value, expires_at_ms)) = live_legacy_raw_tx(tx, key)? else {
        return Ok(None);
    };
    if value != "1" {
        return Ok(None);
    }
    Ok(Some(TypedEventDedupeLease {
        dedupe_key: dedupe_key.to_string(),
        expires_at_ms,
    }))
}

fn legacy_leases_tx(
    tx: &Transaction<'_>,
) -> StorageResult<BTreeMap<String, TypedEventDedupeLease>> {
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
        params![
            DEDUPE_PREFIX.len() as i64,
            DEDUPE_PREFIX,
            crate::time_utils::now_ms()
        ],
        |row| row.get::<_, String>(0),
    )?;
    let mut leases = BTreeMap::new();
    for key in rows {
        if let Some(lease) = live_legacy_lease_tx(tx, &key?)? {
            leases.insert(lease.dedupe_key.clone(), lease);
        }
    }
    Ok(leases)
}

fn typed_lease_tx(
    tx: &Transaction<'_>,
    dedupe_key: &str,
) -> StorageResult<Option<TypedEventDedupeLease>> {
    tx.query_row(
        "SELECT expires_at_ms FROM system_event_dedupe_leases WHERE dedupe_key = ?1",
        [dedupe_key],
        |row| {
            Ok(TypedEventDedupeLease {
                dedupe_key: dedupe_key.to_string(),
                expires_at_ms: row.get(0)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

#[cfg(test)]
fn typed_lease_conn(
    conn: &tokio_rusqlite::rusqlite::Connection,
    dedupe_key: &str,
) -> StorageResult<Option<TypedEventDedupeLease>> {
    conn.query_row(
        "SELECT expires_at_ms FROM system_event_dedupe_leases WHERE dedupe_key = ?1",
        [dedupe_key],
        |row| {
            Ok(TypedEventDedupeLease {
                dedupe_key: dedupe_key.to_string(),
                expires_at_ms: row.get(0)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

fn typed_ids_tx(tx: &Transaction<'_>) -> StorageResult<BTreeSet<String>> {
    let mut statement =
        tx.prepare("SELECT dedupe_key FROM system_event_dedupe_leases ORDER BY dedupe_key")?;
    statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<BTreeSet<_>, _>>()
        .map_err(Into::into)
}

fn upsert_tx(tx: &Transaction<'_>, lease: &TypedEventDedupeLease) -> StorageResult<()> {
    if lease.dedupe_key.is_empty() {
        return Err(storage_error("empty typed system-event dedupe key"));
    }
    tx.execute(
        "INSERT INTO system_event_dedupe_leases(
           dedupe_key, lease_value, expires_at_ms, updated_at_ms
         ) VALUES (?1, '1', ?2, ?3)
         ON CONFLICT(dedupe_key) DO UPDATE SET
           lease_value = excluded.lease_value,
           expires_at_ms = excluded.expires_at_ms,
           updated_at_ms = excluded.updated_at_ms
         WHERE system_event_dedupe_leases.lease_value <> excluded.lease_value
            OR system_event_dedupe_leases.expires_at_ms <> excluded.expires_at_ms",
        params![
            lease.dedupe_key,
            lease.expires_at_ms,
            crate::time_utils::now_ms(),
        ],
    )?;
    Ok(())
}

fn delete_tx(tx: &Transaction<'_>, dedupe_key: &str) -> StorageResult<()> {
    tx.execute(
        "DELETE FROM system_event_dedupe_leases WHERE dedupe_key = ?1",
        [dedupe_key],
    )?;
    Ok(())
}
