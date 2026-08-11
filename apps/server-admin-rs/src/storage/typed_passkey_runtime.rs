use tokio_rusqlite::rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use super::{StorageResult, redis_compat::ConnectionManager, storage_error};

pub(crate) const CHALLENGE_PREFIX: &str = "fn_knock:passkey:challenge:";
pub(crate) const STATE_PREFIX: &str = "fn_knock:passkey:state:";
pub(crate) const BIND_PREFIX: &str = "fn_knock:passkey:bind:";

const SCHEMA_VERSION: i64 = 1;
const SCHEMA_NAME: &str = "typed_passkey_runtime_capabilities";
const SCHEMA_SQL: &str = r#"
CREATE TABLE passkey_runtime_capabilities (
  capability_kind TEXT NOT NULL CHECK (capability_kind IN ('challenge', 'state', 'bind')),
  capability_digest TEXT NOT NULL CHECK (length(capability_digest) = 64),
  challenge_type TEXT,
  state_json TEXT,
  totp_id TEXT,
  expires_at_ms INTEGER NOT NULL CHECK (expires_at_ms >= 0),
  updated_at_ms INTEGER NOT NULL,
  PRIMARY KEY(capability_kind, capability_digest),
  CHECK (
    (capability_kind = 'challenge' AND challenge_type IS NOT NULL AND challenge_type <> '' AND state_json IS NULL AND totp_id IS NULL)
    OR (capability_kind = 'state' AND challenge_type IS NULL AND state_json IS NOT NULL AND json_valid(state_json) AND totp_id IS NULL)
    OR (capability_kind = 'bind' AND challenge_type IS NULL AND state_json IS NULL AND totp_id IS NOT NULL AND totp_id <> '')
  )
);
CREATE INDEX idx_passkey_runtime_capabilities_expiry
  ON passkey_runtime_capabilities(expires_at_ms);
"#;
const MIGRATIONS_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS typed_passkey_runtime_schema_migrations (
  version INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  checksum TEXT NOT NULL,
  applied_at_ms INTEGER NOT NULL
);
"#;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TypedPasskeyRuntimeCapability {
    pub(crate) kind: &'static str,
    pub(crate) digest: String,
    pub(crate) value: String,
    pub(crate) expires_at_ms: i64,
}

#[derive(Clone)]
pub(crate) struct TypedPasskeyRuntimeRepository {
    manager: ConnectionManager,
}

