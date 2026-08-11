use tokio_rusqlite::rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use super::{StorageResult, redis_compat::ConnectionManager, storage_error};

pub(crate) const VALIDATION_PREFIX: &str = "fn_knock:fnos-share:validation:";
pub(crate) const SESSION_PREFIX: &str = "fn_knock:fnos-share:session:";
pub(crate) const LOCK_PREFIX: &str = "fn_knock:lock:fnos-share:validation:";

const SCHEMA_VERSION: i64 = 1;
const SCHEMA_NAME: &str = "typed_fnos_share_runtime";
const SCHEMA_SQL: &str = r#"
CREATE TABLE fnos_share_runtime_capabilities (
  capability_kind TEXT NOT NULL CHECK (capability_kind IN ('validation', 'session', 'lock')),
  key_digest TEXT NOT NULL CHECK (length(key_digest) = 64),
  payload_json TEXT,
  guard_digest TEXT,
  expires_at_ms INTEGER NOT NULL CHECK (expires_at_ms >= 0),
  updated_at_ms INTEGER NOT NULL,
  PRIMARY KEY(capability_kind, key_digest),
  CHECK (
    (capability_kind IN ('validation', 'session') AND payload_json IS NOT NULL AND json_valid(payload_json) AND guard_digest IS NULL)
    OR (capability_kind = 'lock' AND payload_json IS NULL AND guard_digest IS NOT NULL AND length(guard_digest) = 64)
  )
);
CREATE INDEX idx_fnos_share_runtime_expiry ON fnos_share_runtime_capabilities(expires_at_ms);
"#;
const MIGRATIONS_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS typed_fnos_share_schema_migrations (
  version INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  checksum TEXT NOT NULL,
  applied_at_ms INTEGER NOT NULL
);
"#;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TypedFnosShareCapability {
    pub(crate) kind: &'static str,
    pub(crate) key_digest: String,
    pub(crate) payload_json: Option<String>,
    pub(crate) guard_digest: Option<String>,
    pub(crate) expires_at_ms: i64,
}

#[derive(Clone)]
pub(crate) struct TypedFnosShareRepository {
    manager: ConnectionManager,
}

impl TypedFnosShareRepository {
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
                        "SELECT name, checksum FROM typed_fnos_share_schema_migrations WHERE version = ?1",
                        [SCHEMA_VERSION],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                    )
                    .optional()?;
                match applied {
                    Some((name, stored)) if name == SCHEMA_NAME && stored == checksum => {
                        let exists = tx.query_row(
                            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'fnos_share_runtime_capabilities')",
                            [],
                            |row| row.get::<_, bool>(0),
                        )?;
                        if !exists {
                            return Err(storage_error(
                                "typed fnOS share migration is recorded but its table is missing",
                            ));
                        }
                    }
                    Some((name, _)) if name != SCHEMA_NAME => {
                        return Err(storage_error("typed fnOS share migration name mismatch"));
                    }
                    Some(_) => {
                        return Err(storage_error("typed fnOS share migration checksum mismatch"));
                    }
                    None => {
                        tx.execute_batch(SCHEMA_SQL)?;
                        tx.execute(
                            "INSERT INTO typed_fnos_share_schema_migrations(version, name, checksum, applied_at_ms) VALUES (?1, ?2, ?3, ?4)",
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
        tx.execute("DELETE FROM fnos_share_runtime_capabilities", [])?;
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
            let Some((kind, _)) = parse_key(key) else {
                continue;
            };
            let digest = key_digest(key);
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
                let (kind, _) = parse_key(&key)
                    .ok_or_else(|| storage_error("invalid fnOS share runtime key"))?;
                let digest = key_digest(&key);
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
    ) -> StorageResult<Option<TypedFnosShareCapability>> {
        let (kind, _) = parse_key(key).ok_or_else(|| storage_error("invalid fnOS share key"))?;
        let kind = kind.to_string();
        let digest = key_digest(key);
        self.manager
            .call(move |conn| typed_record_conn(conn, &kind, &digest))
            .await
    }

    #[cfg(test)]
    pub(crate) async fn count(&self) -> StorageResult<i64> {
        self.manager
            .call(|conn| {
                conn.query_row(
                    "SELECT COUNT(*) FROM fnos_share_runtime_capabilities",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .map_err(Into::into)
            })
            .await
    }
}

pub(crate) fn owns_key(key: &str) -> bool {
    parse_key(key).is_some()
}

fn parse_key(key: &str) -> Option<(&'static str, &str)> {
    for (prefix, kind) in [
        (VALIDATION_PREFIX, "validation"),
        (SESSION_PREFIX, "session"),
        (LOCK_PREFIX, "lock"),
    ] {
        if let Some(suffix) = key.strip_prefix(prefix).filter(|value| !value.is_empty()) {
            return Some((kind, suffix));
        }
    }
    None
}

fn key_digest(key: &str) -> String {
    crate::crypto_utils::sha256_hex_str(key)
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
) -> StorageResult<Option<TypedFnosShareCapability>> {
    let Some((kind, _)) = parse_key(key) else {
        return Ok(None);
    };
    let Some((raw, expires_at_ms)) = live_legacy_raw_tx(tx, key)? else {
        return Ok(None);
    };
    let (payload_json, guard_digest) = if kind == "lock" {
        if raw.is_empty() {
            return Ok(None);
        }
        (None, Some(crate::crypto_utils::sha256_hex_str(&raw)))
    } else {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
            return Ok(None);
        };
        if !valid_document(kind, &value) {
            return Ok(None);
        }
        (Some(serde_json::to_string(&value)?), None)
    };
    Ok(Some(TypedFnosShareCapability {
        kind,
        key_digest: key_digest(key),
        payload_json,
        guard_digest,
        expires_at_ms,
    }))
}

