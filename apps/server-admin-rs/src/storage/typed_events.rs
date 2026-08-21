use serde_json::Value;
use tokio_rusqlite::rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use super::{StorageResult, redis_compat::ConnectionManager, storage_error};

const TYPED_EVENT_SCHEMA_VERSION: i64 = 1;
const TYPED_EVENT_SCHEMA_NAME: &str = "typed_system_events";
const TYPED_EVENT_SCHEMA_SQL: &str = r#"
CREATE TABLE system_event_documents (
  id TEXT PRIMARY KEY,
  event_json TEXT NOT NULL,
  happened_at_ms INTEGER NOT NULL CHECK (happened_at_ms >= 0),
  expires_at_ms INTEGER NOT NULL CHECK (expires_at_ms >= 0),
  stream_id TEXT NOT NULL,
  updated_at_ms INTEGER NOT NULL
);
CREATE INDEX idx_system_event_documents_happened
  ON system_event_documents(happened_at_ms DESC, id DESC);
CREATE INDEX idx_system_event_documents_expires
  ON system_event_documents(expires_at_ms);
"#;

const TYPED_EVENT_MIGRATIONS_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS typed_event_schema_migrations (
  version INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  checksum TEXT NOT NULL,
  applied_at_ms INTEGER NOT NULL
);
"#;

#[derive(Clone)]
pub(crate) struct TypedEventRepository {
    manager: ConnectionManager,
}

#[derive(Clone, Debug)]
pub(crate) struct TypedSystemEvent {
    pub(crate) event: Value,
    pub(crate) happened_at_ms: i64,
}

impl TypedEventRepository {
    pub(crate) fn new(manager: ConnectionManager) -> Self {
        Self { manager }
    }

