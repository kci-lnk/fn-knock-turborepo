use tokio_rusqlite::rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use super::{StorageResult, redis_compat::ConnectionManager, storage_error};

pub(crate) const COOLDOWN_PREFIX: &str = "fn_knock:wol:runtime:cooldown:";

const SCHEMA_VERSION: i64 = 1;
const SCHEMA_NAME: &str = "typed_wol_wake_cooldowns";
const SCHEMA_SQL: &str = r#"
CREATE TABLE wol_wake_cooldowns (
  target_id TEXT PRIMARY KEY CHECK (target_id <> ''),
  guard_value TEXT NOT NULL CHECK (guard_value = '1'),
  expires_at_ms INTEGER NOT NULL CHECK (expires_at_ms >= 0),
  updated_at_ms INTEGER NOT NULL
);
CREATE INDEX idx_wol_wake_cooldowns_expiry ON wol_wake_cooldowns(expires_at_ms);
"#;
const MIGRATIONS_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS typed_wol_cooldown_schema_migrations (
  version INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  checksum TEXT NOT NULL,
  applied_at_ms INTEGER NOT NULL
);
"#;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TypedWolCooldown {
    pub(crate) target_id: String,
    pub(crate) expires_at_ms: i64,
}

#[derive(Clone)]
pub(crate) struct TypedWolCooldownRepository {
    manager: ConnectionManager,
}

