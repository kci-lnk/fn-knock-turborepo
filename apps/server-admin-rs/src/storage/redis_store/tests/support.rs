use super::*;
use std::path::Path;
use tempfile::TempDir;
use tokio_rusqlite::OptionalExtension;
use tokio_rusqlite::rusqlite::Connection;

pub(super) async fn open_test_store() -> (TempDir, Store) {
    let directory = tempfile::tempdir().expect("create temp dir");
    let store = Store::connect(directory.path().join("fn-knock.sqlite3"))
        .await
        .expect("open store");
    (directory, store)
}

pub(super) fn open_fixture_connection(path: impl AsRef<Path>) -> Connection {
    Connection::open(path).expect("open fixture SQLite connection")
}

pub(super) fn install_failure_trigger(path: impl AsRef<Path>, statement: &str) -> Connection {
    let connection = open_fixture_connection(path);
    connection
        .execute_batch(statement)
        .expect("install typed failure trigger");
    connection
}

pub(super) async fn sqlite_key_expiry_at_ms(path: &Path, key: &str) -> Option<i64> {
    let connection = tokio_rusqlite::Connection::open(path)
        .await
        .expect("open expiry observer");
    let key = key.to_string();
    connection
        .call(move |connection| {
            connection
                .query_row(
                    "SELECT expires_at_ms FROM kv_keys WHERE key = ?1",
                    [&key],
                    |row| row.get::<_, Option<i64>>(0),
                )
                .optional()
        })
        .await
        .expect("query expiry")
        .flatten()
}
