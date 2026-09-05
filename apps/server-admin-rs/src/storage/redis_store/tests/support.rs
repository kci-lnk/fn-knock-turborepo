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

pub(super) async fn block_primary_executor(
    store: &Store,
) -> (std::sync::mpsc::Sender<()>, tokio::task::JoinHandle<()>) {
    let manager = store.manager.clone();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let task = tokio::spawn(async move {
        manager
            .call(move |_| {
                let _ = started_tx.send(());
                release_rx
                    .recv_timeout(std::time::Duration::from_secs(5))
                    .map_err(|error| crate::storage::storage_error(error.to_string()))?;
                Ok(())
            })
            .await
            .expect("primary blocker");
    });
    started_rx.await.expect("primary started");
    (release_tx, task)
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
