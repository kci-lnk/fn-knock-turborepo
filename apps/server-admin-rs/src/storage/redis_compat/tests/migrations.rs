use super::*;

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
async fn system_update_preflight_replaces_a_verified_durable_snapshot() {
    let directory = tempfile::tempdir().expect("create temp dir");
    let path = directory.path().join("fn-knock.sqlite3");
    let backup_path = directory.path().join("fn-knock.sqlite3.pre-update.bak");
    let manager = ConnectionManager::open(&path).await.expect("open sqlite");
    manager
        .set_meta_value("update-snapshot-test", "before")
        .await
        .expect("seed snapshot value");

    manager
        .prepare_for_system_update(&backup_path)
        .await
        .expect("prepare first update snapshot");
    assert_eq!(read_backup_meta(&backup_path), "before");

    assert_eq!(synchronous_mode(&manager).await, 2);

    manager
        .set_meta_value("update-snapshot-test", "after")
        .await
        .expect("update snapshot value");
    manager
        .prepare_for_system_update(&backup_path)
        .await
        .expect("replace update snapshot");
    assert_eq!(read_backup_meta(&backup_path), "after");
    manager
        .checkpoint_for_shutdown()
        .await
        .expect("checkpoint shutdown WAL");
    manager
        .cancel_system_update()
        .await
        .expect("restore normal sync mode");
    assert_eq!(synchronous_mode(&manager).await, 1);
}

#[tokio::test]
async fn failed_system_update_preflight_restores_normal_sync_mode() {
    let directory = tempfile::tempdir().expect("create temp dir");
    let path = directory.path().join("fn-knock.sqlite3");
    let invalid_backup = directory.path().join("invalid-backup");
    let mut invalid_temporary_name = invalid_backup.as_os_str().to_os_string();
    invalid_temporary_name.push(".tmp");
    std::fs::create_dir(PathBuf::from(invalid_temporary_name))
        .expect("create invalid temporary directory");
    let manager = ConnectionManager::open(&path).await.expect("open sqlite");

    manager
        .prepare_for_system_update(&invalid_backup)
        .await
        .expect_err("invalid backup destination must fail");
    assert_eq!(synchronous_mode(&manager).await, 1);
}

#[tokio::test]
async fn wal_checkpoint_waits_for_an_active_analytics_reader() {
    let manager = temp_manager().await;
    let reader_manager = manager.clone();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let reader = tokio::spawn(async move {
        reader_manager
            .call_analytics(move |_conn| {
                let _ = started_tx.send(());
                release_rx
                    .recv_timeout(std::time::Duration::from_secs(5))
                    .map_err(|error| storage_error(format!("release analytics reader: {error}")))?;
                Ok(())
            })
            .await
    });
    started_rx.await.expect("analytics reader started");

    let checkpoint_manager = manager.clone();
    let mut checkpoint =
        tokio::spawn(async move { checkpoint_manager.checkpoint_for_shutdown().await });
    let premature =
        tokio::time::timeout(std::time::Duration::from_millis(50), &mut checkpoint).await;
    release_tx.send(()).expect("release analytics reader");
    reader
        .await
        .expect("analytics reader task")
        .expect("analytics reader result");

    assert!(premature.is_err(), "checkpoint bypassed analytics gate");
    tokio::time::timeout(std::time::Duration::from_secs(5), checkpoint)
        .await
        .expect("checkpoint completed after reader")
        .expect("checkpoint task")
        .expect("checkpoint result");
}

#[tokio::test]
async fn health_probe_does_not_wait_for_the_primary_executor() {
    let manager = temp_manager().await;
    let blocker_manager = manager.clone();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let blocker = blocker_manager.call(move |_conn| {
        let _ = started_tx.send(());
        release_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .map_err(|error| storage_error(format!("release primary blocker: {error}")))?;
        Ok(())
    });
    let probe = async {
        started_rx.await.expect("primary executor started");
        tokio::time::timeout(std::time::Duration::from_millis(250), manager.ping())
            .await
            .expect("health reader must not queue behind primary work")
            .expect("health query succeeds");
        release_tx.send(()).expect("release primary executor");
    };
    let (blocker_result, ()) = tokio::join!(blocker, probe);
    blocker_result.expect("primary blocker result");
}

