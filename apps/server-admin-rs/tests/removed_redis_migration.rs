use std::{fs, process::Command};

use tokio_rusqlite::rusqlite::Connection;

#[test]
fn removed_migration_command_rejects_all_variants_before_opening_storage() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("existing.sqlite3");
    let data_dir = directory.path().join("runtime");
    {
        let connection = Connection::open(&database).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE storage_meta(key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 INSERT INTO storage_meta VALUES ('redis_migration_status', 'unavailable');
                 CREATE TABLE kv_strings(key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 INSERT INTO kv_strings VALUES ('fn_knock:config', '{\"host_mappings\":[{\"host\":\"app.example.test\"}]}');",
            )
            .unwrap();
    }
    let before = fs::read(&database).unwrap();
    for extra_args in [vec![], vec!["--force"], vec!["--help"]] {
        let output = Command::new(env!("CARGO_BIN_EXE_server-admin-rs"))
            .arg("migrate-redis-to-sqlite")
            .args(&extra_args)
            .env("FN_KNOCK_DATA_DIR", &data_dir)
            .env("FN_KNOCK_SQLITE_PATH", &database)
            .env("FN_KNOCK_RUNTIME_TARGET", "fpk")
            .env("FN_KNOCK_LEGACY_REDIS_URL", "redis://127.0.0.1:1/")
            .output()
            .unwrap();
        assert!(!output.status.success(), "args={extra_args:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("unknown command: migrate-redis-to-sqlite"),
            "args={extra_args:?}, stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(fs::read(&database).unwrap(), before);
        assert!(!data_dir.exists());
        assert!(!directory.path().join("existing.sqlite3-wal").exists());
        assert!(!directory.path().join("existing.sqlite3-shm").exists());
    }
}

#[test]
fn cli_help_lists_supported_commands_without_migration() {
    let output = Command::new(env!("CARGO_BIN_EXE_server-admin-rs"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success());
    let help = String::from_utf8_lossy(&output.stdout);
    assert!(help.contains("reset-panel-password"));
    assert!(!help.contains("migrate-redis-to-sqlite"));
}
