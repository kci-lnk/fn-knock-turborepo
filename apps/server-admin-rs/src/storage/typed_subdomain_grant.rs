use serde::Deserialize;
use tokio_rusqlite::rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use super::{StorageResult, redis_compat::ConnectionManager, storage_error};

pub(crate) const GRANT_PREFIX: &str = "fn_knock:auth:subdomain_rule_grant:";
pub(crate) const ACTIVE_INDEX_PREFIX: &str = "fn_knock:auth:subdomain_rule_grant_active:";

const SCHEMA_VERSION: i64 = 1;
const SCHEMA_NAME: &str = "typed_subdomain_rule_grants";
const SCHEMA_SQL: &str = r#"
CREATE TABLE subdomain_rule_grants (
  grant_digest TEXT PRIMARY KEY CHECK (length(grant_digest) = 64),
  host TEXT NOT NULL CHECK (host <> ''),
  policy_version TEXT NOT NULL CHECK (policy_version <> ''),
  group_id TEXT NOT NULL CHECK (group_id <> ''),
  issued_at INTEGER NOT NULL,
  last_access_at INTEGER NOT NULL CHECK (last_access_at > 0),
  hard_expires_at INTEGER NOT NULL,
  expires_at_ms INTEGER NOT NULL CHECK (expires_at_ms >= 0),
  updated_at_ms INTEGER NOT NULL,
  CHECK (hard_expires_at > issued_at)
);
CREATE INDEX idx_subdomain_rule_grants_expiry ON subdomain_rule_grants(expires_at_ms);
CREATE TABLE subdomain_rule_grant_active_entries (
  host_digest TEXT NOT NULL CHECK (length(host_digest) = 64),
  grant_digest TEXT NOT NULL CHECK (length(grant_digest) = 64),
  expires_at_score INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL,
  PRIMARY KEY(host_digest, grant_digest)
);
CREATE INDEX idx_subdomain_rule_grant_active_expiry
  ON subdomain_rule_grant_active_entries(host_digest, expires_at_score);
"#;
const MIGRATIONS_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS typed_subdomain_grant_schema_migrations (
  version INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  checksum TEXT NOT NULL,
  applied_at_ms INTEGER NOT NULL
);
"#;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TypedSubdomainGrant {
    pub(crate) grant_digest: String,
    pub(crate) host: String,
    pub(crate) policy_version: String,
    pub(crate) group_id: String,
    pub(crate) issued_at: i64,
    pub(crate) last_access_at: i64,
    pub(crate) hard_expires_at: i64,
    pub(crate) expires_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct TypedSubdomainGrantActiveEntry {
    pub(crate) host_digest: String,
    pub(crate) grant_digest: String,
    pub(crate) expires_at_score: i64,
}

#[derive(Deserialize)]
struct LegacyGrantRecord {
    host: String,
    policy_version: String,
    group_id: String,
    issued_at: i64,
    last_access_at: i64,
    hard_expires_at: i64,
}

#[derive(Clone)]
pub(crate) struct TypedSubdomainGrantRepository {
    manager: ConnectionManager,
}

