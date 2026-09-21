use std::{
    ops::Deref,
    sync::{Arc, Mutex, OnceLock},
};

use crate::runtime_health::operations::OperationGuard;
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
// Bound retained source size; the parsed Value also has allocation overhead.
// Larger configurations remain supported, but are parsed without retention.
pub(crate) const MAX_CACHED_CONFIG_JSON_BYTES: usize = 1024 * 1024;
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
    parsed_document: Arc<Mutex<Option<ParsedConfigDocument>>>,
}

struct ParsedConfigDocument {
    raw: String,
    document: Arc<ParsedConfigValue>,
}

/// Immutable content and its derived fingerprint share one lifetime. Callers
/// cannot mutate the JSON while retaining a fingerprint for different content.
#[derive(Debug)]
pub(crate) struct ParsedConfigValue {
    value: Value,
    host_fingerprint: OnceLock<String>,
}

impl Deref for ParsedConfigValue {
    type Target = Value;
    fn deref(&self) -> &Value {
        &self.value
    }
}

impl ParsedConfigValue {
    fn new(value: Value) -> Self {
        Self {
            value,
            host_fingerprint: OnceLock::new(),
        }
    }

    pub(crate) fn host_fingerprint(&self, operation: &OperationGuard) -> StorageResult<&str> {
        if let Some(fingerprint) = self.host_fingerprint.get() {
            operation
                .child("task_phase", "config.fingerprint.reuse", false)
                .finish(true, None);
            return Ok(fingerprint);
        }
        // Never called on the SQLite executor. Concurrent first readers may
        // compute the same value; OnceLock publishes one immutable result.
        let phase = operation.child("task_phase", "config.fingerprint.compute", false);
        let result = config_host_mappings_fingerprint(&self.value);
        phase.finish(result.is_ok(), None);
        let _ = self.host_fingerprint.set(result?);
        self.host_fingerprint
            .get()
            .map(String::as_str)
            .ok_or_else(|| storage_error("config fingerprint was not initialized"))
    }
}

pub(crate) fn config_host_mappings_fingerprint(config: &Value) -> StorageResult<String> {
    let empty = Value::Array(Vec::new());
    Ok(crate::crypto_utils::sha256_hex_bytes(serde_json::to_vec(
        config.get("host_mappings").unwrap_or(&empty),
    )?))
}

