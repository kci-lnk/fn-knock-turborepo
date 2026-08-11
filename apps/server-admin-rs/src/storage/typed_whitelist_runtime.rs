use tokio_rusqlite::rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use super::{StorageResult, redis_compat::ConnectionManager, storage_error};

pub(crate) const OWNER_PREFIX: &str = "fn_knock:whitelist:auto_owner:";

const SCHEMA_VERSION: i64 = 1;
const SCHEMA_NAME: &str = "typed_whitelist_owner_runtime";
const SCHEMA_SQL: &str = r#"
CREATE TABLE whitelist_auto_owner_mappings (
  owner_digest TEXT PRIMARY KEY CHECK (length(owner_digest) = 64),
  whitelist_record_id TEXT NOT NULL CHECK (whitelist_record_id <> ''),
  expires_at_ms INTEGER,
  updated_at_ms INTEGER NOT NULL
);
CREATE INDEX idx_whitelist_auto_owner_mapping_expiry
  ON whitelist_auto_owner_mappings(expires_at_ms) WHERE expires_at_ms IS NOT NULL;
CREATE TABLE whitelist_auto_owner_locks (
  owner_digest TEXT PRIMARY KEY CHECK (length(owner_digest) = 64),
  lock_digest TEXT NOT NULL CHECK (length(lock_digest) = 64),
  expires_at_ms INTEGER NOT NULL CHECK (expires_at_ms >= 0),
  updated_at_ms INTEGER NOT NULL
);
CREATE INDEX idx_whitelist_auto_owner_lock_expiry ON whitelist_auto_owner_locks(expires_at_ms);
"#;
const MIGRATIONS_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS typed_whitelist_runtime_schema_migrations (
  version INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  checksum TEXT NOT NULL,
  applied_at_ms INTEGER NOT NULL
);
"#;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TypedWhitelistOwnerRuntime {
    Mapping {
        owner_digest: String,
        record_id: String,
        expires_at_ms: Option<i64>,
    },
    Lock {
        owner_digest: String,
        lock_digest: String,
        expires_at_ms: i64,
    },
}

#[derive(Clone)]
pub(crate) struct TypedWhitelistRuntimeRepository {
    manager: ConnectionManager,
}

