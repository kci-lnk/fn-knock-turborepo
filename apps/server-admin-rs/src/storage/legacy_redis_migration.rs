use std::{collections::BTreeMap, env, time::Duration};

use ::redis::{AsyncConnectionConfig, Value as RedisValue, aio::MultiplexedConnection};
use serde_json::{Value, json};
use url::Url;

use crate::{
    storage::{StorageResult, storage_error},
    store::{Store, node_locale_compare_ordering},
    time_utils,
};

const KEY_PREFIX: &str = "fn_knock:";
const SCAN_COUNT: usize = 500;

const STATUS_KEY: &str = "redis_migration_status";
const STATUS_DONE: &str = "done";
const STATUS_IMPORTED: &str = "imported";
const STATUS_RUNNING: &str = "running";
const STATUS_FAILED: &str = "failed";
const STATUS_UNAVAILABLE: &str = "unavailable";
const SOURCE_KEY: &str = "redis_migration_source";
const KEY_COUNT_KEY: &str = "redis_migration_key_count";
const ENTRY_COUNT_KEY: &str = "redis_migration_entry_count";
const CLEANUP_KEY_COUNT_KEY: &str = "redis_migration_cleanup_key_count";
const COMPLETED_AT_KEY: &str = "redis_migration_completed_at";
const LAST_ERROR_KEY: &str = "redis_migration_last_error";

const TRANSIENT_KEY_PREFIXES: &[&str] = &[
    "fn_knock:acme:runtime-lock",
    "fn_knock:acme:job:",
    "fn_knock:auth_log_data:",
    "fn_knock:auth_logs:",
    "fn_knock:auth:subdomain_rule_grant:",
    "fn_knock:auth:subdomain_rule_grant_active:",
    "fn_knock:auth:subdomain_rule_rate:",
    "fn_knock:backoff:",
    "fn_knock:cleanup:",
    "fn_knock:docker_admin:login_backoff:",
    "fn_knock:fnos-share:validation:",
    "fn_knock:lock:",
    "fn_knock:login_backoff:",
    "fn_knock:nonce:",
    "fn_knock:notifications:runtime:lock:",
    "fn_knock:notifications:runtime:cooldown:",
    "fn_knock:notifications:runtime:window:",
    "fn_knock:oidc:state:",
    "fn_knock:passkey:bind:",
    "fn_knock:passkey:challenge:",
    "fn_knock:passkey:state:",
];

const TRANSIENT_KEY_SUFFIXES: &[&str] = &[":lock", ":lease", ":runtime-lock"];

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct LegacyRedisMigrationOptions {
    pub(crate) require_source: bool,
    pub(crate) force: bool,
    pub(crate) cleanup_source: bool,
}

#[derive(Clone, Debug)]
pub(crate) enum LegacyRedisMigrationOutcome {
    Disabled,
    AlreadyDone,
    SkippedExistingSqlite {
        existing_keys: i64,
    },
    Unavailable {
        reason: String,
    },
    Completed {
        source: String,
        key_count: usize,
        entry_count: usize,
        cleanup_key_count: usize,
    },
}

impl LegacyRedisMigrationOutcome {
    pub(crate) fn summary(&self) -> String {
        match self {
            Self::Disabled => "legacy Redis migration disabled".to_string(),
            Self::AlreadyDone => "legacy Redis migration already completed".to_string(),
            Self::SkippedExistingSqlite { existing_keys } => format!(
                "legacy Redis migration skipped: SQLite already has {existing_keys} fn_knock:* keys"
            ),
            Self::Unavailable { reason } => {
                format!("legacy Redis migration skipped: source unavailable ({reason})")
            }
            Self::Completed {
                source,
                key_count,
                entry_count,
                cleanup_key_count,
            } => format!(
                "legacy Redis migration completed: imported {entry_count} of {key_count} Redis keys from {source}; cleaned {cleanup_key_count} source Redis keys"
            ),
        }
    }
}

struct LegacyRedisConfig {
    url: String,
    fingerprint: String,
    connect_timeout: Duration,
    command_timeout: Duration,
}

struct LegacyRedisSnapshot {
    key_count: usize,
    entries: Vec<Value>,
}

