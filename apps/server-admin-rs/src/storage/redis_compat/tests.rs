use super::*;

async fn temp_manager() -> ConnectionManager {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fn-knock.sqlite3");
    let manager = ConnectionManager::open(&path).await.expect("open sqlite");
    std::mem::forget(dir);
    manager
}

mod collections;
mod migrations;
mod stream_commands;
mod streams_tail;
mod transactions;