#[derive(Clone, Debug)]
pub(crate) struct TypedConfigDocument {
    pub(crate) document: Arc<ParsedConfigValue>,
    pub(crate) host_mappings_generation: u64,
    pub(crate) revision: u64,
    /// Structural JSON equality alone is not byte-identical serialization
    /// (for example -0.0 and 0.0). Reuse derived hashes only for exact input.
    pub(crate) matches_legacy_json: bool,
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
        Self {
            manager,
            parsed_document: Arc::new(Mutex::new(None)),
        }
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
        let parsed_document = self.parsed_document.clone();
        self.manager
            .call(move |conn| load_typed_config_document(conn, &parsed_document))
            .await
    }

    pub(crate) async fn load_shadow(
        &self,
        config_key: &str,
        generation_key: &str,
    ) -> StorageResult<TypedConfigShadowSnapshot> {
        let config_key = config_key.to_string();
        let generation_key = generation_key.to_string();
        let parsed_document = self.parsed_document.clone();
        let operation = self
            .manager
            .diagnostics()
            .scope("task", "config.load_shadow");
        let admission = operation.child("wait", "config.primary_admission", false);
        let (shadow, resume) = self
            .manager
            .call(move |conn| {
                admission.finish(true, None);
                let result = (|| {
                    let tx = operation.sqlite_phase("config.transaction_begin", || {
                        conn.transaction_with_behavior(TransactionBehavior::Immediate)
                    })?;
                    let legacy = operation.sqlite_phase("config.legacy_read", || {
                        Ok::<_, super::StorageError>(LegacyConfigRawSnapshot {
                            config_raw: string_get_tx(&tx, &config_key)?,
                            generation_raw: string_get_tx(&tx, &generation_key)?,
                        })
                    })?;
                    // Preserve typed errors for fallback/repair telemetry.
                    let typed = load_typed_config_document_measured(
                        &tx,
                        &parsed_document,
                        &operation,
                        legacy.config_raw.as_deref(),
                    );
                    operation.sqlite_phase("config.transaction_commit", || tx.commit())?;
                    Ok(TypedConfigShadowSnapshot { legacy, typed })
                })();
                // Keep the handoff guard with the returned value so it includes
                // executor completion, result delivery and async rescheduling.
                let result = result.map(|shadow| {
                    let resume = operation.child("wait", "config.async_resume", false);
                    (shadow, resume)
                });
                operation.finish(result.is_ok(), None);
                result
            })
            .await?;
        resume.finish(true, None);
        Ok(shadow)
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

#[cfg(test)]
fn load_typed_config_document(
    conn: &Connection,
    parsed_document: &Mutex<Option<ParsedConfigDocument>>,
) -> StorageResult<Option<TypedConfigDocument>> {
    load_typed_config_document_measured(conn, parsed_document, &OperationGuard::default(), None)
}

fn load_typed_config_document_measured(
    conn: &Connection,
    parsed_document: &Mutex<Option<ParsedConfigDocument>>,
    operation: &OperationGuard,
    legacy_json: Option<&str>,
) -> StorageResult<Option<TypedConfigDocument>> {
    let mut cached = parsed_document
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let result = read_typed_config_document(conn, &mut cached, operation, legacy_json);
    if !matches!(&result, Ok(Some(_))) {
        // Do not retain an obsolete document after deletion, corruption or a
        // failed database read. Such reads must never fall back to this cache.
        *cached = None;
    }
    result
}

fn read_typed_config_document(
    conn: &Connection,
    cached: &mut Option<ParsedConfigDocument>,
    operation: &OperationGuard,
    legacy_json: Option<&str>,
) -> StorageResult<Option<TypedConfigDocument>> {
    let raw = operation.sqlite_phase("config.typed_query", || conn
        .prepare_cached(
            "SELECT document_json, host_mappings_generation, revision FROM config_documents WHERE singleton = 1",
        )?
        .query_row(
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .optional())?;
    let Some((document_json, generation, revision)) = raw else {
        return Ok(None);
    };
    let matches_legacy_json = legacy_json == Some(document_json.as_str());
    let host_mappings_generation = u64::try_from(generation)
        .map_err(|_| storage_error("typed config generation is invalid"))?;
    let revision = u64::try_from(revision)
        .ok()
        .filter(|revision| *revision > 0)
        .ok_or_else(|| storage_error("typed config revision is invalid"))?;
    // Always read the database and validate metadata above. A revision alone
    // cannot detect legacy writes, restores or a row repaired with a reused
    // revision. Cache only parsing, keyed by the complete persisted JSON, and
    // retain at most one document per repository (shared by its clones).
    let document = match cached.as_ref() {
        Some(parsed) if parsed.raw == document_json => {
            operation.record_bytes("config.cache.hit", document_json.len());
            parsed.document.clone()
        }
        _ => {
            operation.record_bytes(
                if document_json.len() <= MAX_CACHED_CONFIG_JSON_BYTES {
                    "config.cache.miss"
                } else {
                    "config.cache.bypass_oversize"
                },
                document_json.len(),
            );
            let document = operation.sqlite_phase("config.json_parse", || {
                serde_json::from_str::<Value>(&document_json)
                    .map(|value| Arc::new(ParsedConfigValue::new(value)))
            })?;
            *cached = if document_json.len() <= MAX_CACHED_CONFIG_JSON_BYTES {
                Some(ParsedConfigDocument {
                    raw: document_json,
                    document: document.clone(),
                })
            } else {
                None
            };
            document
        }
    };
    Ok(Some(TypedConfigDocument {
        document,
        host_mappings_generation,
        revision,
        matches_legacy_json,
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn database() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        // Omit CHECK constraints so reads can also exercise invalid metadata.
        conn.execute_batch(
            "CREATE TABLE config_documents (
                singleton INTEGER PRIMARY KEY,
                document_json TEXT NOT NULL,
                host_mappings_generation INTEGER NOT NULL,
                revision INTEGER NOT NULL
            );
            INSERT INTO config_documents VALUES (1, '{\"enabled\":true}', 0, 1);",
        )
        .unwrap();
        conn
    }

    #[test]
    fn parsed_config_reuses_unchanged_content_but_reads_current_metadata() {
        let conn = database();
        let cache = Mutex::new(None);
        let first = load_typed_config_document(&conn, &cache).unwrap().unwrap();
        conn.execute(
            "UPDATE config_documents SET revision = 2, host_mappings_generation = 3",
            [],
        )
        .unwrap();
        let second = load_typed_config_document(&conn, &cache).unwrap().unwrap();
        assert!(Arc::ptr_eq(&first.document, &second.document));
        assert_eq!(second.revision, 2);
        assert_eq!(second.host_mappings_generation, 3);

        // A write that reuses the revision must invalidate the parsed value.
        conn.execute(
            "UPDATE config_documents SET document_json = ?1",
            [r#"{"enabled":false}"#],
        )
        .unwrap();
        let changed = load_typed_config_document(&conn, &cache).unwrap().unwrap();
        assert!(!Arc::ptr_eq(&second.document, &changed.document));
        assert_eq!(**changed.document, json!({"enabled": false}));
        assert_eq!(changed.revision, second.revision);
        assert_eq!(**first.document, json!({"enabled": true}));
    }

    #[test]
    fn fingerprint_cache_tracks_content_not_revision_and_preserves_old_readers() {
        let conn = database();
        let cache = Mutex::new(None);
        let recorder = Arc::new(crate::runtime_health::operations::OperationRecorder::default());
        recorder.start();
        let operation = recorder.scope("task", "test");
        conn.execute(
            "UPDATE config_documents SET document_json = ?1",
            [r#"{"host_mappings":[{"host":"first.test"}]}"#],
        )
        .unwrap();
        let first = load_typed_config_document(&conn, &cache).unwrap().unwrap();
        let fingerprint = first
            .document
            .host_fingerprint(&operation)
            .unwrap()
            .to_owned();
        assert_eq!(
            fingerprint,
            config_host_mappings_fingerprint(&first.document).unwrap()
        );
        let second = load_typed_config_document(&conn, &cache).unwrap().unwrap();
        assert_eq!(
            second.document.host_fingerprint(&operation).unwrap(),
            fingerprint
        );
        // Same revision and generation, changed content: no stale derived hash.
        conn.execute(
            "UPDATE config_documents SET document_json = ?1",
            [r#"{"host_mappings":[{"host":"second.test"}]}"#],
        )
        .unwrap();
        let changed = load_typed_config_document(&conn, &cache).unwrap().unwrap();
        assert_eq!(changed.revision, first.revision);
        assert_ne!(
            changed.document.host_fingerprint(&operation).unwrap(),
            fingerprint
        );
        assert_eq!(
            first.document.host_fingerprint(&operation).unwrap(),
            fingerprint
        );
        let stats = recorder.snapshot();
        for (label, count) in [
            ("config.fingerprint.compute", 2),
            ("config.fingerprint.reuse", 2),
        ] {
            assert_eq!(
                stats
                    .operations
                    .iter()
                    .find(|row| row.label == label)
                    .unwrap()
                    .calls,
                count
            );
        }
    }

    #[test]
    fn parsed_config_cache_does_not_hide_corruption_or_deleted_rows() {
        let conn = database();
        let cache = Mutex::new(None);
        load_typed_config_document(&conn, &cache).unwrap().unwrap();
        for statement in [
            "UPDATE config_documents SET revision = 0",
            "UPDATE config_documents SET revision = 1, host_mappings_generation = -1",
            "UPDATE config_documents SET host_mappings_generation = 0, document_json = 'broken'",
            "UPDATE config_documents SET revision = 'invalid-type'",
        ] {
            conn.execute(
                "UPDATE config_documents SET revision = 1, host_mappings_generation = 0, document_json = '{}'",
                [],
            )
            .unwrap();
            load_typed_config_document(&conn, &cache).unwrap().unwrap();
            conn.execute(statement, []).unwrap();
            assert!(load_typed_config_document(&conn, &cache).is_err());
            assert!(cache.lock().unwrap().is_none());
        }
        conn.execute("UPDATE config_documents SET revision = 1", [])
            .unwrap();
        load_typed_config_document(&conn, &cache).unwrap().unwrap();
        conn.execute("DELETE FROM config_documents", []).unwrap();
        assert!(load_typed_config_document(&conn, &cache).unwrap().is_none());
        assert!(cache.lock().unwrap().is_none());
        conn.execute(
            "INSERT INTO config_documents VALUES (1, '{\"restored\":true}', 0, 1)",
            [],
        )
        .unwrap();
        let restored = load_typed_config_document(&conn, &cache).unwrap().unwrap();
        assert_eq!(**restored.document, json!({"restored": true}));
        conn.execute("DROP TABLE config_documents", []).unwrap();
        assert!(load_typed_config_document(&conn, &cache).is_err());
        assert!(cache.lock().unwrap().is_none());
    }

    #[test]
    fn parsed_config_cache_limits_retention_without_rejecting_large_documents() {
        let conn = database();
        let cache = Mutex::new(None);
        for bytes in [
            MAX_CACHED_CONFIG_JSON_BYTES,
            MAX_CACHED_CONFIG_JSON_BYTES + 1,
            16,
        ] {
            let expected = json!("x".repeat(bytes - 2));
            let raw = expected.to_string();
            assert_eq!(raw.len(), bytes);
            conn.execute("UPDATE config_documents SET document_json = ?1", [&raw])
                .unwrap();
            let first = load_typed_config_document(&conn, &cache).unwrap().unwrap();
            let second = load_typed_config_document(&conn, &cache).unwrap().unwrap();
            assert_eq!(**first.document, expected);
            assert_eq!(**second.document, expected);
            let cacheable = bytes <= MAX_CACHED_CONFIG_JSON_BYTES;
            assert_eq!(cache.lock().unwrap().is_some(), cacheable);
            assert_eq!(Arc::ptr_eq(&first.document, &second.document), cacheable);
        }
    }

    #[test]
    #[ignore = "manual comparison of repeated SQLite reads with and without parse reuse"]
    fn parsed_config_cache_benchmark() {
        use std::{hint::black_box, time::Instant};

        let conn = database();
        let raw = json!({
            "host_mappings": (0..512).map(|index| json!({
                "id": index,
                "host": format!("service-{index}.example.test"),
                "target": "http://127.0.0.1:8080",
                "enabled": true,
            })).collect::<Vec<_>>()
        })
        .to_string();
        conn.execute("UPDATE config_documents SET document_json = ?1", [&raw])
            .unwrap();
        let cache = Mutex::new(None);
        load_typed_config_document(&conn, &cache).unwrap().unwrap();
        let started = Instant::now();
        for _ in 0..500 {
            black_box(load_typed_config_document(&conn, &cache).unwrap().unwrap());
        }
        let warm = started.elapsed();
        let started = Instant::now();
        for _ in 0..500 {
            black_box(
                load_typed_config_document(&conn, &Mutex::new(None))
                    .unwrap()
                    .unwrap(),
            );
        }
        let reparsed = started.elapsed();
        eprintln!(
            "500 SQLite reads, {} JSON bytes: reused={warm:?}, reparsed={reparsed:?}",
            raw.len()
        );
        // Timing is intentionally not asserted: this is a local experiment,
        // not a production latency guarantee or a timing-sensitive CI test.
    }
}
