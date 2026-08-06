#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet, HashMap},
    fmt::Display,
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};
use tokio_rusqlite::{
    Connection, OptionalExtension,
    rusqlite::{self, ToSql, params, params_from_iter},
};

use crate::storage::{StorageError, StorageResult, storage_error};

pub(crate) type RedisResult<T> = StorageResult<T>;
#[allow(dead_code)]
pub(crate) type RedisError = StorageError;

#[allow(dead_code)]
pub(crate) trait AsyncCommands {}

#[derive(Clone)]
pub(crate) struct ConnectionManager {
    db: Connection,
    path: PathBuf,
}

impl AsyncCommands for ConnectionManager {}

pub(crate) mod streams {
    use super::*;

    #[derive(Clone, Debug, Default)]
    pub(crate) struct StreamReadOptions {
        pub(crate) count: Option<usize>,
    }

    impl StreamReadOptions {
        pub(crate) fn count(mut self, count: usize) -> Self {
            self.count = Some(count);
            self
        }
    }

    #[derive(Clone, Debug, Default)]
    pub(crate) struct StreamRangeReply {
        pub(crate) ids: Vec<StreamId>,
    }

    #[derive(Clone, Debug, Default)]
    pub(crate) struct StreamReadReply {
        pub(crate) keys: Vec<StreamKey>,
    }

    #[derive(Clone, Debug, Default)]
    pub(crate) struct StreamKey {
        #[allow(dead_code)]
        pub(crate) key: String,
        pub(crate) ids: Vec<StreamId>,
    }

    #[derive(Clone, Debug, Default)]
    pub(crate) struct StreamId {
        pub(crate) id: String,
        fields: HashMap<String, String>,
    }

    impl StreamId {
        pub(crate) fn new(id: String, fields: HashMap<String, String>) -> Self {
            Self { id, fields }
        }

        pub(crate) fn get<T: FromStreamField>(&self, field: &str) -> Option<T> {
            self.fields
                .get(field)
                .and_then(|value| T::from_field(value))
        }
    }

    pub(crate) trait FromStreamField: Sized {
        fn from_field(value: &str) -> Option<Self>;
    }

