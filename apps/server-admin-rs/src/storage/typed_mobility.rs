use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_rusqlite::rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use super::{StorageResult, redis_compat::ConnectionManager, storage_error};

const SCHEMA_VERSION: i64 = 1;
const SCHEMA_NAME: &str = "typed_auth_mobility_aggregates";
const SESSION_PREFIX: &str = "fn_knock:session:";
const BINDING_PREFIX: &str = "fn_knock:auth_mobility:binding:";
const SESSION_INDEX_PREFIX: &str = "fn_knock:auth_mobility:session:";
const TIMELINE_PREFIX: &str = "fn_knock:auth_mobility:timeline:";
const SUMMARY_PREFIX: &str = "fn_knock:auth_mobility:summary:";
const ACTIVE_IP_PREFIX: &str = "fn_knock:auth_mobility:active_ips:";
const ACTIVE_IP_DETAILS_PREFIX: &str = "fn_knock:auth_mobility:active_ip_details:";
const PENDING_PREFIX: &str = "fn_knock:auth_mobility:session_pending_whitelist:";
const OWNER_PREFIX: &str = "fn_knock:auth_mobility:whitelist:";
const OWNER_SUFFIX: &str = ":session";
const MUTATION_LOCK_PREFIX: &str = "fn_knock:auth_mobility:session_mutation_lock:";

const SCHEMA_SQL: &str = r#"
CREATE TABLE mobility_session_aggregates (
  session_id TEXT PRIMARY KEY CHECK (session_id <> ''),
  aggregate_json TEXT NOT NULL,
  session_expires_at_ms INTEGER,
  updated_at_ms INTEGER NOT NULL
);
CREATE INDEX idx_mobility_session_aggregates_expiry
  ON mobility_session_aggregates(session_expires_at_ms);
CREATE TABLE mobility_orphan_bindings (
  binding_key TEXT PRIMARY KEY CHECK (binding_key <> ''),
  binding_json TEXT NOT NULL,
  expires_at_ms INTEGER,
  updated_at_ms INTEGER NOT NULL
);
CREATE INDEX idx_mobility_orphan_bindings_expiry
  ON mobility_orphan_bindings(expires_at_ms);
"#;

const MIGRATIONS_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS typed_mobility_schema_migrations (
  version INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  checksum TEXT NOT NULL,
  applied_at_ms INTEGER NOT NULL
);
"#;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct TypedMobilityExpiringValue {
    pub(crate) value: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) expires_at_ms: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct TypedMobilityBinding {
    pub(crate) key: String,
    pub(crate) value: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) expires_at_ms: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct TypedMobilityActiveIp {
    pub(crate) ip: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) score: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) detail: Option<Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct TypedMobilityPendingWhitelist {
    pub(crate) record_id: String,
    pub(crate) owner_record_key: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct TypedMobilityWhitelistOwner {
    pub(crate) record_id: String,
    pub(crate) session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) expires_at_ms: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct TypedMobilityAggregate {
    pub(crate) session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) session: Option<TypedMobilityExpiringValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) timeline: Option<TypedMobilityExpiringValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) summary: Option<TypedMobilityExpiringValue>,
    #[serde(default)]
    pub(crate) binding_index: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) binding_index_expires_at_ms: Option<i64>,
    #[serde(default)]
    pub(crate) bindings: Vec<TypedMobilityBinding>,
    #[serde(default)]
    pub(crate) active_ips: Vec<TypedMobilityActiveIp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) active_ips_expires_at_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) active_ip_details_expires_at_ms: Option<i64>,
    #[serde(default)]
    pub(crate) pending_whitelist: Vec<TypedMobilityPendingWhitelist>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) pending_whitelist_expires_at_ms: Option<i64>,
    #[serde(default)]
    pub(crate) whitelist_owners: Vec<TypedMobilityWhitelistOwner>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) mutation_lock: Option<TypedMobilityExpiringValue>,
}

impl TypedMobilityAggregate {
    fn empty(session_id: String) -> Self {
        Self {
            session_id,
            session: None,
            timeline: None,
            summary: None,
            binding_index: Vec::new(),
            binding_index_expires_at_ms: None,
            bindings: Vec::new(),
            active_ips: Vec::new(),
            active_ips_expires_at_ms: None,
            active_ip_details_expires_at_ms: None,
            pending_whitelist: Vec::new(),
            pending_whitelist_expires_at_ms: None,
            whitelist_owners: Vec::new(),
            mutation_lock: None,
        }
    }

    fn is_empty(&self) -> bool {
        self.session.is_none()
            && self.timeline.is_none()
            && self.summary.is_none()
            && self.binding_index.is_empty()
            && self.bindings.is_empty()
            && self.active_ips.is_empty()
            && self.pending_whitelist.is_empty()
            && self.whitelist_owners.is_empty()
            && self.mutation_lock.is_none()
    }

