use tokio_rusqlite::rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use super::{StorageResult, redis_compat::ConnectionManager, storage_error};

pub(crate) const LEASE_PREFIX: &str = "fn_knock:notifications:runtime:lock:";
pub(crate) const COOLDOWN_PREFIX: &str = "fn_knock:notifications:runtime:cooldown:";
pub(crate) const WINDOW_PREFIX: &str = "fn_knock:notifications:runtime:window:";
pub(crate) const READY_KEY: &str = "fn_knock:notifications:deliveries:ready";

const SCHEMA_VERSION: i64 = 1;
const SCHEMA_NAME: &str = "typed_notification_runtime";
const SCHEMA_SQL: &str = r#"
CREATE TABLE notification_runtime_leases (
  name TEXT PRIMARY KEY CHECK (name <> ''),
  token TEXT NOT NULL CHECK (token <> ''),
  expires_at_ms INTEGER NOT NULL CHECK (expires_at_ms >= 0),
  updated_at_ms INTEGER NOT NULL
);
CREATE INDEX idx_notification_runtime_leases_expiry
  ON notification_runtime_leases(expires_at_ms);

CREATE TABLE notification_runtime_cooldowns (
  runtime_key TEXT PRIMARY KEY CHECK (runtime_key <> ''),
  rule_id TEXT NOT NULL CHECK (rule_id <> ''),
  group_key_token TEXT NOT NULL CHECK (group_key_token <> ''),
  until_iso TEXT NOT NULL CHECK (until_iso <> ''),
  expires_at_ms INTEGER NOT NULL CHECK (expires_at_ms >= 0),
  updated_at_ms INTEGER NOT NULL
);
CREATE INDEX idx_notification_runtime_cooldowns_expiry
  ON notification_runtime_cooldowns(expires_at_ms);

CREATE TABLE notification_runtime_window_hits (
  runtime_key TEXT NOT NULL CHECK (runtime_key <> ''),
  rule_id TEXT NOT NULL CHECK (rule_id <> ''),
  group_key_token TEXT NOT NULL CHECK (group_key_token <> ''),
  event_id TEXT NOT NULL CHECK (event_id <> ''),
  happened_at_ms INTEGER NOT NULL,
  expires_at_ms INTEGER NOT NULL CHECK (expires_at_ms >= 0),
  updated_at_ms INTEGER NOT NULL,
  PRIMARY KEY (runtime_key, event_id)
);
CREATE INDEX idx_notification_runtime_window_hits_expiry
  ON notification_runtime_window_hits(expires_at_ms);

CREATE TABLE notification_delivery_ready_queue (
  delivery_id TEXT PRIMARY KEY CHECK (delivery_id <> ''),
  ready_at_ms INTEGER NOT NULL,
  expires_at_ms INTEGER NOT NULL CHECK (expires_at_ms >= 0),
  updated_at_ms INTEGER NOT NULL
);
CREATE INDEX idx_notification_delivery_ready_queue_expiry
  ON notification_delivery_ready_queue(expires_at_ms);
