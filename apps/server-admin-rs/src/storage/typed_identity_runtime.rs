use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_rusqlite::rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use super::{StorageResult, redis_compat::ConnectionManager, storage_error};

pub(crate) const OIDC_PREFIX: &str = "fn_knock:oidc:";
pub(crate) const LDAP_PREFIX: &str = "fn_knock:ldap:";

const OIDC_PROVIDER_INDEX: &str = "fn_knock:oidc:providers:index";
const OIDC_PROVIDER_PREFIX: &str = "fn_knock:oidc:providers:data:";
const OIDC_BINDING_INDEX: &str = "fn_knock:oidc:bindings:index";
const OIDC_BINDING_PREFIX: &str = "fn_knock:oidc:bindings:data:";
const OIDC_SUBJECT_PREFIX: &str = "fn_knock:oidc:bindings:subject:";
const OIDC_INVITE_PREFIX: &str = "fn_knock:oidc:invite:";
const OIDC_STATE_PREFIX: &str = "fn_knock:oidc:state:";
const OIDC_LOGIN_ERROR_PREFIX: &str = "fn_knock:oidc:login_error:";

const LDAP_PROVIDER_INDEX: &str = "fn_knock:ldap:providers:index";
const LDAP_PROVIDER_PREFIX: &str = "fn_knock:ldap:providers:data:";
const LDAP_BINDING_INDEX: &str = "fn_knock:ldap:bindings:index";
const LDAP_BINDING_PREFIX: &str = "fn_knock:ldap:bindings:data:";
const LDAP_SUBJECT_PREFIX: &str = "fn_knock:ldap:bindings:subject:";
const LDAP_INVITE_PREFIX: &str = "fn_knock:ldap:invite:";