pub(crate) async fn migrate_if_available(
    store: &Store,
    legacy_redis_url: &str,
    options: LegacyRedisMigrationOptions,
) -> StorageResult<LegacyRedisMigrationOutcome> {
    if env_flag("FN_KNOCK_DISABLE_REDIS_MIGRATION") {
        return Ok(LegacyRedisMigrationOutcome::Disabled);
    }

    let current_status = store.storage_meta_value(STATUS_KEY).await?;
    if current_status.as_deref() == Some(STATUS_DONE) && !options.force {
        return Ok(LegacyRedisMigrationOutcome::AlreadyDone);
    }

    let config = match LegacyRedisConfig::from_url(legacy_redis_url) {
        Ok(config) => config,
        Err(error)
            if options.require_source || current_status.as_deref() == Some(STATUS_IMPORTED) =>
        {
            return Err(error);
        }
        Err(error) => return record_unavailable(store, error.to_string()).await,
    };

    let mut connection = match connect_legacy_redis(&config).await {
        Ok(connection) => connection,
        Err(error)
            if options.require_source || current_status.as_deref() == Some(STATUS_IMPORTED) =>
        {
            return Err(error);
        }
        Err(error) => return record_unavailable(store, error.to_string()).await,
    };

    if current_status.as_deref() == Some(STATUS_IMPORTED) && !options.force {
        let key_count = storage_meta_usize(store, KEY_COUNT_KEY).await?;
        let entry_count = storage_meta_usize(store, ENTRY_COUNT_KEY).await?;
        return complete_imported_migration(
            store,
            &mut connection,
            &config,
            key_count,
            entry_count,
            options.cleanup_source,
        )
        .await;
    }

    let existing_sqlite_keys = store.count_keys_by_prefix(KEY_PREFIX).await?;
    if existing_sqlite_keys > 0
        && !options.force
        && !matches!(
            current_status.as_deref(),
            Some(STATUS_RUNNING | STATUS_FAILED | STATUS_UNAVAILABLE)
        )
    {
        return Ok(LegacyRedisMigrationOutcome::SkippedExistingSqlite {
            existing_keys: existing_sqlite_keys,
        });
    }

    store
        .set_storage_meta_value(STATUS_KEY, STATUS_RUNNING)
        .await?;
    store
        .set_storage_meta_value(SOURCE_KEY, &config.fingerprint)
        .await?;

    match export_legacy_redis_snapshot(&mut connection).await {
        Ok(snapshot) => {
            store
                .replace_backup_entries_by_prefix(KEY_PREFIX, &snapshot.entries, SCAN_COUNT)
                .await?;
            store
                .set_storage_meta_value(STATUS_KEY, STATUS_IMPORTED)
                .await?;
            store
                .set_storage_meta_value(KEY_COUNT_KEY, &snapshot.key_count.to_string())
                .await?;
            store
                .set_storage_meta_value(ENTRY_COUNT_KEY, &snapshot.entries.len().to_string())
                .await?;
            complete_imported_migration(
                store,
                &mut connection,
                &config,
                snapshot.key_count,
                snapshot.entries.len(),
                options.cleanup_source,
            )
            .await
        }
        Err(error) => {
            let message = error.to_string();
            store
                .set_storage_meta_value(STATUS_KEY, STATUS_FAILED)
                .await?;
            store
                .set_storage_meta_value(LAST_ERROR_KEY, &message)
                .await?;
            Err(storage_error(format!(
                "legacy Redis migration failed: {message}"
            )))
        }
    }
}

async fn record_unavailable(
    store: &Store,
    reason: String,
) -> StorageResult<LegacyRedisMigrationOutcome> {
    store
        .set_storage_meta_value(STATUS_KEY, STATUS_UNAVAILABLE)
        .await?;
    store
        .set_storage_meta_value(LAST_ERROR_KEY, &reason)
        .await?;
    Ok(LegacyRedisMigrationOutcome::Unavailable { reason })
}

async fn complete_imported_migration(
    store: &Store,
    connection: &mut MultiplexedConnection,
    config: &LegacyRedisConfig,
    key_count: usize,
    entry_count: usize,
    cleanup_source: bool,
) -> StorageResult<LegacyRedisMigrationOutcome> {
    let cleanup_key_count = if cleanup_source {
        match cleanup_legacy_redis_keys(connection).await {
            Ok(count) => count,
            Err(error) => {
                let message = format!(
                    "legacy Redis migration imported into SQLite but source cleanup failed: {error}"
                );
                store
                    .set_storage_meta_value(LAST_ERROR_KEY, &message)
                    .await?;
                return Err(storage_error(message));
            }
        }
    } else {
        0
    };

    store
        .set_storage_meta_value(STATUS_KEY, STATUS_DONE)
        .await?;
    store
        .set_storage_meta_value(CLEANUP_KEY_COUNT_KEY, &cleanup_key_count.to_string())
        .await?;
    store
        .set_storage_meta_value(COMPLETED_AT_KEY, &time_utils::now_iso())
        .await?;
    store.set_storage_meta_value(LAST_ERROR_KEY, "").await?;
    Ok(LegacyRedisMigrationOutcome::Completed {
        source: config.fingerprint.clone(),
        key_count,
        entry_count,
        cleanup_key_count,
    })
}

