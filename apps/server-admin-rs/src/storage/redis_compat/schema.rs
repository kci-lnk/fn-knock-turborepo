pub(super) struct SchemaMigration {
    pub(super) version: i64,
    pub(super) name: &'static str,
    pub(super) sql: &'static str,
    pub(super) destructive: bool,
}

pub(super) const SCHEMA_MIGRATIONS_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS schema_migrations (
  version INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  checksum TEXT NOT NULL,
  applied_at_ms INTEGER NOT NULL
);
"#;

pub(super) const REDIS_COMPATIBLE_KEYSPACE_SQL: &str = r#"
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

pub(super) const REDIS_COMPATIBLE_STREAM_METADATA_SQL: &str = r#"
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

pub(super) const SCHEMA_MIGRATIONS: &[SchemaMigration] = &[
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