    fn normalized_at(mut self, now: i64) -> Option<Self> {
        if expired(
            self.session.as_ref().and_then(|value| value.expires_at_ms),
            now,
        ) {
            self.session = None;
        }
        if expired(
            self.timeline.as_ref().and_then(|value| value.expires_at_ms),
            now,
        ) {
            self.timeline = None;
        }
        if expired(
            self.summary.as_ref().and_then(|value| value.expires_at_ms),
            now,
        ) {
            self.summary = None;
        }
        if expired(self.binding_index_expires_at_ms, now) {
            self.binding_index.clear();
            self.binding_index_expires_at_ms = None;
        }
        self.bindings
            .retain(|binding| !expired(binding.expires_at_ms, now));
        let indexed = self.binding_index.iter().collect::<BTreeSet<_>>();
        self.bindings.retain(|binding| {
            indexed.contains(&binding.key)
                || binding.value.get("ownerSessionId").and_then(Value::as_str)
                    == Some(self.session_id.as_str())
        });

        let scores_live = !expired(self.active_ips_expires_at_ms, now);
        let details_live = !expired(self.active_ip_details_expires_at_ms, now);
        if !scores_live {
            self.active_ips_expires_at_ms = None;
        }
        if !details_live {
            self.active_ip_details_expires_at_ms = None;
        }
        for active_ip in &mut self.active_ips {
            if !scores_live {
                active_ip.score = None;
            }
            if !details_live {
                active_ip.detail = None;
            }
        }
        self.active_ips
            .retain(|active_ip| active_ip.score.is_some() || active_ip.detail.is_some());

        if expired(self.pending_whitelist_expires_at_ms, now) {
            self.pending_whitelist.clear();
            self.pending_whitelist_expires_at_ms = None;
        }
        self.whitelist_owners
            .retain(|owner| !expired(owner.expires_at_ms, now));
        if expired(
            self.mutation_lock
                .as_ref()
                .and_then(|value| value.expires_at_ms),
            now,
        ) {
            self.mutation_lock = None;
        }
        (!self.is_empty()).then_some(self)
    }
}

fn expired(expires_at_ms: Option<i64>, now: i64) -> bool {
    expires_at_ms.is_some_and(|expires_at_ms| expires_at_ms <= now)
}

#[derive(Default)]
struct AggregateBuilder {
    aggregate: Option<TypedMobilityAggregate>,
    binding_index: BTreeSet<String>,
    bindings: BTreeMap<String, TypedMobilityBinding>,
    active_ips: BTreeMap<String, TypedMobilityActiveIp>,
    pending_whitelist: BTreeMap<String, String>,
    whitelist_owners: BTreeMap<String, TypedMobilityWhitelistOwner>,
}

impl AggregateBuilder {
    fn aggregate_mut(&mut self, session_id: &str) -> &mut TypedMobilityAggregate {
        self.aggregate
            .get_or_insert_with(|| TypedMobilityAggregate::empty(session_id.to_string()))
    }

    fn finish(mut self, session_id: &str) -> TypedMobilityAggregate {
        let mut aggregate = self
            .aggregate
            .take()
            .unwrap_or_else(|| TypedMobilityAggregate::empty(session_id.to_string()));
        aggregate.binding_index = self.binding_index.into_iter().collect();
        aggregate.bindings = self.bindings.into_values().collect();
        aggregate.active_ips = self.active_ips.into_values().collect();
        aggregate.pending_whitelist = self
            .pending_whitelist
            .into_iter()
            .map(
                |(record_id, owner_record_key)| TypedMobilityPendingWhitelist {
                    record_id,
                    owner_record_key,
                },
            )
            .collect();
        aggregate.whitelist_owners = self.whitelist_owners.into_values().collect();
        aggregate
    }
}

#[derive(Clone)]
pub(crate) struct TypedMobilityRepository {
    manager: ConnectionManager,
}