impl TypedWhitelistRuntimeRepository {
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
                        "SELECT name, checksum FROM typed_whitelist_runtime_schema_migrations WHERE version = ?1",
                        [SCHEMA_VERSION],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                    )
                    .optional()?;
                match applied {
                    Some((name, stored)) if name == SCHEMA_NAME && stored == checksum => {
                        let count = tx.query_row(
                            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN ('whitelist_auto_owner_mappings', 'whitelist_auto_owner_locks')",
                            [],
                            |row| row.get::<_, i64>(0),
                        )?;
                        if count != 2 {
                            return Err(storage_error(
                                "typed whitelist runtime migration is recorded but its tables are missing",
                            ));
                        }
                    }
                    Some((name, _)) if name != SCHEMA_NAME => {
                        return Err(storage_error("typed whitelist runtime migration name mismatch"));
                    }
                    Some(_) => {
                        return Err(storage_error("typed whitelist runtime migration checksum mismatch"));
                    }
                    None => {
                        tx.execute_batch(SCHEMA_SQL)?;
                        tx.execute(
                            "INSERT INTO typed_whitelist_runtime_schema_migrations(version, name, checksum, applied_at_ms) VALUES (?1, ?2, ?3, ?4)",
                            params![SCHEMA_VERSION, SCHEMA_NAME, checksum, crate::time_utils::now_ms()],
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
        let records = legacy_records_tx(tx)?;
        tx.execute("DELETE FROM whitelist_auto_owner_locks", [])?;
        tx.execute("DELETE FROM whitelist_auto_owner_mappings", [])?;
        for record in records {
            upsert_tx(tx, &record)?;
        }
        Ok(())
    }

    pub(crate) fn reconcile_legacy_keys_tx(
        tx: &Transaction<'_>,
        keys: &[String],
    ) -> StorageResult<()> {
        for key in keys {
            let Some((kind, owner_digest)) = parse_key(key) else {
                continue;
            };
            match live_legacy_record_tx(tx, key)? {
                Some(record) => upsert_tx(tx, &record)?,
                None => delete_tx(tx, kind, owner_digest)?,
            }
        }
        Ok(())
    }

    pub(crate) async fn verify_and_repair_key(&self, key: &str) -> StorageResult<bool> {
        let key = key.to_string();
        self.manager
            .call(move |conn| {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                let (kind, owner_digest) = parse_key(&key)
                    .ok_or_else(|| storage_error("invalid whitelist owner runtime key"))?;
                let raw = live_legacy_raw_tx(&tx, &key)?;
                let legacy = live_legacy_record_tx(&tx, &key)?;
                let invalid = raw.is_some() && legacy.is_none();
                let typed = typed_record_tx(&tx, kind, owner_digest)?;
                let matched = !invalid && typed == legacy;
                if !matched {
                    match legacy {
                        Some(record) => upsert_tx(&tx, &record)?,
                        None => delete_tx(&tx, kind, owner_digest)?,
                    }
                }
                tx.commit()?;
                Ok(matched)
            })
            .await
    }

    #[cfg(test)]
    pub(crate) async fn load_key(
        &self,
        key: &str,
    ) -> StorageResult<Option<TypedWhitelistOwnerRuntime>> {
        let (kind, digest) =
            parse_key(key).ok_or_else(|| storage_error("invalid whitelist owner runtime key"))?;
        let kind = kind.to_string();
        let digest = digest.to_string();
        self.manager
            .call(move |conn| typed_record_conn(conn, &kind, &digest))
            .await
    }

    #[cfg(test)]
    pub(crate) async fn counts(&self) -> StorageResult<(i64, i64)> {
        self.manager
            .call(|conn| {
                Ok((
                    conn.query_row(
                        "SELECT COUNT(*) FROM whitelist_auto_owner_mappings",
                        [],
                        |row| row.get(0),
                    )?,
                    conn.query_row(
                        "SELECT COUNT(*) FROM whitelist_auto_owner_locks",
                        [],
                        |row| row.get(0),
                    )?,
                ))
            })
            .await
    }
}

pub(crate) fn owns_key(key: &str) -> bool {
    parse_key(key).is_some()
}

fn parse_key(key: &str) -> Option<(&'static str, &str)> {
    let suffix = key.strip_prefix(OWNER_PREFIX)?;
    if let Some(owner_digest) = suffix.strip_suffix(":lock") {
        valid_digest(owner_digest).then_some(("lock", owner_digest))
    } else {
        valid_digest(suffix).then_some(("mapping", suffix))
    }
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn live_legacy_raw_tx(
    tx: &Transaction<'_>,
    key: &str,
) -> StorageResult<Option<(String, Option<i64>)>> {
    tx.query_row(
        "SELECT strings.value, keys.expires_at_ms
         FROM kv_keys AS keys JOIN kv_strings AS strings ON strings.key = keys.key
         WHERE keys.key = ?1 AND keys.kind = 'string'
           AND (keys.expires_at_ms IS NULL OR keys.expires_at_ms > ?2)",
        params![key, crate::time_utils::now_ms()],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<i64>>(1)?)),
    )
    .optional()
    .map_err(Into::into)
}

fn live_legacy_record_tx(
    tx: &Transaction<'_>,
    key: &str,
) -> StorageResult<Option<TypedWhitelistOwnerRuntime>> {
    let Some((kind, owner_digest)) = parse_key(key) else {
        return Ok(None);
    };
    let Some((raw, expires_at_ms)) = live_legacy_raw_tx(tx, key)? else {
        return Ok(None);
    };
    if kind == "mapping" {
        if raw.trim().is_empty() {
            return Ok(None);
        }
        return Ok(Some(TypedWhitelistOwnerRuntime::Mapping {
            owner_digest: owner_digest.to_ascii_lowercase(),
            record_id: raw,
            expires_at_ms,
        }));
    }
    let Some(expires_at_ms) = expires_at_ms else {
        return Ok(None);
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return Ok(None);
    };
    let Some(lock_id) = value
        .get("lockId")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    Ok(Some(TypedWhitelistOwnerRuntime::Lock {
        owner_digest: owner_digest.to_ascii_lowercase(),
        lock_digest: crate::crypto_utils::sha256_hex_str(lock_id),
        expires_at_ms,
    }))
}