const SCHEMA_VERSION: i64 = 1;
const SCHEMA_NAME: &str = "typed_identity_runtime_aggregates";
const SCHEMA_SQL: &str = r#"
CREATE TABLE identity_runtime_aggregates (
  protocol TEXT PRIMARY KEY CHECK (protocol IN ('oidc', 'ldap')),
  aggregate_json TEXT NOT NULL CHECK (json_valid(aggregate_json)),
  provider_count INTEGER NOT NULL CHECK (provider_count >= 0),
  binding_count INTEGER NOT NULL CHECK (binding_count >= 0),
  subject_count INTEGER NOT NULL CHECK (subject_count >= 0),
  capability_count INTEGER NOT NULL CHECK (capability_count >= 0),
  updated_at_ms INTEGER NOT NULL
);
"#;
const MIGRATIONS_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS typed_identity_runtime_schema_migrations (
  version INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  checksum TEXT NOT NULL,
  applied_at_ms INTEGER NOT NULL
);
"#;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct IdentityProtocolAggregate {
    pub(crate) protocol: String,
    pub(crate) provider_index: Vec<IdentityScoredMember>,
    pub(crate) providers: Vec<IdentityDocument>,
    pub(crate) binding_index: Vec<IdentityScoredMember>,
    pub(crate) bindings: Vec<IdentityDocument>,
    pub(crate) subjects: Vec<IdentitySubjectOwner>,
    pub(crate) capabilities: Vec<IdentityCapability>,
    pub(crate) malformed_key_digests: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct IdentityScoredMember {
    pub(crate) id: String,
    pub(crate) score_bits: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct IdentityDocument {
    pub(crate) id: String,
    pub(crate) document: Value,
    pub(crate) expires_at_ms: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct IdentitySubjectOwner {
    pub(crate) subject_key: String,
    pub(crate) binding_id: String,
    pub(crate) expires_at_ms: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct IdentityCapability {
    pub(crate) kind: String,
    pub(crate) digest: String,
    pub(crate) document: Value,
    pub(crate) expires_at_ms: i64,
}

#[derive(Clone)]
pub(crate) struct TypedIdentityRuntimeRepository {
    manager: ConnectionManager,
}

impl TypedIdentityRuntimeRepository {
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
                        "SELECT name, checksum FROM typed_identity_runtime_schema_migrations WHERE version = ?1",
                        [SCHEMA_VERSION],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                    )
                    .optional()?;
                match applied {
                    Some((name, stored)) if name == SCHEMA_NAME && stored == checksum => {
                        let exists = tx.query_row(
                            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'identity_runtime_aggregates')",
                            [],
                            |row| row.get::<_, bool>(0),
                        )?;
                        if !exists {
                            return Err(storage_error(
                                "typed identity runtime migration is recorded but its table is missing",
                            ));
                        }
                    }
                    Some((name, _)) if name != SCHEMA_NAME => {
                        return Err(storage_error("typed identity runtime migration name mismatch"));
                    }
                    Some(_) => {
                        return Err(storage_error(
                            "typed identity runtime migration checksum mismatch",
                        ));
                    }
                    None => {
                        tx.execute_batch(SCHEMA_SQL)?;
                        tx.execute(
                            "INSERT INTO typed_identity_runtime_schema_migrations(version, name, checksum, applied_at_ms) VALUES (?1, ?2, ?3, ?4)",
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
        for protocol in ["oidc", "ldap"] {
            let aggregate = collect_legacy_protocol_tx(tx, protocol)?;
            upsert_aggregate_tx(tx, &aggregate)?;
        }
        Ok(())
    }

    pub(crate) fn reconcile_legacy_keys_tx(
        tx: &Transaction<'_>,
        keys: &[String],
    ) -> StorageResult<()> {
        let mut oidc = false;
        let mut ldap = false;
        for key in keys {
            oidc |= key.starts_with(OIDC_PREFIX);
            ldap |= key.starts_with(LDAP_PREFIX);
        }
        if oidc {
            upsert_aggregate_tx(tx, &collect_legacy_protocol_tx(tx, "oidc")?)?;
        }
        if ldap {
            upsert_aggregate_tx(tx, &collect_legacy_protocol_tx(tx, "ldap")?)?;
        }
        Ok(())
    }

    pub(crate) async fn verify_and_repair_protocol(&self, protocol: &str) -> StorageResult<bool> {
        let protocol = normalize_protocol(protocol)?.to_string();
        self.manager
            .call(move |conn| {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                let legacy = collect_legacy_protocol_tx(&tx, &protocol)?;
                let typed = load_aggregate_tx(&tx, &protocol)?;
                let matched =
                    typed.as_ref() == Some(&legacy) && legacy.malformed_key_digests.is_empty();
                if !matched {
                    upsert_aggregate_tx(&tx, &legacy)?;
                }
                tx.commit()?;
                Ok(matched)
            })
            .await
    }

    #[cfg(test)]
    pub(crate) async fn load_protocol(
        &self,
        protocol: &str,
    ) -> StorageResult<Option<IdentityProtocolAggregate>> {
        let protocol = normalize_protocol(protocol)?.to_string();
        self.manager
            .call(move |conn| {
                conn.query_row(
                    "SELECT aggregate_json FROM identity_runtime_aggregates WHERE protocol = ?1",
                    [protocol],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
                .map(|raw| serde_json::from_str(&raw).map_err(Into::into))
                .transpose()
            })
            .await
    }
}

fn normalize_protocol(protocol: &str) -> StorageResult<&str> {
    match protocol {
        "oidc" | "ldap" => Ok(protocol),
        _ => Err(storage_error("invalid identity runtime protocol")),
    }
}

fn collect_legacy_protocol_tx(
    tx: &Transaction<'_>,
    protocol: &str,
) -> StorageResult<IdentityProtocolAggregate> {
    let protocol = normalize_protocol(protocol)?;
    let (
        provider_index,
        provider_prefix,
        binding_index,
        binding_prefix,
        subject_prefix,
        capability_prefixes,
    ) = match protocol {
        "oidc" => (
            OIDC_PROVIDER_INDEX,
            OIDC_PROVIDER_PREFIX,
            OIDC_BINDING_INDEX,
            OIDC_BINDING_PREFIX,
            OIDC_SUBJECT_PREFIX,
            &[
                ("invitation", OIDC_INVITE_PREFIX),
                ("state", OIDC_STATE_PREFIX),
                ("login_error", OIDC_LOGIN_ERROR_PREFIX),
            ][..],
        ),
        "ldap" => (
            LDAP_PROVIDER_INDEX,
            LDAP_PROVIDER_PREFIX,
            LDAP_BINDING_INDEX,
            LDAP_BINDING_PREFIX,
            LDAP_SUBJECT_PREFIX,
            &[("invitation", LDAP_INVITE_PREFIX)][..],
        ),
        _ => unreachable!(),
    };
    let now = crate::time_utils::now_ms();
    let mut malformed = Vec::new();
    let providers = collect_documents_tx(tx, provider_prefix, now, &mut malformed)?;
    let bindings = collect_documents_tx(tx, binding_prefix, now, &mut malformed)?;
    let subjects = collect_subjects_tx(tx, subject_prefix, now, &mut malformed)?;
    let mut capabilities = Vec::new();
    for (kind, prefix) in capability_prefixes {
        capabilities.extend(collect_capabilities_tx(
            tx,
            kind,
            prefix,
            now,
            &mut malformed,
        )?);
    }
    capabilities
        .sort_by(|left, right| (&left.kind, &left.digest).cmp(&(&right.kind, &right.digest)));
    malformed.sort();
    malformed.dedup();
    Ok(IdentityProtocolAggregate {
        protocol: protocol.to_string(),
        provider_index: collect_zset_tx(tx, provider_index, now)?,
        providers,
        binding_index: collect_zset_tx(tx, binding_index, now)?,
        bindings,
        subjects,
        capabilities,
        malformed_key_digests: malformed,
    })
}

fn collect_zset_tx(
    tx: &Transaction<'_>,
    key: &str,
    now: i64,
) -> StorageResult<Vec<IdentityScoredMember>> {
    let mut statement = tx.prepare(
        "SELECT zset.member, zset.score
         FROM kv_keys AS keys JOIN kv_zset AS zset ON zset.key = keys.key
         WHERE keys.key = ?1 AND keys.kind = 'zset'
           AND (keys.expires_at_ms IS NULL OR keys.expires_at_ms > ?2)
         ORDER BY zset.score DESC, zset.member ASC",
    )?;
    statement
        .query_map(params![key, now], |row| {
            let score = row.get::<_, f64>(1)?;
            Ok(IdentityScoredMember {
                id: row.get(0)?,
                score_bits: score.to_bits(),
            })
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn collect_documents_tx(
    tx: &Transaction<'_>,
    prefix: &str,
    now: i64,
    malformed: &mut Vec<String>,
) -> StorageResult<Vec<IdentityDocument>> {
    let mut statement = tx.prepare(
        "SELECT keys.key, strings.value, keys.expires_at_ms
         FROM kv_keys AS keys JOIN kv_strings AS strings ON strings.key = keys.key
         WHERE keys.kind = 'string' AND keys.key GLOB ?1
           AND (keys.expires_at_ms IS NULL OR keys.expires_at_ms > ?2)
         ORDER BY keys.key",
    )?;
    let rows = statement.query_map(params![format!("{prefix}*"), now], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<i64>>(2)?,
        ))
    })?;
    let mut documents = Vec::new();
    for row in rows {
        let (key, raw, expires_at_ms) = row?;
        let Some(id) = key.strip_prefix(prefix).filter(|value| !value.is_empty()) else {
            malformed.push(key_digest(&key));
            continue;
        };
        match serde_json::from_str::<Value>(&raw) {
            Ok(document) if document.is_object() => documents.push(IdentityDocument {
                id: id.to_string(),
                document,
                expires_at_ms,
            }),
            _ => malformed.push(key_digest(&key)),
        }
    }
    Ok(documents)
}

fn collect_subjects_tx(
    tx: &Transaction<'_>,
    prefix: &str,
    now: i64,
    malformed: &mut Vec<String>,
) -> StorageResult<Vec<IdentitySubjectOwner>> {
    let mut statement = tx.prepare(
        "SELECT keys.key, strings.value, keys.expires_at_ms
         FROM kv_keys AS keys JOIN kv_strings AS strings ON strings.key = keys.key
         WHERE keys.kind = 'string' AND keys.key GLOB ?1
           AND (keys.expires_at_ms IS NULL OR keys.expires_at_ms > ?2)
         ORDER BY keys.key",
    )?;
    let rows = statement.query_map(params![format!("{prefix}*"), now], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<i64>>(2)?,
        ))
    })?;
    let mut subjects = Vec::new();
    for row in rows {
        let (key, binding_id, expires_at_ms) = row?;
        let Some(subject_key) = key.strip_prefix(prefix).filter(|value| !value.is_empty()) else {
            malformed.push(key_digest(&key));
            continue;
        };
        if binding_id.is_empty() {
            malformed.push(key_digest(&key));
            continue;
        }
        subjects.push(IdentitySubjectOwner {
            subject_key: subject_key.to_string(),
            binding_id,
            expires_at_ms,
        });
    }
    Ok(subjects)
}

fn collect_capabilities_tx(
    tx: &Transaction<'_>,
    kind: &str,
    prefix: &str,
    now: i64,
    malformed: &mut Vec<String>,
) -> StorageResult<Vec<IdentityCapability>> {
    let mut statement = tx.prepare(
        "SELECT keys.key, strings.value, keys.expires_at_ms
         FROM kv_keys AS keys JOIN kv_strings AS strings ON strings.key = keys.key
         WHERE keys.kind = 'string' AND keys.key GLOB ?1
           AND (keys.expires_at_ms IS NULL OR keys.expires_at_ms > ?2)
         ORDER BY keys.key",
    )?;
    let rows = statement.query_map(params![format!("{prefix}*"), now], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<i64>>(2)?,
        ))
    })?;
    let mut capabilities = Vec::new();
    for row in rows {
        let (key, raw, expires_at_ms) = row?;
        let Some(digest) = key.strip_prefix(prefix).filter(|value| !value.is_empty()) else {
            malformed.push(key_digest(&key));
            continue;
        };
        let Some(expires_at_ms) = expires_at_ms else {
            malformed.push(key_digest(&key));
            continue;
        };
        match serde_json::from_str::<Value>(&raw) {
            Ok(document) if document.is_object() => capabilities.push(IdentityCapability {
                kind: kind.to_string(),
                digest: digest.to_string(),
                document,
                expires_at_ms,
            }),
            _ => malformed.push(key_digest(&key)),
        }
    }
    Ok(capabilities)
}