impl TypedWolCooldownRepository {
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
                        "SELECT name, checksum FROM typed_wol_cooldown_schema_migrations WHERE version = ?1",
                        [SCHEMA_VERSION],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                    )
                    .optional()?;
                match applied {
                    Some((name, stored)) if name == SCHEMA_NAME && stored == checksum => {
                        let exists = tx.query_row(
                            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'wol_wake_cooldowns')",
                            [],
                            |row| row.get::<_, bool>(0),
                        )?;
                        if !exists {
                            return Err(storage_error(
                                "typed WOL cooldown migration is recorded but its table is missing",
                            ));
                        }
                    }
                    Some((name, _)) if name != SCHEMA_NAME => {
                        return Err(storage_error("typed WOL cooldown migration name mismatch"));
                    }
                    Some(_) => {
                        return Err(storage_error("typed WOL cooldown migration checksum mismatch"));
                    }
                    None => {
                        tx.execute_batch(SCHEMA_SQL)?;
                        tx.execute(
                            "INSERT INTO typed_wol_cooldown_schema_migrations(version, name, checksum, applied_at_ms) VALUES (?1, ?2, ?3, ?4)",
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
        let legacy = legacy_cooldowns_tx(tx)?;
        tx.execute("DELETE FROM wol_wake_cooldowns", [])?;
        for cooldown in legacy {
            upsert_tx(tx, &cooldown)?;
        }
        Ok(())
    }

    pub(crate) fn reconcile_legacy_keys_tx(
        tx: &Transaction<'_>,
        keys: &[String],
    ) -> StorageResult<()> {
        for key in keys {
            let Some(target_id) = key.strip_prefix(COOLDOWN_PREFIX) else {
                continue;
            };
            if target_id.is_empty() {
                return Err(storage_error("empty WOL cooldown target ID"));
            }
            match live_legacy_cooldown_tx(tx, key)? {
                Some(cooldown) => upsert_tx(tx, &cooldown)?,
                None => delete_tx(tx, target_id)?,
            }
        }
        Ok(())
    }

    pub(crate) async fn verify_and_repair(&self, target_id: &str) -> StorageResult<bool> {
        let target_id = target_id.to_string();
        self.manager
            .call(move |conn| {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                let key = format!("{COOLDOWN_PREFIX}{target_id}");
                let raw = live_legacy_raw_tx(&tx, &key)?;
                let legacy = live_legacy_cooldown_tx(&tx, &key)?;
                let invalid = raw.is_some() && legacy.is_none();
                let typed = typed_cooldown_tx(&tx, &target_id)?;
                let matched = !invalid && typed == legacy;
                if !matched {
                    match legacy {
                        Some(cooldown) => upsert_tx(&tx, &cooldown)?,
                        None => delete_tx(&tx, &target_id)?,
                    }
                }
                tx.commit()?;
                Ok(matched)
            })
            .await
    }

    #[cfg(test)]
    pub(crate) async fn load(&self, target_id: &str) -> StorageResult<Option<TypedWolCooldown>> {
        let target_id = target_id.to_string();
        self.manager
            .call(move |conn| typed_cooldown_conn(conn, &target_id))
            .await
    }

    #[cfg(test)]
    pub(crate) async fn count(&self) -> StorageResult<i64> {
        self.manager
            .call(|conn| {
                conn.query_row("SELECT COUNT(*) FROM wol_wake_cooldowns", [], |row| {
                    row.get::<_, i64>(0)
                })
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
         WHERE keys.key = ?1 AND keys.kind = 'string'
           AND keys.expires_at_ms IS NOT NULL AND keys.expires_at_ms > ?2",
        params![key, crate::time_utils::now_ms()],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
    )
    .optional()
    .map_err(Into::into)
}

fn live_legacy_cooldown_tx(
    tx: &Transaction<'_>,
    key: &str,
) -> StorageResult<Option<TypedWolCooldown>> {
    let Some(target_id) = key
        .strip_prefix(COOLDOWN_PREFIX)
        .filter(|id| !id.is_empty())
    else {
        return Ok(None);
    };
    let Some((value, expires_at_ms)) = live_legacy_raw_tx(tx, key)? else {
        return Ok(None);
    };
    if value != "1" {
        return Ok(None);
    }
    Ok(Some(TypedWolCooldown {
        target_id: target_id.to_string(),
        expires_at_ms,
    }))
}

fn legacy_cooldowns_tx(tx: &Transaction<'_>) -> StorageResult<Vec<TypedWolCooldown>> {
    let mut statement = tx.prepare(
        "SELECT keys.key FROM kv_keys AS keys
         JOIN kv_strings AS strings ON strings.key = keys.key
         WHERE substr(keys.key, 1, ?1) = ?2 AND keys.kind = 'string'
           AND keys.expires_at_ms IS NOT NULL AND keys.expires_at_ms > ?3
         ORDER BY keys.key",
    )?;
    let rows = statement.query_map(
        params![
            COOLDOWN_PREFIX.len() as i64,
            COOLDOWN_PREFIX,
            crate::time_utils::now_ms()
        ],
        |row| row.get::<_, String>(0),
    )?;
    let mut cooldowns = Vec::new();
    for key in rows {
        if let Some(cooldown) = live_legacy_cooldown_tx(tx, &key?)? {
            cooldowns.push(cooldown);
        }
    }
    Ok(cooldowns)
}

fn typed_cooldown_tx(
    tx: &Transaction<'_>,
    target_id: &str,
) -> StorageResult<Option<TypedWolCooldown>> {
    tx.query_row(
        "SELECT expires_at_ms FROM wol_wake_cooldowns WHERE target_id = ?1",
        [target_id],
        |row| {
            Ok(TypedWolCooldown {
                target_id: target_id.to_string(),
                expires_at_ms: row.get(0)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

#[cfg(test)]
fn typed_cooldown_conn(
    conn: &tokio_rusqlite::rusqlite::Connection,
    target_id: &str,
) -> StorageResult<Option<TypedWolCooldown>> {
    conn.query_row(
        "SELECT expires_at_ms FROM wol_wake_cooldowns WHERE target_id = ?1",
        [target_id],
        |row| {
            Ok(TypedWolCooldown {
                target_id: target_id.to_string(),
                expires_at_ms: row.get(0)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

fn upsert_tx(tx: &Transaction<'_>, cooldown: &TypedWolCooldown) -> StorageResult<()> {
    if cooldown.target_id.is_empty() {
        return Err(storage_error("empty typed WOL cooldown target ID"));
    }
    tx.execute(
        "INSERT INTO wol_wake_cooldowns(target_id, guard_value, expires_at_ms, updated_at_ms)
         VALUES (?1, '1', ?2, ?3)
         ON CONFLICT(target_id) DO UPDATE SET
           guard_value = excluded.guard_value,
           expires_at_ms = excluded.expires_at_ms,
           updated_at_ms = excluded.updated_at_ms
         WHERE wol_wake_cooldowns.guard_value <> excluded.guard_value
            OR wol_wake_cooldowns.expires_at_ms <> excluded.expires_at_ms",
        params![
            cooldown.target_id,
            cooldown.expires_at_ms,
            crate::time_utils::now_ms()
        ],
    )?;
    Ok(())
}

fn delete_tx(tx: &Transaction<'_>, target_id: &str) -> StorageResult<()> {
    tx.execute(
        "DELETE FROM wol_wake_cooldowns WHERE target_id = ?1",
        [target_id],
    )?;
    Ok(())
}