async fn storage_meta_usize(store: &Store, key: &str) -> StorageResult<usize> {
    Ok(store
        .storage_meta_value(key)
        .await?
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0))
}

impl LegacyRedisConfig {
    fn from_url(raw_url: &str) -> StorageResult<Self> {
        let url = raw_url.trim();
        if url.is_empty() {
            return Err(storage_error("legacy Redis URL is empty"));
        }
        let parsed = Url::parse(url)
            .map_err(|error| storage_error(format!("legacy Redis URL is invalid: {error}")))?;
        if parsed.scheme() != "redis" {
            return Err(storage_error("legacy Redis URL must use redis://"));
        }
        let Some(host) = parsed.host_str() else {
            return Err(storage_error("legacy Redis URL must include a host"));
        };
        let port = parsed.port_or_known_default().unwrap_or(6379);
        let database = parsed.path().trim_start_matches('/');
        let fingerprint = if database.is_empty() {
            format!("redis://{host}:{port}/")
        } else {
            format!("redis://{host}:{port}/{database}")
        };
        Ok(Self {
            url: url.to_string(),
            fingerprint,
            connect_timeout: duration_ms_from_env("FN_KNOCK_LEGACY_REDIS_CONNECT_TIMEOUT_MS", 1200),
            command_timeout: duration_ms_from_env("FN_KNOCK_LEGACY_REDIS_COMMAND_TIMEOUT_MS", 3000),
        })
    }
}

async fn connect_legacy_redis(config: &LegacyRedisConfig) -> StorageResult<MultiplexedConnection> {
    let client = ::redis::Client::open(config.url.as_str()).map_err(redis_error)?;
    let connection_config = AsyncConnectionConfig::new()
        .set_connection_timeout(Some(config.connect_timeout))
        .set_response_timeout(Some(config.command_timeout));
    let mut connection = client
        .get_multiplexed_async_connection_with_config(&connection_config)
        .await
        .map_err(redis_error)?;
    let _: String = ::redis::cmd("PING")
        .query_async(&mut connection)
        .await
        .map_err(redis_error)?;
    Ok(connection)
}

async fn export_legacy_redis_snapshot(
    connection: &mut MultiplexedConnection,
) -> StorageResult<LegacyRedisSnapshot> {
    let keys = scan_legacy_redis_keys(connection).await?;
    let mut entries = Vec::new();
    for key in &keys {
        if !should_migrate_legacy_key(key) {
            continue;
        }
        if let Some(entry) = export_legacy_redis_entry(connection, key).await? {
            entries.push(entry);
        }
    }
    entries.sort_by(|left, right| {
        node_locale_compare_ordering(
            left.get("key").and_then(Value::as_str).unwrap_or(""),
            right.get("key").and_then(Value::as_str).unwrap_or(""),
        )
    });
    Ok(LegacyRedisSnapshot {
        key_count: keys.len(),
        entries,
    })
}

async fn scan_legacy_redis_keys(
    connection: &mut MultiplexedConnection,
) -> StorageResult<Vec<String>> {
    let mut cursor = 0_u64;
    let mut keys = Vec::new();
    loop {
        let (next_cursor, mut batch): (u64, Vec<String>) = ::redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg(format!("{KEY_PREFIX}*"))
            .arg("COUNT")
            .arg(SCAN_COUNT)
            .query_async(connection)
            .await
            .map_err(redis_error)?;
        keys.append(&mut batch);
        if next_cursor == 0 {
            break;
        }
        cursor = next_cursor;
    }
    keys.sort_by(|left, right| node_locale_compare_ordering(left, right));
    keys.dedup();
    Ok(keys)
}

async fn cleanup_legacy_redis_keys(connection: &mut MultiplexedConnection) -> StorageResult<usize> {
    let keys = scan_legacy_redis_keys(connection).await?;
    let mut deleted = 0usize;
    for chunk in keys.chunks(SCAN_COUNT) {
        let count: i64 = ::redis::cmd("DEL")
            .arg(chunk)
            .query_async(connection)
            .await
            .map_err(redis_error)?;
        deleted += count.max(0) as usize;
    }
    Ok(deleted)
}