fn legacy_records_tx(tx: &Transaction<'_>) -> StorageResult<Vec<TypedWhitelistOwnerRuntime>> {
    let mut statement = tx.prepare(
        "SELECT key FROM kv_keys WHERE kind = 'string' AND key LIKE ?1
         AND (expires_at_ms IS NULL OR expires_at_ms > ?2) ORDER BY key",
    )?;
    let rows = statement.query_map(
        params![format!("{OWNER_PREFIX}%"), crate::time_utils::now_ms()],
        |row| row.get::<_, String>(0),
    )?;
    let mut records = Vec::new();
    for key in rows {
        if let Some(record) = live_legacy_record_tx(tx, &key?)? {
            records.push(record);
        }
    }
    Ok(records)
}

fn typed_record_tx(
    tx: &Transaction<'_>,
    kind: &str,
    digest: &str,
) -> StorageResult<Option<TypedWhitelistOwnerRuntime>> {
    typed_record_query(tx, kind, digest)
}

#[cfg(test)]
fn typed_record_conn(
    conn: &tokio_rusqlite::rusqlite::Connection,
    kind: &str,
    digest: &str,
) -> StorageResult<Option<TypedWhitelistOwnerRuntime>> {
    typed_record_query(conn, kind, digest)
}

fn typed_record_query(
    conn: &tokio_rusqlite::rusqlite::Connection,
    kind: &str,
    digest: &str,
) -> StorageResult<Option<TypedWhitelistOwnerRuntime>> {
    if kind == "mapping" {
        return conn
            .query_row(
                "SELECT whitelist_record_id, expires_at_ms FROM whitelist_auto_owner_mappings WHERE owner_digest = ?1",
                [digest],
                |row| Ok(TypedWhitelistOwnerRuntime::Mapping {
                    owner_digest: digest.to_string(), record_id: row.get(0)?, expires_at_ms: row.get(1)?,
                }),
            )
            .optional()
            .map_err(Into::into);
    }
    conn.query_row(
        "SELECT lock_digest, expires_at_ms FROM whitelist_auto_owner_locks WHERE owner_digest = ?1",
        [digest],
        |row| {
            Ok(TypedWhitelistOwnerRuntime::Lock {
                owner_digest: digest.to_string(),
                lock_digest: row.get(0)?,
                expires_at_ms: row.get(1)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

fn upsert_tx(tx: &Transaction<'_>, record: &TypedWhitelistOwnerRuntime) -> StorageResult<()> {
    match record {
        TypedWhitelistOwnerRuntime::Mapping {
            owner_digest,
            record_id,
            expires_at_ms,
        } => {
            tx.execute(
                "INSERT INTO whitelist_auto_owner_mappings(owner_digest, whitelist_record_id, expires_at_ms, updated_at_ms)
                 VALUES (?1, ?2, ?3, ?4) ON CONFLICT(owner_digest) DO UPDATE SET
                   whitelist_record_id = excluded.whitelist_record_id,
                   expires_at_ms = excluded.expires_at_ms, updated_at_ms = excluded.updated_at_ms
                 WHERE whitelist_auto_owner_mappings.whitelist_record_id <> excluded.whitelist_record_id
                    OR whitelist_auto_owner_mappings.expires_at_ms IS NOT excluded.expires_at_ms",
                params![owner_digest, record_id, expires_at_ms, crate::time_utils::now_ms()],
            )?;
        }
        TypedWhitelistOwnerRuntime::Lock {
            owner_digest,
            lock_digest,
            expires_at_ms,
        } => {
            tx.execute(
                "INSERT INTO whitelist_auto_owner_locks(owner_digest, lock_digest, expires_at_ms, updated_at_ms)
                 VALUES (?1, ?2, ?3, ?4) ON CONFLICT(owner_digest) DO UPDATE SET
                   lock_digest = excluded.lock_digest, expires_at_ms = excluded.expires_at_ms,
                   updated_at_ms = excluded.updated_at_ms
                 WHERE whitelist_auto_owner_locks.lock_digest <> excluded.lock_digest
                    OR whitelist_auto_owner_locks.expires_at_ms <> excluded.expires_at_ms",
                params![owner_digest, lock_digest, expires_at_ms, crate::time_utils::now_ms()],
            )?;
        }
    }
    Ok(())
}

fn delete_tx(tx: &Transaction<'_>, kind: &str, digest: &str) -> StorageResult<()> {
    let table = if kind == "mapping" {
        "whitelist_auto_owner_mappings"
    } else {
        "whitelist_auto_owner_locks"
    };
    tx.execute(
        &format!("DELETE FROM {table} WHERE owner_digest = ?1"),
        [digest],
    )?;
    Ok(())
}
