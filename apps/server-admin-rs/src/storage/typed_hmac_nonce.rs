use tokio_rusqlite::rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use super::{StorageResult, redis_compat::ConnectionManager, storage_error};

pub(crate) const NONCE_PREFIX: &str = "fn_knock:nonce:";

const SCHEMA_VERSION: i64 = 1;
const SCHEMA_NAME: &str = "typed_hmac_replay_nonces";
const SCHEMA_SQL: &str = r#"
CREATE TABLE hmac_replay_nonces (
  nonce_digest TEXT PRIMARY KEY CHECK (length(nonce_digest) = 64),
  guard_value TEXT NOT NULL CHECK (guard_value = '1'),
  expires_at_ms INTEGER NOT NULL CHECK (expires_at_ms >= 0),
  updated_at_ms INTEGER NOT NULL
);
CREATE INDEX idx_hmac_replay_nonces_expiry ON hmac_replay_nonces(expires_at_ms);
"#;
const MIGRATIONS_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS typed_hmac_nonce_schema_migrations (
  version INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  checksum TEXT NOT NULL,
  applied_at_ms INTEGER NOT NULL
);
"#;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TypedHmacReplayNonce {
    pub(crate) nonce_digest: String,
    pub(crate) expires_at_ms: i64,
}

#[derive(Clone)]
pub(crate) struct TypedHmacNonceRepository {
    manager: ConnectionManager,
}

impl TypedHmacNonceRepository {
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
                        "SELECT name, checksum FROM typed_hmac_nonce_schema_migrations WHERE version = ?1",
                        [SCHEMA_VERSION],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                    )
                    .optional()?;
                match applied {
                    Some((name, stored)) if name == SCHEMA_NAME && stored == checksum => {
                        let exists = tx.query_row(
                            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'hmac_replay_nonces')",
                            [],
                            |row| row.get::<_, bool>(0),
                        )?;
                        if !exists {
                            return Err(storage_error(
                                "typed HMAC nonce migration is recorded but its table is missing",
                            ));
                        }
                    }
                    Some((name, _)) if name != SCHEMA_NAME => {
                        return Err(storage_error("typed HMAC nonce migration name mismatch"));
                    }
                    Some(_) => {
                        return Err(storage_error("typed HMAC nonce migration checksum mismatch"));
                    }
                    None => {
                        tx.execute_batch(SCHEMA_SQL)?;
                        tx.execute(
                            "INSERT INTO typed_hmac_nonce_schema_migrations(version, name, checksum, applied_at_ms) VALUES (?1, ?2, ?3, ?4)",
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
        let records = legacy_nonces_tx(tx)?;
        tx.execute("DELETE FROM hmac_replay_nonces", [])?;
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
            let Some(nonce) = key
                .strip_prefix(NONCE_PREFIX)
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            let digest = nonce_digest(nonce);
            match live_legacy_nonce_tx(tx, key)? {
                Some(record) => upsert_tx(tx, &record)?,
                None => delete_tx(tx, &digest)?,
            }
        }
        Ok(())
    }

    pub(crate) async fn verify_and_repair(&self, nonce: &str) -> StorageResult<bool> {
        let nonce_digest = nonce_digest(nonce);
        let key = format!("{NONCE_PREFIX}{nonce}");
        self.manager
            .call(move |conn| {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                let raw = live_legacy_raw_tx(&tx, &key)?;
                let legacy = live_legacy_nonce_tx(&tx, &key)?;
                let invalid = raw.is_some() && legacy.is_none();
                let typed = typed_nonce_tx(&tx, &nonce_digest)?;
                let matched = !invalid && typed == legacy;
                if !matched {
                    match legacy {
                        Some(record) => upsert_tx(&tx, &record)?,
                        None => delete_tx(&tx, &nonce_digest)?,
                    }
                }
                tx.commit()?;
                Ok(matched)
            })
            .await
    }

    #[cfg(test)]
    pub(crate) async fn load(&self, nonce: &str) -> StorageResult<Option<TypedHmacReplayNonce>> {
        let digest = nonce_digest(nonce);
        self.manager
            .call(move |conn| typed_nonce_conn(conn, &digest))
            .await
    }

    #[cfg(test)]
    pub(crate) async fn count(&self) -> StorageResult<i64> {
        self.manager
            .call(|conn| {
                conn.query_row("SELECT COUNT(*) FROM hmac_replay_nonces", [], |row| {
                    row.get::<_, i64>(0)
                })
                .map_err(Into::into)
            })
            .await
    }
}

