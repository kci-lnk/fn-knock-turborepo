use serde_json::Value;
use tokio_rusqlite::rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use super::{StorageResult, redis_compat::ConnectionManager, storage_error};

const SCHEMA_VERSION: i64 = 1;
const SCHEMA_NAME: &str = "typed_notification_documents";
const SCHEMA_SQL: &str = r#"
CREATE TABLE notification_documents (
  kind TEXT NOT NULL CHECK (kind IN ('provider', 'rule')),
  id TEXT NOT NULL,
  document_json TEXT NOT NULL,
  sort_score INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL,
  PRIMARY KEY (kind, id)
);
CREATE INDEX idx_notification_documents_kind_score
  ON notification_documents(kind, sort_score DESC, id DESC);
"#;
const MIGRATIONS_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS typed_notification_schema_migrations (
  version INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  checksum TEXT NOT NULL,
  applied_at_ms INTEGER NOT NULL
);
"#;
const HISTORY_SCHEMA_VERSION: i64 = 2;
const HISTORY_SCHEMA_NAME: &str = "typed_notification_history";
const HISTORY_TRACE_SCHEMA_VERSION: i64 = 3;
const HISTORY_TRACE_SCHEMA_NAME: &str = "typed_notification_history_trace_index";
const HISTORY_TRACE_SCHEMA_SQL: &str = r#"
ALTER TABLE notification_history_documents ADD COLUMN trace_id TEXT;
CREATE INDEX idx_notification_history_trace
  ON notification_history_documents(kind, trace_id, sort_score DESC, id DESC);
"#;
const HISTORY_SCHEMA_SQL: &str = r#"
CREATE TABLE notification_history_documents (
  kind TEXT NOT NULL CHECK (kind IN ('trigger', 'delivery')),
  id TEXT NOT NULL,
  document_json TEXT NOT NULL,
  sort_score INTEGER NOT NULL,
  expires_at_ms INTEGER NOT NULL CHECK (expires_at_ms >= 0),
  updated_at_ms INTEGER NOT NULL,
  PRIMARY KEY (kind, id)
);
CREATE INDEX idx_notification_history_kind_score
  ON notification_history_documents(kind, sort_score DESC, id DESC);
CREATE INDEX idx_notification_history_expires
  ON notification_history_documents(expires_at_ms);
"#;

#[derive(Clone)]
pub(crate) struct TypedNotificationRepository {
    manager: ConnectionManager,
}

impl TypedNotificationRepository {
    pub(crate) fn new(manager: ConnectionManager) -> Self {
        Self { manager }
    }