    pub(crate) async fn initialize(&self) -> StorageResult<()> {
        self.manager
            .call(move |conn| {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                tx.execute_batch(TYPED_EVENT_MIGRATIONS_SQL)?;
                let expected_checksum = crate::crypto_utils::sha256_hex_bytes(TYPED_EVENT_SCHEMA_SQL);
                let applied = tx
                    .query_row(
                        "SELECT name, checksum FROM typed_event_schema_migrations WHERE version = ?1",
                        [TYPED_EVENT_SCHEMA_VERSION],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                    )
                    .optional()?;
                match applied {
                    Some((name, checksum))
                        if name == TYPED_EVENT_SCHEMA_NAME && checksum == expected_checksum =>
                    {
                        let table_exists = tx.query_row(
                            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'system_event_documents')",
                            [],
                            |row| row.get::<_, bool>(0),
                        )?;
                        if !table_exists {
                            return Err(storage_error(
                                "typed system-event migration is recorded but its table is missing",
                            ));
                        }
                    }
                    Some((name, _)) if name != TYPED_EVENT_SCHEMA_NAME => {
                        return Err(storage_error("typed system-event migration name mismatch"));
                    }
                    Some(_) => {
                        return Err(storage_error("typed system-event migration checksum mismatch"));
                    }
                    None => {
                        tx.execute_batch(TYPED_EVENT_SCHEMA_SQL)?;
                        tx.execute(
                            "INSERT INTO typed_event_schema_migrations(version, name, checksum, applied_at_ms) VALUES (?1, ?2, ?3, ?4)",
                            params![
                                TYPED_EVENT_SCHEMA_VERSION,
                                TYPED_EVENT_SCHEMA_NAME,
                                expected_checksum,
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

    pub(crate) fn upsert_tx(
        tx: &Transaction<'_>,
        id: &str,
        event_json: &str,
        happened_at_ms: i64,
        expires_at_ms: i64,
        stream_id: &str,
    ) -> StorageResult<()> {
        let _: Value = serde_json::from_str(event_json)?;
        if id.trim().is_empty() || stream_id.trim().is_empty() {
            return Err(storage_error(
                "typed system event requires an id and stream id",
            ));
        }
        tx.execute(
            "INSERT INTO system_event_documents(id, event_json, happened_at_ms, expires_at_ms, stream_id, updated_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
               event_json = excluded.event_json,
               happened_at_ms = excluded.happened_at_ms,
               expires_at_ms = excluded.expires_at_ms,
               stream_id = excluded.stream_id,
               updated_at_ms = excluded.updated_at_ms",
            params![
                id,
                event_json,
                happened_at_ms.max(0),
                expires_at_ms.max(0),
                stream_id,
                crate::time_utils::now_ms(),
            ],
        )?;
        Ok(())
    }

    pub(crate) fn trim_tx(
        tx: &Transaction<'_>,
        cutoff_timestamp: i64,
        max_records: i64,
    ) -> StorageResult<()> {
        tx.execute(
            "DELETE FROM system_event_documents WHERE happened_at_ms <= ?1 OR expires_at_ms <= ?2",
            params![cutoff_timestamp, crate::time_utils::now_ms()],
        )?;
        tx.execute(
            "DELETE FROM system_event_documents
             WHERE id IN (
               SELECT id FROM system_event_documents
               ORDER BY happened_at_ms DESC, id DESC
               LIMIT -1 OFFSET ?1
             )",
            [max_records.max(0)],
        )?;
        Ok(())
    }

    pub(crate) fn update_event_json_tx(
        tx: &Transaction<'_>,
        id: &str,
        event_json: &str,
    ) -> StorageResult<bool> {
        let _: Value = serde_json::from_str(event_json)?;
        let updated = tx.execute(
            "UPDATE system_event_documents
             SET event_json = ?2, updated_at_ms = ?3
             WHERE id = ?1",
            params![id, event_json, crate::time_utils::now_ms()],
        )?;
        Ok(updated > 0)
    }

    pub(crate) fn delete_tx(tx: &Transaction<'_>, ids: &[String]) -> StorageResult<()> {
        for id in ids {
            tx.execute("DELETE FROM system_event_documents WHERE id = ?1", [id])?;
        }
        Ok(())
    }

    pub(crate) fn clear_tx(tx: &Transaction<'_>) -> StorageResult<()> {
        tx.execute("DELETE FROM system_event_documents", [])?;
        Ok(())
    }

    pub(crate) async fn rebuild_from_legacy(
        &self,
        index_key: &str,
        data_prefix: &str,
        stream_id_prefix: &str,
    ) -> StorageResult<()> {
        let index_key = index_key.to_string();
        let data_prefix = data_prefix.to_string();
        let stream_id_prefix = stream_id_prefix.to_string();
        self.manager
            .call(move |conn| {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                tx.execute("DELETE FROM system_event_documents", [])?;
                let now = crate::time_utils::now_ms();
                let mut statement = tx.prepare(
                    "SELECT indexed.member, indexed.score, data.value, data_key.expires_at_ms, stream.value
                     FROM kv_zset AS indexed
                     JOIN kv_strings AS data ON data.key = ?2 || indexed.member
                     JOIN kv_keys AS data_key ON data_key.key = data.key
                     LEFT JOIN kv_strings AS stream ON stream.key = ?3 || indexed.member
                     WHERE indexed.key = ?1
                       AND (data_key.expires_at_ms IS NULL OR data_key.expires_at_ms > ?4)
                     ORDER BY indexed.score ASC, indexed.member ASC",
                )?;
                let rows = statement.query_map(
                    params![index_key, data_prefix, stream_id_prefix, now],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, f64>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, Option<i64>>(3)?,
                            row.get::<_, Option<String>>(4)?,
                        ))
                    },
                )?;
                for row in rows {
                    let (id, score, event_json, expires_at_ms, stream_id) = row?;
                    if serde_json::from_str::<Value>(&event_json).is_err() {
                        continue;
                    }
                    let stream_id = stream_id.unwrap_or_else(|| format!("legacy:{id}"));
                    Self::upsert_tx(
                        &tx,
                        &id,
                        &event_json,
                        score as i64,
                        expires_at_ms.unwrap_or(now.saturating_add(86_400_000)),
                        &stream_id,
                    )?;
                }
                drop(statement);
                tx.commit()?;
                Ok(())
            })
            .await
    }

    pub(crate) async fn load_active(&self) -> StorageResult<Vec<TypedSystemEvent>> {
        self.manager
            .call(move |conn| {
                let now = crate::time_utils::now_ms();
                let mut statement = conn.prepare(
                    "SELECT event_json, happened_at_ms
                     FROM system_event_documents
                     WHERE expires_at_ms > ?1
                     ORDER BY happened_at_ms DESC, id DESC",
                )?;
                let rows = statement.query_map([now], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
                })?;
                let mut events = Vec::new();
                for row in rows {
                    let (event_json, happened_at_ms) = row?;
                    let event = serde_json::from_str(&event_json).map_err(|error| {
                        storage_error(format!("typed system event document is invalid: {error}"))
                    })?;
                    events.push(TypedSystemEvent {
                        event,
                        happened_at_ms,
                    });
                }
                Ok(events)
            })
            .await
    }

    #[cfg(test)]
    pub(crate) async fn count(&self) -> StorageResult<i64> {
        self.manager
            .call(move |conn| {
                conn.query_row("SELECT COUNT(*) FROM system_event_documents", [], |row| {
                    row.get(0)
                })
                .map_err(Into::into)
            })
            .await
    }
}