async fn export_legacy_redis_entry(
    connection: &mut MultiplexedConnection,
    key: &str,
) -> StorageResult<Option<Value>> {
    let value_type: String = ::redis::cmd("TYPE")
        .arg(key)
        .query_async(connection)
        .await
        .map_err(redis_error)?;
    if value_type == "none" {
        return Ok(None);
    }
    let ttl_ms: i64 = ::redis::cmd("PTTL")
        .arg(key)
        .query_async(connection)
        .await
        .map_err(redis_error)?;
    let ttl = if ttl_ms > 0 {
        Value::Number(ttl_ms.into())
    } else {
        Value::Null
    };

    match value_type.as_str() {
        "string" => {
            let value: Option<String> = ::redis::cmd("GET")
                .arg(key)
                .query_async(connection)
                .await
                .map_err(redis_error)?;
            Ok(value.map(|value| {
                json!({
                    "key": key,
                    "type": "string",
                    "ttl_ms": ttl,
                    "value": value,
                })
            }))
        }
        "hash" => {
            let value: BTreeMap<String, String> = ::redis::cmd("HGETALL")
                .arg(key)
                .query_async(connection)
                .await
                .map_err(redis_error)?;
            Ok(Some(json!({
                "key": key,
                "type": "hash",
                "ttl_ms": ttl,
                "value": value,
            })))
        }
        "list" => {
            let value: Vec<String> = ::redis::cmd("LRANGE")
                .arg(key)
                .arg(0)
                .arg(-1)
                .query_async(connection)
                .await
                .map_err(redis_error)?;
            Ok(Some(json!({
                "key": key,
                "type": "list",
                "ttl_ms": ttl,
                "value": value,
            })))
        }
        "set" => {
            let mut value: Vec<String> = ::redis::cmd("SMEMBERS")
                .arg(key)
                .query_async(connection)
                .await
                .map_err(redis_error)?;
            value.sort_by(|left, right| node_locale_compare_ordering(left, right));
            Ok(Some(json!({
                "key": key,
                "type": "set",
                "ttl_ms": ttl,
                "value": value,
            })))
        }
        "zset" => {
            let pairs: Vec<(String, f64)> = ::redis::cmd("ZRANGE")
                .arg(key)
                .arg(0)
                .arg(-1)
                .arg("WITHSCORES")
                .query_async(connection)
                .await
                .map_err(redis_error)?;
            let value = pairs
                .into_iter()
                .map(|(member, score)| json!({ "member": member, "score": score }))
                .collect::<Vec<_>>();
            Ok(Some(json!({
                "key": key,
                "type": "zset",
                "ttl_ms": ttl,
                "value": value,
            })))
        }
        "stream" => {
            let raw: RedisValue = ::redis::cmd("XRANGE")
                .arg(key)
                .arg("-")
                .arg("+")
                .query_async(connection)
                .await
                .map_err(redis_error)?;
            Ok(Some(json!({
                "key": key,
                "type": "stream",
                "ttl_ms": ttl,
                "value": parse_stream_entries(&raw)?,
            })))
        }
        _ => Err(storage_error(format!(
            "unsupported legacy Redis type for migration: {value_type} ({key})"
        ))),
    }
}

fn parse_stream_entries(raw: &RedisValue) -> StorageResult<Vec<Value>> {
    redis_array(raw)?
        .iter()
        .map(parse_stream_entry)
        .collect::<StorageResult<Vec<_>>>()
}

fn parse_stream_entry(raw: &RedisValue) -> StorageResult<Value> {
    let values = redis_array(raw)?;
    if values.len() != 2 {
        return Err(storage_error("legacy Redis stream entry has invalid shape"));
    }
    let id = redis_value_to_string(&values[0])?;
    let fields = redis_array(&values[1])?;
    if fields.is_empty() || fields.len() % 2 != 0 {
        return Err(storage_error(
            "legacy Redis stream entry fields are invalid",
        ));
    }
    let fields = fields
        .iter()
        .map(redis_value_to_string)
        .map(|value| value.map(Value::String))
        .collect::<StorageResult<Vec<_>>>()?;
    Ok(json!({ "id": id, "fields": fields }))
}

fn redis_array(value: &RedisValue) -> StorageResult<&[RedisValue]> {
    match value {
        RedisValue::Array(items) => Ok(items),
        RedisValue::Nil => Ok(&[]),
        _ => Err(storage_error("legacy Redis response is not an array")),
    }
}