impl TypedMobilityRepository {
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
                        "SELECT name, checksum FROM typed_mobility_schema_migrations WHERE version = ?1",
                        [SCHEMA_VERSION],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                    )
                    .optional()?;
                match applied {
                    Some((name, stored)) if name == SCHEMA_NAME && stored == checksum => {
                        for table in ["mobility_session_aggregates", "mobility_orphan_bindings"] {
                            let exists = tx.query_row(
                                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
                                [table],
                                |row| row.get::<_, bool>(0),
                            )?;
                            if !exists {
                                return Err(storage_error(format!(
                                    "typed mobility migration is recorded but {table} is missing"
                                )));
                            }
                        }
                    }
                    Some((name, _)) if name != SCHEMA_NAME => {
                        return Err(storage_error("typed mobility migration name mismatch"));
                    }
                    Some(_) => {
                        return Err(storage_error("typed mobility migration checksum mismatch"));
                    }
                    None => {
                        tx.execute_batch(SCHEMA_SQL)?;
                        tx.execute(
                            "INSERT INTO typed_mobility_schema_migrations(version, name, checksum, applied_at_ms) VALUES (?1, ?2, ?3, ?4)",
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
        let (aggregates, orphan_bindings) = Self::collect_from_legacy_tx(tx)?;
        Self::reconcile_all_tx(tx, &aggregates, &orphan_bindings)
    }

    pub(crate) fn reconcile_legacy_keys_tx(
        tx: &Transaction<'_>,
        keys: &[String],
    ) -> StorageResult<()> {
        let Some(mut targets) = MobilityReconcileTargets::from_keys(keys) else {
            return Self::rebuild_from_legacy_tx(tx);
        };
        if !targets.load_previous_typed_associations(tx)? {
            return Self::rebuild_from_legacy_tx(tx);
        }
        targets.load_current_legacy_associations(tx)?;

        for session_id in &targets.session_ids {
            let previous = match Self::load_session_tx(tx, session_id) {
                Ok(previous) => previous,
                Err(_) => return Self::rebuild_from_legacy_tx(tx),
            };
            if let Some(previous) = previous {
                targets
                    .binding_keys
                    .extend(previous.binding_index.iter().cloned());
                targets
                    .binding_keys
                    .extend(previous.bindings.iter().map(|binding| binding.key.clone()));
            }
        }

        for session_id in &targets.session_ids {
            match Self::collect_session_from_legacy_tx(tx, session_id)? {
                Some(aggregate) => {
                    targets
                        .binding_keys
                        .extend(aggregate.binding_index.iter().cloned());
                    targets
                        .binding_keys
                        .extend(aggregate.bindings.iter().map(|binding| binding.key.clone()));
                    Self::upsert_aggregate_tx(tx, &aggregate)?;
                }
                None => {
                    tx.execute(
                        "DELETE FROM mobility_session_aggregates WHERE session_id = ?1",
                        [session_id],
                    )?;
                }
            }
        }
        for binding_key in &targets.binding_keys {
            Self::reconcile_orphan_binding_tx(tx, binding_key)?;
        }
        Ok(())
    }

    fn load_session_tx(
        tx: &Transaction<'_>,
        session_id: &str,
    ) -> StorageResult<Option<TypedMobilityAggregate>> {
        tx.query_row(
            "SELECT aggregate_json FROM mobility_session_aggregates WHERE session_id = ?1",
            [session_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .map(|raw| serde_json::from_str(&raw).map_err(Into::into))
        .transpose()
    }

    fn collect_session_from_legacy_tx(
        tx: &Transaction<'_>,
        session_id: &str,
    ) -> StorageResult<Option<TypedMobilityAggregate>> {
        let now = crate::time_utils::now_ms();
        let mut builder = AggregateBuilder::default();
        let mut set_component =
            |prefix: &str,
             assign: &mut dyn FnMut(&mut TypedMobilityAggregate, TypedMobilityExpiringValue)|
             -> StorageResult<()> {
                let key = format!("{prefix}{session_id}");
                let Some((raw, expires_at_ms)) = live_string(tx, &key, now)? else {
                    return Ok(());
                };
                let Ok(value) = serde_json::from_str::<Value>(&raw) else {
                    return Ok(());
                };
                assign(
                    builder.aggregate_mut(session_id),
                    TypedMobilityExpiringValue {
                        value,
                        expires_at_ms,
                    },
                );
                Ok(())
            };
        set_component(SESSION_PREFIX, &mut |aggregate, value| {
            aggregate.session = Some(value)
        })?;
        set_component(TIMELINE_PREFIX, &mut |aggregate, value| {
            aggregate.timeline = Some(value)
        })?;
        set_component(SUMMARY_PREFIX, &mut |aggregate, value| {
            aggregate.summary = Some(value)
        })?;
        set_component(MUTATION_LOCK_PREFIX, &mut |aggregate, value| {
            aggregate.mutation_lock = Some(value)
        })?;

        let index_key = format!("{SESSION_INDEX_PREFIX}{session_id}");
        if let Some(expires_at_ms) = live_key_expiry(tx, &index_key, now)? {
            builder
                .aggregate_mut(session_id)
                .binding_index_expires_at_ms = expires_at_ms;
            let mut statement =
                tx.prepare("SELECT member FROM kv_set WHERE key = ?1 ORDER BY member")?;
            let rows = statement.query_map([index_key], |row| row.get::<_, String>(0))?;
            builder.binding_index = rows.collect::<Result<BTreeSet<_>, _>>()?;
        }

        let mut binding_keys = builder.binding_index.clone();
        let mut statement = tx.prepare(
            "SELECT strings.key
             FROM kv_strings AS strings
             JOIN kv_keys AS keys ON keys.key = strings.key
             WHERE strings.key LIKE ?1 ESCAPE '\\'
               AND (keys.expires_at_ms IS NULL OR keys.expires_at_ms > ?2)
               AND json_extract(
                     CASE WHEN json_valid(strings.value) THEN strings.value ELSE '{}' END,
                     '$.ownerSessionId'
                   ) = ?3",
        )?;
        let rows = statement.query_map(
            params![format!("{}%", escape_like(BINDING_PREFIX)), now, session_id],
            |row| row.get::<_, String>(0),
        )?;
        binding_keys.extend(rows.collect::<Result<Vec<_>, _>>()?);
        for binding_key in binding_keys {
            let Some((raw, expires_at_ms)) = live_string(tx, &binding_key, now)? else {
                continue;
            };
            let Ok(value) = serde_json::from_str::<Value>(&raw) else {
                continue;
            };
            builder.bindings.insert(
                binding_key.clone(),
                TypedMobilityBinding {
                    key: binding_key,
                    value,
                    expires_at_ms,
                },
            );
        }

        load_exact_active_ips(tx, session_id, now, &mut builder)?;
        load_exact_pending_whitelist(tx, session_id, now, &mut builder)?;
        load_exact_whitelist_owners(tx, session_id, now, &mut builder)?;

        let aggregate = builder.finish(session_id);
        Ok((!aggregate.is_empty()).then_some(aggregate))
    }

    fn reconcile_orphan_binding_tx(tx: &Transaction<'_>, binding_key: &str) -> StorageResult<()> {
        let now = crate::time_utils::now_ms();
        let Some((raw, expires_at_ms)) = live_string(tx, binding_key, now)? else {
            tx.execute(
                "DELETE FROM mobility_orphan_bindings WHERE binding_key = ?1",
                [binding_key],
            )?;
            return Ok(());
        };
        let Ok(value) = serde_json::from_str::<Value>(&raw) else {
            tx.execute(
                "DELETE FROM mobility_orphan_bindings WHERE binding_key = ?1",
                [binding_key],
            )?;
            return Ok(());
        };
        let has_owner = value
            .get("ownerSessionId")
            .and_then(Value::as_str)
            .is_some_and(|owner| !owner.is_empty());
        let indexed = tx.query_row(
            "SELECT EXISTS(
               SELECT 1
               FROM kv_set AS members
               JOIN kv_keys AS keys ON keys.key = members.key
               WHERE members.member = ?1
                 AND members.key LIKE ?2 ESCAPE '\\'
                 AND (keys.expires_at_ms IS NULL OR keys.expires_at_ms > ?3)
             )",
            params![
                binding_key,
                format!("{}%", escape_like(SESSION_INDEX_PREFIX)),
                now
            ],
            |row| row.get::<_, bool>(0),
        )?;
        if has_owner || indexed {
            tx.execute(
                "DELETE FROM mobility_orphan_bindings WHERE binding_key = ?1",
                [binding_key],
            )?;
            return Ok(());
        }
        let binding_json = serde_json::to_string(&value)?;
        tx.execute(
            "INSERT INTO mobility_orphan_bindings(binding_key, binding_json, expires_at_ms, updated_at_ms)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(binding_key) DO UPDATE SET
               binding_json = excluded.binding_json,
               expires_at_ms = excluded.expires_at_ms,
               updated_at_ms = excluded.updated_at_ms
             WHERE mobility_orphan_bindings.binding_json <> excluded.binding_json
                OR mobility_orphan_bindings.expires_at_ms IS NOT excluded.expires_at_ms",
            params![binding_key, binding_json, expires_at_ms, now],
        )?;
        Ok(())
    }

    fn collect_from_legacy_tx(
        tx: &Transaction<'_>,
    ) -> StorageResult<(Vec<TypedMobilityAggregate>, Vec<TypedMobilityBinding>)> {
        let now = crate::time_utils::now_ms();
        let mut builders = BTreeMap::<String, AggregateBuilder>::new();

        for (key, raw, expires_at_ms) in live_strings_with_prefix(tx, SESSION_PREFIX, now)? {
            let session_id = key.strip_prefix(SESSION_PREFIX).unwrap_or_default();
            if session_id.is_empty() {
                continue;
            }
            let Ok(value) = serde_json::from_str::<Value>(&raw) else {
                continue;
            };
            builders
                .entry(session_id.to_string())
                .or_default()
                .aggregate_mut(session_id)
                .session = Some(TypedMobilityExpiringValue {
                value,
                expires_at_ms,
            });
        }

        load_expiring_json_component(
            tx,
            TIMELINE_PREFIX,
            now,
            &mut builders,
            |aggregate, value| aggregate.timeline = Some(value),
        )?;
        load_expiring_json_component(
            tx,
            SUMMARY_PREFIX,
            now,
            &mut builders,
            |aggregate, value| aggregate.summary = Some(value),
        )?;
        load_expiring_json_component(
            tx,
            MUTATION_LOCK_PREFIX,
            now,
            &mut builders,
            |aggregate, value| aggregate.mutation_lock = Some(value),
        )?;

        let mut binding_sessions = BTreeMap::<String, BTreeSet<String>>::new();
        {
            let mut statement = tx.prepare(
                "SELECT members.key, members.member, keys.expires_at_ms
                 FROM kv_set AS members
                 JOIN kv_keys AS keys ON keys.key = members.key
                 WHERE members.key LIKE ?1 ESCAPE '\\'
                   AND (keys.expires_at_ms IS NULL OR keys.expires_at_ms > ?2)
                 ORDER BY members.key, members.member",
            )?;
            let pattern = format!("{}%", escape_like(SESSION_INDEX_PREFIX));
            let rows = statement.query_map(params![pattern, now], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                ))
            })?;
            for row in rows {
                let (key, binding_key, expires_at_ms) = row?;
                let session_id = key.strip_prefix(SESSION_INDEX_PREFIX).unwrap_or_default();
                if session_id.is_empty() {
                    continue;
                }
                let builder = builders.entry(session_id.to_string()).or_default();
                builder
                    .aggregate_mut(session_id)
                    .binding_index_expires_at_ms = expires_at_ms;
                builder.binding_index.insert(binding_key.clone());
                binding_sessions
                    .entry(binding_key)
                    .or_default()
                    .insert(session_id.to_string());
            }
        }

        let mut orphan_bindings = Vec::new();
        for (key, raw, expires_at_ms) in live_strings_with_prefix(tx, BINDING_PREFIX, now)? {
            let Ok(value) = serde_json::from_str::<Value>(&raw) else {
                continue;
            };
            let binding = TypedMobilityBinding {
                key: key.clone(),
                value: value.clone(),
                expires_at_ms,
            };
            let owner = value
                .get("ownerSessionId")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let mut owners = binding_sessions.remove(&key).unwrap_or_default();
            if let Some(owner) = owner {
                owners.insert(owner);
            }
            if owners.is_empty() {
                orphan_bindings.push(binding);
            } else {
                for session_id in owners {
                    builders
                        .entry(session_id.clone())
                        .or_default()
                        .bindings
                        .insert(key.clone(), binding.clone());
                }
            }
        }

        load_active_ips(tx, now, &mut builders)?;
        load_pending_whitelist(tx, now, &mut builders)?;
        load_whitelist_owners(tx, now, &mut builders)?;

        let aggregates = builders
            .into_iter()
            .map(|(session_id, builder)| builder.finish(&session_id))
            .filter(|aggregate| !aggregate.is_empty())
            .collect::<Vec<_>>();
        Ok((aggregates, orphan_bindings))
    }

    pub(crate) fn reconcile_all_tx(
        tx: &Transaction<'_>,
        aggregates: &[TypedMobilityAggregate],
        orphan_bindings: &[TypedMobilityBinding],
    ) -> StorageResult<()> {
        let mut stale_session_ids = {
            let mut statement = tx.prepare("SELECT session_id FROM mobility_session_aggregates")?;
            statement
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<Result<BTreeSet<_>, _>>()?
        };
        for aggregate in aggregates {
            stale_session_ids.remove(&aggregate.session_id);
            Self::upsert_aggregate_tx(tx, aggregate)?;
        }
        for session_id in stale_session_ids {
            tx.execute(
                "DELETE FROM mobility_session_aggregates WHERE session_id = ?1",
                [session_id],
            )?;
        }

        let mut stale_binding_keys = {
            let mut statement = tx.prepare("SELECT binding_key FROM mobility_orphan_bindings")?;
            statement
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<Result<BTreeSet<_>, _>>()?
        };
        for binding in orphan_bindings {
            stale_binding_keys.remove(&binding.key);
            let binding_json = serde_json::to_string(&binding.value)?;
            tx.execute(
                "INSERT INTO mobility_orphan_bindings(binding_key, binding_json, expires_at_ms, updated_at_ms)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(binding_key) DO UPDATE SET
                   binding_json = excluded.binding_json,
                   expires_at_ms = excluded.expires_at_ms,
                   updated_at_ms = excluded.updated_at_ms
                 WHERE mobility_orphan_bindings.binding_json <> excluded.binding_json
                    OR mobility_orphan_bindings.expires_at_ms IS NOT excluded.expires_at_ms",
                params![
                    binding.key,
                    binding_json,
                    binding.expires_at_ms,
                    crate::time_utils::now_ms(),
                ],
            )?;
        }
        for binding_key in stale_binding_keys {
            tx.execute(
                "DELETE FROM mobility_orphan_bindings WHERE binding_key = ?1",
                [binding_key],
            )?;
        }
        Ok(())
    }

    pub(crate) fn upsert_aggregate_tx(
        tx: &Transaction<'_>,
        aggregate: &TypedMobilityAggregate,
    ) -> StorageResult<()> {
        if aggregate.session_id.is_empty() {
            return Err(storage_error(
                "typed mobility aggregate requires a session id",
            ));
        }
        let aggregate_json = serde_json::to_string(aggregate)?;
        let _: TypedMobilityAggregate = serde_json::from_str(&aggregate_json)?;
        tx.execute(
            "INSERT INTO mobility_session_aggregates(session_id, aggregate_json, session_expires_at_ms, updated_at_ms)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(session_id) DO UPDATE SET
               aggregate_json = excluded.aggregate_json,
               session_expires_at_ms = excluded.session_expires_at_ms,
               updated_at_ms = excluded.updated_at_ms
             WHERE mobility_session_aggregates.aggregate_json <> excluded.aggregate_json
                OR mobility_session_aggregates.session_expires_at_ms IS NOT excluded.session_expires_at_ms",
            params![
                aggregate.session_id,
                aggregate_json,
                aggregate
                    .session
                    .as_ref()
                    .and_then(|session| session.expires_at_ms),
                crate::time_utils::now_ms(),
            ],
        )?;
        Ok(())
    }

    #[allow(dead_code)]
    pub(crate) async fn load_session(
        &self,
        session_id: &str,
    ) -> StorageResult<Option<TypedMobilityAggregate>> {
        let session_id = session_id.to_string();
        self.manager
            .call(move |conn| {
                let raw = conn
                    .query_row(
                        "SELECT aggregate_json FROM mobility_session_aggregates WHERE session_id = ?1",
                        [session_id],
                        |row| row.get::<_, String>(0),
                    )
                    .optional()?;
                raw.map(|raw| serde_json::from_str(&raw).map_err(Into::into))
                    .transpose()
            })
            .await
    }

    pub(crate) async fn verify_and_repair_session(&self, session_id: &str) -> StorageResult<bool> {
        let session_id = session_id.to_string();
        self.manager
            .call(move |conn| {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                let now = crate::time_utils::now_ms();
                let typed_raw = tx
                    .query_row(
                        "SELECT aggregate_json FROM mobility_session_aggregates WHERE session_id = ?1",
                        [session_id.as_str()],
                        |row| row.get::<_, String>(0),
                    )
                    .optional()?;
                let typed = typed_raw
                    .as_deref()
                    .map(serde_json::from_str::<TypedMobilityAggregate>)
                    .transpose()
                    .ok()
                    .flatten()
                    .and_then(|aggregate| aggregate.normalized_at(now));
                let (aggregates, orphan_bindings) = Self::collect_from_legacy_tx(&tx)?;
                let legacy = aggregates
                    .iter()
                    .find(|aggregate| aggregate.session_id == session_id)
                    .cloned();
                let matched = typed == legacy;
                if !matched {
                    Self::reconcile_all_tx(&tx, &aggregates, &orphan_bindings)?;
                }
                tx.commit()?;
                Ok(matched)
            })
            .await
    }

    pub(crate) async fn verify_and_repair_session_authority(
        &self,
        session_id: &str,
    ) -> StorageResult<bool> {
        let session_id = session_id.to_string();
        let compare_session_id = session_id.clone();
        let matched = self
            .manager
            .call(move |conn| {
                // A matched authorization read stays read-only and does not
                // reserve SQLite's single writer on every authenticated
                // request. A mismatch is repaired below in a short write.
                let tx = conn.transaction()?;
                let now = crate::time_utils::now_ms();
                let typed_raw = tx
                    .query_row(
                        "SELECT aggregate_json FROM mobility_session_aggregates WHERE session_id = ?1",
                        [compare_session_id.as_str()],
                        |row| row.get::<_, String>(0),
                    )
                    .optional()?;
                let typed = typed_raw
                    .as_deref()
                    .and_then(|raw| serde_json::from_str::<TypedMobilityAggregate>(raw).ok())
                    .and_then(|aggregate| aggregate.normalized_at(now))
                    .and_then(|aggregate| aggregate.session);
                let session_key = format!("{SESSION_PREFIX}{compare_session_id}");
                let legacy =
                    live_string(&tx, &session_key, now)?.and_then(|(raw, expires_at_ms)| {
                        serde_json::from_str::<Value>(&raw).ok().map(|value| {
                            TypedMobilityExpiringValue {
                                value,
                                expires_at_ms,
                            }
                        })
                    });
                let matched = typed == legacy;
                tx.commit()?;
                Ok(matched)
            })
            .await?;
        if matched {
            return Ok(true);
        }
        self.manager
            .call(move |conn| {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                let session_key = format!("{SESSION_PREFIX}{session_id}");
                Self::reconcile_legacy_keys_tx(&tx, &[session_key])?;
                tx.commit()?;
                Ok(())
            })
            .await?;
        Ok(false)
    }

    #[cfg(test)]
    pub(crate) async fn counts(&self) -> StorageResult<(i64, i64)> {
        self.manager
            .call(|conn| {
                let aggregates = conn.query_row(
                    "SELECT COUNT(*) FROM mobility_session_aggregates",
                    [],
                    |row| row.get::<_, i64>(0),
                )?;
                let orphans =
                    conn.query_row("SELECT COUNT(*) FROM mobility_orphan_bindings", [], |row| {
                        row.get::<_, i64>(0)
                    })?;
                Ok((aggregates, orphans))
            })
            .await
    }

    #[cfg(test)]
    pub(crate) async fn aggregate_updated_at_ms(
        &self,
        session_id: &str,
    ) -> StorageResult<Option<i64>> {
        let session_id = session_id.to_string();
        self.manager
            .call(move |conn| {
                conn.query_row(
                    "SELECT updated_at_ms FROM mobility_session_aggregates WHERE session_id = ?1",
                    [session_id],
                    |row| row.get::<_, i64>(0),
                )
                .optional()
                .map_err(Into::into)
            })
            .await
    }
}

