use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use tokio_rusqlite::rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use super::{StorageResult, redis_compat::ConnectionManager, storage_error};

pub(crate) const SESSION_PREFIX: &str = "fn_knock:docker_admin:session:v1:";
pub(crate) const LOGIN_BACKOFF_PREFIX: &str = "fn_knock:docker_admin:login_backoff:v1:";

const SCHEMA_VERSION: i64 = 1;
const SCHEMA_NAME: &str = "typed_docker_admin_security_state";
const SCHEMA_SQL: &str = r#"
CREATE TABLE docker_admin_session_documents (
  session_id TEXT PRIMARY KEY CHECK (session_id <> ''),
  session_json TEXT NOT NULL,
  expires_at_ms INTEGER NOT NULL CHECK (expires_at_ms >= 0),
  updated_at_ms INTEGER NOT NULL
);
CREATE INDEX idx_docker_admin_session_documents_expiry
  ON docker_admin_session_documents(expires_at_ms);
CREATE TABLE docker_admin_login_backoff_attempts (
  ip TEXT PRIMARY KEY CHECK (ip <> ''),
  attempt_json TEXT NOT NULL,
  expires_at_ms INTEGER NOT NULL CHECK (expires_at_ms >= 0),
  updated_at_ms INTEGER NOT NULL
);
CREATE INDEX idx_docker_admin_login_backoff_expiry
  ON docker_admin_login_backoff_attempts(expires_at_ms);
"#;
const MIGRATIONS_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS typed_docker_admin_schema_migrations (
  version INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  checksum TEXT NOT NULL,
  applied_at_ms INTEGER NOT NULL
);
"#;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TypedDockerAdminRecord {
    pub(crate) id: String,
    pub(crate) document_json: String,
    pub(crate) expires_at_ms: i64,
}

#[derive(Clone, Copy)]
enum RecordKind {
    Session,
    LoginBackoff,
}

impl RecordKind {
    fn prefix(self) -> &'static str {
        match self {
            Self::Session => SESSION_PREFIX,
            Self::LoginBackoff => LOGIN_BACKOFF_PREFIX,
        }
    }

    fn table(self) -> &'static str {
        match self {
            Self::Session => "docker_admin_session_documents",
            Self::LoginBackoff => "docker_admin_login_backoff_attempts",
        }
    }

    fn id_column(self) -> &'static str {
        match self {
            Self::Session => "session_id",
            Self::LoginBackoff => "ip",
        }
    }

    fn json_column(self) -> &'static str {
        match self {
            Self::Session => "session_json",
            Self::LoginBackoff => "attempt_json",
        }
    }

    fn valid(self, id: &str, raw: &str) -> bool {
        let Ok(value) = serde_json::from_str::<Value>(raw) else {
            return false;
        };
        let Some(object) = value.as_object() else {
            return false;
        };
        match self {
            Self::Session => {
                object.get("id").and_then(Value::as_str) == Some(id)
                    && object
                        .get("expires_at")
                        .and_then(Value::as_str)
                        .is_some_and(|value| !value.is_empty())
                    && object.get("ip").and_then(Value::as_str).is_some()
                    && object.get("user_agent").and_then(Value::as_str).is_some()
            }
            Self::LoginBackoff => {
                object.get("ip").and_then(Value::as_str) == Some(id)
                    && object.get("attempts").and_then(Value::as_u64).is_some()
                    && object
                        .get("blocked_until")
                        .and_then(Value::as_i64)
                        .is_some()
            }
        }
    }
}

#[derive(Clone)]
pub(crate) struct TypedDockerAdminRepository {
    manager: ConnectionManager,
}