"#;
const MIGRATIONS_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS typed_notification_runtime_schema_migrations (
  version INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  checksum TEXT NOT NULL,
  applied_at_ms INTEGER NOT NULL
);
"#;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TypedNotificationRuntimeLease {
    pub(crate) name: String,
    pub(crate) token: String,
    pub(crate) expires_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TypedNotificationRuntimeCooldown {
    pub(crate) runtime_key: String,
    pub(crate) rule_id: String,
    pub(crate) group_key_token: String,
    pub(crate) until_iso: String,
    pub(crate) expires_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TypedNotificationWindowHit {
    pub(crate) runtime_key: String,
    pub(crate) rule_id: String,
    pub(crate) group_key_token: String,
    pub(crate) event_id: String,
    pub(crate) happened_at_ms: i64,
    pub(crate) expires_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TypedNotificationReadyDelivery {
    pub(crate) delivery_id: String,
    pub(crate) ready_at_ms: i64,
    pub(crate) expires_at_ms: i64,
}

#[derive(Clone)]
pub(crate) struct TypedNotificationRuntimeRepository {
    manager: ConnectionManager,
}

impl TypedNotificationRuntimeRepository {
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
                        "SELECT name, checksum FROM typed_notification_runtime_schema_migrations WHERE version = ?1",
                        [SCHEMA_VERSION],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                    )
                    .optional()?;
                match applied {
                    Some((name, stored)) if name == SCHEMA_NAME && stored == checksum => {
                        for table in [
                            "notification_runtime_leases",
                            "notification_runtime_cooldowns",
                            "notification_runtime_window_hits",
                            "notification_delivery_ready_queue",
                        ] {
                            let exists = tx.query_row(
                                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
                                [table],
                                |row| row.get::<_, bool>(0),
                            )?;
                            if !exists {
                                return Err(storage_error(format!(
                                    "typed notification runtime migration is recorded but {table} is missing"
                                )));
                            }
                        }
                    }
                    Some((name, _)) if name != SCHEMA_NAME => {
                        return Err(storage_error(
                            "typed notification runtime migration name mismatch",
                        ));
                    }
                    Some(_) => {
                        return Err(storage_error(
                            "typed notification runtime migration checksum mismatch",
                        ));
                    }
                    None => {
                        tx.execute_batch(SCHEMA_SQL)?;
                        tx.execute(
                            "INSERT INTO typed_notification_runtime_schema_migrations(version, name, checksum, applied_at_ms) VALUES (?1, ?2, ?3, ?4)",
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
        tx.execute("DELETE FROM notification_runtime_leases", [])?;
        tx.execute("DELETE FROM notification_runtime_cooldowns", [])?;
        tx.execute("DELETE FROM notification_runtime_window_hits", [])?;
        tx.execute("DELETE FROM notification_delivery_ready_queue", [])?;

        for key in legacy_keys_with_prefix_tx(tx, LEASE_PREFIX)? {
            reconcile_lease_tx(tx, &key)?;
        }
        for key in legacy_keys_with_prefix_tx(tx, COOLDOWN_PREFIX)? {
            reconcile_cooldown_tx(tx, &key)?;
        }
        for key in legacy_keys_with_prefix_tx(tx, WINDOW_PREFIX)? {
            reconcile_window_tx(tx, &key)?;
        }
        reconcile_ready_queue_tx(tx)?;
        Ok(())
    }

    pub(crate) fn reconcile_legacy_keys_tx(
        tx: &Transaction<'_>,
        keys: &[String],
    ) -> StorageResult<()> {
        for key in keys {
            if key.starts_with(LEASE_PREFIX) {
                reconcile_lease_tx(tx, key)?;
            } else if key.starts_with(COOLDOWN_PREFIX) {
                reconcile_cooldown_tx(tx, key)?;
            } else if key.starts_with(WINDOW_PREFIX) {
                reconcile_window_tx(tx, key)?;
            } else if key == READY_KEY {
                reconcile_ready_queue_tx(tx)?;
            }
        }
        Ok(())
    }

    pub(crate) async fn verify_and_repair_key(&self, key: &str) -> StorageResult<bool> {
        let key = key.to_string();
        self.manager
            .call(move |conn| {
                let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                let matched = if key.starts_with(LEASE_PREFIX) {
                    let legacy = legacy_lease_tx(&tx, &key)?;
                    let typed = typed_lease_tx(&tx, lease_name(&key)?)?;
                    let matched = legacy == typed;
                    if !matched {
                        reconcile_lease_tx(&tx, &key)?;
                    }
                    matched
                } else if key.starts_with(COOLDOWN_PREFIX) {
                    let legacy = legacy_cooldown_tx(&tx, &key)?;
                    let typed = typed_cooldown_tx(&tx, &key)?;
                    let matched = legacy == typed;
                    if !matched {
                        reconcile_cooldown_tx(&tx, &key)?;
                    }
                    matched
                } else if key.starts_with(WINDOW_PREFIX) {
                    let legacy = legacy_window_tx(&tx, &key)?;
                    let typed = typed_window_tx(&tx, &key)?;
                    let matched = legacy == typed;
                    if !matched {
                        reconcile_window_tx(&tx, &key)?;
                    }
                    matched
                } else if key == READY_KEY {
                    let legacy = legacy_ready_queue_tx(&tx)?;
                    let typed = typed_ready_queue_tx(&tx)?;
                    let matched = legacy == typed;
                    if !matched {
                        reconcile_ready_queue_tx(&tx)?;
                    }
                    matched
                } else {
                    return Err(storage_error("unowned notification runtime key"));
                };
                tx.commit()?;
                Ok(matched)
            })
            .await
    }

    #[cfg(test)]
    pub(crate) async fn load_lease(
        &self,
        name: &str,
    ) -> StorageResult<Option<TypedNotificationRuntimeLease>> {
        let name = name.to_string();
        self.manager
            .call(move |conn| {
                conn.query_row(
                    "SELECT token, expires_at_ms FROM notification_runtime_leases WHERE name = ?1",
                    [&name],
                    |row| {
                        Ok(TypedNotificationRuntimeLease {
                            name: name.clone(),
                            token: row.get(0)?,
                            expires_at_ms: row.get(1)?,
                        })
                    },
                )
                .optional()
                .map_err(Into::into)
            })
            .await
    }

    #[cfg(test)]
    pub(crate) async fn load_window(
        &self,
        key: &str,
    ) -> StorageResult<Vec<TypedNotificationWindowHit>> {
        let key = key.to_string();
        self.manager
            .call(move |conn| typed_window_conn(conn, &key))
            .await
    }

    #[cfg(test)]
    pub(crate) async fn load_cooldown(
        &self,
        key: &str,
    ) -> StorageResult<Option<TypedNotificationRuntimeCooldown>> {
        let key = key.to_string();
        self.manager
            .call(move |conn| {
                conn.query_row(
                    "SELECT rule_id, group_key_token, until_iso, expires_at_ms
                     FROM notification_runtime_cooldowns WHERE runtime_key = ?1",
                    [&key],
                    |row| {
                        Ok(TypedNotificationRuntimeCooldown {
                            runtime_key: key.clone(),
                            rule_id: row.get(0)?,
                            group_key_token: row.get(1)?,
                            until_iso: row.get(2)?,
                            expires_at_ms: row.get(3)?,
                        })
                    },
                )
                .optional()
                .map_err(Into::into)
            })
            .await
    }

    #[cfg(test)]
    pub(crate) async fn load_ready_queue(
        &self,
    ) -> StorageResult<Vec<TypedNotificationReadyDelivery>> {
        self.manager.call(|conn| typed_ready_queue_conn(conn)).await
    }
}

fn legacy_keys_with_prefix_tx(tx: &Transaction<'_>, prefix: &str) -> StorageResult<Vec<String>> {
    let mut statement = tx.prepare(
        "SELECT key FROM kv_keys
         WHERE substr(key, 1, ?1) = ?2
         ORDER BY key",
    )?;
    statement
        .query_map(params![prefix.len() as i64, prefix], |row| {
            row.get::<_, String>(0)
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn live_string_tx(tx: &Transaction<'_>, key: &str) -> StorageResult<Option<(String, i64)>> {
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

fn lease_name(key: &str) -> StorageResult<&str> {
    key.strip_prefix(LEASE_PREFIX)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| storage_error("invalid notification runtime lease key"))
}

fn runtime_group_identity<'a>(key: &'a str, prefix: &str) -> StorageResult<(&'a str, &'a str)> {
    let suffix = key
        .strip_prefix(prefix)
        .ok_or_else(|| storage_error("invalid notification runtime group key"))?;
    suffix
        .split_once(':')
        .filter(|(rule_id, token)| !rule_id.is_empty() && !token.is_empty())
        .ok_or_else(|| storage_error("invalid notification runtime group identity"))
}

fn legacy_lease_tx(
    tx: &Transaction<'_>,
    key: &str,
) -> StorageResult<Option<TypedNotificationRuntimeLease>> {
    let name = lease_name(key)?;
    let Some((token, expires_at_ms)) = live_string_tx(tx, key)? else {
        return Ok(None);
    };
    if token.is_empty() {
        return Ok(None);
    }
    Ok(Some(TypedNotificationRuntimeLease {
        name: name.to_string(),
        token,
        expires_at_ms,
    }))
}

fn typed_lease_tx(
    tx: &Transaction<'_>,
    name: &str,
) -> StorageResult<Option<TypedNotificationRuntimeLease>> {
    tx.query_row(
        "SELECT token, expires_at_ms FROM notification_runtime_leases WHERE name = ?1",
        [name],
        |row| {
            Ok(TypedNotificationRuntimeLease {
                name: name.to_string(),
                token: row.get(0)?,
                expires_at_ms: row.get(1)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

fn reconcile_lease_tx(tx: &Transaction<'_>, key: &str) -> StorageResult<()> {
    let name = lease_name(key)?;
    match legacy_lease_tx(tx, key)? {
        Some(lease) => {
            tx.execute(
                "INSERT INTO notification_runtime_leases(name, token, expires_at_ms, updated_at_ms)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(name) DO UPDATE SET token = excluded.token,
                   expires_at_ms = excluded.expires_at_ms, updated_at_ms = excluded.updated_at_ms
                 WHERE notification_runtime_leases.token <> excluded.token
                    OR notification_runtime_leases.expires_at_ms <> excluded.expires_at_ms",
                params![
                    lease.name,
                    lease.token,
                    lease.expires_at_ms,
                    crate::time_utils::now_ms()
                ],
            )?;
        }
        None => {
            tx.execute(
                "DELETE FROM notification_runtime_leases WHERE name = ?1",
                [name],
            )?;
        }
    }
    Ok(())
}

fn legacy_cooldown_tx(
    tx: &Transaction<'_>,
    key: &str,
) -> StorageResult<Option<TypedNotificationRuntimeCooldown>> {
    let (rule_id, group_key_token) = runtime_group_identity(key, COOLDOWN_PREFIX)?;
    let Some((until_iso, expires_at_ms)) = live_string_tx(tx, key)? else {
        return Ok(None);
    };
    if until_iso.is_empty() {
        return Ok(None);
    }
    Ok(Some(TypedNotificationRuntimeCooldown {
        runtime_key: key.to_string(),
        rule_id: rule_id.to_string(),
        group_key_token: group_key_token.to_string(),
        until_iso,
        expires_at_ms,
    }))
}

fn typed_cooldown_tx(
    tx: &Transaction<'_>,
    key: &str,
) -> StorageResult<Option<TypedNotificationRuntimeCooldown>> {
    tx.query_row(
        "SELECT rule_id, group_key_token, until_iso, expires_at_ms
         FROM notification_runtime_cooldowns WHERE runtime_key = ?1",
        [key],
        |row| {
            Ok(TypedNotificationRuntimeCooldown {
                runtime_key: key.to_string(),
                rule_id: row.get(0)?,
                group_key_token: row.get(1)?,
                until_iso: row.get(2)?,
                expires_at_ms: row.get(3)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

fn reconcile_cooldown_tx(tx: &Transaction<'_>, key: &str) -> StorageResult<()> {
    runtime_group_identity(key, COOLDOWN_PREFIX)?;
    match legacy_cooldown_tx(tx, key)? {
        Some(cooldown) => {
            tx.execute(
                "INSERT INTO notification_runtime_cooldowns(
                   runtime_key, rule_id, group_key_token, until_iso, expires_at_ms, updated_at_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(runtime_key) DO UPDATE SET rule_id = excluded.rule_id,
                   group_key_token = excluded.group_key_token, until_iso = excluded.until_iso,
                   expires_at_ms = excluded.expires_at_ms, updated_at_ms = excluded.updated_at_ms
                 WHERE notification_runtime_cooldowns.rule_id <> excluded.rule_id
                    OR notification_runtime_cooldowns.group_key_token <> excluded.group_key_token
                    OR notification_runtime_cooldowns.until_iso <> excluded.until_iso
                    OR notification_runtime_cooldowns.expires_at_ms <> excluded.expires_at_ms",
                params![
                    cooldown.runtime_key,
                    cooldown.rule_id,
                    cooldown.group_key_token,
                    cooldown.until_iso,
                    cooldown.expires_at_ms,
                    crate::time_utils::now_ms()
                ],
            )?;
        }
        None => {
            tx.execute(
                "DELETE FROM notification_runtime_cooldowns WHERE runtime_key = ?1",
                [key],
            )?;
        }
    }
    Ok(())
}

fn legacy_window_tx(
    tx: &Transaction<'_>,
    key: &str,
) -> StorageResult<Vec<TypedNotificationWindowHit>> {
    let (rule_id, group_key_token) = runtime_group_identity(key, WINDOW_PREFIX)?;
    let expires_at_ms = tx
        .query_row(
            "SELECT expires_at_ms FROM kv_keys
             WHERE key = ?1 AND kind = 'zset' AND expires_at_ms IS NOT NULL
               AND expires_at_ms > ?2",
            params![key, crate::time_utils::now_ms()],
            |row| row.get::<_, i64>(0),
        )
        .optional()?;
    let Some(expires_at_ms) = expires_at_ms else {
        return Ok(Vec::new());
    };
    let mut statement =
        tx.prepare("SELECT member, score FROM kv_zset WHERE key = ?1 ORDER BY score, member")?;
    let rows = statement.query_map([key], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
    })?;
    let mut hits = Vec::new();
    for row in rows {
        let (event_id, score) = row?;
        if event_id.is_empty()
            || !score.is_finite()
            || score.fract() != 0.0
            || score > i64::MAX as f64
            || score < i64::MIN as f64
        {
            return Err(storage_error("invalid notification window hit"));
        }
        hits.push(TypedNotificationWindowHit {
            runtime_key: key.to_string(),
            rule_id: rule_id.to_string(),
            group_key_token: group_key_token.to_string(),
            event_id,
            happened_at_ms: score as i64,
            expires_at_ms,
        });
    }
    Ok(hits)
}

fn typed_window_tx(
    tx: &Transaction<'_>,
    key: &str,
) -> StorageResult<Vec<TypedNotificationWindowHit>> {
    let mut statement = tx.prepare(
        "SELECT rule_id, group_key_token, event_id, happened_at_ms, expires_at_ms
         FROM notification_runtime_window_hits
         WHERE runtime_key = ?1 ORDER BY happened_at_ms, event_id",
    )?;
    statement
        .query_map([key], |row| {
            Ok(TypedNotificationWindowHit {
                runtime_key: key.to_string(),
                rule_id: row.get(0)?,
                group_key_token: row.get(1)?,
                event_id: row.get(2)?,
                happened_at_ms: row.get(3)?,
                expires_at_ms: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(Into::into)
}

#[cfg(test)]
fn typed_window_conn(
    conn: &tokio_rusqlite::rusqlite::Connection,
    key: &str,
) -> StorageResult<Vec<TypedNotificationWindowHit>> {
    let mut statement = conn.prepare(
        "SELECT rule_id, group_key_token, event_id, happened_at_ms, expires_at_ms
         FROM notification_runtime_window_hits
         WHERE runtime_key = ?1 ORDER BY happened_at_ms, event_id",
    )?;
    statement
        .query_map([key], |row| {
            Ok(TypedNotificationWindowHit {
                runtime_key: key.to_string(),
                rule_id: row.get(0)?,
                group_key_token: row.get(1)?,
                event_id: row.get(2)?,
                happened_at_ms: row.get(3)?,
                expires_at_ms: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn reconcile_window_tx(tx: &Transaction<'_>, key: &str) -> StorageResult<()> {
    let hits = legacy_window_tx(tx, key)?;
    tx.execute(
        "DELETE FROM notification_runtime_window_hits WHERE runtime_key = ?1",
        [key],
    )?;
    for hit in hits {
        tx.execute(
            "INSERT INTO notification_runtime_window_hits(
               runtime_key, rule_id, group_key_token, event_id, happened_at_ms,
               expires_at_ms, updated_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                hit.runtime_key,
                hit.rule_id,
                hit.group_key_token,
                hit.event_id,
                hit.happened_at_ms,
                hit.expires_at_ms,
                crate::time_utils::now_ms()
            ],
        )?;
    }
    Ok(())
}

fn legacy_ready_queue_tx(
    tx: &Transaction<'_>,
) -> StorageResult<Vec<TypedNotificationReadyDelivery>> {
    let expires_at_ms = tx
        .query_row(
            "SELECT expires_at_ms FROM kv_keys
             WHERE key = ?1 AND kind = 'zset' AND expires_at_ms IS NOT NULL
               AND expires_at_ms > ?2",
            params![READY_KEY, crate::time_utils::now_ms()],
            |row| row.get::<_, i64>(0),
        )
        .optional()?;
    let Some(expires_at_ms) = expires_at_ms else {
        return Ok(Vec::new());
    };
    let mut statement =
        tx.prepare("SELECT member, score FROM kv_zset WHERE key = ?1 ORDER BY score, member")?;
    let rows = statement.query_map([READY_KEY], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
    })?;
    let mut deliveries = Vec::new();
    for row in rows {
        let (delivery_id, score) = row?;
        if delivery_id.is_empty()
            || !score.is_finite()
            || score.fract() != 0.0
            || score > i64::MAX as f64
            || score < i64::MIN as f64
        {
            return Err(storage_error("invalid notification ready queue member"));
        }
        deliveries.push(TypedNotificationReadyDelivery {
            delivery_id,
            ready_at_ms: score as i64,
            expires_at_ms,
        });
    }
    Ok(deliveries)
}

fn typed_ready_queue_tx(
    tx: &Transaction<'_>,
) -> StorageResult<Vec<TypedNotificationReadyDelivery>> {
    let mut statement = tx.prepare(
        "SELECT delivery_id, ready_at_ms, expires_at_ms
         FROM notification_delivery_ready_queue ORDER BY ready_at_ms, delivery_id",
    )?;
    statement
        .query_map([], |row| {
            Ok(TypedNotificationReadyDelivery {
                delivery_id: row.get(0)?,
                ready_at_ms: row.get(1)?,
                expires_at_ms: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(Into::into)
}

#[cfg(test)]
fn typed_ready_queue_conn(
    conn: &tokio_rusqlite::rusqlite::Connection,
) -> StorageResult<Vec<TypedNotificationReadyDelivery>> {
    let mut statement = conn.prepare(
        "SELECT delivery_id, ready_at_ms, expires_at_ms
         FROM notification_delivery_ready_queue ORDER BY ready_at_ms, delivery_id",
    )?;
    statement
        .query_map([], |row| {
            Ok(TypedNotificationReadyDelivery {
                delivery_id: row.get(0)?,
                ready_at_ms: row.get(1)?,
                expires_at_ms: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn reconcile_ready_queue_tx(tx: &Transaction<'_>) -> StorageResult<()> {
    let deliveries = legacy_ready_queue_tx(tx)?;
    tx.execute("DELETE FROM notification_delivery_ready_queue", [])?;
    for delivery in deliveries {
        tx.execute(
            "INSERT INTO notification_delivery_ready_queue(
               delivery_id, ready_at_ms, expires_at_ms, updated_at_ms
             ) VALUES (?1, ?2, ?3, ?4)",
            params![
                delivery.delivery_id,
                delivery.ready_at_ms,
                delivery.expires_at_ms,
                crate::time_utils::now_ms()
            ],
        )?;
    }
    Ok(())
}