#[derive(Default)]
struct MobilityReconcileTargets {
    session_ids: BTreeSet<String>,
    binding_keys: BTreeSet<String>,
    whitelist_record_ids: BTreeSet<String>,
}

impl MobilityReconcileTargets {
    fn from_keys(keys: &[String]) -> Option<Self> {
        let mut targets = Self::default();
        for key in keys {
            if !key.starts_with(SESSION_PREFIX) && !key.starts_with("fn_knock:auth_mobility:") {
                continue;
            }
            if let Some(binding_key) = key.strip_prefix(BINDING_PREFIX) {
                if binding_key.is_empty() {
                    return None;
                }
                targets.binding_keys.insert(key.clone());
                continue;
            }
            if let Some(record_key) = key.strip_prefix(OWNER_PREFIX) {
                let record_id = record_key.strip_suffix(OWNER_SUFFIX)?;
                if record_id.is_empty() {
                    return None;
                }
                targets.whitelist_record_ids.insert(record_id.to_string());
                continue;
            }
            let session_id = [
                SESSION_PREFIX,
                SESSION_INDEX_PREFIX,
                TIMELINE_PREFIX,
                SUMMARY_PREFIX,
                ACTIVE_IP_PREFIX,
                ACTIVE_IP_DETAILS_PREFIX,
                PENDING_PREFIX,
                MUTATION_LOCK_PREFIX,
            ]
            .into_iter()
            .find_map(|prefix| key.strip_prefix(prefix));
            let session_id = session_id?;
            if session_id.is_empty() {
                return None;
            }
            targets.session_ids.insert(session_id.to_string());
        }
        Some(targets)
    }