impl TypedDockerAdminRepository {
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
                        "SELECT name, checksum FROM typed_docker_admin_schema_migrations WHERE version = ?1",
                        [SCHEMA_VERSION],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                    )
                    .optional()?;
                match applied {
                    Some((name, stored)) if name == SCHEMA_NAME && stored == checksum => {
                        for table in [
                            "docker_admin_session_documents",
                            "docker_admin_login_backoff_attempts",
                        ] {
                            let exists = tx.query_row(
                                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
                                [table],
                                |row| row.get::<_, bool>(0),
                            )?;
                            if !exists {
                                return Err(storage_error(format!(
                                    "typed Docker-admin migration is recorded but {table} is missing"
                                )));
                            }
                        }
                    }
                    Some((name, _)) if name != SCHEMA_NAME => {
                        return Err(storage_error("typed Docker-admin migration name mismatch"));
                    }
                    Some(_) => {
                        return Err(storage_error(
                            "typed Docker-admin migration checksum mismatch",
                        ));
                    }
                    None => {
                        tx.execute_batch(SCHEMA_SQL)?;
                        tx.execute(
                            "INSERT INTO typed_docker_admin_schema_migrations(version, name, checksum, applied_at_ms) VALUES (?1, ?2, ?3, ?4)",
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
        for kind in [RecordKind::Session, RecordKind::LoginBackoff] {
            reconcile_all_kind_tx(tx, kind)?;
        }
        Ok(())
    }

    pub(crate) fn reconcile_legacy_keys_tx(
        tx: &Transaction<'_>,
        keys: &[String],
    ) -> StorageResult<()> {
        for key in keys {
            let kind_and_id = [RecordKind::Session, RecordKind::LoginBackoff]
                .into_iter()
                .find_map(|kind| key.strip_prefix(kind.prefix()).map(|id| (kind, id)));
            let Some((kind, id)) = kind_and_id else {
                continue;
            };
            if id.is_empty() {
                return Self::rebuild_from_legacy_tx(tx);
            }
            match live_legacy_record_tx(tx, kind, id)? {
                Some(record) => upsert_tx(tx, kind, &record)?,
                None => delete_typed_tx(tx, kind, id)?,
            }
        }
        Ok(())
    }

    pub(crate) async fn verify_and_repair_session(&self, session_id: &str) -> StorageResult<bool> {
        self.verify_and_repair_one(RecordKind::Session, session_id)
            .await
    }

    pub(crate) async fn verify_and_repair_login_backoff(&self, ip: &str) -> StorageResult<bool> {
        self.verify_and_repair_one(RecordKind::LoginBackoff, ip)
            .await
    }

    async fn verify_and_repair_one(&self, kind: RecordKind, id: &str) -> StorageResult<bool> {
        let id = id.to_string();
        self.manager
            .call(move |conn| {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                let raw = live_legacy_raw_tx(&tx, kind, &id)?;
                let invalid = raw
                    .as_ref()
                    .is_some_and(|(document, _)| !kind.valid(&id, document));
                let legacy = raw.and_then(|(document_json, expires_at_ms)| {
                    kind.valid(&id, &document_json)
                        .then_some(TypedDockerAdminRecord {
                            id: id.clone(),
                            document_json,
                            expires_at_ms,
                        })
                });
                let typed = typed_record_tx(&tx, kind, &id)?;
                let matched = !invalid && typed == legacy;
                if !matched {
                    match legacy {
                        Some(record) => upsert_tx(&tx, kind, &record)?,
                        None => delete_typed_tx(&tx, kind, &id)?,
                    }
                }
                tx.commit()?;
                Ok(matched)
            })
            .await
    }

    #[cfg(test)]
    pub(crate) async fn load_session(
        &self,
        session_id: &str,
    ) -> StorageResult<Option<TypedDockerAdminRecord>> {
        self.load(RecordKind::Session, session_id).await
    }

    #[cfg(test)]
    pub(crate) async fn load_login_backoff(
        &self,
        ip: &str,
    ) -> StorageResult<Option<TypedDockerAdminRecord>> {
        self.load(RecordKind::LoginBackoff, ip).await
    }

    #[cfg(test)]
    async fn load(
        &self,
        kind: RecordKind,
        id: &str,
    ) -> StorageResult<Option<TypedDockerAdminRecord>> {
        let id = id.to_string();
        self.manager
            .call(move |conn| typed_record_conn(conn, kind, &id))
            .await
    }

    #[cfg(test)]
    pub(crate) async fn counts(&self) -> StorageResult<(i64, i64)> {
        self.manager
            .call(|conn| {
                let sessions = conn.query_row(
                    "SELECT COUNT(*) FROM docker_admin_session_documents",
                    [],
                    |row| row.get::<_, i64>(0),
                )?;
                let backoffs = conn.query_row(
                    "SELECT COUNT(*) FROM docker_admin_login_backoff_attempts",
                    [],
                    |row| row.get::<_, i64>(0),
                )?;
                Ok((sessions, backoffs))
            })
            .await
    }
}

fn reconcile_all_kind_tx(tx: &Transaction<'_>, kind: RecordKind) -> StorageResult<()> {
    let legacy = legacy_records_tx(tx, kind)?;
    let table = kind.table();
    let id_column = kind.id_column();
    let mut stale = {
        let sql = format!("SELECT {id_column} FROM {table}");
        let mut statement = tx.prepare(&sql)?;
        statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<BTreeSet<_>, _>>()?
    };
    for record in legacy.values() {
        stale.remove(&record.id);
        upsert_tx(tx, kind, record)?;
    }
    for id in stale {
        delete_typed_tx(tx, kind, &id)?;
    }
    Ok(())
}

fn live_legacy_record_tx(
    tx: &Transaction<'_>,
    kind: RecordKind,
    id: &str,
) -> StorageResult<Option<TypedDockerAdminRecord>> {
    let Some((document_json, expires_at_ms)) = live_legacy_raw_tx(tx, kind, id)? else {
        return Ok(None);
    };
    if !kind.valid(id, &document_json) {
        return Ok(None);
    }
    Ok(Some(TypedDockerAdminRecord {
        id: id.to_string(),
        document_json,
        expires_at_ms,
    }))
}

fn live_legacy_raw_tx(
    tx: &Transaction<'_>,
    kind: RecordKind,
    id: &str,
) -> StorageResult<Option<(String, i64)>> {
    let key = format!("{}{id}", kind.prefix());
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

fn legacy_records_tx(
    tx: &Transaction<'_>,
    kind: RecordKind,
) -> StorageResult<BTreeMap<String, TypedDockerAdminRecord>> {
    let pattern = format!("{}%", kind.prefix());
    let mut statement = tx.prepare(
        "SELECT keys.key, strings.value, keys.expires_at_ms
         FROM kv_keys AS keys
         JOIN kv_strings AS strings ON strings.key = keys.key
         WHERE keys.key LIKE ?1
           AND keys.kind = 'string'
           AND keys.expires_at_ms IS NOT NULL
           AND keys.expires_at_ms > ?2
         ORDER BY keys.key",
    )?;
    let rows = statement.query_map(params![pattern, crate::time_utils::now_ms()], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;
    let mut records = BTreeMap::new();
    for row in rows {
        let (key, document_json, expires_at_ms) = row?;
        let Some(id) = key.strip_prefix(kind.prefix()) else {
            continue;
        };
        if id.is_empty() || !kind.valid(id, &document_json) {
            continue;
        }
        records.insert(
            id.to_string(),
            TypedDockerAdminRecord {
                id: id.to_string(),
                document_json,
                expires_at_ms,
            },
        );
    }
    Ok(records)
}

fn typed_record_tx(
    tx: &Transaction<'_>,
    kind: RecordKind,
    id: &str,
) -> StorageResult<Option<TypedDockerAdminRecord>> {
    let sql = format!(
        "SELECT {}, expires_at_ms FROM {} WHERE {} = ?1",
        kind.json_column(),
        kind.table(),
        kind.id_column()
    );
    tx.query_row(&sql, [id], |row| {
        Ok(TypedDockerAdminRecord {
            id: id.to_string(),
            document_json: row.get(0)?,
            expires_at_ms: row.get(1)?,
        })
    })
    .optional()
    .map_err(Into::into)
}

#[cfg(test)]
fn typed_record_conn(
    conn: &tokio_rusqlite::rusqlite::Connection,
    kind: RecordKind,
    id: &str,
) -> StorageResult<Option<TypedDockerAdminRecord>> {
    let sql = format!(
        "SELECT {}, expires_at_ms FROM {} WHERE {} = ?1",
        kind.json_column(),
        kind.table(),
        kind.id_column()
    );
    conn.query_row(&sql, [id], |row| {
        Ok(TypedDockerAdminRecord {
            id: id.to_string(),
            document_json: row.get(0)?,
            expires_at_ms: row.get(1)?,
        })
    })
    .optional()
    .map_err(Into::into)
}

fn upsert_tx(
    tx: &Transaction<'_>,
    kind: RecordKind,
    record: &TypedDockerAdminRecord,
) -> StorageResult<()> {
    if record.id.is_empty() || !kind.valid(&record.id, &record.document_json) {
        return Err(storage_error("invalid typed Docker-admin security record"));
    }
    let sql = format!(
        "INSERT INTO {}({}, {}, expires_at_ms, updated_at_ms)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT({}) DO UPDATE SET
           {} = excluded.{},
           expires_at_ms = excluded.expires_at_ms,
           updated_at_ms = excluded.updated_at_ms
         WHERE {}.{} <> excluded.{}
            OR {}.expires_at_ms <> excluded.expires_at_ms",
        kind.table(),
        kind.id_column(),
        kind.json_column(),
        kind.id_column(),
        kind.json_column(),
        kind.json_column(),
        kind.table(),
        kind.json_column(),
        kind.json_column(),
        kind.table(),
    );
    tx.execute(
        &sql,
        params![
            record.id,
            record.document_json,
            record.expires_at_ms,
            crate::time_utils::now_ms(),
        ],
    )?;
    Ok(())
}

fn delete_typed_tx(tx: &Transaction<'_>, kind: RecordKind, id: &str) -> StorageResult<()> {
    let sql = format!(
        "DELETE FROM {} WHERE {} = ?1",
        kind.table(),
        kind.id_column()
    );
    tx.execute(&sql, [id])?;
    Ok(())
}
