use serde_json::Value;
use tokio_rusqlite::rusqlite::{
    Connection, OptionalExtension, Transaction, TransactionBehavior, params,
};

use super::{
    StorageResult,
    redis_compat::{ConnectionManager, string_get_tx},
    storage_error,
};

const TYPED_SCHEMA_VERSION: i64 = 1;
const TYPED_SCHEMA_NAME: &str = "typed_config_document";
const TYPED_SCHEMA_SQL: &str = r#"
CREATE TABLE config_documents (
  singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
  document_json TEXT NOT NULL,
  host_mappings_generation INTEGER NOT NULL CHECK (host_mappings_generation >= 0),
  revision INTEGER NOT NULL CHECK (revision > 0),
  updated_at_ms INTEGER NOT NULL
);
"#;

const TYPED_MIGRATIONS_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS typed_schema_migrations (
  version INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  checksum TEXT NOT NULL,
  applied_at_ms INTEGER NOT NULL
);
"#;

#[derive(Clone)]
pub(crate) struct TypedConfigRepository {
    manager: ConnectionManager,
}

#[derive(Clone, Debug)]
pub(crate) struct TypedConfigDocument {
    pub(crate) document: Value,
    pub(crate) host_mappings_generation: u64,
    pub(crate) revision: u64,
}

pub(crate) struct LegacyConfigRawSnapshot {
    pub(crate) config_raw: Option<String>,
    pub(crate) generation_raw: Option<String>,
}

pub(crate) struct TypedConfigShadowSnapshot {
    pub(crate) legacy: LegacyConfigRawSnapshot,
    pub(crate) typed: StorageResult<Option<TypedConfigDocument>>,
}

pub(crate) struct ReconciledLegacyConfig {
    pub(crate) legacy: LegacyConfigRawSnapshot,
    pub(crate) typed_revision: u64,
}

impl TypedConfigRepository {
    pub(crate) fn new(manager: ConnectionManager) -> Self {
        Self { manager }
    }