impl TypedSubdomainGrantRepository {
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
                        "SELECT name, checksum FROM typed_subdomain_grant_schema_migrations WHERE version = ?1",
                        [SCHEMA_VERSION],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                    )
                    .optional()?;
                match applied {
                    Some((name, stored)) if name == SCHEMA_NAME && stored == checksum => {
                        let count = tx.query_row(
                            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN ('subdomain_rule_grants', 'subdomain_rule_grant_active_entries')",
                            [],
                            |row| row.get::<_, i64>(0),
                        )?;
                        if count != 2 {
                            return Err(storage_error(
                                "typed subdomain grant migration is recorded but its tables are missing",
                            ));
                        }
                    }
                    Some((name, _)) if name != SCHEMA_NAME => {
                        return Err(storage_error("typed subdomain grant migration name mismatch"));
                    }
                    Some(_) => {
                        return Err(storage_error("typed subdomain grant migration checksum mismatch"));
                    }
                    None => {
                        tx.execute_batch(SCHEMA_SQL)?;
                        tx.execute(
                            "INSERT INTO typed_subdomain_grant_schema_migrations(version, name, checksum, applied_at_ms) VALUES (?1, ?2, ?3, ?4)",
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
        let grants = legacy_grants_tx(tx)?;
        let active = legacy_all_active_entries_tx(tx)?;
        tx.execute("DELETE FROM subdomain_rule_grant_active_entries", [])?;
        tx.execute("DELETE FROM subdomain_rule_grants", [])?;
        for grant in grants {
            upsert_grant_tx(tx, &grant)?;
        }
        for entry in active {
            upsert_active_tx(tx, &entry)?;
        }
        Ok(())
    }

    pub(crate) fn reconcile_legacy_keys_tx(
        tx: &Transaction<'_>,
        keys: &[String],
    ) -> StorageResult<()> {
        for key in keys {
            if let Some(grant_digest) = parse_grant_key(key) {
                match live_legacy_grant_tx(tx, key)? {
                    Some(grant) => upsert_grant_tx(tx, &grant)?,
                    None => {
                        delete_grant_tx(tx, grant_digest)?;
                        tx.execute(
                            "DELETE FROM subdomain_rule_grant_active_entries WHERE grant_digest = ?1",
                            [grant_digest],
                        )?;
                    }
                }
            } else if let Some(host_digest) = parse_active_key(key) {
                replace_active_index_tx(tx, host_digest, &legacy_active_entries_tx(tx, key)?)?;
            }
        }
        Ok(())
    }

    pub(crate) async fn verify_and_repair_key(&self, key: &str) -> StorageResult<bool> {
        let key = key.to_string();
        self.manager
            .call(move |conn| {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                let matched = if let Some(grant_digest) = parse_grant_key(&key) {
                    let raw = live_legacy_string_tx(&tx, &key)?;
                    let legacy = live_legacy_grant_tx(&tx, &key)?;
                    let invalid = raw.is_some() && legacy.is_none();
                    let typed = typed_grant_tx(&tx, grant_digest)?;
                    let mut matched = !invalid && typed == legacy;
                    if !matched {
                        match &legacy {
                            Some(grant) => upsert_grant_tx(&tx, grant)?,
                            None => {
                                delete_grant_tx(&tx, grant_digest)?;
                                tx.execute(
                                    "DELETE FROM subdomain_rule_grant_active_entries WHERE grant_digest = ?1",
                                    [grant_digest],
                                )?;
                            }
                        }
                    }
                    if let Some(grant) = legacy {
                        let active_key = active_key(&grant.host);
                        let host_digest = parse_active_key(&active_key)
                            .ok_or_else(|| storage_error("invalid subdomain grant active key"))?;
                        let (active, active_invalid) = legacy_active_entries_checked_tx(&tx, &active_key)?;
                        let typed_active = typed_active_entries_tx(&tx, host_digest)?;
                        if active_invalid || typed_active != active {
                            matched = false;
                            replace_active_index_tx(&tx, host_digest, &active)?;
                        }
                    }
                    matched
                } else if let Some(host_digest) = parse_active_key(&key) {
                    let (legacy, invalid) = legacy_active_entries_checked_tx(&tx, &key)?;
                    let typed = typed_active_entries_tx(&tx, host_digest)?;
                    let matched = !invalid && typed == legacy;
                    if !matched {
                        replace_active_index_tx(&tx, host_digest, &legacy)?;
                    }
                    matched
                } else {
                    return Err(storage_error("invalid subdomain grant runtime key"));
                };
                tx.commit()?;
                Ok(matched)
            })
            .await
    }

    #[cfg(test)]
    pub(crate) async fn load_grant(&self, key: &str) -> StorageResult<Option<TypedSubdomainGrant>> {
        let digest = parse_grant_key(key)
            .ok_or_else(|| storage_error("invalid subdomain grant key"))?
            .to_string();
        self.manager
            .call(move |conn| typed_grant_conn(conn, &digest))
            .await
    }

    #[cfg(test)]
    pub(crate) async fn active_entries(
        &self,
        key: &str,
    ) -> StorageResult<Vec<TypedSubdomainGrantActiveEntry>> {
        let digest = parse_active_key(key)
            .ok_or_else(|| storage_error("invalid subdomain grant active key"))?
            .to_string();
        self.manager
            .call(move |conn| typed_active_entries_conn(conn, &digest))
            .await
    }

    #[cfg(test)]
    pub(crate) async fn counts(&self) -> StorageResult<(i64, i64)> {
        self.manager
            .call(|conn| {
                Ok((
                    conn.query_row("SELECT COUNT(*) FROM subdomain_rule_grants", [], |row| {
                        row.get(0)
                    })?,
                    conn.query_row(
                        "SELECT COUNT(*) FROM subdomain_rule_grant_active_entries",
                        [],
                        |row| row.get(0),
                    )?,
                ))
            })
            .await
    }
}

pub(crate) fn owns_key(key: &str) -> bool {
    parse_grant_key(key).is_some() || parse_active_key(key).is_some()
}

fn parse_grant_key(key: &str) -> Option<&str> {
    key.strip_prefix(GRANT_PREFIX)
        .filter(|digest| digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

fn parse_active_key(key: &str) -> Option<&str> {
    key.strip_prefix(ACTIVE_INDEX_PREFIX)
        .filter(|digest| digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

fn active_key(host: &str) -> String {
    format!(
        "{ACTIVE_INDEX_PREFIX}{}",
        crate::crypto_utils::sha256_hex_str(host)
    )
}

fn live_legacy_string_tx(tx: &Transaction<'_>, key: &str) -> StorageResult<Option<(String, i64)>> {
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

fn live_legacy_grant_tx(
    tx: &Transaction<'_>,
    key: &str,
) -> StorageResult<Option<TypedSubdomainGrant>> {
    let Some(grant_digest) = parse_grant_key(key) else {
        return Ok(None);
    };
    let Some((raw, expires_at_ms)) = live_legacy_string_tx(tx, key)? else {
        return Ok(None);
    };
    let Ok(record) = serde_json::from_str::<LegacyGrantRecord>(&raw) else {
        return Ok(None);
    };
    if record.host.trim().is_empty()
        || record.policy_version.trim().is_empty()
        || record.group_id.trim().is_empty()
        || record.last_access_at <= 0
        || record.hard_expires_at <= record.issued_at
    {
        return Ok(None);
    }
    Ok(Some(TypedSubdomainGrant {
        grant_digest: grant_digest.to_ascii_lowercase(),
        host: record.host,
        policy_version: record.policy_version,
        group_id: record.group_id,
        issued_at: record.issued_at,
        last_access_at: record.last_access_at,
        hard_expires_at: record.hard_expires_at,
        expires_at_ms,
    }))
}

fn legacy_grants_tx(tx: &Transaction<'_>) -> StorageResult<Vec<TypedSubdomainGrant>> {
    let mut statement = tx.prepare(
        "SELECT key FROM kv_keys
         WHERE kind = 'string' AND expires_at_ms IS NOT NULL AND expires_at_ms > ?1
           AND key LIKE ?2 ORDER BY key",
    )?;
    let rows = statement.query_map(
        params![crate::time_utils::now_ms(), format!("{GRANT_PREFIX}%")],
        |row| row.get::<_, String>(0),
    )?;
    let mut records = Vec::new();
    for key in rows {
        if let Some(record) = live_legacy_grant_tx(tx, &key?)? {
            records.push(record);
        }
    }
    Ok(records)
}

fn legacy_active_entries_checked_tx(
    tx: &Transaction<'_>,
    key: &str,
) -> StorageResult<(Vec<TypedSubdomainGrantActiveEntry>, bool)> {
    let Some(host_digest) = parse_active_key(key) else {
        return Ok((Vec::new(), true));
    };
    let mut statement =
        tx.prepare("SELECT member, score FROM kv_zset WHERE key = ?1 ORDER BY member")?;
    let rows = statement.query_map([key], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
    })?;
    let mut entries = Vec::new();
    let mut invalid = false;
    for row in rows {
        let (member, score) = row?;
        let Some(grant_digest) = parse_grant_key(&member) else {
            invalid = true;
            continue;
        };
        if live_legacy_grant_tx(tx, &member)?.is_none() {
            continue;
        }
        if !score.is_finite()
            || score.fract() != 0.0
            || score < i64::MIN as f64
            || score > i64::MAX as f64
        {
            invalid = true;
            continue;
        }
        entries.push(TypedSubdomainGrantActiveEntry {
            host_digest: host_digest.to_ascii_lowercase(),
            grant_digest: grant_digest.to_ascii_lowercase(),
            expires_at_score: score as i64,
        });
    }
    entries.sort();
    Ok((entries, invalid))
}

fn legacy_active_entries_tx(
    tx: &Transaction<'_>,
    key: &str,
) -> StorageResult<Vec<TypedSubdomainGrantActiveEntry>> {
    legacy_active_entries_checked_tx(tx, key).map(|(entries, _)| entries)
}

fn legacy_all_active_entries_tx(
    tx: &Transaction<'_>,
) -> StorageResult<Vec<TypedSubdomainGrantActiveEntry>> {
    let mut statement =
        tx.prepare("SELECT key FROM kv_keys WHERE kind = 'zset' AND key LIKE ?1 ORDER BY key")?;
    let keys = statement.query_map([format!("{ACTIVE_INDEX_PREFIX}%")], |row| {
        row.get::<_, String>(0)
    })?;
    let mut entries = Vec::new();
    for key in keys {
        entries.extend(legacy_active_entries_tx(tx, &key?)?);
    }
    entries.sort();
    Ok(entries)
}

fn typed_grant_tx(
    tx: &Transaction<'_>,
    digest: &str,
) -> StorageResult<Option<TypedSubdomainGrant>> {
    typed_grant_query(tx, digest)
}

#[cfg(test)]
fn typed_grant_conn(
    conn: &tokio_rusqlite::rusqlite::Connection,
    digest: &str,
) -> StorageResult<Option<TypedSubdomainGrant>> {
    typed_grant_query(conn, digest)
}

fn typed_grant_query(
    conn: &tokio_rusqlite::rusqlite::Connection,
    digest: &str,
) -> StorageResult<Option<TypedSubdomainGrant>> {
    conn.query_row(
        "SELECT host, policy_version, group_id, issued_at, last_access_at, hard_expires_at, expires_at_ms
         FROM subdomain_rule_grants WHERE grant_digest = ?1",
        [digest],
        |row| Ok(TypedSubdomainGrant {
            grant_digest: digest.to_string(), host: row.get(0)?, policy_version: row.get(1)?,
            group_id: row.get(2)?, issued_at: row.get(3)?, last_access_at: row.get(4)?,
            hard_expires_at: row.get(5)?, expires_at_ms: row.get(6)?,
        }),
    ).optional().map_err(Into::into)
}

fn typed_active_entries_tx(
    tx: &Transaction<'_>,
    digest: &str,
) -> StorageResult<Vec<TypedSubdomainGrantActiveEntry>> {
    typed_active_entries_query(tx, digest)
}

#[cfg(test)]
fn typed_active_entries_conn(
    conn: &tokio_rusqlite::rusqlite::Connection,
    digest: &str,
) -> StorageResult<Vec<TypedSubdomainGrantActiveEntry>> {
    typed_active_entries_query(conn, digest)
}

fn typed_active_entries_query(
    conn: &tokio_rusqlite::rusqlite::Connection,
    digest: &str,
) -> StorageResult<Vec<TypedSubdomainGrantActiveEntry>> {
    let mut statement = conn.prepare(
        "SELECT grant_digest, expires_at_score FROM subdomain_rule_grant_active_entries
         WHERE host_digest = ?1 ORDER BY grant_digest",
    )?;
    let rows = statement.query_map([digest], |row| {
        Ok(TypedSubdomainGrantActiveEntry {
            host_digest: digest.to_string(),
            grant_digest: row.get(0)?,
            expires_at_score: row.get(1)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn upsert_grant_tx(tx: &Transaction<'_>, grant: &TypedSubdomainGrant) -> StorageResult<()> {
    tx.execute(
        "INSERT INTO subdomain_rule_grants(
           grant_digest, host, policy_version, group_id, issued_at, last_access_at,
           hard_expires_at, expires_at_ms, updated_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(grant_digest) DO UPDATE SET host = excluded.host,
           policy_version = excluded.policy_version, group_id = excluded.group_id,
           issued_at = excluded.issued_at, last_access_at = excluded.last_access_at,
           hard_expires_at = excluded.hard_expires_at, expires_at_ms = excluded.expires_at_ms,
           updated_at_ms = excluded.updated_at_ms
         WHERE subdomain_rule_grants.host <> excluded.host
            OR subdomain_rule_grants.policy_version <> excluded.policy_version
            OR subdomain_rule_grants.group_id <> excluded.group_id
            OR subdomain_rule_grants.issued_at <> excluded.issued_at
            OR subdomain_rule_grants.last_access_at <> excluded.last_access_at
            OR subdomain_rule_grants.hard_expires_at <> excluded.hard_expires_at
            OR subdomain_rule_grants.expires_at_ms <> excluded.expires_at_ms",
        params![
            grant.grant_digest,
            grant.host,
            grant.policy_version,
            grant.group_id,
            grant.issued_at,
            grant.last_access_at,
            grant.hard_expires_at,
            grant.expires_at_ms,
            crate::time_utils::now_ms()
        ],
    )?;
    Ok(())
}

fn delete_grant_tx(tx: &Transaction<'_>, digest: &str) -> StorageResult<()> {
    tx.execute(
        "DELETE FROM subdomain_rule_grants WHERE grant_digest = ?1",
        [digest],
    )?;
    Ok(())
}

fn replace_active_index_tx(
    tx: &Transaction<'_>,
    host_digest: &str,
    entries: &[TypedSubdomainGrantActiveEntry],
) -> StorageResult<()> {
    tx.execute(
        "DELETE FROM subdomain_rule_grant_active_entries WHERE host_digest = ?1",
        [host_digest],
    )?;
    for entry in entries {
        upsert_active_tx(tx, entry)?;
    }
    Ok(())
}

fn upsert_active_tx(
    tx: &Transaction<'_>,
    entry: &TypedSubdomainGrantActiveEntry,
) -> StorageResult<()> {
    tx.execute(
        "INSERT INTO subdomain_rule_grant_active_entries(host_digest, grant_digest, expires_at_score, updated_at_ms)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(host_digest, grant_digest) DO UPDATE SET
           expires_at_score = excluded.expires_at_score, updated_at_ms = excluded.updated_at_ms
         WHERE subdomain_rule_grant_active_entries.expires_at_score <> excluded.expires_at_score",
        params![entry.host_digest, entry.grant_digest, entry.expires_at_score, crate::time_utils::now_ms()],
    )?;
    Ok(())
}