fn nonce_digest(nonce: &str) -> String {
    crate::crypto_utils::sha256_hex_str(nonce)
}

fn live_legacy_raw_tx(tx: &Transaction<'_>, key: &str) -> StorageResult<Option<(String, i64)>> {
    tx.query_row(
        "SELECT strings.value, keys.expires_at_ms
         FROM kv_keys AS keys JOIN kv_strings AS strings ON strings.key = keys.key
         WHERE keys.key = ?1 AND keys.kind = 'string'
           AND keys.expires_at_ms IS NOT NULL AND keys.expires_at_ms > ?2",
        params![key, crate::time_utils::now_ms()],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
    )
    .optional()
    .map_err(Into::into)
}

fn live_legacy_nonce_tx(
    tx: &Transaction<'_>,
    key: &str,
) -> StorageResult<Option<TypedHmacReplayNonce>> {
    let Some(nonce) = key
        .strip_prefix(NONCE_PREFIX)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    let Some((value, expires_at_ms)) = live_legacy_raw_tx(tx, key)? else {
        return Ok(None);
    };
    if value != "1" {
        return Ok(None);
    }
    Ok(Some(TypedHmacReplayNonce {
        nonce_digest: nonce_digest(nonce),
        expires_at_ms,
    }))
}

fn legacy_nonces_tx(tx: &Transaction<'_>) -> StorageResult<Vec<TypedHmacReplayNonce>> {
    let mut statement = tx.prepare(
        "SELECT key FROM kv_keys
         WHERE kind = 'string' AND expires_at_ms IS NOT NULL AND expires_at_ms > ?1
           AND substr(key, 1, ?2) = ?3
         ORDER BY key",
    )?;
    let rows = statement.query_map(
        params![
            crate::time_utils::now_ms(),
            NONCE_PREFIX.len() as i64,
            NONCE_PREFIX
        ],
        |row| row.get::<_, String>(0),
    )?;
    let mut records = Vec::new();
    for key in rows {
        if let Some(record) = live_legacy_nonce_tx(tx, &key?)? {
            records.push(record);
        }
    }
    Ok(records)
}

fn typed_nonce_tx(
    tx: &Transaction<'_>,
    nonce_digest: &str,
) -> StorageResult<Option<TypedHmacReplayNonce>> {
    tx.query_row(
        "SELECT expires_at_ms FROM hmac_replay_nonces WHERE nonce_digest = ?1",
        [nonce_digest],
        |row| {
            Ok(TypedHmacReplayNonce {
                nonce_digest: nonce_digest.to_string(),
                expires_at_ms: row.get(0)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

#[cfg(test)]
fn typed_nonce_conn(
    conn: &tokio_rusqlite::rusqlite::Connection,
    nonce_digest: &str,
) -> StorageResult<Option<TypedHmacReplayNonce>> {
    conn.query_row(
        "SELECT expires_at_ms FROM hmac_replay_nonces WHERE nonce_digest = ?1",
        [nonce_digest],
        |row| {
            Ok(TypedHmacReplayNonce {
                nonce_digest: nonce_digest.to_string(),
                expires_at_ms: row.get(0)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

fn upsert_tx(tx: &Transaction<'_>, nonce: &TypedHmacReplayNonce) -> StorageResult<()> {
    if nonce.nonce_digest.len() != 64 {
        return Err(storage_error("invalid typed HMAC nonce digest"));
    }
    tx.execute(
        "INSERT INTO hmac_replay_nonces(nonce_digest, guard_value, expires_at_ms, updated_at_ms)
         VALUES (?1, '1', ?2, ?3)
         ON CONFLICT(nonce_digest) DO UPDATE SET
           guard_value = excluded.guard_value,
           expires_at_ms = excluded.expires_at_ms,
           updated_at_ms = excluded.updated_at_ms
         WHERE hmac_replay_nonces.guard_value <> excluded.guard_value
            OR hmac_replay_nonces.expires_at_ms <> excluded.expires_at_ms",
        params![
            nonce.nonce_digest,
            nonce.expires_at_ms,
            crate::time_utils::now_ms()
        ],
    )?;
    Ok(())
}

fn delete_tx(tx: &Transaction<'_>, nonce_digest: &str) -> StorageResult<()> {
    tx.execute(
        "DELETE FROM hmac_replay_nonces WHERE nonce_digest = ?1",
        [nonce_digest],
    )?;
    Ok(())
}