    impl FromStreamField for String {
        fn from_field(value: &str) -> Option<Self> {
            Some(value.to_string())
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum CmdOutput {
    Nil,
    Int(i64),
    String(String),
    OptionalString(Option<String>),
    Strings(Vec<String>),
    OptionalStrings(Vec<Option<String>>),
    StringPairs(Vec<String>),
    ZPairs(Vec<(String, f64)>),
    StreamEntries(Vec<(String, Vec<String>)>),
    Scan(String, Vec<String>),
    Ints(Vec<i64>),
}

pub(crate) trait FromCmdOutput: Sized {
    fn from_cmd_output(output: CmdOutput) -> RedisResult<Self>;
}

impl FromCmdOutput for () {
    fn from_cmd_output(_: CmdOutput) -> RedisResult<Self> {
        Ok(())
    }
}

impl FromCmdOutput for i64 {
    fn from_cmd_output(output: CmdOutput) -> RedisResult<Self> {
        match output {
            CmdOutput::Int(value) => Ok(value),
            _ => Err(storage_error("unexpected integer command result")),
        }
    }
}

impl FromCmdOutput for usize {
    fn from_cmd_output(output: CmdOutput) -> RedisResult<Self> {
        match output {
            CmdOutput::Int(value) => Ok(value.max(0) as usize),
            _ => Err(storage_error("unexpected usize command result")),
        }
    }
}

impl FromCmdOutput for String {
    fn from_cmd_output(output: CmdOutput) -> RedisResult<Self> {
        match output {
            CmdOutput::String(value) => Ok(value),
            CmdOutput::OptionalString(Some(value)) => Ok(value),
            _ => Err(storage_error("unexpected string command result")),
        }
    }
}

impl FromCmdOutput for Option<String> {
    fn from_cmd_output(output: CmdOutput) -> RedisResult<Self> {
        match output {
            CmdOutput::OptionalString(value) => Ok(value),
            CmdOutput::String(value) => Ok(Some(value)),
            CmdOutput::Nil => Ok(None),
            _ => Err(storage_error("unexpected optional string command result")),
        }
    }
}

impl FromCmdOutput for Vec<String> {
    fn from_cmd_output(output: CmdOutput) -> RedisResult<Self> {
        match output {
            CmdOutput::Strings(value) | CmdOutput::StringPairs(value) => Ok(value),
            _ => Err(storage_error("unexpected string vector command result")),
        }
    }
}

impl FromCmdOutput for Vec<Option<String>> {
    fn from_cmd_output(output: CmdOutput) -> RedisResult<Self> {
        match output {
            CmdOutput::OptionalStrings(value) => Ok(value),
            _ => Err(storage_error(
                "unexpected optional string vector command result",
            )),
        }
    }
}

impl FromCmdOutput for Vec<i64> {
    fn from_cmd_output(output: CmdOutput) -> RedisResult<Self> {
        match output {
            CmdOutput::Ints(value) => Ok(value),
            _ => Err(storage_error("unexpected integer vector command result")),
        }
    }
}

impl FromCmdOutput for (String, Vec<String>) {
    fn from_cmd_output(output: CmdOutput) -> RedisResult<Self> {
        match output {
            CmdOutput::Scan(cursor, keys) => Ok((cursor, keys)),
            _ => Err(storage_error("unexpected scan command result")),
        }
    }
}

impl FromCmdOutput for Vec<(String, f64)> {
    fn from_cmd_output(output: CmdOutput) -> RedisResult<Self> {
        match output {
            CmdOutput::ZPairs(value) => Ok(value),
            _ => Err(storage_error("unexpected zset pair command result")),
        }
    }
}

impl FromCmdOutput for Vec<(String, Vec<String>)> {
    fn from_cmd_output(output: CmdOutput) -> RedisResult<Self> {
        match output {
            CmdOutput::StreamEntries(value) => Ok(value),
            _ => Err(storage_error("unexpected stream command result")),
        }
    }
}

pub(crate) trait FromPipeOutput: Sized {
    fn from_pipe_outputs(outputs: Vec<CmdOutput>) -> RedisResult<Self>;
}

impl FromPipeOutput for () {
    fn from_pipe_outputs(_: Vec<CmdOutput>) -> RedisResult<Self> {
        Ok(())
    }
}

impl FromPipeOutput for Vec<i64> {
    fn from_pipe_outputs(outputs: Vec<CmdOutput>) -> RedisResult<Self> {
        outputs
            .into_iter()
            .map(|output| match output {
                CmdOutput::Int(value) => Ok(value),
                _ => Err(storage_error("unexpected pipeline integer result")),
            })
            .collect()
    }
}

pub(crate) trait FromOptionalString: Sized {
    fn from_optional_string(value: Option<String>) -> RedisResult<Self>;
}

impl FromOptionalString for Option<String> {
    fn from_optional_string(value: Option<String>) -> RedisResult<Self> {
        Ok(value)
    }
}

impl FromOptionalString for String {
    fn from_optional_string(value: Option<String>) -> RedisResult<Self> {
        value.ok_or_else(|| storage_error("missing string value"))
    }
}

pub(crate) trait FromDeleteCount: Sized {
    fn from_delete_count(value: usize) -> Self;
}

impl FromDeleteCount for () {
    fn from_delete_count(_: usize) -> Self {}
}

impl FromDeleteCount for usize {
    fn from_delete_count(value: usize) -> Self {
        value
    }
}

impl FromDeleteCount for i64 {
    fn from_delete_count(value: usize) -> Self {
        value as i64
    }
}

pub(crate) trait IntoKey {
    fn into_key(self) -> String;
}

impl IntoKey for &str {
    fn into_key(self) -> String {
        self.to_string()
    }
}

impl IntoKey for String {
    fn into_key(self) -> String {
        self
    }
}

impl IntoKey for &String {
    fn into_key(self) -> String {
        self.clone()
    }
}

pub(crate) trait IntoKeys {
    fn into_keys(self) -> Vec<String>;
}

impl<T: IntoKey> IntoKeys for T {
    fn into_keys(self) -> Vec<String> {
        vec![self.into_key()]
    }
}

impl IntoKeys for &[String] {
    fn into_keys(self) -> Vec<String> {
        self.to_vec()
    }
}

impl IntoKeys for &Vec<String> {
    fn into_keys(self) -> Vec<String> {
        self.clone()
    }
}

impl IntoKeys for Vec<String> {
    fn into_keys(self) -> Vec<String> {
        self
    }
}

impl<const N: usize> IntoKeys for &[&str; N] {
    fn into_keys(self) -> Vec<String> {
        self.iter().map(|value| (*value).to_string()).collect()
    }
}

pub(crate) trait IntoMembers {
    fn into_members(self) -> Vec<String>;
}

impl<T: IntoKey> IntoMembers for T {
    fn into_members(self) -> Vec<String> {
        vec![self.into_key()]
    }
}

impl IntoMembers for &[String] {
    fn into_members(self) -> Vec<String> {
        self.to_vec()
    }
}

impl IntoMembers for &Vec<String> {
    fn into_members(self) -> Vec<String> {
        self.clone()
    }
}

impl IntoMembers for Vec<String> {
    fn into_members(self) -> Vec<String> {
        self
    }
}

pub(crate) trait ToRedisArgs {
    fn append_args(&self, args: &mut Vec<String>);
}

macro_rules! impl_display_arg {
    ($($ty:ty),* $(,)?) => {
        $(
            impl ToRedisArgs for $ty {
                fn append_args(&self, args: &mut Vec<String>) {
                    args.push(self.to_string());
                }
            }
        )*
    };
}

impl_display_arg!(i64, i32, isize, usize, u64, u32, f64);

impl ToRedisArgs for &str {
    fn append_args(&self, args: &mut Vec<String>) {
        args.push((*self).to_string());
    }
}

impl ToRedisArgs for String {
    fn append_args(&self, args: &mut Vec<String>) {
        args.push(self.clone());
    }
}

impl ToRedisArgs for &String {
    fn append_args(&self, args: &mut Vec<String>) {
        args.push((*self).clone());
    }
}

impl ToRedisArgs for &[String] {
    fn append_args(&self, args: &mut Vec<String>) {
        args.extend(self.iter().cloned());
    }
}

impl ToRedisArgs for &Vec<String> {
    fn append_args(&self, args: &mut Vec<String>) {
        args.extend(self.iter().cloned());
    }
}

impl ToRedisArgs for Vec<String> {
    fn append_args(&self, args: &mut Vec<String>) {
        args.extend(self.iter().cloned());
    }
}

impl ToRedisArgs for Vec<&String> {
    fn append_args(&self, args: &mut Vec<String>) {
        args.extend(self.iter().map(|value| (*value).clone()));
    }
}

impl<const N: usize> ToRedisArgs for &[&str; N] {
    fn append_args(&self, args: &mut Vec<String>) {
        args.extend(self.iter().map(|value| (*value).to_string()));
    }
}

#[derive(Clone, Debug)]
struct CommandSpec {
    name: String,
    args: Vec<String>,
    ignore: bool,
}

impl CommandSpec {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_ascii_uppercase(),
            args: Vec::new(),
            ignore: false,
        }
    }
}

struct SchemaMigration {
    version: i64,
    name: &'static str,
    sql: &'static str,
    destructive: bool,
}

const SCHEMA_MIGRATIONS_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS schema_migrations (
  version INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  checksum TEXT NOT NULL,
  applied_at_ms INTEGER NOT NULL
);
"#;

const REDIS_COMPATIBLE_KEYSPACE_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS storage_meta (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL,
  updated_at_ms INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS kv_keys (
  key TEXT PRIMARY KEY,
  kind TEXT NOT NULL,
  expires_at_ms INTEGER
);
CREATE INDEX IF NOT EXISTS idx_kv_keys_kind ON kv_keys(kind);
CREATE INDEX IF NOT EXISTS idx_kv_keys_expires ON kv_keys(expires_at_ms);
CREATE TABLE IF NOT EXISTS kv_strings (
  key TEXT PRIMARY KEY REFERENCES kv_keys(key) ON DELETE CASCADE,
  value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS kv_hash (
  key TEXT NOT NULL REFERENCES kv_keys(key) ON DELETE CASCADE,
  field TEXT NOT NULL,
  value TEXT NOT NULL,
  PRIMARY KEY (key, field)
);
CREATE TABLE IF NOT EXISTS kv_list (
  key TEXT NOT NULL REFERENCES kv_keys(key) ON DELETE CASCADE,
  idx INTEGER NOT NULL,
  value TEXT NOT NULL,
  PRIMARY KEY (key, idx)
);
CREATE TABLE IF NOT EXISTS kv_set (
  key TEXT NOT NULL REFERENCES kv_keys(key) ON DELETE CASCADE,
  member TEXT NOT NULL,
  PRIMARY KEY (key, member)
);
CREATE TABLE IF NOT EXISTS kv_zset (
  key TEXT NOT NULL REFERENCES kv_keys(key) ON DELETE CASCADE,
  member TEXT NOT NULL,
  score REAL NOT NULL,
  PRIMARY KEY (key, member)
);
CREATE INDEX IF NOT EXISTS idx_kv_zset_score ON kv_zset(key, score, member);
CREATE TABLE IF NOT EXISTS kv_stream (
  key TEXT NOT NULL REFERENCES kv_keys(key) ON DELETE CASCADE,
  id TEXT NOT NULL,
  seq INTEGER PRIMARY KEY AUTOINCREMENT,
  fields_json TEXT NOT NULL,
  UNIQUE (key, id)
);
CREATE INDEX IF NOT EXISTS idx_kv_stream_key_seq ON kv_stream(key, seq);
"#;

const REDIS_COMPATIBLE_STREAM_METADATA_SQL: &str = r#"
CREATE TABLE kv_stream_v2 (
  key TEXT NOT NULL REFERENCES kv_keys(key) ON DELETE CASCADE,
  id TEXT NOT NULL,
  id_ms INTEGER NOT NULL CHECK (id_ms >= 0),
  id_sequence INTEGER NOT NULL CHECK (id_sequence >= 0),
  seq INTEGER PRIMARY KEY AUTOINCREMENT,
  fields_json TEXT NOT NULL,
  CHECK (id = printf('%lld-%lld', id_ms, id_sequence)),
  UNIQUE (key, id)
);
INSERT INTO kv_stream_v2(key, id, id_ms, id_sequence, seq, fields_json)
SELECT
  key,
  id,
  CAST(substr(id, 1, instr(id, '-') - 1) AS INTEGER),
  CAST(substr(id, instr(id, '-') + 1) AS INTEGER),
  seq,
  fields_json
FROM kv_stream;
DROP TABLE kv_stream;
ALTER TABLE kv_stream_v2 RENAME TO kv_stream;
CREATE INDEX idx_kv_stream_key_seq ON kv_stream(key, seq);
CREATE INDEX idx_kv_stream_key_id_parts
  ON kv_stream(key, id_ms, id_sequence);

CREATE TABLE IF NOT EXISTS kv_stream_meta (
  key TEXT PRIMARY KEY REFERENCES kv_keys(key) ON DELETE CASCADE,
  last_generated_ms INTEGER NOT NULL,
  last_generated_seq INTEGER NOT NULL
);

INSERT INTO kv_stream_meta(key, last_generated_ms, last_generated_seq)
SELECT stream.key, stream.id_ms, stream.id_sequence
FROM kv_stream AS stream
WHERE NOT EXISTS (
  SELECT 1
  FROM kv_stream AS newer
  WHERE newer.key = stream.key
    AND (newer.id_ms, newer.id_sequence) > (stream.id_ms, stream.id_sequence)
);

DELETE FROM kv_keys
WHERE kind = 'hash' AND NOT EXISTS (SELECT 1 FROM kv_hash WHERE kv_hash.key = kv_keys.key);
DELETE FROM kv_keys
WHERE kind = 'list' AND NOT EXISTS (SELECT 1 FROM kv_list WHERE kv_list.key = kv_keys.key);
DELETE FROM kv_keys
WHERE kind = 'set' AND NOT EXISTS (SELECT 1 FROM kv_set WHERE kv_set.key = kv_keys.key);
DELETE FROM kv_keys
WHERE kind = 'zset' AND NOT EXISTS (SELECT 1 FROM kv_zset WHERE kv_zset.key = kv_keys.key);
"#;

const SCHEMA_MIGRATIONS: &[SchemaMigration] = &[
    SchemaMigration {
        version: 1,
        name: "redis_compatible_keyspace",
        sql: REDIS_COMPATIBLE_KEYSPACE_SQL,
        destructive: false,
    },
    SchemaMigration {
        version: 2,
        name: "redis_compatible_stream_metadata",
        sql: REDIS_COMPATIBLE_STREAM_METADATA_SQL,
        destructive: true,
    },
];

pub(crate) struct Cmd {
    spec: CommandSpec,
}

pub(crate) fn cmd(name: &str) -> Cmd {
    Cmd {
        spec: CommandSpec::new(name),
    }
}

impl Cmd {
    pub(crate) fn arg<T: ToRedisArgs>(mut self, value: T) -> Self {
        value.append_args(&mut self.spec.args);
        self
    }

    pub(crate) async fn query_async<T: FromCmdOutput>(
        self,
        conn: &mut ConnectionManager,
    ) -> RedisResult<T> {
        T::from_cmd_output(conn.execute_command(self.spec).await?)
    }
}

pub(crate) struct Pipeline {
    commands: Vec<CommandSpec>,
    current: Option<CommandSpec>,
}

pub(crate) fn pipe() -> Pipeline {
    Pipeline {
        commands: Vec::new(),
        current: None,
    }
}

impl Pipeline {
    pub(crate) fn cmd(&mut self, name: &str) -> &mut Self {
        self.flush_current();
        self.current = Some(CommandSpec::new(name));
        self
    }

    pub(crate) fn arg<T: ToRedisArgs>(&mut self, value: T) -> &mut Self {
        if let Some(current) = &mut self.current {
            value.append_args(&mut current.args);
        }
        self
    }

    pub(crate) fn ignore(&mut self) -> &mut Self {
        if let Some(current) = &mut self.current {
            current.ignore = true;
        } else if let Some(last) = self.commands.last_mut() {
            last.ignore = true;
        }
        self.flush_current();
        self
    }

    pub(crate) fn set<K: IntoKey, V: Display>(&mut self, key: K, value: V) -> &mut Self {
        self.push_simple("SET", vec![key.into_key(), value.to_string()])
    }

    pub(crate) fn set_ex<K: IntoKey, V: Display>(
        &mut self,
        key: K,
        value: V,
        ttl_seconds: u64,
    ) -> &mut Self {
        self.push_simple(
            "SETEX",
            vec![key.into_key(), ttl_seconds.to_string(), value.to_string()],
        )
    }

    pub(crate) fn del<K: IntoKeys>(&mut self, keys: K) -> &mut Self {
        self.push_simple("DEL", keys.into_keys())
    }

    pub(crate) fn hset<K: IntoKey, F: Display, V: Display>(
        &mut self,
        key: K,
        field: F,
        value: V,
    ) -> &mut Self {
        self.push_simple(
            "HSET",
            vec![key.into_key(), field.to_string(), value.to_string()],
        )
    }

    pub(crate) fn hset_multiple(&mut self, key: &str, values: &[(&String, &String)]) -> &mut Self {
        let mut args = vec![key.to_string()];
        for (field, value) in values {
            args.push((*field).clone());
            args.push((*value).clone());
        }
        self.push_simple("HSET", args)
    }

    pub(crate) fn hdel<K: IntoKey, F: IntoMembers>(&mut self, key: K, fields: F) -> &mut Self {
        let mut args = vec![key.into_key()];
        args.extend(fields.into_members());
        self.push_simple("HDEL", args)
    }

    pub(crate) fn sadd<K: IntoKey, M: IntoMembers>(&mut self, key: K, members: M) -> &mut Self {
        let mut args = vec![key.into_key()];
        args.extend(members.into_members());
        self.push_simple("SADD", args)
    }

    pub(crate) fn srem<K: IntoKey, M: IntoMembers>(&mut self, key: K, members: M) -> &mut Self {
        let mut args = vec![key.into_key()];
        args.extend(members.into_members());
        self.push_simple("SREM", args)
    }

    pub(crate) fn zadd<K: IntoKey, M: Display, S: Display>(
        &mut self,
        key: K,
        member: M,
        score: S,
    ) -> &mut Self {
        self.push_simple(
            "ZADD",
            vec![key.into_key(), score.to_string(), member.to_string()],
        )
    }

    pub(crate) fn zrem<K: IntoKey, M: IntoMembers>(&mut self, key: K, members: M) -> &mut Self {
        let mut args = vec![key.into_key()];
        args.extend(members.into_members());
        self.push_simple("ZREM", args)
    }

    pub(crate) fn zrembyscore<K: IntoKey, Min: Display, Max: Display>(
        &mut self,
        key: K,
        min_score: Min,
        max_score: Max,
    ) -> &mut Self {
        self.push_simple(
            "ZREMRANGEBYSCORE",
            vec![key.into_key(), min_score.to_string(), max_score.to_string()],
        )
    }

    pub(crate) fn zcard<K: IntoKey>(&mut self, key: K) -> &mut Self {
        self.push_simple("ZCARD", vec![key.into_key()])
    }

    pub(crate) fn ttl<K: IntoKey>(&mut self, key: K) -> &mut Self {
        self.push_simple("TTL", vec![key.into_key()])
    }

    pub(crate) fn expire<K: IntoKey, T: Display>(&mut self, key: K, ttl_seconds: T) -> &mut Self {
        self.push_simple("EXPIRE", vec![key.into_key(), ttl_seconds.to_string()])
    }

    pub(crate) async fn query_async<T: FromPipeOutput>(
        mut self,
        conn: &mut ConnectionManager,
    ) -> RedisResult<T> {
        self.flush_current();
        T::from_pipe_outputs(conn.execute_pipeline(self.commands).await?)
    }

    pub(crate) async fn query_async_replacing_prefix<T: FromPipeOutput>(
        mut self,
        conn: &mut ConnectionManager,
        prefix: &str,
    ) -> RedisResult<(usize, T)> {
        self.flush_current();
        let (deleted, outputs) = conn
            .execute_pipeline_replacing_prefix(prefix, self.commands)
            .await?;
        Ok((deleted, T::from_pipe_outputs(outputs)?))
    }

    pub(crate) async fn query_async_if_hash_field_matches<T, F>(
        mut self,
        conn: &mut ConnectionManager,
        key: &str,
        field: &str,
        matches: F,
    ) -> RedisResult<(bool, T)>
    where
        T: FromPipeOutput,
        F: FnOnce(Option<&str>) -> bool + Send + 'static,
    {
        self.flush_current();
        let (matched, outputs) = conn
            .execute_pipeline_if_hash_field_matches(key, field, self.commands, matches)
            .await?;
        Ok((matched, T::from_pipe_outputs(outputs)?))
    }

    fn push_simple(&mut self, name: &str, args: Vec<String>) -> &mut Self {
        self.flush_current();
        self.commands.push(CommandSpec {
            name: name.to_ascii_uppercase(),
            args,
            ignore: false,
        });
        self
    }

    fn flush_current(&mut self) {
        if let Some(current) = self.current.take() {
            self.commands.push(current);
        }
    }
}

impl ConnectionManager {
    pub(crate) async fn open(path: &Path) -> RedisResult<Self> {
        if let Some(parent) = path.parent() {
            let should_secure_parent = !tokio::fs::try_exists(parent).await?
                || parent.file_name() == Some(std::ffi::OsStr::new("storage"));
            tokio::fs::create_dir_all(parent).await?;
            if should_secure_parent {
                secure_directory_permissions(parent).await?;
            }
        }
        let db = Connection::open(path).await?;
        let manager = Self {
            db,
            path: path.to_path_buf(),
        };
        manager.initialize().await?;
        secure_sqlite_file_permissions(path).await?;
        Ok(manager)
    }

    async fn initialize(&self) -> RedisResult<()> {
        let path = self.path.clone();
        self.call(move |conn| {
            conn.pragma_update(None, "journal_mode", "WAL")?;
            conn.pragma_update(None, "synchronous", "NORMAL")?;
            conn.pragma_update(None, "foreign_keys", "ON")?;
            conn.busy_timeout(std::time::Duration::from_secs(5))?;
            run_schema_migrations(conn, &path)?;
            Ok(())
        })
        .await
    }

    pub(crate) async fn meta_value(&self, key: &str) -> RedisResult<Option<String>> {
        let key = key.to_string();
        self.call(move |conn| {
            conn.query_row(
                "SELECT value FROM storage_meta WHERE key = ?1",
                params![key],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(Into::into)
        })
        .await
    }

    pub(crate) async fn set_meta_value(&self, key: &str, value: &str) -> RedisResult<()> {
        let key = key.to_string();
        let value = value.to_string();
        self.call(move |conn| {
            conn.execute(
                "INSERT INTO storage_meta(key, value, updated_at_ms) VALUES (?1, ?2, ?3)
                 ON CONFLICT(key) DO UPDATE SET
                   value = excluded.value,
                   updated_at_ms = excluded.updated_at_ms",
                params![key, value, now_ms()],
            )?;
            Ok(())
        })
        .await
    }

    pub(crate) async fn key_count_by_prefix(&self, prefix: &str) -> RedisResult<i64> {
        let prefix = prefix.to_string();
        self.call(move |conn| {
            let tx = immediate_transaction(conn)?;
            purge_expired_all_tx(&tx)?;
            let pattern = format!("{}%", escape_like_pattern(&prefix));
            let count = tx.query_row(
                "SELECT COUNT(*) FROM kv_keys WHERE key LIKE ?1 ESCAPE '\\'",
                params![pattern],
                |row| row.get::<_, i64>(0),
            )?;
            tx.commit()?;
            Ok(count)
        })
        .await
    }

    pub(crate) async fn purge_expired_keys(&self) -> RedisResult<usize> {
        self.call(move |conn| {
            let tx = immediate_transaction(conn)?;
            let deleted = tx.execute(
                "DELETE FROM kv_keys WHERE expires_at_ms IS NOT NULL AND expires_at_ms <= ?1",
                params![now_ms()],
            )?;
            tx.commit()?;
            Ok(deleted)
        })
        .await
    }

    async fn call<T, F>(&self, f: F) -> RedisResult<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut rusqlite::Connection) -> RedisResult<T> + Send + 'static,
    {
        self.db.call(f).await.map_err(StorageError::from)
    }

    pub(crate) async fn get<K: IntoKey, T: FromOptionalString>(
        &mut self,
        key: K,
    ) -> RedisResult<T> {
        let key = key.into_key();
        let value = self
            .call(move |conn| {
                purge_expired(conn, &key)?;
                if key_kind(conn, &key)? != Some("string".to_string()) {
                    return Ok(None);
                }
                conn.query_row(
                    "SELECT value FROM kv_strings WHERE key = ?1",
                    params![key],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(Into::into)
            })
            .await?;
        T::from_optional_string(value)
    }

    pub(crate) async fn set<K: IntoKey, V: Display>(
        &mut self,
        key: K,
        value: V,
    ) -> RedisResult<()> {
        self.set_internal(key.into_key(), value.to_string(), None)
            .await
    }

    pub(crate) async fn set_ex<K: IntoKey, V: Display>(
        &mut self,
        key: K,
        value: V,
        ttl_seconds: u64,
    ) -> RedisResult<()> {
        self.set_internal(
            key.into_key(),
            value.to_string(),
            Some(ttl_seconds.max(1) as i64 * 1000),
        )
        .await
    }

    pub(crate) async fn del<K: IntoKeys, T: FromDeleteCount>(&mut self, keys: K) -> RedisResult<T> {
        let keys = keys.into_keys();
        let deleted = self
            .call(move |conn| {
                let tx = immediate_transaction(conn)?;
                let mut deleted = 0usize;
                for key in keys {
                    purge_expired_tx(&tx, &key)?;
                    deleted += delete_key_tx(&tx, &key)?;
                }
                tx.commit()?;
                Ok(deleted)
            })
            .await?;
        Ok(T::from_delete_count(deleted))
    }

    pub(crate) async fn exists<K: IntoKey>(&mut self, key: K) -> RedisResult<i64> {
        let key = key.into_key();
        self.call(move |conn| {
            purge_expired(conn, &key)?;
            Ok((key_kind(conn, &key)?.is_some()) as i64)
        })
        .await
    }

    pub(crate) async fn ttl<K: IntoKey>(&mut self, key: K) -> RedisResult<i64> {
        let key = key.into_key();
        self.call(move |conn| {
            purge_expired(conn, &key)?;
            let expires_at: Option<Option<i64>> = conn
                .query_row(
                    "SELECT expires_at_ms FROM kv_keys WHERE key = ?1",
                    params![key],
                    |row| row.get(0),
                )
                .optional()?;
            Ok(match expires_at {
                None => -2,
                Some(None) => -1,
                Some(Some(value)) => ((value - now_ms()).max(0) + 999) / 1000,
            })
        })
        .await
    }

    pub(crate) async fn expire<K: IntoKey>(&mut self, key: K, ttl_seconds: i64) -> RedisResult<()> {
        let key = key.into_key();
        let expires_at = now_ms() + ttl_seconds.max(1) * 1000;
        self.call(move |conn| {
            purge_expired(conn, &key)?;
            conn.execute(
                "UPDATE kv_keys SET expires_at_ms = ?2 WHERE key = ?1",
                params![key, expires_at],
            )?;
            Ok(())
        })
        .await
    }

    pub(crate) async fn hget<K: IntoKey, F: Display, T: FromOptionalString>(
        &mut self,
        key: K,
        field: F,
    ) -> RedisResult<T> {
        let key = key.into_key();
        let field = field.to_string();
        let value = self
            .call(move |conn| {
                purge_expired(conn, &key)?;
                if key_kind(conn, &key)? != Some("hash".to_string()) {
                    return Ok(None);
                }
                conn.query_row(
                    "SELECT value FROM kv_hash WHERE key = ?1 AND field = ?2",
                    params![key, field],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(Into::into)
            })
            .await?;
        T::from_optional_string(value)
    }

    pub(crate) async fn hgetall<K: IntoKey>(
        &mut self,
        key: K,
    ) -> RedisResult<HashMap<String, String>> {
        let key = key.into_key();
        self.call(move |conn| {
            purge_expired(conn, &key)?;
            if key_kind(conn, &key)? != Some("hash".to_string()) {
                return Ok(HashMap::new());
            }
            let mut stmt =
                conn.prepare("SELECT field, value FROM kv_hash WHERE key = ?1 ORDER BY field")?;
            let rows = stmt.query_map(params![key], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            rows.collect::<Result<HashMap<_, _>, _>>()
                .map_err(Into::into)
        })
        .await
    }

    pub(crate) async fn hvals<K: IntoKey>(&mut self, key: K) -> RedisResult<Vec<String>> {
        let key = key.into_key();
        self.call(move |conn| {
            purge_expired(conn, &key)?;
            if key_kind(conn, &key)? != Some("hash".to_string()) {
                return Ok(Vec::new());
            }
            query_strings(
                conn,
                "SELECT value FROM kv_hash WHERE key = ?1 ORDER BY field",
                &[&key],
            )
        })
        .await
    }

    pub(crate) async fn hset<K: IntoKey, F: Display, V: Display>(
        &mut self,
        key: K,
        field: F,
        value: V,
    ) -> RedisResult<()> {
        let key = key.into_key();
        let field = field.to_string();
        let value = value.to_string();
        self.call(move |conn| {
            let tx = immediate_transaction(conn)?;
            ensure_key_tx(&tx, &key, "hash", None)?;
            tx.execute(
                "INSERT OR REPLACE INTO kv_hash(key, field, value) VALUES (?1, ?2, ?3)",
                params![key, field, value],
            )?;
            tx.commit()?;
            Ok(())
        })
        .await
    }

    #[allow(dead_code)]
    pub(crate) async fn hdel<K: IntoKey, F: IntoMembers>(
        &mut self,
        key: K,
        fields: F,
    ) -> RedisResult<()> {
        let key = key.into_key();
        let fields = fields.into_members();
        self.call(move |conn| {
            let tx = immediate_transaction(conn)?;
            purge_expired_tx(&tx, &key)?;
            for field in fields {
                tx.execute(
                    "DELETE FROM kv_hash WHERE key = ?1 AND field = ?2",
                    params![key, field],
                )?;
            }
            delete_collection_key_if_empty_tx(&tx, &key, "hash")?;
            tx.commit()?;
            Ok(())
        })
        .await
    }

    pub(crate) async fn smembers<K: IntoKey>(&mut self, key: K) -> RedisResult<Vec<String>> {
        let key = key.into_key();
        self.call(move |conn| {
            purge_expired(conn, &key)?;
            if key_kind(conn, &key)? != Some("set".to_string()) {
                return Ok(Vec::new());
            }
            query_strings(
                conn,
                "SELECT member FROM kv_set WHERE key = ?1 ORDER BY member",
                &[&key],
            )
        })
        .await
    }

    pub(crate) async fn sadd<K: IntoKey, M: IntoMembers>(
        &mut self,
        key: K,
        members: M,
    ) -> RedisResult<()> {
        let key = key.into_key();
        let members = members.into_members();
        self.call(move |conn| {
            let tx = immediate_transaction(conn)?;
            ensure_key_tx(&tx, &key, "set", None)?;
            for member in members {
                tx.execute(
                    "INSERT OR IGNORE INTO kv_set(key, member) VALUES (?1, ?2)",
                    params![key, member],
                )?;
            }
            tx.commit()?;
            Ok(())
        })
        .await
    }

    pub(crate) async fn srem<K: IntoKey, M: IntoMembers>(
        &mut self,
        key: K,
        members: M,
    ) -> RedisResult<()> {
        let key = key.into_key();
        let members = members.into_members();
        self.call(move |conn| {
            let tx = immediate_transaction(conn)?;
            purge_expired_tx(&tx, &key)?;
            for member in members {
                tx.execute(
                    "DELETE FROM kv_set WHERE key = ?1 AND member = ?2",
                    params![key, member],
                )?;
            }
            delete_collection_key_if_empty_tx(&tx, &key, "set")?;
            tx.commit()?;
            Ok(())
        })
        .await
    }

    pub(crate) async fn zadd<K: IntoKey, M: Display, S: Display + Send>(
        &mut self,
        key: K,
        member: M,
        score: S,
    ) -> RedisResult<()> {
        let key = key.into_key();
        let member = member.to_string();
        let score = parse_f64(&score.to_string())?;
        self.call(move |conn| {
            let tx = immediate_transaction(conn)?;
            ensure_key_tx(&tx, &key, "zset", None)?;
            tx.execute(
                "INSERT OR REPLACE INTO kv_zset(key, member, score) VALUES (?1, ?2, ?3)",
                params![key, member, score],
            )?;
            tx.commit()?;
            Ok(())
        })
        .await
    }

    pub(crate) async fn zrem<K: IntoKey, M: IntoMembers>(
        &mut self,
        key: K,
        members: M,
    ) -> RedisResult<()> {
        let key = key.into_key();
        let members = members.into_members();
        self.call(move |conn| {
            let tx = immediate_transaction(conn)?;
            purge_expired_tx(&tx, &key)?;
            for member in members {
                tx.execute(
                    "DELETE FROM kv_zset WHERE key = ?1 AND member = ?2",
                    params![key, member],
                )?;
            }
            delete_collection_key_if_empty_tx(&tx, &key, "zset")?;
            tx.commit()?;
            Ok(())
        })
        .await
    }

    pub(crate) async fn zrembyscore<K: IntoKey>(
        &mut self,
        key: K,
        min_score: i64,
        max_score: i64,
    ) -> RedisResult<()> {
        let key = key.into_key();
        self.call(move |conn| {
            let tx = immediate_transaction(conn)?;
            purge_expired_tx(&tx, &key)?;
            delete_zset_score_range_tx(
                &tx,
                &key,
                ScoreBound::inclusive(min_score as f64),
                ScoreBound::inclusive(max_score as f64),
            )?;
            tx.commit()?;
            Ok(())
        })
        .await
    }

    pub(crate) async fn zcard<K: IntoKey>(&mut self, key: K) -> RedisResult<i64> {
        let key = key.into_key();
        self.call(move |conn| {
            purge_expired(conn, &key)?;
            count_rows(conn, "SELECT COUNT(*) FROM kv_zset WHERE key = ?1", &[&key])
        })
        .await
    }

    pub(crate) async fn zcount<K: IntoKey>(
        &mut self,
        key: K,
        min_score: i64,
        max_score: i64,
    ) -> RedisResult<i64> {
        let key = key.into_key();
        self.call(move |conn| {
            purge_expired(conn, &key)?;
            count_rows(
                conn,
                "SELECT COUNT(*) FROM kv_zset WHERE key = ?1 AND score >= ?2 AND score <= ?3",
                &[&key, &(min_score as f64), &(max_score as f64)],
            )
        })
        .await
    }

    pub(crate) async fn zscore<K: IntoKey, M: Display>(
        &mut self,
        key: K,
        member: M,
    ) -> RedisResult<i64> {
        let key = key.into_key();
        let member = member.to_string();
        self.call(move |conn| {
            purge_expired(conn, &key)?;
            let score = conn
                .query_row(
                    "SELECT score FROM kv_zset WHERE key = ?1 AND member = ?2",
                    params![key, member],
                    |row| row.get::<_, f64>(0),
                )
                .optional()?
                .ok_or_else(|| storage_error("zscore member not found"))?;
            Ok(score.trunc() as i64)
        })
        .await
    }

    pub(crate) async fn zrange<K: IntoKey>(
        &mut self,
        key: K,
        start: isize,
        end: isize,
    ) -> RedisResult<Vec<String>> {
        self.zrange_ordered(key.into_key(), start, end, false).await
    }

    pub(crate) async fn zrevrange<K: IntoKey>(
        &mut self,
        key: K,
        start: isize,
        end: isize,
    ) -> RedisResult<Vec<String>> {
        self.zrange_ordered(key.into_key(), start, end, true).await
    }

    pub(crate) async fn zrangebyscore<K: IntoKey>(
        &mut self,
        key: K,
        min_score: i64,
        max_score: i64,
    ) -> RedisResult<Vec<String>> {
        let key = key.into_key();
        self.call(move |conn| {
            purge_expired(conn, &key)?;
            query_strings(
                conn,
                "SELECT member FROM kv_zset
                 WHERE key = ?1 AND score >= ?2 AND score <= ?3
                 ORDER BY score ASC, member ASC",
                &[&key, &(min_score as f64), &(max_score as f64)],
            )
        })
        .await
    }

    pub(crate) async fn lrange<K: IntoKey>(
        &mut self,
        key: K,
        start: isize,
        end: isize,
    ) -> RedisResult<Vec<String>> {
        let key = key.into_key();
        self.call(move |conn| {
            purge_expired(conn, &key)?;
            let len = count_rows(conn, "SELECT COUNT(*) FROM kv_list WHERE key = ?1", &[&key])?;
            let Some((offset, limit)) = normalize_range(len, start, end) else {
                return Ok(Vec::new());
            };
            let mut stmt = conn.prepare(
                "SELECT value FROM kv_list WHERE key = ?1 ORDER BY idx ASC LIMIT ?2 OFFSET ?3",
            )?;
            let rows = stmt.query_map(params![key, limit, offset], |row| row.get(0))?;
            rows.collect::<Result<Vec<String>, _>>().map_err(Into::into)
        })
        .await
    }

    pub(crate) async fn llen<K: IntoKey>(&mut self, key: K) -> RedisResult<i64> {
        let key = key.into_key();
        self.call(move |conn| {
            purge_expired(conn, &key)?;
            count_rows(conn, "SELECT COUNT(*) FROM kv_list WHERE key = ?1", &[&key])
        })
        .await
    }

    pub(crate) async fn xrevrange_count<K: IntoKey>(
        &mut self,
        key: K,
        max: &str,
        min: &str,
        count: usize,
    ) -> RedisResult<streams::StreamRangeReply> {
        let key = key.into_key();
        let max = max.to_string();
        let min = min.to_string();
        self.call(move |conn| {
            purge_expired(conn, &key)?;
            let min = parse_stream_bound(&min, true)?;
            let max = parse_stream_bound(&max, false)?;
            let ids = query_stream_rows(conn, &key, min, false, max, true, count)?
                .into_iter()
                .map(|(id, fields_json)| stream_id_from_row(id, fields_json))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(streams::StreamRangeReply { ids })
        })
        .await
    }

    pub(crate) async fn xread_options(
        &mut self,
        keys: &[&str],
        last_ids: &[&str],
        options: &streams::StreamReadOptions,
    ) -> RedisResult<Option<streams::StreamReadReply>> {
        let key = keys.first().copied().unwrap_or("").to_string();
        let last_id = last_ids.first().copied().unwrap_or("0-0").to_string();
        let count = options.count.unwrap_or(100).max(1);
        self.call(move |conn| {
            purge_expired(conn, &key)?;
            let last_stream_id = parse_stream_id(&last_id).unwrap_or_default();
            let ids =
                query_stream_rows(conn, &key, Some(last_stream_id), true, None, false, count)?
                    .into_iter()
                    .map(|(id, fields_json)| stream_id_from_row(id, fields_json))
                    .collect::<Result<Vec<_>, _>>()?;
            if ids.is_empty() {
                Ok(None)
            } else {
                Ok(Some(streams::StreamReadReply {
                    keys: vec![streams::StreamKey { key, ids }],
                }))
            }
        })
        .await
    }

    async fn set_internal(
        &mut self,
        key: String,
        value: String,
        ttl_ms_from_now: Option<i64>,
    ) -> RedisResult<()> {
        self.call(move |conn| {
            let tx = immediate_transaction(conn)?;
            set_string_tx(&tx, &key, &value, ttl_ms_from_now.map(|ttl| now_ms() + ttl))?;
            tx.commit()?;
            Ok(())
        })
        .await
    }

    async fn zrange_ordered(
        &mut self,
        key: String,
        start: isize,
        end: isize,
        reverse: bool,
    ) -> RedisResult<Vec<String>> {
        self.call(move |conn| {
            purge_expired(conn, &key)?;
            let len = count_rows(conn, "SELECT COUNT(*) FROM kv_zset WHERE key = ?1", &[&key])?;
            let Some((offset, limit)) = normalize_range(len, start, end) else {
                return Ok(Vec::new());
            };
            let order = if reverse {
                "ORDER BY score DESC, member DESC"
            } else {
                "ORDER BY score ASC, member ASC"
            };
            let sql =
                format!("SELECT member FROM kv_zset WHERE key = ?1 {order} LIMIT ?2 OFFSET ?3");
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(params![key, limit, offset], |row| row.get(0))?;
            rows.collect::<Result<Vec<String>, _>>().map_err(Into::into)
        })
        .await
    }

    async fn execute_pipeline(
        &mut self,
        commands: Vec<CommandSpec>,
    ) -> RedisResult<Vec<CmdOutput>> {
        self.call(move |conn| {
            let tx = immediate_transaction(conn)?;
            let mut outputs = Vec::new();
            for command in commands {
                let ignore = command.ignore;
                let output = execute_command_tx(&tx, command)?;
                if !ignore {
                    outputs.push(output);
                }
            }
            tx.commit()?;
            Ok(outputs)
        })
        .await
    }

    async fn execute_pipeline_replacing_prefix(
        &mut self,
        prefix: &str,
        commands: Vec<CommandSpec>,
    ) -> RedisResult<(usize, Vec<CmdOutput>)> {
        let prefix = prefix.to_string();
        self.call(move |conn| {
            let tx = immediate_transaction(conn)?;
            purge_expired_all_tx(&tx)?;
            let pattern = format!("{}%", escape_like_pattern(&prefix));
            let deleted = tx.execute(
                "DELETE FROM kv_keys WHERE key LIKE ?1 ESCAPE '\\'",
                params![pattern],
            )?;
            let mut outputs = Vec::new();
            for command in commands {
                let ignore = command.ignore;
                let output = execute_command_tx(&tx, command)?;
                if !ignore {
                    outputs.push(output);
                }
            }
            tx.commit()?;
            Ok((deleted, outputs))
        })
        .await
    }

    async fn execute_pipeline_if_hash_field_matches<F>(
        &mut self,
        key: &str,
        field: &str,
        commands: Vec<CommandSpec>,
        matches: F,
    ) -> RedisResult<(bool, Vec<CmdOutput>)>
    where
        F: FnOnce(Option<&str>) -> bool + Send + 'static,
    {
        let key = key.to_string();
        let field = field.to_string();
        self.call(move |conn| {
            let tx = immediate_transaction(conn)?;
            purge_expired_tx(&tx, &key)?;
            let current = tx
                .query_row(
                    "SELECT value FROM kv_hash WHERE key = ?1 AND field = ?2",
                    params![key, field],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            if !matches(current.as_deref()) {
                tx.commit()?;
                return Ok((false, Vec::new()));
            }

            let mut outputs = Vec::new();
            for command in commands {
                let ignore = command.ignore;
                let output = execute_command_tx(&tx, command)?;
                if !ignore {
                    outputs.push(output);
                }
            }
            tx.commit()?;
            Ok((true, outputs))
        })
        .await
    }

    async fn execute_command(&mut self, command: CommandSpec) -> RedisResult<CmdOutput> {
        self.call(move |conn| {
            let tx = immediate_transaction(conn)?;
            let output = execute_command_tx(&tx, command)?;
            tx.commit()?;
            Ok(output)
        })
        .await
    }
}

fn execute_command_tx(
    tx: &rusqlite::Transaction<'_>,
    command: CommandSpec,
) -> RedisResult<CmdOutput> {
    let args = command.args;
    match command.name.as_str() {
        "PING" => Ok(CmdOutput::Nil),
        "TYPE" => {
            let key = arg(&args, 0)?;
            purge_expired_tx(tx, key)?;
            Ok(CmdOutput::String(
                key_kind_tx(tx, key)?.unwrap_or_else(|| "none".to_string()),
            ))
        }
        "PTTL" => {
            let key = arg(&args, 0)?;
            purge_expired_tx(tx, key)?;
            let expires_at: Option<Option<i64>> = tx
                .query_row(
                    "SELECT expires_at_ms FROM kv_keys WHERE key = ?1",
                    params![key],
                    |row| row.get(0),
                )
                .optional()?;
            Ok(CmdOutput::Int(match expires_at {
                None => -2,
                Some(None) => -1,
                Some(Some(value)) => (value - now_ms()).max(0),
            }))
        }
        "TTL" => {
            let key = arg(&args, 0)?;
            purge_expired_tx(tx, key)?;
            let expires_at: Option<Option<i64>> = tx
                .query_row(
                    "SELECT expires_at_ms FROM kv_keys WHERE key = ?1",
                    params![key],
                    |row| row.get(0),
                )
                .optional()?;
            Ok(CmdOutput::Int(match expires_at {
                None => -2,
                Some(None) => -1,
                Some(Some(value)) => ((value - now_ms()).max(0) + 999) / 1000,
            }))
        }
        "GET" => {
            let key = arg(&args, 0)?;
            Ok(CmdOutput::OptionalString(string_get_tx(tx, key)?))
        }
        "MGET" => {
            let mut values = Vec::with_capacity(args.len());
            for key in args {
                values.push(string_get_tx(tx, &key)?);
            }
            Ok(CmdOutput::OptionalStrings(values))
        }
        "SET" => set_command_tx(tx, &args),
        "SETEX" => {
            let key = arg(&args, 0)?.to_string();
            let ttl = parse_i64(arg(&args, 1)?)?.max(1);
            let value = arg(&args, 2)?.to_string();
            set_string_tx(tx, &key, &value, Some(now_ms() + ttl * 1000))?;
            Ok(CmdOutput::Nil)
        }
        "DEL" => {
            let mut deleted = 0usize;
            for key in args {
                purge_expired_tx(tx, &key)?;
                deleted += delete_key_tx(tx, &key)?;
            }
            Ok(CmdOutput::Int(deleted as i64))
        }
        "EXPIRE" => {
            let key = arg(&args, 0)?;
            let ttl = parse_i64(arg(&args, 1)?)?.max(1);
            purge_expired_tx(tx, key)?;
            tx.execute(
                "UPDATE kv_keys SET expires_at_ms = ?2 WHERE key = ?1",
                params![key, now_ms() + ttl * 1000],
            )?;
            Ok(CmdOutput::Nil)
        }
        "PEXPIRE" => {
            let key = arg(&args, 0)?;
            let ttl = parse_i64(arg(&args, 1)?)?.max(1);
            purge_expired_tx(tx, key)?;
            tx.execute(
                "UPDATE kv_keys SET expires_at_ms = ?2 WHERE key = ?1",
                params![key, now_ms() + ttl],
            )?;
            Ok(CmdOutput::Nil)
        }
        "HSET" => {
            let key = arg(&args, 0)?.to_string();
            ensure_key_tx(tx, &key, "hash", None)?;
            for pair in args[1..].chunks(2) {
                if pair.len() == 2 {
                    tx.execute(
                        "INSERT OR REPLACE INTO kv_hash(key, field, value) VALUES (?1, ?2, ?3)",
                        params![key, pair[0], pair[1]],
                    )?;
                }
            }
            Ok(CmdOutput::Nil)
        }
        "HDEL" => {
            let key = arg(&args, 0)?;
            purge_expired_tx(tx, key)?;
            for field in &args[1..] {
                tx.execute(
                    "DELETE FROM kv_hash WHERE key = ?1 AND field = ?2",
                    params![key, field],
                )?;
            }
            delete_collection_key_if_empty_tx(tx, key, "hash")?;
            Ok(CmdOutput::Nil)
        }
        "HMGET" => {
            let key = arg(&args, 0)?;
            purge_expired_tx(tx, key)?;
            let mut values = Vec::new();
            for field in &args[1..] {
                values.push(
                    tx.query_row(
                        "SELECT value FROM kv_hash WHERE key = ?1 AND field = ?2",
                        params![key, field],
                        |row| row.get::<_, String>(0),
                    )
                    .optional()?,
                );
            }
            Ok(CmdOutput::OptionalStrings(values))
        }
        "HINCRBY" => {
            let key = arg(&args, 0)?.to_string();
            let field = arg(&args, 1)?.to_string();
            let delta = parse_i64(arg(&args, 2)?)?;
            ensure_key_tx(tx, &key, "hash", None)?;
            let current = tx
                .query_row(
                    "SELECT value FROM kv_hash WHERE key = ?1 AND field = ?2",
                    params![key, field],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
                .and_then(|value| value.parse::<i64>().ok())
                .unwrap_or(0);
            let next = current + delta;
            tx.execute(
                "INSERT OR REPLACE INTO kv_hash(key, field, value) VALUES (?1, ?2, ?3)",
                params![key, field, next.to_string()],
            )?;
            Ok(CmdOutput::Int(next))
        }
        "SADD" => {
            let key = arg(&args, 0)?.to_string();
            ensure_key_tx(tx, &key, "set", None)?;
            for member in &args[1..] {
                tx.execute(
                    "INSERT OR IGNORE INTO kv_set(key, member) VALUES (?1, ?2)",
                    params![key, member],
                )?;
            }
            Ok(CmdOutput::Nil)
        }
        "SREM" => {
            let key = arg(&args, 0)?;
            purge_expired_tx(tx, key)?;
            for member in &args[1..] {
                tx.execute(
                    "DELETE FROM kv_set WHERE key = ?1 AND member = ?2",
                    params![key, member],
                )?;
            }
            delete_collection_key_if_empty_tx(tx, key, "set")?;
            Ok(CmdOutput::Nil)
        }
        "ZADD" => {
            let key = arg(&args, 0)?.to_string();
            ensure_key_tx(tx, &key, "zset", None)?;
            for pair in args[1..].chunks(2) {
                if pair.len() == 2 {
                    tx.execute(
                        "INSERT OR REPLACE INTO kv_zset(key, member, score) VALUES (?1, ?2, ?3)",
                        params![key, pair[1], parse_f64(&pair[0])?],
                    )?;
                }
            }
            Ok(CmdOutput::Nil)
        }
        "ZREM" => {
            let key = arg(&args, 0)?;
            purge_expired_tx(tx, key)?;
            for member in &args[1..] {
                tx.execute(
                    "DELETE FROM kv_zset WHERE key = ?1 AND member = ?2",
                    params![key, member],
                )?;
            }
            delete_collection_key_if_empty_tx(tx, key, "zset")?;
            Ok(CmdOutput::Nil)
        }
        "ZREMRANGEBYSCORE" => {
            let key = arg(&args, 0)?;
            let min = parse_score_bound(arg(&args, 1)?)?;
            let max = parse_score_bound(arg(&args, 2)?)?;
            purge_expired_tx(tx, key)?;
            delete_zset_score_range_tx(tx, key, min, max)?;
            Ok(CmdOutput::Nil)
        }
        "ZCARD" => {
            let key = arg(&args, 0)?;
            purge_expired_tx(tx, key)?;
            Ok(CmdOutput::Int(count_rows_tx(
                tx,
                "SELECT COUNT(*) FROM kv_zset WHERE key = ?1",
                &[&key],
            )?))
        }
        "ZCOUNT" => {
            let key = arg(&args, 0)?;
            let min = parse_score_bound(arg(&args, 1)?)?;
            let max = parse_score_bound(arg(&args, 2)?)?;
            purge_expired_tx(tx, key)?;
            Ok(CmdOutput::Int(count_zset_score_range_tx(
                tx, key, min, max,
            )?))
        }
        "ZRANGEBYSCORE" => zrangebyscore_command_tx(tx, &args, false),
        "ZREVRANGEBYSCORE" => zrangebyscore_command_tx(tx, &args, true),
        "ZRANGE" => zrange_command_tx(tx, &args, false),
        "RPUSH" => {
            let key = arg(&args, 0)?.to_string();
            ensure_key_tx(tx, &key, "list", None)?;
            let idx = count_rows_tx(tx, "SELECT COUNT(*) FROM kv_list WHERE key = ?1", &[&key])?;
            for (offset, value) in args[1..].iter().enumerate() {
                tx.execute(
                    "INSERT INTO kv_list(key, idx, value) VALUES (?1, ?2, ?3)",
                    params![key, idx + offset as i64, value],
                )?;
            }
            Ok(CmdOutput::Nil)
        }
        "LTRIM" => {
            let key = arg(&args, 0)?.to_string();
            let start = parse_i64(arg(&args, 1)?)? as isize;
            let end = parse_i64(arg(&args, 2)?)? as isize;
            purge_expired_tx(tx, &key)?;
            let values = list_range_tx(tx, &key, start, end)?;
            tx.execute("DELETE FROM kv_list WHERE key = ?1", params![key])?;
            for (idx, value) in values.iter().enumerate() {
                tx.execute(
                    "INSERT INTO kv_list(key, idx, value) VALUES (?1, ?2, ?3)",
                    params![key, idx as i64, value],
                )?;
            }
            delete_collection_key_if_empty_tx(tx, &key, "list")?;
            Ok(CmdOutput::Nil)
        }
        "INCRBY" => {
            let key = arg(&args, 0)?.to_string();
            let delta = parse_i64(arg(&args, 1)?)?;
            let current = string_get_tx(tx, &key)?
                .and_then(|value| value.parse::<i64>().ok())
                .unwrap_or(0);
            let next = current + delta;
            set_string_preserve_ttl_tx(tx, &key, &next.to_string())?;
            Ok(CmdOutput::Int(next))
        }
        "SCAN" => {
            let mut prefix = "";
            for pair in args.windows(2) {
                if pair[0].eq_ignore_ascii_case("MATCH") {
                    prefix = pair[1].trim_end_matches('*');
                }
            }
            let keys = scan_keys_tx(tx, prefix)?;
            Ok(CmdOutput::Scan("0".to_string(), keys))
        }
        "XADD" => xadd_command_tx(tx, &args),
        "XRANGE" => {
            let key = arg(&args, 0)?;
            let min = parse_stream_bound(arg(&args, 1)?, true)?;
            let max = parse_stream_bound(arg(&args, 2)?, false)?;
            purge_expired_tx(tx, key)?;
            let entries = query_stream_rows(tx, key, min, false, max, false, usize::MAX)?;
            Ok(CmdOutput::StreamEntries(
                entries
                    .into_iter()
                    .map(|(id, fields_json)| Ok((id, stream_fields_vec(&fields_json)?)))
                    .collect::<Result<Vec<_>, rusqlite::Error>>()?,
            ))
        }
        "XTRIM" => {
            let key = arg(&args, 0)?;
            purge_expired_tx(tx, key)?;
            let strategy = arg(&args, 1)?.to_ascii_uppercase();
            match strategy.as_str() {
                "MINID" => {
                    let min_id = parse_stream_id(arg(&args, args.len().saturating_sub(1))?)
                        .ok_or_else(|| storage_error("XTRIM MINID requires a valid stream ID"))?;
                    let (min_ms, min_sequence) = stream_id_sql_tuple(min_id)?;
                    tx.execute(
                        "DELETE FROM kv_stream
                         WHERE key = ?1 AND (id_ms, id_sequence) < (?2, ?3)",
                        params![key, min_ms, min_sequence],
                    )?;
                }
                "MAXLEN" => {
                    let max_len = parse_i64(arg(&args, args.len().saturating_sub(1))?)?.max(0);
                    tx.execute(
                        "DELETE FROM kv_stream
                         WHERE key = ?1 AND rowid IN (
                           SELECT rowid FROM kv_stream
                           WHERE key = ?1
                           ORDER BY id_ms DESC, id_sequence DESC
                           LIMIT -1 OFFSET ?2
                         )",
                        params![key, max_len],
                    )?;
                }
                _ => return Err(storage_error("XTRIM requires MINID or MAXLEN")),
            }
            Ok(CmdOutput::Nil)
        }
        "XDEL" => {
            let key = arg(&args, 0)?;
            for id in &args[1..] {
                tx.execute(
                    "DELETE FROM kv_stream WHERE key = ?1 AND id = ?2",
                    params![key, id],
                )?;
            }
            Ok(CmdOutput::Nil)
        }
        "EVAL" => eval_command_tx(tx, &args),
        _ => Err(storage_error(format!(
            "unsupported Redis-compatible command {}",
            command.name
        ))),
    }
}

fn eval_command_tx(tx: &rusqlite::Transaction<'_>, args: &[String]) -> RedisResult<CmdOutput> {
    let script = arg(args, 0)?;
    let key_count = usize::try_from(parse_i64(arg(args, 1)?)?)
        .map_err(|_| storage_error("EVAL key count must be non-negative"))?;
    let keys_start = 2_usize;
    let argv_start = keys_start
        .checked_add(key_count)
        .filter(|index| *index <= args.len())
        .ok_or_else(|| storage_error("EVAL key count exceeds supplied arguments"))?;
    let keys = &args[keys_start..argv_start];
    let argv = &args[argv_start..];

    if script.contains("fn-knock:eval:increment-counter-with-ttl:v1") {
        let key = keys
            .first()
            .ok_or_else(|| storage_error("counter EVAL key missing"))?;
        let ttl = parse_i64(
            argv.first()
                .ok_or_else(|| storage_error("counter EVAL TTL missing"))?,
        )?
        .max(1);
        let current = string_get_tx(tx, key)?
            .and_then(|value| value.parse::<i64>().ok())
            .unwrap_or(0);
        let next = current
            .checked_add(1)
            .ok_or_else(|| storage_error("counter overflow"))?;
        if next == 1 {
            set_string_tx(
                tx,
                key,
                &next.to_string(),
                Some(now_ms().saturating_add(ttl.saturating_mul(1000))),
            )?;
        } else {
            set_string_preserve_ttl_tx(tx, key, &next.to_string())?;
        }
        return Ok(CmdOutput::Int(next));
    }

    if script.contains("fn-knock:eval:set-expiring-string-with-zset-limit:v1") {
        let data_key = keys
            .first()
            .ok_or_else(|| storage_error("limited string EVAL data key missing"))?;
        let index_key = keys
            .get(1)
            .ok_or_else(|| storage_error("limited string EVAL index key missing"))?;
        let value = argv
            .first()
            .ok_or_else(|| storage_error("limited string EVAL value missing"))?;
        let ttl = parse_i64(
            argv.get(1)
                .ok_or_else(|| storage_error("limited string EVAL TTL missing"))?,
        )?
        .max(1);
        let now_score = parse_i64(
            argv.get(2)
                .ok_or_else(|| storage_error("limited string EVAL current score missing"))?,
        )?;
        let expires_at_score = parse_i64(
            argv.get(3)
                .ok_or_else(|| storage_error("limited string EVAL expiry score missing"))?,
        )?;
        let limit = parse_i64(
            argv.get(4)
                .ok_or_else(|| storage_error("limited string EVAL limit missing"))?,
        )?
        .max(1);

        purge_expired_tx(tx, index_key)?;
        delete_zset_score_range_tx(
            tx,
            index_key,
            ScoreBound::inclusive(f64::NEG_INFINITY),
            ScoreBound::inclusive(now_score as f64),
        )?;
        let tracked = count_rows_tx(
            tx,
            "SELECT COUNT(*) FROM kv_zset WHERE key = ?1 AND member = ?2",
            &[index_key, data_key],
        )? > 0;
        let existing = string_get_tx(tx, data_key)?.is_some();
        let active = count_rows_tx(
            tx,
            "SELECT COUNT(*) FROM kv_zset WHERE key = ?1",
            &[index_key],
        )?;
        if !tracked && !existing && active >= limit {
            return Ok(CmdOutput::Int(0));
        }

        set_string_tx(
            tx,
            data_key,
            value,
            Some(now_ms().saturating_add(ttl.saturating_mul(1000))),
        )?;
        ensure_key_tx(tx, index_key, "zset", None)?;
        tx.execute(
            "INSERT OR REPLACE INTO kv_zset(key, member, score) VALUES (?1, ?2, ?3)",
            params![index_key, data_key, expires_at_score],
        )?;
        return Ok(CmdOutput::Int(1));
    }

    if script.contains("fn-knock:eval:claim-ldap-binding:v2") {
        if keys.len() < 4 || argv.len() < 5 {
            return Err(storage_error("LDAP binding EVAL arguments missing"));
        }
        let Some(invite_raw) = string_get_tx(tx, &keys[0])? else {
            return Ok(CmdOutput::Int(0));
        };
        let Ok(invite) = serde_json::from_str::<serde_json::Value>(&invite_raw) else {
            return Ok(CmdOutput::Int(0));
        };
        if invite
            .get("provider_id")
            .and_then(serde_json::Value::as_str)
            != Some(argv[3].as_str())
            || invite.get("totp_id").and_then(serde_json::Value::as_str) != Some(argv[4].as_str())
            || string_get_tx(tx, &keys[1])?.is_some()
            || string_get_tx(tx, &keys[2])?.is_some()
        {
            return Ok(CmdOutput::Int(0));
        }
        let score = argv[2]
            .parse::<f64>()
            .map_err(|_| storage_error("LDAP binding EVAL score is invalid"))?;
        set_string_tx(tx, &keys[1], &argv[0], None)?;
        set_string_tx(tx, &keys[2], &argv[1], None)?;
        ensure_key_tx(tx, &keys[3], "zset", None)?;
        tx.execute(
            "INSERT OR REPLACE INTO kv_zset(key, member, score) VALUES (?1, ?2, ?3)",
            params![&keys[3], &argv[0], score],
        )?;
        delete_key_tx(tx, &keys[0])?;
        return Ok(CmdOutput::Int(1));
    }

    if script.contains("fn-knock:eval:update-ldap-binding-if-owned:v1") {
        if keys.len() < 3 || argv.len() < 3 {
            return Err(storage_error("LDAP binding update EVAL arguments missing"));
        }
        if string_get_tx(tx, &keys[0])?.as_deref() != Some(argv[0].as_str())
            || string_get_tx(tx, &keys[1])?.is_none()
        {
            return Ok(CmdOutput::Int(0));
        }
        let score = argv[2]
            .parse::<f64>()
            .map_err(|_| storage_error("LDAP binding update EVAL score is invalid"))?;
        set_string_tx(tx, &keys[1], &argv[1], None)?;
        ensure_key_tx(tx, &keys[2], "zset", None)?;
        tx.execute(
            "INSERT OR REPLACE INTO kv_zset(key, member, score) VALUES (?1, ?2, ?3)",
            params![&keys[2], &argv[0], score],
        )?;
        return Ok(CmdOutput::Int(1));
    }

    if script.contains("fn-knock:eval:cas-config-host-generation-raw:v1") {
        let config_key = keys
            .first()
            .ok_or_else(|| storage_error("config CAS config key missing"))?;
        let generation_key = keys
            .get(1)
            .ok_or_else(|| storage_error("config CAS generation key missing"))?;
        let config_expected_exists = argv
            .first()
            .ok_or_else(|| storage_error("config CAS config exists flag missing"))?;
        let config_expected_raw = argv
            .get(1)
            .ok_or_else(|| storage_error("config CAS expected config missing"))?;
        let generation_expected_exists = argv
            .get(2)
            .ok_or_else(|| storage_error("config CAS generation exists flag missing"))?;
        let generation_expected_raw = argv
            .get(3)
            .ok_or_else(|| storage_error("config CAS expected generation missing"))?;
        let replacement_config_raw = argv
            .get(4)
            .ok_or_else(|| storage_error("config CAS replacement config missing"))?;
        let replacement_generation_raw = argv
            .get(5)
            .ok_or_else(|| storage_error("config CAS replacement generation missing"))?;

        let read_raw = |key: &str| -> RedisResult<Option<String>> {
            purge_expired_tx(tx, key)?;
            match key_kind_tx(tx, key)? {
                None => Ok(None),
                Some(kind) if kind == "string" => string_get_tx(tx, key),
                Some(_) => Err(storage_error("config CAS key must contain a string")),
            }
        };
        let raw_matches = |current: Option<&str>, exists: &str, expected: &str| match exists {
            "0" => Ok(current.is_none()),
            "1" => Ok(current == Some(expected)),
            _ => Err(storage_error("config CAS exists flag is invalid")),
        };
        let current_config_raw = read_raw(config_key)?;
        let current_generation_raw = read_raw(generation_key)?;
        if !raw_matches(
            current_config_raw.as_deref(),
            config_expected_exists,
            config_expected_raw,
        )? || !raw_matches(
            current_generation_raw.as_deref(),
            generation_expected_exists,
            generation_expected_raw,
        )? {
            return Ok(CmdOutput::Int(0));
        }
        set_string_tx(tx, config_key, replacement_config_raw, None)?;
        set_string_tx(tx, generation_key, replacement_generation_raw, None)?;
        return Ok(CmdOutput::Int(1));
    }

    if script.contains("fn-knock:eval:update-json-cas:v1")
        || script.contains("fn-knock:eval:update-session-json-cas:v2")
    {
        let key = keys
            .first()
            .ok_or_else(|| storage_error("JSON update EVAL key missing"))?;
        let expected_raw = argv
            .first()
            .ok_or_else(|| storage_error("JSON update EVAL expected value missing"))?;
        let next_raw = argv
            .get(1)
            .ok_or_else(|| storage_error("JSON update EVAL next value missing"))?;
        let Some(current_raw) = string_get_tx(tx, key)? else {
            return Ok(CmdOutput::Int(-1));
        };
        if current_raw != *expected_raw {
            return Ok(CmdOutput::Int(0));
        }
        let changed = tx.execute(
            "UPDATE kv_strings SET value = ?2 WHERE key = ?1",
            params![key, next_raw],
        )?;
        if changed == 0 {
            return Ok(CmdOutput::Int(-1));
        }
        return Ok(CmdOutput::Int(1));
    }

    if script.contains("fn-knock:eval:initialize-login-mobility-if-session-live:v1") {
        let session_key = keys
            .first()
            .ok_or_else(|| storage_error("login mobility EVAL session key missing"))?;
        if string_get_tx(tx, session_key)?.is_none() {
            return Ok(CmdOutput::Int(0));
        }
        let binding_key = keys
            .get(1)
            .ok_or_else(|| storage_error("login mobility EVAL binding key missing"))?;
        let timeline_key = keys
            .get(2)
            .ok_or_else(|| storage_error("login mobility EVAL timeline key missing"))?;
        let summary_key = keys
            .get(3)
            .ok_or_else(|| storage_error("login mobility EVAL summary key missing"))?;
        let index_key = keys
            .get(4)
            .ok_or_else(|| storage_error("login mobility EVAL index key missing"))?;
        let whitelist_owner_key = keys
            .get(5)
            .ok_or_else(|| storage_error("login mobility EVAL whitelist owner key missing"))?;
        let ttl = parse_i64(
            argv.get(3)
                .ok_or_else(|| storage_error("login mobility EVAL TTL missing"))?,
        )?
        .max(1);
        for (key, value_index) in [
            (binding_key, 0_usize),
            (timeline_key, 1_usize),
            (summary_key, 2_usize),
        ] {
            let value = argv
                .get(value_index)
                .ok_or_else(|| storage_error("login mobility EVAL value missing"))?;
            execute_command_tx(
                tx,
                CommandSpec {
                    name: "SETEX".to_string(),
                    args: vec![key.clone(), ttl.to_string(), value.clone()],
                    ignore: false,
                },
            )?;
        }
        execute_command_tx(
            tx,
            CommandSpec {
                name: "SADD".to_string(),
                args: vec![index_key.clone(), binding_key.clone()],
                ignore: false,
            },
        )?;
        execute_command_tx(
            tx,
            CommandSpec {
                name: "EXPIRE".to_string(),
                args: vec![index_key.clone(), ttl.to_string()],
                ignore: false,
            },
        )?;
        let session_id = argv
            .get(4)
            .ok_or_else(|| storage_error("login mobility EVAL session ID missing"))?;
        execute_command_tx(
            tx,
            CommandSpec {
                name: "SETEX".to_string(),
                args: vec![
                    whitelist_owner_key.clone(),
                    ttl.to_string(),
                    session_id.clone(),
                ],
                ignore: false,
            },
        )?;
        return Ok(CmdOutput::Int(1));
    }

    if script.contains("fn-knock:eval:add-pending-whitelist-if-session-live:v1") {
        let session_key = keys
            .first()
            .ok_or_else(|| storage_error("pending whitelist EVAL session key missing"))?;
        let pending_key = keys
            .get(1)
            .ok_or_else(|| storage_error("pending whitelist EVAL pending key missing"))?;
        if string_get_tx(tx, session_key)?.is_none() {
            return Ok(CmdOutput::Int(0));
        }
        let record_id = argv
            .first()
            .ok_or_else(|| storage_error("pending whitelist EVAL record ID missing"))?;
        let owner_record_key = argv
            .get(1)
            .ok_or_else(|| storage_error("pending whitelist EVAL owner key missing"))?;
        let ttl = parse_i64(
            argv.get(2)
                .ok_or_else(|| storage_error("pending whitelist EVAL TTL missing"))?,
        )?
        .max(1);
        execute_command_tx(
            tx,
            CommandSpec {
                name: "HSET".to_string(),
                args: vec![
                    pending_key.clone(),
                    record_id.clone(),
                    owner_record_key.clone(),
                ],
                ignore: false,
            },
        )?;
        execute_command_tx(
            tx,
            CommandSpec {
                name: "EXPIRE".to_string(),
                args: vec![pending_key.clone(), ttl.to_string()],
                ignore: false,
            },
        )?;
        return Ok(CmdOutput::Int(1));
    }

    if script.contains("fn-knock:eval:save-timeline-if-session-live:v1") {
        let session_key = keys
            .first()
            .ok_or_else(|| storage_error("timeline EVAL session key missing"))?;
        if string_get_tx(tx, session_key)?.is_none() {
            return Ok(CmdOutput::Int(0));
        }
        let timeline_key = keys
            .get(1)
            .ok_or_else(|| storage_error("timeline EVAL timeline key missing"))?;
        let summary_key = keys
            .get(2)
            .ok_or_else(|| storage_error("timeline EVAL summary key missing"))?;
        let events = argv
            .first()
            .ok_or_else(|| storage_error("timeline EVAL events missing"))?;
        let summary = argv
            .get(1)
            .ok_or_else(|| storage_error("timeline EVAL summary missing"))?;
        let ttl = parse_i64(
            argv.get(2)
                .ok_or_else(|| storage_error("timeline EVAL TTL missing"))?,
        )?;
        let expires_at = (ttl > 0).then(|| now_ms().saturating_add(ttl.saturating_mul(1000)));
        set_string_tx(tx, timeline_key, events, expires_at)?;
        set_string_tx(tx, summary_key, summary, expires_at)?;
        return Ok(CmdOutput::Int(1));
    }

    if script.contains("fn-knock:eval:destroy-mobility-session:v1") {
        if keys.len() < 7 {
            return Err(storage_error("destroy mobility EVAL keys missing"));
        }
        let session_id = argv
            .first()
            .ok_or_else(|| storage_error("destroy mobility EVAL session ID missing"))?;
        let owner_prefix = argv
            .get(1)
            .ok_or_else(|| storage_error("destroy mobility EVAL owner prefix missing"))?;
        for key in keys {
            purge_expired_tx(tx, key)?;
        }

        let mut binding_keys = {
            let mut statement = tx.prepare("SELECT member FROM kv_set WHERE key = ?1")?;
            let rows = statement.query_map(params![&keys[0]], |row| row.get::<_, String>(0))?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        if !binding_keys.iter().any(|key| key == &keys[5]) {
            binding_keys.push(keys[5].clone());
        }

        let mut whitelist_ids = BTreeSet::new();
        for binding_key in binding_keys {
            let Some(raw) = string_get_tx(tx, &binding_key)? else {
                continue;
            };
            let parsed = serde_json::from_str::<serde_json::Value>(&raw).ok();
            let owner_matches = binding_key == keys[5]
                || parsed
                    .as_ref()
                    .and_then(|value| value.get("ownerSessionId"))
                    .and_then(serde_json::Value::as_str)
                    == Some(session_id.as_str());
            if !owner_matches {
                continue;
            }
            if let Some(id) = parsed
                .as_ref()
                .and_then(|value| value.get("whitelistRecordId"))
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                whitelist_ids.insert(id.to_string());
            }
            delete_key_tx(tx, &binding_key)?;
        }

        let active_values = {
            let mut statement = tx.prepare("SELECT value FROM kv_hash WHERE key = ?1")?;
            let rows = statement.query_map(params![&keys[1]], |row| row.get::<_, String>(0))?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        let mut owner_record_keys = BTreeSet::new();
        for raw in active_values {
            let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
                continue;
            };
            if let Some(id) = value
                .get("whitelistRecordId")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                whitelist_ids.insert(id.to_string());
            }
            if let Some(key) = value
                .get("autoWhitelistOwnerRecordKey")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                owner_record_keys.insert(key.to_string());
            }
        }

        let pending = {
            let mut statement = tx.prepare("SELECT field, value FROM kv_hash WHERE key = ?1")?;
            let rows = statement.query_map(params![&keys[6]], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        for (record_id, owner_record_key) in pending {
            if !record_id.trim().is_empty() {
                whitelist_ids.insert(record_id);
            }
            if !owner_record_key.trim().is_empty() {
                owner_record_keys.insert(owner_record_key);
            }
        }

        for owner_record_key in owner_record_keys {
            delete_key_tx(tx, &owner_record_key)?;
        }
        for record_id in &whitelist_ids {
            let owner_key = format!("{owner_prefix}{record_id}:session");
            if string_get_tx(tx, &owner_key)?.as_deref() == Some(session_id.as_str()) {
                delete_key_tx(tx, &owner_key)?;
            }
        }
        for key in [&keys[0], &keys[1], &keys[2], &keys[3], &keys[4], &keys[6]] {
            delete_key_tx(tx, key)?;
        }
        return Ok(CmdOutput::Strings(whitelist_ids.into_iter().collect()));
    }

    if script.contains("fn-knock:eval:save-active-ip-if-session-live:v1") {
        let session_key = keys
            .first()
            .ok_or_else(|| storage_error("active IP EVAL session key missing"))?;
        let zset_key = keys
            .get(1)
            .ok_or_else(|| storage_error("active IP EVAL zset key missing"))?;
        let detail_key = keys
            .get(2)
            .ok_or_else(|| storage_error("active IP EVAL detail key missing"))?;
        if string_get_tx(tx, session_key)?.is_none() {
            return Ok(CmdOutput::Int(0));
        }
        let ip = argv
            .first()
            .ok_or_else(|| storage_error("active IP EVAL IP missing"))?;
        let score = argv
            .get(1)
            .ok_or_else(|| storage_error("active IP EVAL score missing"))?;
        let detail = argv
            .get(2)
            .ok_or_else(|| storage_error("active IP EVAL detail missing"))?;
        let ttl = parse_i64(
            argv.get(3)
                .ok_or_else(|| storage_error("active IP EVAL TTL missing"))?,
        )?
        .max(1);
        execute_command_tx(
            tx,
            CommandSpec {
                name: "ZADD".to_string(),
                args: vec![zset_key.clone(), score.clone(), ip.clone()],
                ignore: false,
            },
        )?;
        execute_command_tx(
            tx,
            CommandSpec {
                name: "HSET".to_string(),
                args: vec![detail_key.clone(), ip.clone(), detail.clone()],
                ignore: false,
            },
        )?;
        for key in [zset_key, detail_key] {
            execute_command_tx(
                tx,
                CommandSpec {
                    name: "EXPIRE".to_string(),
                    args: vec![key.clone(), ttl.to_string()],
                    ignore: false,
                },
            )?;
        }
        return Ok(CmdOutput::Int(1));
    }

    if script.contains("fn-knock:eval:save-owned-binding-if-session-live:v1") {
        let session_key = keys
            .first()
            .ok_or_else(|| storage_error("owned binding EVAL session key missing"))?;
        let binding_key = keys
            .get(1)
            .ok_or_else(|| storage_error("owned binding EVAL binding key missing"))?;
        let index_key = keys
            .get(2)
            .ok_or_else(|| storage_error("owned binding EVAL index key missing"))?;
        if string_get_tx(tx, session_key)?.is_none() {
            return Ok(CmdOutput::Int(0));
        }
        let binding = argv
            .first()
            .ok_or_else(|| storage_error("owned binding EVAL value missing"))?;
        let binding_ttl = parse_i64(
            argv.get(1)
                .ok_or_else(|| storage_error("owned binding EVAL TTL missing"))?,
        )?
        .max(1);
        let index_ttl = parse_i64(
            argv.get(2)
                .ok_or_else(|| storage_error("owned binding EVAL index TTL missing"))?,
        )?;
        execute_command_tx(
            tx,
            CommandSpec {
                name: "SETEX".to_string(),
                args: vec![
                    binding_key.clone(),
                    binding_ttl.to_string(),
                    binding.clone(),
                ],
                ignore: false,
            },
        )?;
        execute_command_tx(
            tx,
            CommandSpec {
                name: "SADD".to_string(),
                args: vec![index_key.clone(), binding_key.clone()],
                ignore: false,
            },
        )?;
        if index_ttl > 0 {
            execute_command_tx(
                tx,
                CommandSpec {
                    name: "EXPIRE".to_string(),
                    args: vec![index_key.clone(), index_ttl.to_string()],
                    ignore: false,
                },
            )?;
        }
        return Ok(CmdOutput::Int(1));
    }

    if script.contains("fn-knock:eval:save-binding-keep-ttl-if-session-live:v1") {
        let session_key = keys
            .first()
            .ok_or_else(|| storage_error("binding keep-TTL EVAL session key missing"))?;
        let binding_key = keys
            .get(1)
            .ok_or_else(|| storage_error("binding keep-TTL EVAL binding key missing"))?;
        if string_get_tx(tx, session_key)?.is_none() || string_get_tx(tx, binding_key)?.is_none() {
            return Ok(CmdOutput::Int(0));
        }
        let binding = argv
            .first()
            .ok_or_else(|| storage_error("binding keep-TTL EVAL value missing"))?;
        let changed = tx.execute(
            "UPDATE kv_strings SET value = ?2 WHERE key = ?1",
            params![binding_key, binding],
        )?;
        return Ok(CmdOutput::Int((changed > 0) as i64));
    }

    if script.contains("fn-knock:eval:update-binding-keep-ttl-if-exists:v1") {
        let binding_key = keys
            .first()
            .ok_or_else(|| storage_error("binding update EVAL key missing"))?;
        let index_key = keys
            .get(1)
            .ok_or_else(|| storage_error("binding update EVAL index key missing"))?;
        let Some(current_raw) = string_get_tx(tx, binding_key)? else {
            return Ok(CmdOutput::Int(0));
        };
        let expected_owner = argv
            .get(1)
            .ok_or_else(|| storage_error("binding update EVAL expected owner missing"))?;
        let current_owner = serde_json::from_str::<serde_json::Value>(&current_raw)
            .ok()
            .and_then(|value| {
                value
                    .get("ownerSessionId")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string)
            });
        if current_owner.as_deref() != Some(expected_owner.as_str()) {
            return Ok(CmdOutput::Int(0));
        }
        let binding = argv
            .first()
            .ok_or_else(|| storage_error("binding update EVAL value missing"))?;
        let changed = tx.execute(
            "UPDATE kv_strings SET value = ?2 WHERE key = ?1",
            params![binding_key, binding],
        )?;
        if changed > 0 {
            execute_command_tx(
                tx,
                CommandSpec {
                    name: "SREM".to_string(),
                    args: vec![index_key.clone(), binding_key.clone()],
                    ignore: false,
                },
            )?;
        }
        return Ok(CmdOutput::Int((changed > 0) as i64));
    }

    if script.contains("fn-knock:eval:set-whitelist-owner-if-session-live:v1") {
        let session_key = keys
            .first()
            .ok_or_else(|| storage_error("whitelist owner EVAL session key missing"))?;
        let owner_key = keys
            .get(1)
            .ok_or_else(|| storage_error("whitelist owner EVAL owner key missing"))?;
        if string_get_tx(tx, session_key)?.is_none() {
            return Ok(CmdOutput::Int(0));
        }
        let session_id = argv
            .first()
            .ok_or_else(|| storage_error("whitelist owner EVAL session ID missing"))?;
        let ttl = parse_i64(
            argv.get(1)
                .ok_or_else(|| storage_error("whitelist owner EVAL TTL missing"))?,
        )?
        .max(1);
        execute_command_tx(
            tx,
            CommandSpec {
                name: "SETEX".to_string(),
                args: vec![owner_key.clone(), ttl.to_string(), session_id.clone()],
                ignore: false,
            },
        )?;
        return Ok(CmdOutput::Int(1));
    }

    if script.contains("fn-knock:eval:json-lock-refresh:v1")
        || script.contains("fn-knock:eval:json-lock-release:v1")
        || script.contains("pcall(cjson.decode, raw)")
            && script.contains("decoded[\"lockId\"] ~= ARGV[1]")
    {
        let key = keys
            .first()
            .ok_or_else(|| storage_error("JSON lock EVAL key missing"))?;
        let expected_lock_id = argv
            .first()
            .ok_or_else(|| storage_error("JSON lock EVAL lock id missing"))?;
        let owned = string_get_tx(tx, key)?
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
            .and_then(|value| {
                value
                    .get("lockId")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string)
            })
            .is_some_and(|lock_id| lock_id == *expected_lock_id);
        if !owned {
            return Ok(CmdOutput::Int(0));
        }

        if script.contains("fn-knock:eval:json-lock-refresh:v1")
            || script.contains("redis.call(\"SET\", KEYS[1], ARGV[2]")
        {
            let value = argv
                .get(1)
                .ok_or_else(|| storage_error("JSON lock EVAL value missing"))?;
            let ttl_seconds = parse_i64(
                argv.get(2)
                    .ok_or_else(|| storage_error("JSON lock EVAL TTL missing"))?,
            )?;
            if ttl_seconds <= 0 {
                return Err(storage_error("JSON lock EVAL TTL must be positive"));
            }
            set_string_tx(
                tx,
                key,
                value,
                Some(now_ms().saturating_add(ttl_seconds.saturating_mul(1000))),
            )?;
            return Ok(CmdOutput::Int(1));
        }

        if script.contains("fn-knock:eval:json-lock-release:v1")
            || script.contains("redis.call(\"DEL\", KEYS[1])")
        {
            delete_key_tx(tx, key)?;
            return Ok(CmdOutput::Int(1));
        }

        return Err(storage_error("unsupported JSON lock EVAL operation"));
    }

    if script.contains("fn-knock:eval:delete-if-value:v1")
        || script.contains("redis.call('GET', KEYS[1]) == ARGV[1]")
        || script.contains("redis.call(\"GET\", KEYS[1])") && script.contains("value == ARGV[1]")
    {
        let key = keys
            .first()
            .ok_or_else(|| storage_error("EVAL key missing"))?;
        let expected = argv
            .first()
            .ok_or_else(|| storage_error("EVAL argv missing"))?;
        let current = string_get_tx(tx, key)?;
        if current.as_deref() == Some(expected.as_str()) {
            delete_key_tx(tx, key)?;
            return Ok(CmdOutput::Int(1));
        }
        return Ok(CmdOutput::Int(0));
    }

    if script.contains("fn-knock:eval:consume-value:v1")
        || script.contains("local value = redis.call(\"GET\", KEYS[1])")
            && script.contains("redis.call(\"DEL\", KEYS[1])")
    {
        let key = keys
            .first()
            .ok_or_else(|| storage_error("EVAL key missing"))?;
        let value = string_get_tx(tx, key)?;
        if value.is_some() {
            delete_key_tx(tx, key)?;
        }
        return Ok(CmdOutput::OptionalString(value));
    }

    if script.contains("fn-knock:eval:login-backoff:v1")
        || script.contains("local key = KEYS[1]") && script.contains("blockedUntil")
    {
        let key = keys
            .first()
            .ok_or_else(|| storage_error("EVAL key missing"))?;
        let ip = argv
            .first()
            .ok_or_else(|| storage_error("login backoff ip missing"))?;
        let now = parse_i64(
            argv.get(1)
                .ok_or_else(|| storage_error("login backoff now missing"))?,
        )?;
        let ttl = parse_i64(
            argv.get(2)
                .ok_or_else(|| storage_error("login backoff ttl missing"))?,
        )?;
        let base_delay = parse_i64(
            argv.get(3)
                .ok_or_else(|| storage_error("login backoff base missing"))?,
        )?;
        let max_delay = parse_i64(
            argv.get(4)
                .ok_or_else(|| storage_error("login backoff max missing"))?,
        )?;
        let jitter_factor = parse_f64(
            argv.get(5)
                .ok_or_else(|| storage_error("login backoff jitter missing"))?,
        )?;
        let attempts = string_get_tx(tx, key)?
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
            .and_then(|value| value.get("attempts").and_then(serde_json::Value::as_i64))
            .unwrap_or(0)
            + 1;
        let exp_delay =
            (2_i64.saturating_pow((attempts - 1).clamp(0, 30) as u32) * base_delay).max(0);
        let seed = format!("{ip}:{attempts}:{now}");
        let mut hash = 0_i64;
        for byte in seed.bytes() {
            hash = (hash * 33 + byte as i64) % 1_000_003;
        }
        let ratio = (hash % 10_000) as f64 / 10_000.0;
        let jitter = ((ratio * 2.0) - 1.0) * (exp_delay as f64 * jitter_factor);
        let backoff_ms = ((exp_delay as f64 + jitter).floor() as i64).clamp(0, max_delay);
        let blocked_until = now + backoff_ms;
        let state = serde_json::json!({
            "ip": ip,
            "attempts": attempts,
            "lastAttempt": now,
            "blockedUntil": blocked_until
        })
        .to_string();
        set_string_tx(tx, key, &state, Some(now_ms() + ttl.max(1) * 1000))?;
        return Ok(CmdOutput::Ints(vec![
            attempts,
            (backoff_ms + 999) / 1000,
            blocked_until,
        ]));
    }

    if script.contains("fn-knock:eval:zset-claim:v1")
        || script.contains("ZRANGEBYSCORE")
            && script.contains("ZREM")
            && script.contains("unpack(ids)")
    {
        let key = keys
            .first()
            .ok_or_else(|| storage_error("EVAL key missing"))?;
        let max = parse_i64(
            argv.first()
                .ok_or_else(|| storage_error("ready max missing"))?,
        )?;
        let limit = parse_i64(
            argv.get(1)
                .ok_or_else(|| storage_error("ready limit missing"))?,
        )?;
        let ids = zrangebyscore_tx(
            tx,
            key,
            ScoreBound::inclusive(f64::NEG_INFINITY),
            ScoreBound::inclusive(max as f64),
            Some(limit as usize),
            false,
        )?;
        for id in &ids {
            tx.execute(
                "DELETE FROM kv_zset WHERE key = ?1 AND member = ?2",
                params![key, id],
            )?;
        }
        delete_collection_key_if_empty_tx(tx, key, "zset")?;
        return Ok(CmdOutput::Strings(ids));
    }

    Err(storage_error("unsupported Redis-compatible EVAL script"))
}

fn set_command_tx(tx: &rusqlite::Transaction<'_>, args: &[String]) -> RedisResult<CmdOutput> {
    let key = arg(args, 0)?.to_string();
    let value = arg(args, 1)?.to_string();
    let mut ttl_ms = None;
    let mut nx = false;
    let mut xx = false;
    let mut index = 2;
    while index < args.len() {
        match args[index].to_ascii_uppercase().as_str() {
            "EX" => {
                ttl_ms = args
                    .get(index + 1)
                    .map(|value| parse_i64(value))
                    .transpose()?;
                ttl_ms = ttl_ms.map(|value| value.max(1) * 1000);
                index += 2;
            }
            "PX" => {
                ttl_ms = args
                    .get(index + 1)
                    .map(|value| parse_i64(value))
                    .transpose()?;
                ttl_ms = ttl_ms.map(|value| value.max(1));
                index += 2;
            }
            "NX" => {
                nx = true;
                index += 1;
            }
            "XX" => {
                xx = true;
                index += 1;
            }
            _ => index += 1,
        }
    }
    purge_expired_tx(tx, &key)?;
    if nx && xx {
        return Err(storage_error("SET cannot combine NX and XX"));
    }
    if nx && key_kind_tx(tx, &key)?.is_some() {
        return Ok(CmdOutput::OptionalString(None));
    }
    if xx && key_kind_tx(tx, &key)?.is_none() {
        return Ok(CmdOutput::OptionalString(None));
    }
    set_string_tx(tx, &key, &value, ttl_ms.map(|ttl| now_ms() + ttl))?;
    Ok(if nx || xx {
        CmdOutput::OptionalString(Some("OK".to_string()))
    } else {
        CmdOutput::String("OK".to_string())
    })
}

fn zrange_command_tx(
    tx: &rusqlite::Transaction<'_>,
    args: &[String],
    reverse: bool,
) -> RedisResult<CmdOutput> {
    let key = arg(args, 0)?;
    let start = parse_i64(arg(args, 1)?)? as isize;
    let end = parse_i64(arg(args, 2)?)? as isize;
    let with_scores = args
        .iter()
        .any(|arg| arg.eq_ignore_ascii_case("WITHSCORES"));
    purge_expired_tx(tx, key)?;
    let len = count_rows_tx(tx, "SELECT COUNT(*) FROM kv_zset WHERE key = ?1", &[&key])?;
    let Some((offset, limit)) = normalize_range(len, start, end) else {
        return Ok(if with_scores {
            CmdOutput::ZPairs(Vec::new())
        } else {
            CmdOutput::Strings(Vec::new())
        });
    };
    let order = if reverse {
        "ORDER BY score DESC, member DESC"
    } else {
        "ORDER BY score ASC, member ASC"
    };
    let sql =
        format!("SELECT member, score FROM kv_zset WHERE key = ?1 {order} LIMIT ?2 OFFSET ?3");
    let mut stmt = tx.prepare(&sql)?;
    let rows = stmt.query_map(params![key, limit, offset], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
    })?;
    let pairs = rows.collect::<Result<Vec<_>, _>>()?;
    if with_scores {
        Ok(CmdOutput::ZPairs(pairs))
    } else {
        Ok(CmdOutput::Strings(
            pairs.into_iter().map(|(member, _)| member).collect(),
        ))
    }
}

fn zrangebyscore_command_tx(
    tx: &rusqlite::Transaction<'_>,
    args: &[String],
    reverse: bool,
) -> RedisResult<CmdOutput> {
    let key = arg(args, 0)?;
    let (min, max) = if reverse {
        (
            parse_score_bound(arg(args, 2)?)?,
            parse_score_bound(arg(args, 1)?)?,
        )
    } else {
        (
            parse_score_bound(arg(args, 1)?)?,
            parse_score_bound(arg(args, 2)?)?,
        )
    };
    let with_scores = args
        .iter()
        .any(|arg| arg.eq_ignore_ascii_case("WITHSCORES"));
    let limit = args
        .windows(3)
        .find(|window| window[0].eq_ignore_ascii_case("LIMIT"))
        .and_then(|window| parse_i64(&window[2]).ok())
        .map(|value| value.max(0) as usize);
    let members = zrangebyscore_tx(tx, key, min, max, limit, reverse)?;
    if with_scores {
        let mut pairs = Vec::with_capacity(members.len() * 2);
        for member in members {
            let score = tx
                .query_row(
                    "SELECT score FROM kv_zset WHERE key = ?1 AND member = ?2",
                    params![key, member],
                    |row| row.get::<_, f64>(0),
                )
                .optional()?
                .unwrap_or(0.0);
            pairs.push(member);
            pairs.push(score.to_string());
        }
        Ok(CmdOutput::StringPairs(pairs))
    } else {
        Ok(CmdOutput::Strings(members))
    }
}

fn xadd_command_tx(tx: &rusqlite::Transaction<'_>, args: &[String]) -> RedisResult<CmdOutput> {
    let key = arg(args, 0)?.to_string();
    let raw_id = arg(args, 1)?.to_string();
    if args.len() <= 2 || !args[2..].len().is_multiple_of(2) {
        return Err(storage_error("XADD requires field/value pairs"));
    }
    ensure_key_tx(tx, &key, "stream", None)?;
    let last_id = stream_last_generated_id_tx(tx, &key)?;
    let next_id = if raw_id == "*" {
        let now = now_ms().max(0) as u128;
        if now > last_id.milliseconds {
            ParsedStreamId {
                milliseconds: now,
                sequence: 0,
            }
        } else {
            ParsedStreamId {
                milliseconds: last_id.milliseconds,
                sequence: last_id
                    .sequence
                    .checked_add(1)
                    .ok_or_else(|| storage_error("stream ID sequence overflow"))?,
            }
        }
    } else {
        let explicit = parse_stream_id(&raw_id)
            .ok_or_else(|| storage_error(format!("invalid XADD stream ID: {raw_id}")))?;
        if explicit == ParsedStreamId::default() || explicit <= last_id {
            return Err(storage_error(
                "XADD stream ID must be greater than the last generated ID",
            ));
        }
        explicit
    };
    let id = format!("{}-{}", next_id.milliseconds, next_id.sequence);
    let mut fields = Vec::with_capacity(args[2..].len());
    for value in &args[2..] {
        fields.push(value.clone());
    }
    tx.execute(
        "INSERT INTO kv_stream(key, id, id_ms, id_sequence, fields_json)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            key,
            id,
            i64::try_from(next_id.milliseconds)
                .map_err(|_| storage_error("stream ID milliseconds exceed SQLite range"))?,
            i64::try_from(next_id.sequence)
                .map_err(|_| storage_error("stream ID sequence exceed SQLite range"))?,
            serde_json::to_string(&fields)?
        ],
    )?;
    set_stream_last_generated_id_tx(tx, &key, next_id)?;
    Ok(CmdOutput::String(id))
}

fn string_get_tx(tx: &rusqlite::Transaction<'_>, key: &str) -> RedisResult<Option<String>> {
    purge_expired_tx(tx, key)?;
    if key_kind_tx(tx, key)? != Some("string".to_string()) {
        return Ok(None);
    }
    tx.query_row(
        "SELECT value FROM kv_strings WHERE key = ?1",
        params![key],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(Into::into)
}

fn set_string_tx(
    tx: &rusqlite::Transaction<'_>,
    key: &str,
    value: &str,
    expires_at_ms: Option<i64>,
) -> RedisResult<()> {
    ensure_key_with_ttl_policy_tx(tx, key, "string", expires_at_ms, false)?;
    tx.execute(
        "INSERT OR REPLACE INTO kv_strings(key, value) VALUES (?1, ?2)",
        params![key, value],
    )?;
    Ok(())
}

fn set_string_preserve_ttl_tx(
    tx: &rusqlite::Transaction<'_>,
    key: &str,
    value: &str,
) -> RedisResult<()> {
    ensure_key_tx(tx, key, "string", None)?;
    tx.execute(
        "INSERT OR REPLACE INTO kv_strings(key, value) VALUES (?1, ?2)",
        params![key, value],
    )?;
    Ok(())
}

fn zrangebyscore_tx(
    tx: &rusqlite::Transaction<'_>,
    key: &str,
    min: ScoreBound,
    max: ScoreBound,
    limit: Option<usize>,
    reverse: bool,
) -> RedisResult<Vec<String>> {
    purge_expired_tx(tx, key)?;
    let order = if reverse {
        "ORDER BY score DESC, member DESC"
    } else {
        "ORDER BY score ASC, member ASC"
    };
    let limit_sql = limit
        .map(|limit| format!(" LIMIT {}", limit))
        .unwrap_or_default();
    let min_op = if min.exclusive { ">" } else { ">=" };
    let max_op = if max.exclusive { "<" } else { "<=" };
    let sql = format!(
        "SELECT member FROM kv_zset WHERE key = ?1 AND score {min_op} ?2 AND score {max_op} ?3 {order}{limit_sql}"
    );
    let mut stmt = tx.prepare(&sql)?;
    let rows = stmt.query_map(params![key, min.value, max.value], |row| row.get(0))?;
    rows.collect::<Result<Vec<String>, _>>().map_err(Into::into)
}

fn delete_zset_score_range_tx(
    tx: &rusqlite::Transaction<'_>,
    key: &str,
    min: ScoreBound,
    max: ScoreBound,
) -> RedisResult<usize> {
    let min_op = if min.exclusive { ">" } else { ">=" };
    let max_op = if max.exclusive { "<" } else { "<=" };
    let sql =
        format!("DELETE FROM kv_zset WHERE key = ?1 AND score {min_op} ?2 AND score {max_op} ?3");
    let deleted = tx.execute(&sql, params![key, min.value, max.value])?;
    delete_collection_key_if_empty_tx(tx, key, "zset")?;
    Ok(deleted)
}

fn count_zset_score_range_tx(
    tx: &rusqlite::Transaction<'_>,
    key: &str,
    min: ScoreBound,
    max: ScoreBound,
) -> RedisResult<i64> {
    let min_op = if min.exclusive { ">" } else { ">=" };
    let max_op = if max.exclusive { "<" } else { "<=" };
    let sql = format!(
        "SELECT COUNT(*) FROM kv_zset WHERE key = ?1 AND score {min_op} ?2 AND score {max_op} ?3"
    );
    tx.query_row(&sql, params![key, min.value, max.value], |row| row.get(0))
        .map_err(Into::into)
}

fn list_range_tx(
    tx: &rusqlite::Transaction<'_>,
    key: &str,
    start: isize,
    end: isize,
) -> RedisResult<Vec<String>> {
    let len = count_rows_tx(tx, "SELECT COUNT(*) FROM kv_list WHERE key = ?1", &[&key])?;
    let Some((offset, limit)) = normalize_range(len, start, end) else {
        return Ok(Vec::new());
    };
    let mut stmt =
        tx.prepare("SELECT value FROM kv_list WHERE key = ?1 ORDER BY idx ASC LIMIT ?2 OFFSET ?3")?;
    let rows = stmt.query_map(params![key, limit, offset], |row| row.get(0))?;
    rows.collect::<Result<Vec<String>, _>>().map_err(Into::into)
}

fn scan_keys_tx(tx: &rusqlite::Transaction<'_>, prefix: &str) -> RedisResult<Vec<String>> {
    purge_expired_all_tx(tx)?;
    let pattern = format!("{}%", escape_like_pattern(prefix));
    let mut stmt =
        tx.prepare("SELECT key FROM kv_keys WHERE key LIKE ?1 ESCAPE '\\' ORDER BY key ASC")?;
    let rows = stmt.query_map(params![pattern], |row| row.get::<_, String>(0))?;
    let mut keys = rows.collect::<Result<Vec<_>, _>>()?;
    keys.sort_by(|left, right| node_locale_compare_ordering(left, right));
    Ok(keys)
}

fn purge_expired(conn: &rusqlite::Connection, key: &str) -> RedisResult<()> {
    conn.execute(
        "DELETE FROM kv_keys WHERE key = ?1 AND expires_at_ms IS NOT NULL AND expires_at_ms <= ?2",
        params![key, now_ms()],
    )?;
    Ok(())
}

fn purge_expired_tx(tx: &rusqlite::Transaction<'_>, key: &str) -> RedisResult<()> {
    tx.execute(
        "DELETE FROM kv_keys WHERE key = ?1 AND expires_at_ms IS NOT NULL AND expires_at_ms <= ?2",
        params![key, now_ms()],
    )?;
    Ok(())
}

fn purge_expired_all_tx(tx: &rusqlite::Transaction<'_>) -> RedisResult<()> {
    tx.execute(
        "DELETE FROM kv_keys WHERE expires_at_ms IS NOT NULL AND expires_at_ms <= ?1",
        params![now_ms()],
    )?;
    Ok(())
}

fn key_kind(conn: &rusqlite::Connection, key: &str) -> RedisResult<Option<String>> {
    conn.query_row(
        "SELECT kind FROM kv_keys WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .optional()
    .map_err(Into::into)
}

fn key_kind_tx(tx: &rusqlite::Transaction<'_>, key: &str) -> RedisResult<Option<String>> {
    tx.query_row(
        "SELECT kind FROM kv_keys WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .optional()
    .map_err(Into::into)
}

fn ensure_key_tx(
    tx: &rusqlite::Transaction<'_>,
    key: &str,
    kind: &str,
    expires_at_ms: Option<i64>,
) -> RedisResult<()> {
    ensure_key_with_ttl_policy_tx(tx, key, kind, expires_at_ms, true)
}

fn ensure_key_with_ttl_policy_tx(
    tx: &rusqlite::Transaction<'_>,
    key: &str,
    kind: &str,
    expires_at_ms: Option<i64>,
    preserve_existing_ttl_when_none: bool,
) -> RedisResult<()> {
    purge_expired_tx(tx, key)?;
    if let Some(existing) = key_kind_tx(tx, key)?
        && existing != kind
    {
        delete_key_tx(tx, key)?;
    }
    if preserve_existing_ttl_when_none {
        tx.execute(
            "INSERT INTO kv_keys(key, kind, expires_at_ms) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET kind = excluded.kind,
               expires_at_ms = COALESCE(excluded.expires_at_ms, kv_keys.expires_at_ms)",
            params![key, kind, expires_at_ms],
        )?;
    } else {
        tx.execute(
            "INSERT INTO kv_keys(key, kind, expires_at_ms) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET kind = excluded.kind,
               expires_at_ms = excluded.expires_at_ms",
            params![key, kind, expires_at_ms],
        )?;
    }
    Ok(())
}

fn delete_key_tx(tx: &rusqlite::Transaction<'_>, key: &str) -> RedisResult<usize> {
    Ok(tx.execute("DELETE FROM kv_keys WHERE key = ?1", params![key])?)
}

fn delete_collection_key_if_empty_tx(
    tx: &rusqlite::Transaction<'_>,
    key: &str,
    kind: &str,
) -> RedisResult<()> {
    let table = match kind {
        "hash" => "kv_hash",
        "list" => "kv_list",
        "set" => "kv_set",
        "zset" => "kv_zset",
        _ => {
            return Err(storage_error(format!(
                "unsupported collection kind: {kind}"
            )));
        }
    };
    if key_kind_tx(tx, key)?.as_deref() != Some(kind) {
        return Ok(());
    }
    let sql = format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE key = ?1)");
    let has_members = tx.query_row(&sql, params![key], |row| row.get::<_, bool>(0))?;
    if !has_members {
        delete_key_tx(tx, key)?;
    }
    Ok(())
}

fn immediate_transaction(
    conn: &mut rusqlite::Connection,
) -> rusqlite::Result<rusqlite::Transaction<'_>> {
    conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
}

fn run_schema_migrations(conn: &mut rusqlite::Connection, path: &Path) -> RedisResult<()> {
    conn.execute_batch(SCHEMA_MIGRATIONS_SQL)?;
    let latest_known_version = SCHEMA_MIGRATIONS
        .last()
        .map(|migration| migration.version)
        .unwrap_or_default();
    let latest_applied_version: Option<i64> =
        conn.query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
            row.get(0)
        })?;
    if let Some(applied_version) =
        latest_applied_version.filter(|version| *version > latest_known_version)
    {
        return Err(storage_error(format!(
            "SQLite schema version {} is newer than this server supports ({latest_known_version})",
            applied_version
        )));
    }

    for migration in SCHEMA_MIGRATIONS {
        run_schema_migration(conn, path, migration)?;
    }
    Ok(())
}

fn run_schema_migration(
    conn: &mut rusqlite::Connection,
    path: &Path,
    migration: &SchemaMigration,
) -> RedisResult<()> {
    let expected_checksum = migration_checksum(migration.sql);
    let applied = conn
        .query_row(
            "SELECT name, checksum FROM schema_migrations WHERE version = ?1",
            params![migration.version],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;

    if let Some((name, checksum)) = applied {
        if name != migration.name {
            return Err(storage_error(format!(
                "SQLite schema migration {} name mismatch: expected {}, found {}",
                migration.version, migration.name, name
            )));
        }
        if checksum == expected_checksum {
            return Ok(());
        }
        if is_legacy_bootstrap_migration(conn, migration, &checksum)? {
            conn.execute(
                "UPDATE schema_migrations SET checksum = ?2, applied_at_ms = ?3 WHERE version = ?1",
                params![migration.version, expected_checksum, now_ms()],
            )?;
            return Ok(());
        }
        return Err(storage_error(format!(
            "SQLite schema migration {} checksum mismatch",
            migration.version
        )));
    }

    if migration.destructive {
        create_migration_backup(conn, path, migration)?;
    }
    let tx = immediate_transaction(conn)?;
    tx.execute_batch(migration.sql)?;
    tx.execute(
        "INSERT INTO schema_migrations(version, name, checksum, applied_at_ms)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            migration.version,
            migration.name,
            expected_checksum,
            now_ms()
        ],
    )?;
    tx.commit()?;
    Ok(())
}

fn is_legacy_bootstrap_migration(
    conn: &rusqlite::Connection,
    migration: &SchemaMigration,
    checksum: &str,
) -> RedisResult<bool> {
    Ok(migration.version == 1
        && migration.name == "redis_compatible_keyspace"
        && checksum == "v1"
        && sqlite_table_exists(conn, "storage_meta")?
        && sqlite_table_exists(conn, "kv_keys")?)
}

fn sqlite_table_exists(conn: &rusqlite::Connection, name: &str) -> RedisResult<bool> {
    let exists = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        params![name],
        |row| row.get::<_, i64>(0),
    )?;
    Ok(exists == 1)
}

fn create_migration_backup(
    conn: &mut rusqlite::Connection,
    path: &Path,
    migration: &SchemaMigration,
) -> RedisResult<Option<PathBuf>> {
    if !path.exists() {
        return Ok(None);
    }
    conn.execute_batch("PRAGMA wal_checkpoint(FULL);")?;
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("fn-knock.sqlite3");
    let backup_path = path.with_file_name(format!(
        "{file_name}.migration-v{}.{}.bak",
        migration.version,
        now_ms()
    ));
    std::fs::copy(path, &backup_path)?;
    Ok(Some(backup_path))
}

fn migration_checksum(sql: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(sql.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

fn escape_like_pattern(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' | '%' | '_' => {
                escaped.push('\\');
                escaped.push(character);
            }
            _ => escaped.push(character),
        }
    }
    escaped
}

fn count_rows(conn: &rusqlite::Connection, sql: &str, args: &[&dyn ToSql]) -> RedisResult<i64> {
    conn.query_row(sql, params_from_iter(args.iter().copied()), |row| {
        row.get(0)
    })
    .map_err(Into::into)
}

fn count_rows_tx(
    tx: &rusqlite::Transaction<'_>,
    sql: &str,
    args: &[&dyn ToSql],
) -> RedisResult<i64> {
    tx.query_row(sql, params_from_iter(args.iter().copied()), |row| {
        row.get(0)
    })
    .map_err(Into::into)
}

fn query_strings(
    conn: &rusqlite::Connection,
    sql: &str,
    args: &[&dyn ToSql],
) -> RedisResult<Vec<String>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params_from_iter(args.iter().copied()), |row| row.get(0))?;
    rows.collect::<Result<Vec<String>, _>>().map_err(Into::into)
}