    fn load_previous_typed_associations(&mut self, tx: &Transaction<'_>) -> StorageResult<bool> {
        if self.binding_keys.is_empty() && self.whitelist_record_ids.is_empty() {
            return Ok(true);
        }
        let mut statement =
            tx.prepare("SELECT session_id, aggregate_json FROM mobility_session_aggregates")?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (session_id, raw) = row?;
            let Ok(aggregate) = serde_json::from_str::<TypedMobilityAggregate>(&raw) else {
                return Ok(false);
            };
            let contains_binding = aggregate
                .binding_index
                .iter()
                .chain(aggregate.bindings.iter().map(|binding| &binding.key))
                .any(|key| self.binding_keys.contains(key));
            let contains_owner = aggregate
                .whitelist_owners
                .iter()
                .any(|owner| self.whitelist_record_ids.contains(&owner.record_id));
            if contains_binding || contains_owner {
                self.session_ids.insert(session_id);
            }
        }
        Ok(true)
    }

    fn load_current_legacy_associations(&mut self, tx: &Transaction<'_>) -> StorageResult<()> {
        let now = crate::time_utils::now_ms();
        for binding_key in &self.binding_keys {
            if let Some((raw, _)) = live_string(tx, binding_key, now)?
                && let Ok(value) = serde_json::from_str::<Value>(&raw)
                && let Some(owner) = value
                    .get("ownerSessionId")
                    .and_then(Value::as_str)
                    .filter(|owner| !owner.is_empty())
            {
                self.session_ids.insert(owner.to_string());
            }
            let mut statement = tx.prepare(
                "SELECT members.key
                 FROM kv_set AS members
                 JOIN kv_keys AS keys ON keys.key = members.key
                 WHERE members.member = ?1
                   AND members.key LIKE ?2 ESCAPE '\\'
                   AND (keys.expires_at_ms IS NULL OR keys.expires_at_ms > ?3)",
            )?;
            let rows = statement.query_map(
                params![
                    binding_key,
                    format!("{}%", escape_like(SESSION_INDEX_PREFIX)),
                    now
                ],
                |row| row.get::<_, String>(0),
            )?;
            for row in rows {
                let key = row?;
                if let Some(session_id) = key.strip_prefix(SESSION_INDEX_PREFIX)
                    && !session_id.is_empty()
                {
                    self.session_ids.insert(session_id.to_string());
                }
            }
        }
        for record_id in &self.whitelist_record_ids {
            let key = format!("{OWNER_PREFIX}{record_id}{OWNER_SUFFIX}");
            if let Some((session_id, _)) = live_string(tx, &key, now)?
                && !session_id.is_empty()
            {
                self.session_ids.insert(session_id);
            }
        }
        Ok(())
    }
}