impl TypedPasskeyRuntimeRepository {
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
                        "SELECT name, checksum FROM typed_passkey_runtime_schema_migrations WHERE version = ?1",
                        [SCHEMA_VERSION],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                    )
                    .optional()?;
                match applied {
                    Some((name, stored)) if name == SCHEMA_NAME && stored == checksum => {
                        let exists = tx.query_row(
                            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'passkey_runtime_capabilities')",
                            [],
                            |row| row.get::<_, bool>(0),
                        )?;
                        if !exists {
                            return Err(storage_error(
                                "typed passkey runtime migration is recorded but its table is missing",
                            ));
                        }
                    }
                    Some((name, _)) if name != SCHEMA_NAME => {
                        return Err(storage_error("typed passkey runtime migration name mismatch"));
                    }
                    Some(_) => {
                        return Err(storage_error("typed passkey runtime migration checksum mismatch"));
                    }
                    None => {
                        tx.execute_batch(SCHEMA_SQL)?;
                        tx.execute(
                            "INSERT INTO typed_passkey_runtime_schema_migrations(version, name, checksum, applied_at_ms) VALUES (?1, ?2, ?3, ?4)",
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
        tx.execute("DELETE FROM passkey_runtime_capabilities", [])?;
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
            let Some((kind, capability)) = parse_key(key) else {
                continue;
            };
            let digest = capability_digest(capability);
            match live_legacy_record_tx(tx, key)? {
                Some(record) => upsert_tx(tx, &record)?,
                None => delete_tx(tx, kind, &digest)?,
            }
        }
        Ok(())
    }

    pub(crate) async fn verify_and_repair_key(&self, key: &str) -> StorageResult<bool> {
        let key = key.to_string();
        self.manager
            .call(move |conn| {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                let (kind, capability) = parse_key(&key)
                    .ok_or_else(|| storage_error("invalid passkey runtime capability key"))?;
                let digest = capability_digest(capability);
                let raw = live_legacy_raw_tx(&tx, &key)?;
                let legacy = live_legacy_record_tx(&tx, &key)?;
                let invalid = raw.is_some() && legacy.is_none();
                let typed = typed_record_tx(&tx, kind, &digest)?;
                let matched = !invalid && typed == legacy;
                if !matched {
                    match legacy {
                        Some(record) => upsert_tx(&tx, &record)?,
                        None => delete_tx(&tx, kind, &digest)?,
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
    ) -> StorageResult<Option<TypedPasskeyRuntimeCapability>> {
        let (kind, capability) = parse_key(key)
            .ok_or_else(|| storage_error("invalid passkey runtime capability key"))?;
        let kind = kind.to_string();
        let digest = capability_digest(capability);
        self.manager
            .call(move |conn| typed_record_conn(conn, &kind, &digest))
            .await
    }

    #[cfg(test)]
    pub(crate) async fn count(&self) -> StorageResult<i64> {
        self.manager
            .call(|conn| {
                conn.query_row(
                    "SELECT COUNT(*) FROM passkey_runtime_capabilities",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .map_err(Into::into)
            })
            .await
    }
}

fn parse_key(key: &str) -> Option<(&'static str, &str)> {
    for (prefix, kind) in [
        (CHALLENGE_PREFIX, "challenge"),
        (STATE_PREFIX, "state"),
        (BIND_PREFIX, "bind"),
    ] {
        if let Some(capability) = key.strip_prefix(prefix).filter(|value| !value.is_empty()) {
            return Some((kind, capability));
        }
    }
    None
}

fn capability_digest(capability: &str) -> String {
    crate::crypto_utils::sha256_hex_str(capability)
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

fn live_legacy_record_tx(
    tx: &Transaction<'_>,
    key: &str,
) -> StorageResult<Option<TypedPasskeyRuntimeCapability>> {
    let Some((kind, capability)) = parse_key(key) else {
        return Ok(None);
    };
    let Some((value, expires_at_ms)) = live_legacy_raw_tx(tx, key)? else {
        return Ok(None);
    };
    let valid = match kind {
        "challenge" | "bind" => !value.is_empty(),
        "state" => serde_json::from_str::<serde_json::Value>(&value).is_ok(),
        _ => false,
    };
    if !valid {
        return Ok(None);
    }
    Ok(Some(TypedPasskeyRuntimeCapability {
        kind,
        digest: capability_digest(capability),
        value,
        expires_at_ms,
    }))
}

fn legacy_records_tx(tx: &Transaction<'_>) -> StorageResult<Vec<TypedPasskeyRuntimeCapability>> {
    let mut statement = tx.prepare(
        "SELECT key FROM kv_keys
         WHERE kind = 'string' AND expires_at_ms IS NOT NULL AND expires_at_ms > ?1
           AND (key LIKE ?2 OR key LIKE ?3 OR key LIKE ?4)
         ORDER BY key",
    )?;
    let rows = statement.query_map(
        params![
            crate::time_utils::now_ms(),
            format!("{CHALLENGE_PREFIX}%"),
            format!("{STATE_PREFIX}%"),
            format!("{BIND_PREFIX}%")
        ],
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
) -> StorageResult<Option<TypedPasskeyRuntimeCapability>> {
    typed_record_query(tx, kind, digest)
}

fn typed_record_query(
    conn: &tokio_rusqlite::rusqlite::Connection,
    kind: &str,
    digest: &str,
) -> StorageResult<Option<TypedPasskeyRuntimeCapability>> {
    conn.query_row(
        "SELECT COALESCE(challenge_type, state_json, totp_id), expires_at_ms
         FROM passkey_runtime_capabilities
         WHERE capability_kind = ?1 AND capability_digest = ?2",
        params![kind, digest],
        |row| {
            Ok(TypedPasskeyRuntimeCapability {
                kind: match kind {
                    "challenge" => "challenge",
                    "state" => "state",
                    "bind" => "bind",
                    _ => "invalid",
                },
                digest: digest.to_string(),
                value: row.get(0)?,
                expires_at_ms: row.get(1)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

#[cfg(test)]
fn typed_record_conn(
    conn: &tokio_rusqlite::rusqlite::Connection,
    kind: &str,
    digest: &str,
) -> StorageResult<Option<TypedPasskeyRuntimeCapability>> {
    typed_record_query(conn, kind, digest)
}

fn upsert_tx(tx: &Transaction<'_>, record: &TypedPasskeyRuntimeCapability) -> StorageResult<()> {
    let (challenge_type, state_json, totp_id) = match record.kind {
        "challenge" => (Some(record.value.as_str()), None, None),
        "state" => (None, Some(record.value.as_str()), None),
        "bind" => (None, None, Some(record.value.as_str())),
        _ => return Err(storage_error("invalid typed passkey capability kind")),
    };
    tx.execute(
        "INSERT INTO passkey_runtime_capabilities(
           capability_kind, capability_digest, challenge_type, state_json, totp_id,
           expires_at_ms, updated_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(capability_kind, capability_digest) DO UPDATE SET
           challenge_type = excluded.challenge_type,
           state_json = excluded.state_json,
           totp_id = excluded.totp_id,
           expires_at_ms = excluded.expires_at_ms,
           updated_at_ms = excluded.updated_at_ms
         WHERE challenge_type IS NOT excluded.challenge_type
            OR state_json IS NOT excluded.state_json
            OR totp_id IS NOT excluded.totp_id
            OR expires_at_ms <> excluded.expires_at_ms",
        params![
            record.kind,
            record.digest,
            challenge_type,
            state_json,
            totp_id,
            record.expires_at_ms,
            crate::time_utils::now_ms()
        ],
    )?;
    Ok(())
}

fn delete_tx(tx: &Transaction<'_>, kind: &str, digest: &str) -> StorageResult<()> {
    tx.execute(
        "DELETE FROM passkey_runtime_capabilities
         WHERE capability_kind = ?1 AND capability_digest = ?2",
        params![kind, digest],
    )?;
    Ok(())
}