fn stream_id_from_row(id: String, fields_json: String) -> rusqlite::Result<streams::StreamId> {
    let fields = stream_fields_vec(&fields_json)?;
    let mut object = HashMap::new();
    for pair in fields.chunks(2) {
        if let [field, value] = pair {
            object.insert(field.clone(), value.clone());
        }
    }
    Ok(streams::StreamId::new(id, object))
}

fn stream_fields_vec(fields_json: &str) -> rusqlite::Result<Vec<String>> {
    let value = serde_json::from_str::<serde_json::Value>(fields_json)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    match value {
        serde_json::Value::Array(items) => {
            let mut fields = Vec::with_capacity(items.len());
            for item in items {
                let Some(text) = item.as_str() else {
                    return Err(invalid_stream_fields_json());
                };
                fields.push(text.to_string());
            }
            if fields.is_empty() || fields.len() % 2 != 0 {
                return Err(invalid_stream_fields_json());
            }
            Ok(fields)
        }
        serde_json::Value::Object(object) => {
            let ordered = object.into_iter().collect::<BTreeMap<_, _>>();
            let mut fields = Vec::with_capacity(ordered.len() * 2);
            for (key, value) in ordered {
                let Some(text) = value.as_str() else {
                    return Err(invalid_stream_fields_json());
                };
                fields.push(key);
                fields.push(text.to_string());
            }
            Ok(fields)
        }
        _ => Err(invalid_stream_fields_json()),
    }
}

