use serde_json::Value;
use tokio_rusqlite::rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use super::{StorageResult, redis_compat::ConnectionManager, storage_error};

const SCHEMA_VERSION: i64 = 1;
const SCHEMA_NAME: &str = "typed_whitelist_documents";
const SCHEMA_SQL: &str = r#"
CREATE TABLE whitelist_documents (
  kind TEXT NOT NULL CHECK (kind IN ('record', 'region')),
  id TEXT NOT NULL,
  document_json TEXT NOT NULL,
  sort_score INTEGER NOT NULL,
  expires_at INTEGER,
  status TEXT NOT NULL,
  updated_at_ms INTEGER NOT NULL,
  PRIMARY KEY (kind, id)
);
CREATE INDEX idx_whitelist_documents_kind_score
  ON whitelist_documents(kind, sort_score DESC, id DESC);
CREATE INDEX idx_whitelist_documents_kind_expiry
  ON whitelist_documents(kind, expires_at)
  WHERE expires_at IS NOT NULL;
"#;
const MIGRATIONS_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS typed_whitelist_schema_migrations (
  version INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  checksum TEXT NOT NULL,
  applied_at_ms INTEGER NOT NULL
);
"#;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TypedWhitelistDocument {
    pub(crate) kind: &'static str,
    pub(crate) id: String,
    pub(crate) document_json: String,
    pub(crate) sort_score: i64,
    pub(crate) expires_at: Option<i64>,
    pub(crate) status: String,
}

#[derive(Clone)]
pub(crate) struct TypedWhitelistRepository {
    manager: ConnectionManager,
}

impl TypedWhitelistRepository {
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
                        "SELECT name, checksum FROM typed_whitelist_schema_migrations WHERE version = ?1",
                        [SCHEMA_VERSION],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                    )
                    .optional()?;
                match applied {
                    Some((name, stored)) if name == SCHEMA_NAME && stored == checksum => {
                        let exists = tx.query_row(
                            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'whitelist_documents')",
                            [],
                            |row| row.get::<_, bool>(0),
                        )?;
                        if !exists {
                            return Err(storage_error(
                                "typed whitelist migration is recorded but its table is missing",
                            ));
                        }
                    }
                    Some((name, _)) if name != SCHEMA_NAME => {
                        return Err(storage_error("typed whitelist migration name mismatch"));
                    }
                    Some(_) => {
                        return Err(storage_error("typed whitelist migration checksum mismatch"));
                    }
                    None => {
                        tx.execute_batch(SCHEMA_SQL)?;
                        tx.execute(
                            "INSERT INTO typed_whitelist_schema_migrations(version, name, checksum, applied_at_ms) VALUES (?1, ?2, ?3, ?4)",
                            params![
                                SCHEMA_VERSION,
                                SCHEMA_NAME,
                                checksum,
                                crate::time_utils::now_ms()
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
        document: &TypedWhitelistDocument,
    ) -> StorageResult<()> {
        let parsed: Value = serde_json::from_str(&document.document_json)?;
        if !parsed.is_object()
            || document.id.trim().is_empty()
            || !matches!(document.kind, "record" | "region")
            || document.status.trim().is_empty()
        {
            return Err(storage_error("invalid typed whitelist document"));
        }
        tx.execute(
            "INSERT INTO whitelist_documents(
               kind, id, document_json, sort_score, expires_at, status, updated_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(kind, id) DO UPDATE SET
               document_json = excluded.document_json,
               sort_score = excluded.sort_score,
               expires_at = excluded.expires_at,
               status = excluded.status,
               updated_at_ms = excluded.updated_at_ms",
            params![
                document.kind,
                document.id,
                document.document_json,
                document.sort_score,
                document.expires_at,
                document.status,
                crate::time_utils::now_ms(),
            ],
        )?;
        Ok(())
    }

    pub(crate) fn delete_tx(tx: &Transaction<'_>, kind: &str, id: &str) -> StorageResult<()> {
        tx.execute(
            "DELETE FROM whitelist_documents WHERE kind = ?1 AND id = ?2",
            params![kind, id],
        )?;
        Ok(())
    }

    pub(crate) fn delete_kind_tx(tx: &Transaction<'_>, kind: &str) -> StorageResult<()> {
        tx.execute("DELETE FROM whitelist_documents WHERE kind = ?1", [kind])?;
        Ok(())
    }

    pub(crate) fn replace_all_tx(
        tx: &Transaction<'_>,
        documents: &[TypedWhitelistDocument],
    ) -> StorageResult<()> {
        tx.execute("DELETE FROM whitelist_documents", [])?;
        for document in documents {
            Self::upsert_tx(tx, document)?;
        }
        Ok(())
    }

    pub(crate) async fn load_one(
        &self,
        kind: &str,
        id: &str,
    ) -> StorageResult<Option<TypedWhitelistDocument>> {
        let kind = kind.to_string();
        let id = id.to_string();
        self.manager
            .call(move |conn| {
                conn.query_row(
                    "SELECT document_json, sort_score, expires_at, status
                     FROM whitelist_documents WHERE kind = ?1 AND id = ?2",
                    params![kind, id],
                    |row| {
                        Ok(TypedWhitelistDocument {
                            kind: if kind == "record" { "record" } else { "region" },
                            id: id.clone(),
                            document_json: row.get(0)?,
                            sort_score: row.get(1)?,
                            expires_at: row.get(2)?,
                            status: row.get(3)?,
                        })
                    },
                )
                .optional()
                .map_err(Into::into)
            })
            .await
    }

    pub(crate) async fn load_kind(&self, kind: &str) -> StorageResult<Vec<TypedWhitelistDocument>> {
        let kind = kind.to_string();
        self.manager
            .call(move |conn| {
                let mut statement = conn.prepare(
                    "SELECT id, document_json, sort_score, expires_at, status
                     FROM whitelist_documents
                     WHERE kind = ?1
                     ORDER BY sort_score DESC, id DESC",
                )?;
                let rows = statement.query_map([kind.as_str()], |row| {
                    Ok(TypedWhitelistDocument {
                        kind: if kind == "record" { "record" } else { "region" },
                        id: row.get(0)?,
                        document_json: row.get(1)?,
                        sort_score: row.get(2)?,
                        expires_at: row.get(3)?,
                        status: row.get(4)?,
                    })
                })?;
                rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
            })
            .await
    }

    #[cfg(test)]
    pub(crate) async fn count(&self) -> StorageResult<i64> {
        self.manager
            .call(|conn| {
                conn.query_row("SELECT COUNT(*) FROM whitelist_documents", [], |row| {
                    row.get::<_, i64>(0)
                })
                .map_err(Into::into)
            })
            .await
    }
}