fn live_string(
    tx: &Transaction<'_>,
    key: &str,
    now: i64,
) -> StorageResult<Option<(String, Option<i64>)>> {
    tx.query_row(
        "SELECT strings.value, keys.expires_at_ms
         FROM kv_strings AS strings
         JOIN kv_keys AS keys ON keys.key = strings.key
         WHERE strings.key = ?1
           AND (keys.expires_at_ms IS NULL OR keys.expires_at_ms > ?2)",
        params![key, now],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<i64>>(1)?)),
    )
    .optional()
    .map_err(Into::into)
}

fn live_key_expiry(
    tx: &Transaction<'_>,
    key: &str,
    now: i64,
) -> StorageResult<Option<Option<i64>>> {
    tx.query_row(
        "SELECT expires_at_ms FROM kv_keys
         WHERE key = ?1 AND (expires_at_ms IS NULL OR expires_at_ms > ?2)",
        params![key, now],
        |row| row.get::<_, Option<i64>>(0),
    )
    .optional()
    .map_err(Into::into)
}

fn load_exact_active_ips(
    tx: &Transaction<'_>,
    session_id: &str,
    now: i64,
    builder: &mut AggregateBuilder,
) -> StorageResult<()> {
    let scores_key = format!("{ACTIVE_IP_PREFIX}{session_id}");
    if let Some(expires_at_ms) = live_key_expiry(tx, &scores_key, now)? {
        builder.aggregate_mut(session_id).active_ips_expires_at_ms = expires_at_ms;
        let mut statement =
            tx.prepare("SELECT member, score FROM kv_zset WHERE key = ?1 ORDER BY member")?;
        let rows = statement.query_map([scores_key], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
        })?;
        for row in rows {
            let (ip, score) = row?;
            builder.active_ips.insert(
                ip.clone(),
                TypedMobilityActiveIp {
                    ip,
                    score: score.is_finite().then_some(score),
                    detail: None,
                },
            );
        }
    }

    let details_key = format!("{ACTIVE_IP_DETAILS_PREFIX}{session_id}");
    if let Some(expires_at_ms) = live_key_expiry(tx, &details_key, now)? {
        builder
            .aggregate_mut(session_id)
            .active_ip_details_expires_at_ms = expires_at_ms;
        let mut statement =
            tx.prepare("SELECT field, value FROM kv_hash WHERE key = ?1 ORDER BY field")?;
        let rows = statement.query_map([details_key], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (ip, raw) = row?;
            let Ok(detail) = serde_json::from_str::<Value>(&raw) else {
                continue;
            };
            builder
                .active_ips
                .entry(ip.clone())
                .and_modify(|entry| entry.detail = Some(detail.clone()))
                .or_insert(TypedMobilityActiveIp {
                    ip,
                    score: None,
                    detail: Some(detail),
                });
        }
    }
    Ok(())
}