fn redis_value_to_string(value: &RedisValue) -> StorageResult<String> {
    match value {
        RedisValue::BulkString(bytes) => Ok(String::from_utf8_lossy(bytes).into_owned()),
        RedisValue::SimpleString(value) => Ok(value.clone()),
        RedisValue::Okay => Ok("OK".to_string()),
        RedisValue::Int(value) => Ok(value.to_string()),
        RedisValue::Double(value) => Ok(value.to_string()),
        RedisValue::Boolean(value) => Ok(value.to_string()),
        RedisValue::VerbatimString { text, .. } => Ok(text.clone()),
        _ => ::redis::from_redis_value_ref::<String>(value)
            .map_err(|error| storage_error(format!("legacy Redis value is not a string: {error}"))),
    }
}

fn should_migrate_legacy_key(key: &str) -> bool {
    key.starts_with(KEY_PREFIX)
        && !TRANSIENT_KEY_PREFIXES
            .iter()
            .any(|prefix| key.starts_with(prefix))
        && !TRANSIENT_KEY_SUFFIXES
            .iter()
            .any(|suffix| key.ends_with(suffix))
        && !is_ddns_runtime_key(key)
        && !is_frpc_runtime_key(key)
}

fn is_ddns_runtime_key(key: &str) -> bool {
    matches!(
        key,
        "fn_knock:ddns:last_ip" | "fn_knock:ddns:last_check" | "fn_knock:ddns:logs:seq"
    ) || {
        let parts = key.split(':').collect::<Vec<_>>();
        parts.len() == 6
            && parts[0] == "fn_knock"
            && parts[1] == "ddns"
            && parts[2] == "v2"
            && parts[3] == "target"
            && matches!(parts[5], "last_ip" | "last_check")
    }
}

fn is_frpc_runtime_key(key: &str) -> bool {
    let parts = key.split(':').collect::<Vec<_>>();
    parts.len() >= 6
        && parts[0] == "fn_knock"
        && parts[1] == "frpc"
        && parts[2] == "v2"
        && parts[3] == "instance"
        && matches!(&parts[5..], ["runtime"] | ["logs", "seq"])
}

fn duration_ms_from_env(name: &str, default_ms: u64) -> Duration {
    env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|value| *value > 0)
        .map(Duration::from_millis)
        .unwrap_or_else(|| Duration::from_millis(default_ms))
}

fn env_flag(name: &str) -> bool {
    env::var(name)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

fn redis_error(error: ::redis::RedisError) -> crate::storage::StorageError {
    storage_error(format!("legacy Redis error: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_key_filter_keeps_visible_history_and_drops_transient_runtime() {
        assert!(should_migrate_legacy_key("fn_knock:config"));
        assert!(should_migrate_legacy_key("fn_knock:events:data:1"));
        assert!(should_migrate_legacy_key("fn_knock:traffic:global:in"));
        assert!(should_migrate_legacy_key("fn_knock:waf:logs:dates"));
        assert!(should_migrate_legacy_key("fn_knock:session:abc"));
        assert!(should_migrate_legacy_key(
            "fn_knock:docker_admin:session:v1:abc"
        ));

        assert!(!should_migrate_legacy_key("fn_knock:lock:ddns"));
        assert!(!should_migrate_legacy_key("fn_knock:nonce:abc"));
        assert!(!should_migrate_legacy_key("fn_knock:passkey:challenge:abc"));
        assert!(!should_migrate_legacy_key("fn_knock:passkey:state:abc"));
        assert!(!should_migrate_legacy_key("fn_knock:oidc:state:abc"));
        assert!(!should_migrate_legacy_key(
            "fn_knock:auth:subdomain_rule_grant_active:app.example.com"
        ));
        assert!(!should_migrate_legacy_key(
            "fn_knock:notifications:runtime:lock:dispatch"
        ));
    }

    #[test]
    fn parses_stream_entries_without_losing_field_order() {
        let raw = RedisValue::Array(vec![RedisValue::Array(vec![
            RedisValue::BulkString(b"1-0".to_vec()),
            RedisValue::Array(vec![
                RedisValue::BulkString(b"z".to_vec()),
                RedisValue::BulkString(b"last".to_vec()),
                RedisValue::BulkString(b"a".to_vec()),
                RedisValue::BulkString(b"first".to_vec()),
            ]),
        ])]);

        let entries = parse_stream_entries(&raw).expect("parse stream");
        assert_eq!(
            entries,
            vec![json!({
                "id": "1-0",
                "fields": ["z", "last", "a", "first"],
            })]
        );
    }
}
