use super::*;

pub(super) fn run_schema_migrations(
    conn: &mut rusqlite::Connection,
    path: &Path,
) -> RedisResult<()> {
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

pub(super) fn run_schema_migration(
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

pub(super) fn is_legacy_bootstrap_migration(
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

pub(super) fn sqlite_table_exists(conn: &rusqlite::Connection, name: &str) -> RedisResult<bool> {
    let exists = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        params![name],
        |row| row.get::<_, i64>(0),
    )?;
    Ok(exists == 1)
}

pub(super) fn checkpoint_wal(conn: &rusqlite::Connection, mode: &str) -> RedisResult<()> {
    let sql = format!("PRAGMA wal_checkpoint({mode})");
    let (busy, log_frames, checkpointed_frames) = conn.query_row(&sql, [], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;
    if busy != 0 {
        return Err(storage_error(format!(
            "SQLite WAL checkpoint remained busy ({checkpointed_frames}/{log_frames} frames)"
        )));
    }
    Ok(())
}

pub(super) fn verify_sqlite_integrity(conn: &rusqlite::Connection) -> RedisResult<()> {
    let mut statement = conn.prepare("PRAGMA quick_check")?;
    let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
    let results = rows.collect::<Result<Vec<_>, _>>()?;
    if results.len() == 1 && results[0].eq_ignore_ascii_case("ok") {
        return Ok(());
    }
    Err(storage_error(format!(
        "SQLite integrity check failed: {}",
        results.join("; ")
    )))
}

pub(super) fn create_consistent_sqlite_backup(
    conn: &rusqlite::Connection,
    backup_path: &Path,
) -> RedisResult<()> {
    let parent = backup_path
        .parent()
        .ok_or_else(|| storage_error("SQLite update backup path has no parent directory"))?;
    std::fs::create_dir_all(parent)?;

    let mut temporary_name = backup_path.as_os_str().to_os_string();
    temporary_name.push(".tmp");
    let temporary_path = PathBuf::from(temporary_name);
    remove_file_if_exists(&temporary_path)?;

    let temporary_path_text = temporary_path
        .to_str()
        .ok_or_else(|| storage_error("SQLite update backup path is not valid UTF-8"))?
        .to_string();
    conn.execute("VACUUM INTO ?1", params![temporary_path_text])?;

    let verification = rusqlite::Connection::open_with_flags(
        &temporary_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    verify_sqlite_integrity(&verification)?;
    verification.close().map_err(|(_, error)| error)?;
    #[cfg(unix)]
    std::fs::set_permissions(&temporary_path, std::fs::Permissions::from_mode(0o600))?;
    sync_file(&temporary_path)?;
    let previous_path = replace_backup_file(&temporary_path, backup_path)?;
    sync_file(backup_path)?;
    sync_directory(parent)?;
    if let Some(previous_path) = previous_path {
        remove_file_if_exists(&previous_path)?;
        sync_directory(parent)?;
    }
    Ok(())
}

pub(super) fn replace_backup_file(
    temporary_path: &Path,
    backup_path: &Path,
) -> RedisResult<Option<PathBuf>> {
    if !backup_path.exists() {
        std::fs::rename(temporary_path, backup_path)?;
        return Ok(None);
    }

    let mut previous_name = backup_path.as_os_str().to_os_string();
    previous_name.push(".previous");
    let previous_path = PathBuf::from(previous_name);
    remove_file_if_exists(&previous_path)?;
    std::fs::rename(backup_path, &previous_path)?;
    if let Err(error) = std::fs::rename(temporary_path, backup_path) {
        if let Err(restore_error) = std::fs::rename(&previous_path, backup_path) {
            return Err(storage_error(format!(
                "failed to install SQLite backup: {error}; failed to restore previous backup: {restore_error}"
            )));
        }
        return Err(error.into());
    }
    Ok(Some(previous_path))
}

pub(super) fn remove_file_if_exists(path: &Path) -> RedisResult<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

pub(super) fn sync_file(path: &Path) -> RedisResult<()> {
    std::fs::OpenOptions::new()
        .read(true)
        .open(path)?
        .sync_all()?;
    Ok(())
}

#[cfg(unix)]
pub(super) fn sync_directory(path: &Path) -> RedisResult<()> {
    std::fs::File::open(path)?.sync_all()?;
    Ok(())
}

#[cfg(not(unix))]
pub(super) fn sync_directory(_path: &Path) -> RedisResult<()> {
    Ok(())
}

pub(super) fn create_migration_backup(
    conn: &mut rusqlite::Connection,
    path: &Path,
    migration: &SchemaMigration,
) -> RedisResult<Option<PathBuf>> {
    if !path.exists() {
        return Ok(None);
    }
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("fn-knock.sqlite3");
    let backup_path = path.with_file_name(format!(
        "{file_name}.migration-v{}.{}.bak",
        migration.version,
        now_ms()
    ));
    create_consistent_sqlite_backup(conn, &backup_path)?;
    Ok(Some(backup_path))
}

pub(super) fn migration_checksum(sql: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(sql.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}