fn load_exact_pending_whitelist(
    tx: &Transaction<'_>,
    session_id: &str,
    now: i64,
    builder: &mut AggregateBuilder,
) -> StorageResult<()> {
    let key = format!("{PENDING_PREFIX}{session_id}");
    let Some(expires_at_ms) = live_key_expiry(tx, &key, now)? else {
        return Ok(());
    };
    builder
        .aggregate_mut(session_id)
        .pending_whitelist_expires_at_ms = expires_at_ms;
    let mut statement =
        tx.prepare("SELECT field, value FROM kv_hash WHERE key = ?1 ORDER BY field")?;
    let rows = statement.query_map([key], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    builder.pending_whitelist = rows.collect::<Result<BTreeMap<_, _>, _>>()?;
    Ok(())
}

fn load_exact_whitelist_owners(
    tx: &Transaction<'_>,
    session_id: &str,
    now: i64,
    builder: &mut AggregateBuilder,
) -> StorageResult<()> {
    let mut statement = tx.prepare(
        "SELECT strings.key, keys.expires_at_ms
         FROM kv_strings AS strings
         JOIN kv_keys AS keys ON keys.key = strings.key
         WHERE strings.key LIKE ?1 ESCAPE '\\'
           AND strings.value = ?2
           AND (keys.expires_at_ms IS NULL OR keys.expires_at_ms > ?3)
         ORDER BY strings.key",
    )?;
    let rows = statement.query_map(
        params![format!("{}%", escape_like(OWNER_PREFIX)), session_id, now],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<i64>>(1)?)),
    )?;
    for row in rows {
        let (key, expires_at_ms) = row?;
        if !key.ends_with(OWNER_SUFFIX) {
            continue;
        }
        let record_id = &key[OWNER_PREFIX.len()..key.len() - OWNER_SUFFIX.len()];
        if record_id.is_empty() {
            continue;
        }
        builder.whitelist_owners.insert(
            record_id.to_string(),
            TypedMobilityWhitelistOwner {
                record_id: record_id.to_string(),
                session_id: session_id.to_string(),
                expires_at_ms,
            },
        );
    }
    Ok(())
}

fn escape_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn live_strings_with_prefix(
    tx: &Transaction<'_>,
    prefix: &str,
    now: i64,
) -> StorageResult<Vec<(String, String, Option<i64>)>> {
    let mut statement = tx.prepare(
        "SELECT strings.key, strings.value, keys.expires_at_ms
         FROM kv_strings AS strings
         JOIN kv_keys AS keys ON keys.key = strings.key
         WHERE strings.key LIKE ?1 ESCAPE '\\'
           AND (keys.expires_at_ms IS NULL OR keys.expires_at_ms > ?2)
         ORDER BY strings.key",
    )?;
    let pattern = format!("{}%", escape_like(prefix));
    let rows = statement.query_map(params![pattern, now], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<i64>>(2)?,
        ))
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn load_expiring_json_component(
    tx: &Transaction<'_>,
    prefix: &str,
    now: i64,
    builders: &mut BTreeMap<String, AggregateBuilder>,
    assign: impl Fn(&mut TypedMobilityAggregate, TypedMobilityExpiringValue),
) -> StorageResult<()> {
    for (key, raw, expires_at_ms) in live_strings_with_prefix(tx, prefix, now)? {
        let session_id = key.strip_prefix(prefix).unwrap_or_default();
        if session_id.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        assign(
            builders
                .entry(session_id.to_string())
                .or_default()
                .aggregate_mut(session_id),
            TypedMobilityExpiringValue {
                value,
                expires_at_ms,
            },
        );
    }
    Ok(())
}