    pub(crate) async fn initialize(&self) -> StorageResult<()> {
        self.manager
            .call(move |conn| {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                tx.execute_batch(MIGRATIONS_SQL)?;
                let checksum = crate::crypto_utils::sha256_hex_bytes(SCHEMA_SQL);
                let applied = tx
                    .query_row(
                        "SELECT name, checksum FROM typed_notification_schema_migrations WHERE version = ?1",
                        [SCHEMA_VERSION],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                    )
                    .optional()?;
                match applied {
                    Some((name, stored)) if name == SCHEMA_NAME && stored == checksum => {
                        let exists = tx.query_row(
                            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'notification_documents')",
                            [],
                            |row| row.get::<_, bool>(0),
                        )?;
                        if !exists {
                            return Err(storage_error(
                                "typed notification migration is recorded but its table is missing",
                            ));
                        }
                    }
                    Some((name, _)) if name != SCHEMA_NAME => {
                        return Err(storage_error("typed notification migration name mismatch"));
                    }
                    Some(_) => {
                        return Err(storage_error("typed notification migration checksum mismatch"));
                    }
                    None => {
                        tx.execute_batch(SCHEMA_SQL)?;
                        tx.execute(
                            "INSERT INTO typed_notification_schema_migrations(version, name, checksum, applied_at_ms) VALUES (?1, ?2, ?3, ?4)",
                            params![SCHEMA_VERSION, SCHEMA_NAME, checksum, crate::time_utils::now_ms()],
                        )?;
                    }
                }
                let history_checksum = crate::crypto_utils::sha256_hex_bytes(HISTORY_SCHEMA_SQL);
                let history_applied = tx
                    .query_row(
                        "SELECT name, checksum FROM typed_notification_schema_migrations WHERE version = ?1",
                        [HISTORY_SCHEMA_VERSION],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                    )
                    .optional()?;
                match history_applied {
                    Some((name, checksum))
                        if name == HISTORY_SCHEMA_NAME && checksum == history_checksum =>
                    {
                        let exists = tx.query_row(
                            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'notification_history_documents')",
                            [],
                            |row| row.get::<_, bool>(0),
                        )?;
                        if !exists {
                            return Err(storage_error(
                                "typed notification history migration is recorded but its table is missing",
                            ));
                        }
                    }
                    Some((name, _)) if name != HISTORY_SCHEMA_NAME => {
                        return Err(storage_error(
                            "typed notification history migration name mismatch",
                        ));
                    }
                    Some(_) => {
                        return Err(storage_error(
                            "typed notification history migration checksum mismatch",
                        ));
                    }
                    None => {
                        tx.execute_batch(HISTORY_SCHEMA_SQL)?;
                        tx.execute(
                            "INSERT INTO typed_notification_schema_migrations(version, name, checksum, applied_at_ms) VALUES (?1, ?2, ?3, ?4)",
                            params![HISTORY_SCHEMA_VERSION, HISTORY_SCHEMA_NAME, history_checksum, crate::time_utils::now_ms()],
                        )?;
                    }
                }
                let trace_checksum =
                    crate::crypto_utils::sha256_hex_bytes(HISTORY_TRACE_SCHEMA_SQL);
                let trace_applied = tx
                    .query_row(
                        "SELECT name, checksum FROM typed_notification_schema_migrations WHERE version = ?1",
                        [HISTORY_TRACE_SCHEMA_VERSION],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                    )
                    .optional()?;
                match trace_applied {
                    Some((name, checksum))
                        if name == HISTORY_TRACE_SCHEMA_NAME && checksum == trace_checksum => {}
                    Some((name, _)) if name != HISTORY_TRACE_SCHEMA_NAME => {
                        return Err(storage_error("typed notification trace migration name mismatch"));
                    }
                    Some(_) => {
                        return Err(storage_error("typed notification trace migration checksum mismatch"));
                    }
                    None => {
                        tx.execute_batch(HISTORY_TRACE_SCHEMA_SQL)?;
                        tx.execute(
                            "INSERT INTO typed_notification_schema_migrations(version, name, checksum, applied_at_ms) VALUES (?1, ?2, ?3, ?4)",
                            params![
                                HISTORY_TRACE_SCHEMA_VERSION,
                                HISTORY_TRACE_SCHEMA_NAME,
                                trace_checksum,
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
        kind: &str,
        id: &str,
        document_json: &str,
        sort_score: i64,
    ) -> StorageResult<()> {
        let _: Value = serde_json::from_str(document_json)?;
        if id.trim().is_empty() || !matches!(kind, "provider" | "rule") {
            return Err(storage_error("invalid typed notification record identity"));
        }
        tx.execute(
            "INSERT INTO notification_documents(kind, id, document_json, sort_score, updated_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(kind, id) DO UPDATE SET
               document_json = excluded.document_json,
               sort_score = excluded.sort_score,
               updated_at_ms = excluded.updated_at_ms",
            params![
                kind,
                id,
                document_json,
                sort_score,
                crate::time_utils::now_ms()
            ],
        )?;
        Ok(())
    }

    pub(crate) fn delete_tx(tx: &Transaction<'_>, kind: &str, id: &str) -> StorageResult<()> {
        tx.execute(
            "DELETE FROM notification_documents WHERE kind = ?1 AND id = ?2",
            params![kind, id],
        )?;
        Ok(())
    }

    pub(crate) async fn rebuild_from_legacy(
        &self,
        records: Vec<(String, String, String)>,
    ) -> StorageResult<()> {
        self.manager
            .call(move |conn| {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                for (kind, index_key, data_prefix) in records {
                    tx.execute(
                        "DELETE FROM notification_documents WHERE kind = ?1",
                        [&kind],
                    )?;
                    let mut statement = tx.prepare(
                        "SELECT indexed.member, indexed.score, data.value
                         FROM kv_zset AS indexed
                         JOIN kv_strings AS data ON data.key = ?2 || indexed.member
                         WHERE indexed.key = ?1
                         ORDER BY indexed.score DESC, indexed.member DESC",
                    )?;
                    let rows = statement.query_map(params![index_key, data_prefix], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, f64>(1)?,
                            row.get::<_, String>(2)?,
                        ))
                    })?;
                    for row in rows {
                        let (id, score, document_json) = row?;
                        if serde_json::from_str::<Value>(&document_json).is_err() {
                            continue;
                        }
                        Self::upsert_tx(&tx, &kind, &id, &document_json, score as i64)?;
                    }
                    drop(statement);
                }
                tx.commit()?;
                Ok(())
            })
            .await
    }

    pub(crate) async fn rebuild_history_from_legacy(
        &self,
        records: Vec<(String, String, String)>,
    ) -> StorageResult<()> {
        self.manager
            .call(move |conn| {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                for (kind, index_key, data_prefix) in records {
                    tx.execute(
                        "DELETE FROM notification_history_documents WHERE kind = ?1",
                        [&kind],
                    )?;
                    let mut statement = tx.prepare(
                        "SELECT indexed.member, indexed.score, data.value, metadata.expires_at_ms
                         FROM kv_zset AS indexed
                         JOIN kv_strings AS data ON data.key = ?2 || indexed.member
                         JOIN kv_keys AS metadata ON metadata.key = data.key
                         WHERE indexed.key = ?1 AND metadata.expires_at_ms > ?3
                         ORDER BY indexed.score DESC, indexed.member DESC",
                    )?;
                    let rows = statement.query_map(
                        params![index_key, data_prefix, crate::time_utils::now_ms()],
                        |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, f64>(1)?,
                                row.get::<_, String>(2)?,
                                row.get::<_, i64>(3)?,
                            ))
                        },
                    )?;
                    for row in rows {
                        let (id, score, document_json, expires_at_ms) = row?;
                        Self::upsert_history_tx(
                            &tx,
                            &kind,
                            &id,
                            &document_json,
                            score as i64,
                            expires_at_ms,
                        )?;
                    }
                    drop(statement);
                }
                tx.commit()?;
                Ok(())
            })
            .await
    }

    pub(crate) fn upsert_history_tx(
        tx: &Transaction<'_>,
        kind: &str,
        id: &str,
        document_json: &str,
        sort_score: i64,
        expires_at_ms: i64,
    ) -> StorageResult<()> {
        let document: Value = serde_json::from_str(document_json)?;
        let trace_id = crate::trace_id::record_trace_id(&document);
        if id.trim().is_empty() || !matches!(kind, "trigger" | "delivery") {
            return Err(storage_error("invalid typed notification history identity"));
        }
        tx.execute(
            "INSERT INTO notification_history_documents(kind, id, document_json, sort_score, expires_at_ms, updated_at_ms, trace_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(kind, id) DO UPDATE SET document_json = excluded.document_json,
               sort_score = excluded.sort_score, expires_at_ms = excluded.expires_at_ms,
               updated_at_ms = excluded.updated_at_ms, trace_id = excluded.trace_id",
            params![
                kind,
                id,
                document_json,
                sort_score,
                expires_at_ms.max(0),
                crate::time_utils::now_ms(),
                trace_id,
            ],
        )?;
        Ok(())
    }

    pub(crate) fn trim_history_tx(
        tx: &Transaction<'_>,
        kind: &str,
        cutoff_score: i64,
        max_records: i64,
    ) -> StorageResult<()> {
        tx.execute(
            "DELETE FROM notification_history_documents
             WHERE kind = ?1 AND (expires_at_ms <= ?2 OR sort_score < ?3)",
            params![kind, crate::time_utils::now_ms(), cutoff_score],
        )?;
        tx.execute(
            "DELETE FROM notification_history_documents
             WHERE kind = ?1 AND id IN (
               SELECT id FROM notification_history_documents
               WHERE kind = ?1
               ORDER BY sort_score DESC, id DESC
               LIMIT -1 OFFSET ?2
             )",
            params![kind, max_records.max(1)],
        )?;
        Ok(())
    }

    pub(crate) fn delete_history_tx(
        tx: &Transaction<'_>,
        kind: &str,
        ids: &[String],
    ) -> StorageResult<()> {
        for id in ids {
            tx.execute(
                "DELETE FROM notification_history_documents WHERE kind = ?1 AND id = ?2",
                params![kind, id],
            )?;
        }
        Ok(())
    }

    pub(crate) async fn load_history(&self, kind: &str) -> StorageResult<Vec<Value>> {
        let kind = kind.to_string();
        self.manager
            .call(move |conn| {
                let mut statement = conn.prepare(
                    "SELECT document_json FROM notification_history_documents
                     WHERE kind = ?1 AND expires_at_ms > ?2
                     ORDER BY sort_score DESC, id DESC",
                )?;
                let rows = statement
                    .query_map(params![kind, crate::time_utils::now_ms()], |row| {
                        row.get::<_, String>(0)
                    })?;
                let mut documents = Vec::new();
                for row in rows {
                    documents.push(serde_json::from_str(&row?).map_err(|error| {
                        storage_error(format!("typed notification history is invalid: {error}"))
                    })?);
                }
                Ok(documents)
            })
            .await
    }

    pub(crate) async fn load_history_by_trace(
        &self,
        kind: &str,
        trace_id: &str,
    ) -> StorageResult<Vec<Value>> {
        let kind = kind.to_string();
        let trace_id = trace_id.to_string();
        self.manager
            .call(move |conn| {
                let mut statement = conn.prepare(
                    "SELECT document_json FROM notification_history_documents
                     WHERE kind = ?1 AND trace_id = ?2 AND expires_at_ms > ?3
                     ORDER BY sort_score ASC, id ASC",
                )?;
                let rows = statement.query_map(
                    params![kind, trace_id, crate::time_utils::now_ms()],
                    |row| row.get::<_, String>(0),
                )?;
                let mut documents = Vec::new();
                for row in rows {
                    documents.push(serde_json::from_str(&row?)?);
                }
                Ok(documents)
            })
            .await
    }

    pub(crate) async fn load_history_one(
        &self,
        kind: &str,
        id: &str,
    ) -> StorageResult<Option<Value>> {
        let kind = kind.to_string();
        let id = id.to_string();
        self.manager
            .call(move |conn| {
                let raw = conn
                    .query_row(
                        "SELECT document_json FROM notification_history_documents
                         WHERE kind = ?1 AND id = ?2 AND expires_at_ms > ?3",
                        params![kind, id, crate::time_utils::now_ms()],
                        |row| row.get::<_, String>(0),
                    )
                    .optional()?;
                raw.map(|raw| {
                    serde_json::from_str(&raw).map_err(|error| {
                        storage_error(format!(
                            "typed notification history record is invalid: {error}"
                        ))
                    })
                })
                .transpose()
            })
            .await
    }

    #[cfg(test)]
    pub(crate) async fn count_history(&self, kind: &str) -> StorageResult<i64> {
        let kind = kind.to_string();
        self.manager
            .call(move |conn| {
                conn.query_row(
                    "SELECT COUNT(*) FROM notification_history_documents WHERE kind = ?1 AND expires_at_ms > ?2",
                    params![kind, crate::time_utils::now_ms()],
                    |row| row.get(0),
                )
                .map_err(Into::into)
            })
            .await
    }

    pub(crate) async fn load_kind(&self, kind: &str) -> StorageResult<Vec<Value>> {
        let kind = kind.to_string();
        self.manager
            .call(move |conn| {
                let mut statement = conn.prepare(
                    "SELECT document_json FROM notification_documents
                     WHERE kind = ?1
                     ORDER BY sort_score DESC, id DESC",
                )?;
                let rows = statement.query_map([kind], |row| row.get::<_, String>(0))?;
                let mut documents = Vec::new();
                for row in rows {
                    let raw = row?;
                    documents.push(serde_json::from_str(&raw).map_err(|error| {
                        storage_error(format!("typed notification document is invalid: {error}"))
                    })?);
                }
                Ok(documents)
            })
            .await
    }

    pub(crate) async fn load_one(&self, kind: &str, id: &str) -> StorageResult<Option<Value>> {
        let kind = kind.to_string();
        let id = id.to_string();
        self.manager
            .call(move |conn| {
                let raw = conn
                    .query_row(
                        "SELECT document_json FROM notification_documents WHERE kind = ?1 AND id = ?2",
                        params![kind, id],
                        |row| row.get::<_, String>(0),
                    )
                    .optional()?;
                raw.map(|raw| {
                    serde_json::from_str(&raw).map_err(|error| {
                        storage_error(format!("typed notification document is invalid: {error}"))
                    })
                })
                .transpose()
            })
            .await
    }

    #[cfg(test)]
    pub(crate) async fn count_kind(&self, kind: &str) -> StorageResult<i64> {
        let kind = kind.to_string();
        self.manager
            .call(move |conn| {
                conn.query_row(
                    "SELECT COUNT(*) FROM notification_documents WHERE kind = ?1",
                    [kind],
                    |row| row.get(0),
                )
                .map_err(Into::into)
            })
            .await
    }
}