fn invalid_stream_fields_json() -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "invalid stream fields JSON",
    )))
}

fn normalize_range(len: i64, start: isize, end: isize) -> Option<(i64, i64)> {
    if len <= 0 {
        return None;
    }
    let len = len as isize;
    let start = if start < 0 {
        (len + start).max(0)
    } else {
        start
    };
    let end = if end < 0 { len + end } else { end.min(len - 1) };
    if start >= len || end < 0 {
        return None;
    }
    if start > end {
        return None;
    }
    Some((start as i64, (end - start + 1) as i64))
}

fn arg(args: &[String], index: usize) -> RedisResult<&str> {
    args.get(index)
        .map(String::as_str)
        .ok_or_else(|| storage_error(format!("missing command argument {index}")))
}

fn parse_i64(value: &str) -> RedisResult<i64> {
    value
        .parse::<i64>()
        .map_err(|_| storage_error(format!("invalid integer argument: {value}")))
}

fn parse_f64(value: &str) -> RedisResult<f64> {
    value
        .parse::<f64>()
        .map_err(|_| storage_error(format!("invalid float argument: {value}")))
}

#[derive(Clone, Copy, Debug)]
struct ScoreBound {
    value: f64,
    exclusive: bool,
}

impl ScoreBound {
    fn inclusive(value: f64) -> Self {
        Self {
            value,
            exclusive: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
struct ParsedStreamId {
    milliseconds: u128,
    sequence: u128,
}

fn parse_score_bound(value: &str) -> RedisResult<ScoreBound> {
    let (exclusive, raw_value) = value
        .strip_prefix('(')
        .map(|value| (true, value))
        .unwrap_or((false, value));
    if raw_value.is_empty() {
        return Err(storage_error("invalid score bound: empty exclusive bound"));
    }
    Ok(ScoreBound {
        value: parse_score_value(raw_value)?,
        exclusive,
    })
}

fn parse_score_value(value: &str) -> RedisResult<f64> {
    match value {
        "-inf" => Ok(f64::NEG_INFINITY),
        "+inf" | "inf" => Ok(f64::INFINITY),
        _ => parse_f64(value),
    }
}

fn parse_stream_id(value: &str) -> Option<ParsedStreamId> {
    if value == "0" {
        return Some(ParsedStreamId::default());
    }
    let (milliseconds, sequence) = value.split_once('-')?;
    Some(ParsedStreamId {
        milliseconds: milliseconds.parse().ok()?,
        sequence: sequence.parse().ok()?,
    })
}

fn parse_stream_bound(value: &str, is_min: bool) -> RedisResult<Option<ParsedStreamId>> {
    if (is_min && value == "-") || (!is_min && value == "+") {
        return Ok(None);
    }
    parse_stream_id(value)
        .map(Some)
        .ok_or_else(|| storage_error(format!("invalid stream ID bound: {value}")))
}

fn stream_id_sql_tuple(id: ParsedStreamId) -> RedisResult<(i64, i64)> {
    Ok((
        i64::try_from(id.milliseconds)
            .map_err(|_| storage_error("stream ID milliseconds exceed SQLite range"))?,
        i64::try_from(id.sequence)
            .map_err(|_| storage_error("stream ID sequence exceeds SQLite range"))?,
    ))
}

fn query_stream_rows(
    conn: &rusqlite::Connection,
    key: &str,
    min: Option<ParsedStreamId>,
    min_exclusive: bool,
    max: Option<ParsedStreamId>,
    reverse: bool,
    count: usize,
) -> RedisResult<Vec<(String, String)>> {
    let (min_ms, min_sequence) = stream_id_sql_tuple(min.unwrap_or_default())?;
    let (max_ms, max_sequence) = stream_id_sql_tuple(max.unwrap_or(ParsedStreamId {
        milliseconds: i64::MAX as u128,
        sequence: i64::MAX as u128,
    }))?;
    let min_operator = if min_exclusive { ">" } else { ">=" };
    let order = if reverse { "DESC" } else { "ASC" };
    let sql = format!(
        "SELECT id, fields_json
         FROM kv_stream
         WHERE key = ?1
           AND (id_ms, id_sequence) {min_operator} (?2, ?3)
           AND (id_ms, id_sequence) <= (?4, ?5)
         ORDER BY id_ms {order}, id_sequence {order}
         LIMIT ?6"
    );
    let limit = i64::try_from(count).unwrap_or(i64::MAX);
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        params![key, min_ms, min_sequence, max_ms, max_sequence, limit],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    )?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn stream_last_generated_id_tx(
    tx: &rusqlite::Transaction<'_>,
    key: &str,
) -> RedisResult<ParsedStreamId> {
    if let Some((milliseconds, sequence)) = tx
        .query_row(
            "SELECT last_generated_ms, last_generated_seq FROM kv_stream_meta WHERE key = ?1",
            params![key],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()?
    {
        return Ok(ParsedStreamId {
            milliseconds: milliseconds.max(0) as u128,
            sequence: sequence.max(0) as u128,
        });
    }

    tx.query_row(
        "SELECT id_ms, id_sequence
         FROM kv_stream
         WHERE key = ?1
         ORDER BY id_ms DESC, id_sequence DESC
         LIMIT 1",
        params![key],
        |row| {
            Ok(ParsedStreamId {
                milliseconds: row.get::<_, i64>(0)?.max(0) as u128,
                sequence: row.get::<_, i64>(1)?.max(0) as u128,
            })
        },
    )
    .optional()
    .map(Option::unwrap_or_default)
    .map_err(Into::into)
}

fn set_stream_last_generated_id_tx(
    tx: &rusqlite::Transaction<'_>,
    key: &str,
    id: ParsedStreamId,
) -> RedisResult<()> {
    let milliseconds = i64::try_from(id.milliseconds)
        .map_err(|_| storage_error("stream ID milliseconds exceed SQLite range"))?;
    let sequence = i64::try_from(id.sequence)
        .map_err(|_| storage_error("stream ID sequence exceeds SQLite range"))?;
    tx.execute(
        "INSERT INTO kv_stream_meta(key, last_generated_ms, last_generated_seq)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET
           last_generated_ms = excluded.last_generated_ms,
           last_generated_seq = excluded.last_generated_seq",
        params![key, milliseconds, sequence],
    )?;
    Ok(())
}

#[cfg(unix)]
async fn secure_directory_permissions(path: &Path) -> RedisResult<()> {
    tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).await?;
    Ok(())
}

#[cfg(not(unix))]
async fn secure_directory_permissions(_path: &Path) -> RedisResult<()> {
    Ok(())
}

#[cfg(unix)]
async fn secure_sqlite_file_permissions(path: &Path) -> RedisResult<()> {
    set_file_mode_if_exists(path, 0o600).await?;
    set_file_mode_if_exists(&sqlite_companion_path(path, "-wal"), 0o600).await?;
    set_file_mode_if_exists(&sqlite_companion_path(path, "-shm"), 0o600).await?;
    Ok(())
}

#[cfg(not(unix))]
async fn secure_sqlite_file_permissions(_path: &Path) -> RedisResult<()> {
    Ok(())
}

#[cfg(unix)]
async fn set_file_mode_if_exists(path: &Path, mode: u32) -> RedisResult<()> {
    match tokio::fs::metadata(path).await {
        Ok(_) => {
            tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).await?;
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(unix)]
fn sqlite_companion_path(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    PathBuf::from(value)
}

fn now_ms() -> i64 {
    crate::time_utils::now_ms()
}

fn node_locale_compare_ordering(left: &str, right: &str) -> Ordering {
    crate::store::node_locale_compare_ordering(left, right)
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn temp_manager() -> ConnectionManager {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("fn-knock.sqlite3");
        let manager = ConnectionManager::open(&path).await.expect("open sqlite");
        std::mem::forget(dir);
        manager
    }

    #[tokio::test]
    async fn schema_migration_checksum_mismatch_fails_startup() {
        let manager = temp_manager().await;
        manager
            .call(|conn| {
                conn.execute(
                    "UPDATE schema_migrations SET checksum = 'bad' WHERE version = 1",
                    [],
                )?;
                Ok(())
            })
            .await
            .expect("corrupt checksum");

        let error = manager.initialize().await.expect_err("checksum must fail");
        assert!(error.to_string().contains("checksum mismatch"));
    }

    #[tokio::test]
    async fn schema_migration_rejects_future_database_version() {
        let manager = temp_manager().await;
        manager
            .call(|conn| {
                conn.execute(
                    "INSERT INTO schema_migrations(version, name, checksum, applied_at_ms)
                     VALUES (999, 'future', 'sha256:future', ?1)",
                    params![now_ms()],
                )?;
                Ok(())
            })
            .await
            .expect("insert future migration");

        let error = manager.initialize().await.expect_err("future DB must fail");
        assert!(
            error
                .to_string()
                .contains("newer than this server supports")
        );
    }

    #[tokio::test]
    async fn schema_migration_normalizes_legacy_v1_marker() {
        let manager = temp_manager().await;
        manager
            .call(|conn| {
                conn.execute(
                    "UPDATE schema_migrations SET checksum = 'v1' WHERE version = 1",
                    [],
                )?;
                Ok(())
            })
            .await
            .expect("write legacy checksum");

        manager.initialize().await.expect("legacy marker upgrades");
        let checksum = manager
            .call(|conn| {
                conn.query_row(
                    "SELECT checksum FROM schema_migrations WHERE version = 1",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .map_err(Into::into)
            })
            .await
            .expect("read checksum");
        assert_eq!(checksum, migration_checksum(REDIS_COMPATIBLE_KEYSPACE_SQL));
    }

    #[tokio::test]
    async fn schema_migration_v2_backfills_numeric_stream_ids_and_metadata() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("fn-knock.sqlite3");
        let future_ms = now_ms() + 60_000;
        let future_id = format!("{future_ms}-0");
        {
            let conn = rusqlite::Connection::open(&path).expect("open legacy sqlite");
            conn.pragma_update(None, "foreign_keys", "ON")
                .expect("enable foreign keys");
            conn.execute_batch(SCHEMA_MIGRATIONS_SQL)
                .expect("create migration table");
            conn.execute_batch(REDIS_COMPATIBLE_KEYSPACE_SQL)
                .expect("create v1 keyspace");
            conn.execute(
                "INSERT INTO schema_migrations(version, name, checksum, applied_at_ms)
                 VALUES (1, 'redis_compatible_keyspace', ?1, ?2)",
                params![migration_checksum(REDIS_COMPATIBLE_KEYSPACE_SQL), now_ms()],
            )
            .expect("record v1 migration");
            conn.execute(
                "INSERT INTO kv_keys(key, kind) VALUES ('fn_knock:test:legacy-stream', 'stream')",
                [],
            )
            .expect("create legacy stream key");
            for id in ["10-0".to_string(), "9-0".to_string(), future_id.clone()] {
                conn.execute(
                    "INSERT INTO kv_stream(key, id, fields_json) VALUES (?1, ?2, ?3)",
                    params![
                        "fn_knock:test:legacy-stream",
                        id,
                        serde_json::to_string(&vec!["value", id.as_str()]).unwrap()
                    ],
                )
                .expect("seed legacy stream entry");
            }
        }

        let mut manager = ConnectionManager::open(&path)
            .await
            .expect("migrate v1 database");
        let read = manager
            .xread_options(
                &["fn_knock:test:legacy-stream"],
                &["0-0"],
                &streams::StreamReadOptions::default().count(10),
            )
            .await
            .expect("read migrated stream")
            .expect("stream has rows");
        assert_eq!(
            read.keys[0]
                .ids
                .iter()
                .map(|entry| entry.id.as_str())
                .collect::<Vec<_>>(),
            vec!["9-0", "10-0", future_id.as_str()]
        );

        let _: () = cmd("XDEL")
            .arg("fn_knock:test:legacy-stream")
            .arg(vec![
                "9-0".to_string(),
                "10-0".to_string(),
                future_id.clone(),
            ])
            .query_async(&mut manager)
            .await
            .expect("empty migrated stream");
        drop(manager);

        let mut reopened = ConnectionManager::open(&path)
            .await
            .expect("reopen database");
        let generated: String = cmd("XADD")
            .arg("fn_knock:test:legacy-stream")
            .arg("*")
            .arg("value")
            .arg("after-reopen")
            .query_async(&mut reopened)
            .await
            .expect("append after reopen");
        assert_eq!(generated, format!("{future_ms}-1"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn sqlite_database_file_is_owner_only() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("storage").join("fn-knock.sqlite3");
        let _manager = ConnectionManager::open(&path).await.expect("open sqlite");

        let file_mode = tokio::fs::metadata(&path)
            .await
            .expect("stat sqlite")
            .permissions()
            .mode()
            & 0o777;
        let dir_mode = tokio::fs::metadata(path.parent().expect("sqlite parent"))
            .await
            .expect("stat sqlite parent")
            .permissions()
            .mode()
            & 0o777;

        assert_eq!(file_mode, 0o600);
        assert_eq!(dir_mode, 0o700);
    }

    #[tokio::test]
    async fn set_clears_existing_ttl() {
        let mut conn = temp_manager().await;
        conn.set_ex("fn_knock:test:string", "old", 60)
            .await
            .expect("set expiring value");
        assert!(conn.ttl("fn_knock:test:string").await.expect("read ttl") > 0);

        conn.set("fn_knock:test:string", "new")
            .await
            .expect("overwrite value");
        let value: Option<String> = conn.get("fn_knock:test:string").await.expect("read value");
        assert_eq!(value.as_deref(), Some("new"));
        assert_eq!(
            conn.ttl("fn_knock:test:string")
                .await
                .expect("read cleared ttl"),
            -1
        );
    }

    #[tokio::test]
    async fn sorted_set_score_bounds_follow_redis_exclusive_syntax() {
        let mut conn = temp_manager().await;
        conn.zadd("fn_knock:test:zset-bounds", "a", 10)
            .await
            .expect("zadd a");
        conn.zadd("fn_knock:test:zset-bounds", "b", 20)
            .await
            .expect("zadd b");
        conn.zadd("fn_knock:test:zset-bounds", "c", 30)
            .await
            .expect("zadd c");

        let count: i64 = cmd("ZCOUNT")
            .arg("fn_knock:test:zset-bounds")
            .arg("10")
            .arg("(30")
            .query_async(&mut conn)
            .await
            .expect("zcount exclusive max");
        assert_eq!(count, 2);

        let members: Vec<String> = cmd("ZRANGEBYSCORE")
            .arg("fn_knock:test:zset-bounds")
            .arg("(10")
            .arg("30")
            .query_async(&mut conn)
            .await
            .expect("zrangebyscore exclusive min");
        assert_eq!(members, vec!["b".to_string(), "c".to_string()]);

        let reverse_pairs: Vec<String> = cmd("ZREVRANGEBYSCORE")
            .arg("fn_knock:test:zset-bounds")
            .arg("+inf")
            .arg("(10")
            .arg("WITHSCORES")
            .arg("LIMIT")
            .arg(0)
            .arg(2)
            .query_async(&mut conn)
            .await
            .expect("zrevrangebyscore reverse score bounds");
        assert_eq!(
            reverse_pairs,
            vec![
                "c".to_string(),
                "30".to_string(),
                "b".to_string(),
                "20".to_string(),
            ]
        );

        let _: () = cmd("ZREMRANGEBYSCORE")
            .arg("fn_knock:test:zset-bounds")
            .arg("-inf")
            .arg("(20")
            .query_async(&mut conn)
            .await
            .expect("zremrangebyscore exclusive max");
        assert_eq!(
            conn.zrange("fn_knock:test:zset-bounds", 0, -1)
                .await
                .expect("zrange remaining"),
            vec!["b".to_string(), "c".to_string()]
        );
    }

    #[tokio::test]
    async fn ranges_outside_collection_bounds_are_empty() {
        let mut conn = temp_manager().await;
        let _: () = cmd("RPUSH")
            .arg("fn_knock:test:range-list")
            .arg(vec!["a".to_string(), "b".to_string(), "c".to_string()])
            .query_async(&mut conn)
            .await
            .expect("seed list");
        for (member, score) in [("a", 1), ("b", 2), ("c", 3)] {
            conn.zadd("fn_knock:test:range-zset", member, score)
                .await
                .expect("seed zset");
        }

        assert!(
            conn.lrange("fn_knock:test:range-list", 3, -1)
                .await
                .expect("list start at length")
                .is_empty()
        );
        assert!(
            conn.zrange("fn_knock:test:range-zset", 4, 10)
                .await
                .expect("zset start beyond length")
                .is_empty()
        );
        assert!(
            conn.lrange("fn_knock:test:range-list", 0, -4)
                .await
                .expect("list end before first item")
                .is_empty()
        );
        assert_eq!(
            conn.zrange("fn_knock:test:range-zset", -100, -1)
                .await
                .expect("large negative start"),
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
    }

    #[tokio::test]
    async fn empty_collections_remove_their_redis_keys() {
        let mut conn = temp_manager().await;

        conn.hset("fn_knock:test:empty-hash", "field", "value")
            .await
            .expect("seed hash");
        conn.hdel("fn_knock:test:empty-hash", "field")
            .await
            .expect("empty hash");
        assert_eq!(conn.exists("fn_knock:test:empty-hash").await.unwrap(), 0);

        conn.sadd("fn_knock:test:empty-set", "member")
            .await
            .expect("seed set");
        conn.srem("fn_knock:test:empty-set", "member")
            .await
            .expect("empty set");
        assert_eq!(conn.exists("fn_knock:test:empty-set").await.unwrap(), 0);

        conn.zadd("fn_knock:test:empty-zset", "member", 1)
            .await
            .expect("seed zset");
        let _: () = cmd("ZREMRANGEBYSCORE")
            .arg("fn_knock:test:empty-zset")
            .arg("-inf")
            .arg("+inf")
            .query_async(&mut conn)
            .await
            .expect("empty zset");
        assert_eq!(conn.exists("fn_knock:test:empty-zset").await.unwrap(), 0);

        let _: () = cmd("RPUSH")
            .arg("fn_knock:test:empty-list")
            .arg("value")
            .query_async(&mut conn)
            .await
            .expect("seed list");
        let _: () = cmd("LTRIM")
            .arg("fn_knock:test:empty-list")
            .arg(1)
            .arg(0)
            .query_async(&mut conn)
            .await
            .expect("empty list");
        assert_eq!(conn.exists("fn_knock:test:empty-list").await.unwrap(), 0);
    }

    #[tokio::test]
    async fn stream_ids_follow_numeric_order_and_remain_monotonic() {
        let mut conn = temp_manager().await;
        for id in ["9-0", "10-0"] {
            let inserted: String = cmd("XADD")
                .arg("fn_knock:test:numeric-stream")
                .arg(id)
                .arg("value")
                .arg(id)
                .query_async(&mut conn)
                .await
                .expect("insert explicit stream ID");
            assert_eq!(inserted, id);
        }

        let ascending: Vec<(String, Vec<String>)> = cmd("XRANGE")
            .arg("fn_knock:test:numeric-stream")
            .arg("-")
            .arg("+")
            .query_async(&mut conn)
            .await
            .expect("numeric xrange");
        assert_eq!(
            ascending
                .iter()
                .map(|(id, _)| id.as_str())
                .collect::<Vec<_>>(),
            vec!["9-0", "10-0"]
        );

        let read = conn
            .xread_options(
                &["fn_knock:test:numeric-stream"],
                &["0-0"],
                &streams::StreamReadOptions::default().count(10),
            )
            .await
            .expect("numeric xread")
            .expect("xread rows");
        assert_eq!(
            read.keys[0]
                .ids
                .iter()
                .map(|entry| entry.id.as_str())
                .collect::<Vec<_>>(),
            vec!["9-0", "10-0"]
        );

        let descending = conn
            .xrevrange_count("fn_knock:test:numeric-stream", "+", "-", 10)
            .await
            .expect("numeric xrevrange");
        assert_eq!(
            descending
                .ids
                .iter()
                .map(|entry| entry.id.as_str())
                .collect::<Vec<_>>(),
            vec!["10-0", "9-0"]
        );

        let _: () = cmd("XTRIM")
            .arg("fn_knock:test:numeric-stream")
            .arg("MINID")
            .arg("~")
            .arg("10-0")
            .query_async(&mut conn)
            .await
            .expect("numeric xtrim");
        let remaining: Vec<(String, Vec<String>)> = cmd("XRANGE")
            .arg("fn_knock:test:numeric-stream")
            .arg("-")
            .arg("+")
            .query_async(&mut conn)
            .await
            .expect("remaining stream entries");
        assert_eq!(remaining[0].0, "10-0");

        let _: () = cmd("XDEL")
            .arg("fn_knock:test:numeric-stream")
            .arg("10-0")
            .query_async(&mut conn)
            .await
            .expect("empty stream");
        assert_eq!(
            conn.exists("fn_knock:test:numeric-stream").await.unwrap(),
            1
        );

        let generated: String = cmd("XADD")
            .arg("fn_knock:test:numeric-stream")
            .arg("*")
            .arg("value")
            .arg("generated")
            .query_async(&mut conn)
            .await
            .expect("generate monotonic ID");
        assert!(parse_stream_id(&generated).unwrap() > parse_stream_id("10-0").unwrap());

        let duplicate = cmd("XADD")
            .arg("fn_knock:test:numeric-stream")
            .arg("10-0")
            .arg("value")
            .arg("duplicate")
            .query_async::<String>(&mut conn)
            .await;
        assert!(duplicate.is_err());

        let future_ms = now_ms() + 60_000;
        let future_id = format!("{future_ms}-0");
        let _: String = cmd("XADD")
            .arg("fn_knock:test:clock-rollback-stream")
            .arg(&future_id)
            .arg("value")
            .arg("future")
            .query_async(&mut conn)
            .await
            .expect("seed future stream ID");
        let _: () = cmd("XDEL")
            .arg("fn_knock:test:clock-rollback-stream")
            .arg(&future_id)
            .query_async(&mut conn)
            .await
            .expect("remove future stream entry");
        let after_rollback: String = cmd("XADD")
            .arg("fn_knock:test:clock-rollback-stream")
            .arg("*")
            .arg("value")
            .arg("after-rollback")
            .query_async(&mut conn)
            .await
            .expect("generate after clock rollback");
        assert_eq!(after_rollback, format!("{future_ms}-1"));
    }

    #[tokio::test]
    async fn xtrim_maxlen_keeps_newest_entries() {
        let mut conn = temp_manager().await;
        for id in ["1-0", "2-0", "3-0"] {
            let _: String = cmd("XADD")
                .arg("fn_knock:test:maxlen-stream")
                .arg(id)
                .arg("value")
                .arg(id)
                .query_async(&mut conn)
                .await
                .expect("seed stream");
        }
        let _: () = cmd("XTRIM")
            .arg("fn_knock:test:maxlen-stream")
            .arg("MAXLEN")
            .arg("~")
            .arg(2)
            .query_async(&mut conn)
            .await
            .expect("trim stream length");
        let remaining: Vec<(String, Vec<String>)> = cmd("XRANGE")
            .arg("fn_knock:test:maxlen-stream")
            .arg("-")
            .arg("+")
            .query_async(&mut conn)
            .await
            .expect("read trimmed stream");
        assert_eq!(
            remaining
                .iter()
                .map(|(id, _)| id.as_str())
                .collect::<Vec<_>>(),
            vec!["2-0", "3-0"]
        );
    }

    #[tokio::test]
    async fn keyspace_primitives_cover_core_types() {
        let mut conn = temp_manager().await;

        let _: () = cmd("HSET")
            .arg("fn_knock:test:hash")
            .arg("field")
            .arg("value")
            .query_async(&mut conn)
            .await
            .expect("hset");
        let hash = conn.hgetall("fn_knock:test:hash").await.expect("hgetall");
        assert_eq!(hash.get("field").map(String::as_str), Some("value"));

        conn.sadd("fn_knock:test:set", vec!["b".to_string(), "a".to_string()])
            .await
            .expect("sadd");
        assert_eq!(
            conn.smembers("fn_knock:test:set").await.expect("smembers"),
            vec!["a".to_string(), "b".to_string()]
        );

        let _: () = cmd("RPUSH")
            .arg("fn_knock:test:list")
            .arg(vec!["one".to_string(), "two".to_string()])
            .query_async(&mut conn)
            .await
            .expect("rpush");
        assert_eq!(
            conn.lrange("fn_knock:test:list", 0, -1)
                .await
                .expect("lrange"),
            vec!["one".to_string(), "two".to_string()]
        );

        conn.zadd("fn_knock:test:zset", "low", 1)
            .await
            .expect("zadd low");
        conn.zadd("fn_knock:test:zset", "high", 2)
            .await
            .expect("zadd high");
        assert_eq!(
            conn.zrevrange("fn_knock:test:zset", 0, 0)
                .await
                .expect("zrevrange"),
            vec!["high".to_string()]
        );

        let stream_id: String = cmd("XADD")
            .arg("fn_knock:test:stream")
            .arg("*")
            .arg("kind")
            .arg("created")
            .query_async(&mut conn)
            .await
            .expect("xadd");
        let stream_entries: Vec<(String, Vec<String>)> = cmd("XRANGE")
            .arg("fn_knock:test:stream")
            .arg("-")
            .arg("+")
            .query_async(&mut conn)
            .await
            .expect("xrange");
        assert_eq!(stream_entries.len(), 1);
        assert_eq!(stream_entries[0].0, stream_id);
        assert_eq!(
            stream_entries[0].1,
            vec!["kind".to_string(), "created".to_string()]
        );

        let (_, keys): (String, Vec<String>) = cmd("SCAN")
            .arg("0")
            .arg("MATCH")
            .arg("fn_knock:test:*")
            .arg("COUNT")
            .arg(100)
            .query_async(&mut conn)
            .await
            .expect("scan");
        assert!(keys.contains(&"fn_knock:test:hash".to_string()));
        assert!(keys.contains(&"fn_knock:test:stream".to_string()));
    }

    #[tokio::test]
    async fn pipeline_can_replace_prefix_atomically() {
        let mut conn = temp_manager().await;
        conn.set("fn_knock:old:a", "a").await.expect("set old a");
        conn.set("fn_knock:old:b", "b").await.expect("set old b");
        conn.set("other:old", "keep")
            .await
            .expect("set outside key");

        let mut restore = pipe();
        restore.set("fn_knock:new", "value").ignore();
        let (deleted, _): (usize, ()) = restore
            .query_async_replacing_prefix(&mut conn, "fn_knock:")
            .await
            .expect("replace prefix");

        assert_eq!(deleted, 2);
        let old: Option<String> = conn.get("fn_knock:old:a").await.expect("read old");
        let restored: Option<String> = conn.get("fn_knock:new").await.expect("read restored");
        let outside: Option<String> = conn.get("other:old").await.expect("read outside");
        assert_eq!(old, None);
        assert_eq!(restored.as_deref(), Some("value"));
        assert_eq!(outside.as_deref(), Some("keep"));
    }

    #[tokio::test]
    async fn xread_uses_stream_id_order_when_cursor_entry_was_deleted() {
        let mut conn = temp_manager().await;
        for id in ["1-0", "2-0", "3-0"] {
            let _: String = cmd("XADD")
                .arg("fn_knock:test:stream-deleted-cursor")
                .arg(id)
                .arg("value")
                .arg(id)
                .query_async(&mut conn)
                .await
                .expect("xadd");
        }
        let _: () = cmd("XDEL")
            .arg("fn_knock:test:stream-deleted-cursor")
            .arg("2-0")
            .query_async(&mut conn)
            .await
            .expect("xdel cursor row");

        let reply = conn
            .xread_options(
                &["fn_knock:test:stream-deleted-cursor"],
                &["2-0"],
                &streams::StreamReadOptions::default().count(10),
            )
            .await
            .expect("xread")
            .expect("reply");

        let ids = reply.keys[0]
            .ids
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["3-0"]);
    }

    #[tokio::test]
    async fn stream_entries_preserve_field_order_and_duplicates() {
        let mut conn = temp_manager().await;
        let _: String = cmd("XADD")
            .arg("fn_knock:test:stream-order")
            .arg("1-0")
            .arg("z")
            .arg("last")
            .arg("a")
            .arg("first")
            .arg("z")
            .arg("again")
            .query_async(&mut conn)
            .await
            .expect("xadd ordered fields");

        let entries: Vec<(String, Vec<String>)> = cmd("XRANGE")
            .arg("fn_knock:test:stream-order")
            .arg("-")
            .arg("+")
            .query_async(&mut conn)
            .await
            .expect("xrange ordered fields");

        assert_eq!(
            entries,
            vec![(
                "1-0".to_string(),
                vec![
                    "z".to_string(),
                    "last".to_string(),
                    "a".to_string(),
                    "first".to_string(),
                    "z".to_string(),
                    "again".to_string(),
                ],
            )]
        );
    }
}