fn valid_document(kind: &str, value: &serde_json::Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    let nonempty = |name: &str| {
        object
            .get(name)
            .and_then(serde_json::Value::as_str)
            .is_some_and(|value| !value.trim().is_empty())
    };
    match kind {
        "validation" => {
            object
                .get("valid")
                .is_some_and(serde_json::Value::is_boolean)
                && nonempty("validationState")
                && nonempty("shareId")
                && nonempty("backendId")
                && nonempty("cleanPath")
                && nonempty("checkedAt")
        }
        "session" => {
            nonempty("shareId")
                && nonempty("backendId")
                && nonempty("cleanPath")
                && nonempty("issuedAt")
                && nonempty("lastSeenAt")
        }
        _ => false,
    }
}

fn legacy_records_tx(tx: &Transaction<'_>) -> StorageResult<Vec<TypedFnosShareCapability>> {
    let mut statement = tx.prepare(
        "SELECT key FROM kv_keys
         WHERE kind = 'string' AND expires_at_ms IS NOT NULL AND expires_at_ms > ?1
           AND (key LIKE ?2 OR key LIKE ?3 OR key LIKE ?4)
         ORDER BY key",
    )?;
    let rows = statement.query_map(
        params![
            crate::time_utils::now_ms(),
            format!("{VALIDATION_PREFIX}%"),
            format!("{SESSION_PREFIX}%"),
            format!("{LOCK_PREFIX}%")
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
) -> StorageResult<Option<TypedFnosShareCapability>> {
    typed_record_query(tx, kind, digest)
}

#[cfg(test)]
fn typed_record_conn(
    conn: &tokio_rusqlite::rusqlite::Connection,
    kind: &str,
    digest: &str,
) -> StorageResult<Option<TypedFnosShareCapability>> {
    typed_record_query(conn, kind, digest)
}

fn typed_record_query(
    conn: &tokio_rusqlite::rusqlite::Connection,
    kind: &str,
    digest: &str,
) -> StorageResult<Option<TypedFnosShareCapability>> {
    conn.query_row(
        "SELECT payload_json, guard_digest, expires_at_ms
         FROM fnos_share_runtime_capabilities
         WHERE capability_kind = ?1 AND key_digest = ?2",
        params![kind, digest],
        |row| {
            Ok(TypedFnosShareCapability {
                kind: match kind {
                    "validation" => "validation",
                    "session" => "session",
                    _ => "lock",
                },
                key_digest: digest.to_string(),
                payload_json: row.get(0)?,
                guard_digest: row.get(1)?,
                expires_at_ms: row.get(2)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

fn upsert_tx(tx: &Transaction<'_>, record: &TypedFnosShareCapability) -> StorageResult<()> {
    tx.execute(
        "INSERT INTO fnos_share_runtime_capabilities(
           capability_kind, key_digest, payload_json, guard_digest, expires_at_ms, updated_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(capability_kind, key_digest) DO UPDATE SET
           payload_json = excluded.payload_json,
           guard_digest = excluded.guard_digest,
           expires_at_ms = excluded.expires_at_ms,
           updated_at_ms = excluded.updated_at_ms
         WHERE fnos_share_runtime_capabilities.payload_json IS NOT excluded.payload_json
            OR fnos_share_runtime_capabilities.guard_digest IS NOT excluded.guard_digest
            OR fnos_share_runtime_capabilities.expires_at_ms <> excluded.expires_at_ms",
        params![
            record.kind,
            record.key_digest,
            record.payload_json,
            record.guard_digest,
            record.expires_at_ms,
            crate::time_utils::now_ms()
        ],
    )?;
    Ok(())
}

fn delete_tx(tx: &Transaction<'_>, kind: &str, digest: &str) -> StorageResult<()> {
    tx.execute(
        "DELETE FROM fnos_share_runtime_capabilities
         WHERE capability_kind = ?1 AND key_digest = ?2",
        params![kind, digest],
    )?;
    Ok(())
}