#[tokio::test]
async fn canceled_primary_waiter_is_never_submitted_to_sqlite() {
    let manager = temp_manager().await;
    let blocker_manager = manager.clone();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let blocker = blocker_manager.call(move |_conn| {
        let _ = started_tx.send(());
        release_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .map_err(|error| storage_error(format!("release primary blocker: {error}")))?;
        Ok(())
    });
    let executed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let executed_in_call = executed.clone();
    let cancellation = async {
        started_rx.await.expect("primary executor started");
        let canceled = tokio::time::timeout(
            std::time::Duration::from_millis(50),
            manager.call(move |_conn| {
                executed_in_call.store(true, std::sync::atomic::Ordering::Release);
                Ok(())
            }),
        )
        .await;
        assert!(canceled.is_err(), "queued call unexpectedly completed");
        assert_eq!(manager.primary_queue_status().queue_depth, 0);
        assert_eq!(manager.primary_queue_status().canceled_operations, 1);
        release_tx.send(()).expect("release primary executor");
    };
    let (blocker_result, ()) = tokio::join!(blocker, cancellation);
    blocker_result.expect("primary blocker result");
    assert!(
        !executed.load(std::sync::atomic::Ordering::Acquire),
        "canceled waiter was submitted after its caller disappeared"
    );
}

#[tokio::test]
async fn canceled_auth_reader_waiter_is_never_submitted_to_sqlite() {
    let manager = temp_manager().await;
    let blocker_manager = manager.clone();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let blocker = blocker_manager.call_auth_read(move |_conn| {
        let _ = started_tx.send(());
        release_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .map_err(|error| storage_error(format!("release auth reader: {error}")))?;
        Ok(())
    });
    let executed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let executed_in_call = executed.clone();
    let cancellation = async {
        started_rx.await.expect("auth reader started");
        let canceled = tokio::time::timeout(
            std::time::Duration::from_millis(50),
            manager.call_auth_read(move |_conn| {
                executed_in_call.store(true, std::sync::atomic::Ordering::Release);
                Ok(())
            }),
        )
        .await;
        assert!(canceled.is_err(), "queued auth read unexpectedly completed");
        release_tx.send(()).expect("release auth reader");
    };
    let (blocker_result, ()) = tokio::join!(blocker, cancellation);
    blocker_result.expect("auth reader blocker result");
    assert!(
        !executed.load(std::sync::atomic::Ordering::Acquire),
        "canceled auth read was submitted after its caller disappeared"
    );
}

#[tokio::test]
async fn canceled_exclusive_call_retains_checkpoint_gate_until_sqlite_finishes() {
    let manager = temp_manager().await;
    let exclusive_manager = manager.clone();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let exclusive = tokio::time::timeout(
        std::time::Duration::from_millis(50),
        exclusive_manager.call_exclusive(move |_conn| {
            let _ = started_tx.send(());
            release_rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .map_err(|error| storage_error(format!("release exclusive call: {error}")))?;
            Ok(())
        }),
    );
    let wait_until_submitted = async {
        started_rx.await.expect("exclusive call started");
    };
    let (canceled, ()) = tokio::join!(exclusive, wait_until_submitted);
    assert!(canceled.is_err(), "exclusive caller unexpectedly completed");

    let executed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let executed_in_call = executed.clone();
    let blocked_reader = tokio::time::timeout(
        std::time::Duration::from_millis(50),
        manager.call_analytics(move |_conn| {
            executed_in_call.store(true, std::sync::atomic::Ordering::Release);
            Ok(())
        }),
    )
    .await;
    assert!(
        blocked_reader.is_err(),
        "reader bypassed canceled exclusive call"
    );
    assert!(!executed.load(std::sync::atomic::Ordering::Acquire));

    release_tx.send(()).expect("release exclusive SQLite call");
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        manager.call_analytics(|_conn| Ok(())),
    )
    .await
    .expect("checkpoint gate released after SQLite completion")
    .expect("analytics reader succeeds after exclusive completion");
}

#[tokio::test]
async fn destructive_migration_backup_is_a_verified_sqlite_snapshot() {
    let directory = tempfile::tempdir().expect("create temp dir");
    let path = directory.path().join("fn-knock.sqlite3");
    let manager = ConnectionManager::open(&path).await.expect("open sqlite");
    manager
        .set_meta_value("update-snapshot-test", "migration")
        .await
        .expect("seed migration snapshot value");
    let source_path = path.clone();
    let backup_path = manager
        .call(move |conn| {
            create_migration_backup(conn, &source_path, &SCHEMA_MIGRATIONS[1])?
                .ok_or_else(|| storage_error("migration backup was not created"))
        })
        .await
        .expect("create migration snapshot");

    assert_eq!(read_backup_meta(&backup_path), "migration");
}

async fn synchronous_mode(manager: &ConnectionManager) -> i64 {
    manager
        .call(|conn| {
            conn.query_row("PRAGMA synchronous", [], |row| row.get::<_, i64>(0))
                .map_err(Into::into)
        })
        .await
        .expect("read synchronous mode")
}

fn read_backup_meta(path: &Path) -> String {
    let conn =
        rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .expect("open update snapshot");
    verify_sqlite_integrity(&conn).expect("snapshot integrity");
    conn.query_row(
        "SELECT value FROM storage_meta WHERE key = 'update-snapshot-test'",
        [],
        |row| row.get(0),
    )
    .expect("read snapshot value")
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