    pub(crate) async fn initialize(&self) -> StorageResult<()> {
        self.manager
            .call(move |conn| {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                tx.execute_batch(TYPED_MIGRATIONS_SQL)?;
                let latest: Option<i64> = tx.query_row(
                    "SELECT MAX(version) FROM typed_schema_migrations",
                    [],
                    |row| row.get(0),
                )?;
                if let Some(version) = latest.filter(|version| *version > TYPED_SCHEMA_VERSION) {
                    return Err(storage_error(format!(
                        "typed SQLite schema version {version} is newer than this server supports ({TYPED_SCHEMA_VERSION})"
                    )));
                }

                let expected_checksum = crate::crypto_utils::sha256_hex_bytes(TYPED_SCHEMA_SQL);
                let applied = tx
                    .query_row(
                        "SELECT name, checksum FROM typed_schema_migrations WHERE version = ?1",
                        params![TYPED_SCHEMA_VERSION],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                    )
                    .optional()?;
                match applied {
                    Some((name, checksum))
                        if name == TYPED_SCHEMA_NAME && checksum == expected_checksum =>
                    {
                        let table_exists = tx.query_row(
                            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'config_documents')",
                            [],
                            |row| row.get::<_, bool>(0),
                        )?;
                        if !table_exists {
                            return Err(storage_error(
                                "typed config migration is recorded but its table is missing",
                            ));
                        }
                    }
                    Some((name, _)) if name != TYPED_SCHEMA_NAME => {
                        return Err(storage_error(format!(
                            "typed SQLite schema migration {TYPED_SCHEMA_VERSION} name mismatch"
                        )));
                    }
                    Some(_) => {
                        return Err(storage_error(format!(
                            "typed SQLite schema migration {TYPED_SCHEMA_VERSION} checksum mismatch"
                        )));
                    }
                    None => {
                        tx.execute_batch(TYPED_SCHEMA_SQL)?;
                        tx.execute(
                            "INSERT INTO typed_schema_migrations(version, name, checksum, applied_at_ms) VALUES (?1, ?2, ?3, ?4)",
                            params![
                                TYPED_SCHEMA_VERSION,
                                TYPED_SCHEMA_NAME,
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

    #[cfg(test)]
    pub(crate) async fn load(&self) -> StorageResult<Option<TypedConfigDocument>> {
        self.manager
            .call(move |conn| load_typed_config_document(conn))
            .await
    }

    pub(crate) async fn load_shadow(
        &self,
        config_key: &str,
        generation_key: &str,
    ) -> StorageResult<TypedConfigShadowSnapshot> {
        let config_key = config_key.to_string();
        let generation_key = generation_key.to_string();
        self.manager
            .call(move |conn| {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                let legacy = LegacyConfigRawSnapshot {
                    config_raw: string_get_tx(&tx, &config_key)?,
                    generation_raw: string_get_tx(&tx, &generation_key)?,
                };
                // A corrupt or missing typed table must not make the legacy
                // read path unavailable during the shadow phase. Preserve the
                // typed error as comparison telemetry instead.
                let typed = load_typed_config_document(&tx);
                tx.commit()?;
                Ok(TypedConfigShadowSnapshot { legacy, typed })
            })
            .await
    }

    pub(crate) async fn reconcile_from_legacy(
        &self,
        config_key: &str,
        generation_key: &str,
        default_document: &Value,
        revision_floor: u64,
    ) -> StorageResult<ReconciledLegacyConfig> {
        let config_key = config_key.to_string();
        let generation_key = generation_key.to_string();
        let default_document_json = serde_json::to_string(default_document)?;
        self.manager
            .call(move |conn| {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                let legacy = LegacyConfigRawSnapshot {
                    config_raw: string_get_tx(&tx, &config_key)?,
                    generation_raw: string_get_tx(&tx, &generation_key)?,
                };
                let document_json = legacy
                    .config_raw
                    .as_deref()
                    .unwrap_or(&default_document_json);
                let host_mappings_generation = legacy
                    .generation_raw
                    .as_deref()
                    .unwrap_or("0")
                    .parse::<u64>()
                    .map_err(|_| storage_error("host mappings generation is invalid"))?;
                let typed_revision = upsert_config_document_with_revision_floor_tx(
                    &tx,
                    document_json,
                    host_mappings_generation,
                    revision_floor,
                )?;
                tx.commit()?;
                Ok(ReconciledLegacyConfig {
                    legacy,
                    typed_revision,
                })
            })
            .await
    }
}

fn load_typed_config_document(conn: &Connection) -> StorageResult<Option<TypedConfigDocument>> {
    let raw = conn
        .query_row(
            "SELECT document_json, host_mappings_generation, revision FROM config_documents WHERE singleton = 1",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .optional()?;
    let Some((document_json, generation, revision)) = raw else {
        return Ok(None);
    };
    let host_mappings_generation = u64::try_from(generation)
        .map_err(|_| storage_error("typed config generation is invalid"))?;
    let revision = u64::try_from(revision)
        .ok()
        .filter(|revision| *revision > 0)
        .ok_or_else(|| storage_error("typed config revision is invalid"))?;
    Ok(Some(TypedConfigDocument {
        document: serde_json::from_str(&document_json)?,
        host_mappings_generation,
        revision,
    }))
}

pub(crate) fn upsert_config_document_tx(
    tx: &Transaction<'_>,
    document_json: &str,
    host_mappings_generation: u64,
) -> StorageResult<u64> {
    upsert_config_document_with_revision_floor_tx(tx, document_json, host_mappings_generation, 1)
}

fn upsert_config_document_with_revision_floor_tx(
    tx: &Transaction<'_>,
    document_json: &str,
    host_mappings_generation: u64,
    revision_floor: u64,
) -> StorageResult<u64> {
    let _: Value = serde_json::from_str(document_json)?;
    let generation = i64::try_from(host_mappings_generation)
        .map_err(|_| storage_error("typed config generation exceeds SQLite range"))?;
    let revision_floor = i64::try_from(revision_floor.max(1))
        .map_err(|_| storage_error("typed config revision exceeds SQLite range"))?;
    tx.execute(
        "INSERT INTO config_documents(singleton, document_json, host_mappings_generation, revision, updated_at_ms)
         VALUES (1, ?1, ?2, ?4, ?3)
         ON CONFLICT(singleton) DO UPDATE SET
           document_json = excluded.document_json,
           host_mappings_generation = excluded.host_mappings_generation,
           revision = max(config_documents.revision + 1, excluded.revision),
           updated_at_ms = excluded.updated_at_ms
         WHERE config_documents.document_json != excluded.document_json
            OR config_documents.host_mappings_generation != excluded.host_mappings_generation
            OR config_documents.revision < excluded.revision",
        params![document_json, generation, crate::time_utils::now_ms(), revision_floor],
    )?;
    let revision = tx.query_row(
        "SELECT revision FROM config_documents WHERE singleton = 1",
        [],
        |row| row.get::<_, i64>(0),
    )?;
    u64::try_from(revision)
        .ok()
        .filter(|revision| *revision > 0)
        .ok_or_else(|| storage_error("typed config revision is invalid"))
}