fn key_digest(key: &str) -> String {
    crate::crypto_utils::sha256_hex_str(key)
}

fn load_aggregate_tx(
    tx: &Transaction<'_>,
    protocol: &str,
) -> StorageResult<Option<IdentityProtocolAggregate>> {
    let raw = tx
        .query_row(
            "SELECT aggregate_json FROM identity_runtime_aggregates WHERE protocol = ?1",
            [protocol],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    Ok(raw.and_then(|value| serde_json::from_str(&value).ok()))
}

fn upsert_aggregate_tx(
    tx: &Transaction<'_>,
    aggregate: &IdentityProtocolAggregate,
) -> StorageResult<()> {
    let aggregate_json = serde_json::to_string(aggregate)?;
    tx.execute(
        "INSERT INTO identity_runtime_aggregates(
           protocol, aggregate_json, provider_count, binding_count, subject_count,
           capability_count, updated_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(protocol) DO UPDATE SET
           aggregate_json = excluded.aggregate_json,
           provider_count = excluded.provider_count,
           binding_count = excluded.binding_count,
           subject_count = excluded.subject_count,
           capability_count = excluded.capability_count,
           updated_at_ms = excluded.updated_at_ms
         WHERE identity_runtime_aggregates.aggregate_json <> excluded.aggregate_json",
        params![
            aggregate.protocol,
            aggregate_json,
            aggregate.providers.len() as i64,
            aggregate.bindings.len() as i64,
            aggregate.subjects.len() as i64,
            aggregate.capabilities.len() as i64,
            crate::time_utils::now_ms(),
        ],
    )?;
    Ok(())
}