fn load_active_ips(
    tx: &Transaction<'_>,
    now: i64,
    builders: &mut BTreeMap<String, AggregateBuilder>,
) -> StorageResult<()> {
    {
        let mut statement = tx.prepare(
            "SELECT scores.key, scores.member, scores.score, keys.expires_at_ms
             FROM kv_zset AS scores
             JOIN kv_keys AS keys ON keys.key = scores.key
             WHERE scores.key LIKE ?1 ESCAPE '\\'
               AND (keys.expires_at_ms IS NULL OR keys.expires_at_ms > ?2)
             ORDER BY scores.key, scores.member",
        )?;
        let rows = statement.query_map(
            params![format!("{}%", escape_like(ACTIVE_IP_PREFIX)), now],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, f64>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                ))
            },
        )?;
        for row in rows {
            let (key, ip, score, expires_at_ms) = row?;
            let session_id = key.strip_prefix(ACTIVE_IP_PREFIX).unwrap_or_default();
            if session_id.is_empty() {
                continue;
            }
            let builder = builders.entry(session_id.to_string()).or_default();
            builder.aggregate_mut(session_id).active_ips_expires_at_ms = expires_at_ms;
            builder.active_ips.insert(
                ip.clone(),
                TypedMobilityActiveIp {
                    ip,
                    score: score.is_finite().then_some(score),
                    detail: None,
                },
            );
        }
    }
    {
        let mut statement = tx.prepare(
            "SELECT details.key, details.field, details.value, keys.expires_at_ms
             FROM kv_hash AS details
             JOIN kv_keys AS keys ON keys.key = details.key
             WHERE details.key LIKE ?1 ESCAPE '\\'
               AND (keys.expires_at_ms IS NULL OR keys.expires_at_ms > ?2)
             ORDER BY details.key, details.field",
        )?;
        let rows = statement.query_map(
            params![format!("{}%", escape_like(ACTIVE_IP_DETAILS_PREFIX)), now],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                ))
            },
        )?;
        for row in rows {
            let (key, ip, raw, expires_at_ms) = row?;
            let session_id = key
                .strip_prefix(ACTIVE_IP_DETAILS_PREFIX)
                .unwrap_or_default();
            let Ok(detail) = serde_json::from_str::<Value>(&raw) else {
                continue;
            };
            if session_id.is_empty() {
                continue;
            }
            let builder = builders.entry(session_id.to_string()).or_default();
            builder
                .aggregate_mut(session_id)
                .active_ip_details_expires_at_ms = expires_at_ms;
            builder
                .active_ips
                .entry(ip.clone())
                .and_modify(|entry| entry.detail = Some(detail.clone()))
                .or_insert(TypedMobilityActiveIp {
                    ip,
                    score: None,
                    detail: Some(detail),
                });
        }
    }
    Ok(())
}

fn load_pending_whitelist(
    tx: &Transaction<'_>,
    now: i64,
    builders: &mut BTreeMap<String, AggregateBuilder>,
) -> StorageResult<()> {
    let mut statement = tx.prepare(
        "SELECT pending.key, pending.field, pending.value, keys.expires_at_ms
         FROM kv_hash AS pending
         JOIN kv_keys AS keys ON keys.key = pending.key
         WHERE pending.key LIKE ?1 ESCAPE '\\'
           AND (keys.expires_at_ms IS NULL OR keys.expires_at_ms > ?2)
         ORDER BY pending.key, pending.field",
    )?;
    let rows = statement.query_map(
        params![format!("{}%", escape_like(PENDING_PREFIX)), now],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<i64>>(3)?,
            ))
        },
    )?;
    for row in rows {
        let (key, record_id, owner_record_key, expires_at_ms) = row?;
        let session_id = key.strip_prefix(PENDING_PREFIX).unwrap_or_default();
        if session_id.is_empty() {
            continue;
        }
        let builder = builders.entry(session_id.to_string()).or_default();
        builder
            .aggregate_mut(session_id)
            .pending_whitelist_expires_at_ms = expires_at_ms;
        builder
            .pending_whitelist
            .insert(record_id, owner_record_key);
    }
    Ok(())
}

fn load_whitelist_owners(
    tx: &Transaction<'_>,
    now: i64,
    builders: &mut BTreeMap<String, AggregateBuilder>,
) -> StorageResult<()> {
    for (key, session_id, expires_at_ms) in live_strings_with_prefix(tx, OWNER_PREFIX, now)? {
        if !key.ends_with(OWNER_SUFFIX) || session_id.is_empty() {
            continue;
        }
        let record_id = &key[OWNER_PREFIX.len()..key.len() - OWNER_SUFFIX.len()];
        if record_id.is_empty() {
            continue;
        }
        builders
            .entry(session_id.clone())
            .or_default()
            .whitelist_owners
            .insert(
                record_id.to_string(),
                TypedMobilityWhitelistOwner {
                    record_id: record_id.to_string(),
                    session_id,
                    expires_at_ms,
                },
            );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn like_prefixes_escape_wildcards() {
        assert_eq!(escape_like("a%b_c\\d"), "a\\%b\\_c\\\\d");
    }

    #[test]
    fn empty_aggregate_has_no_persisted_components() {
        assert!(TypedMobilityAggregate::empty("session".to_string()).is_empty());
    }

    #[test]
    fn mobility_prefix_is_scoped_to_auth_mobility() {
        assert!(BINDING_PREFIX.starts_with("fn_knock:auth_mobility:"));
    }
}
