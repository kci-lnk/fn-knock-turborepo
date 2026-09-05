use super::*;

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
        Self::initialize_primary(&db, path).await?;
        let analytics_db = Connection::open_with_flags(
            path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .await?;
        let auth_read_db = Connection::open_with_flags(
            path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .await?;
        let health_db = Connection::open_with_flags(
            path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .await?;
        let manager = Self {
            db,
            analytics_db,
            auth_read_db,
            health_db,
            checkpoint_gate: Arc::new(RwLock::new(())),
            primary_admission: Arc::new(Semaphore::new(1)),
            analytics_admission: Arc::new(Semaphore::new(1)),
            auth_read_admission: Arc::new(Semaphore::new(1)),
            health_admission: Arc::new(Semaphore::new(1)),
            primary_metrics: Arc::new(PrimaryExecutorMetrics::default()),
            #[cfg(test)]
            path: path.to_path_buf(),
        };
        manager.initialize_readers().await?;
        secure_sqlite_file_permissions(path).await?;
        Ok(manager)
    }

    #[cfg(test)]
    pub(super) async fn initialize(&self) -> RedisResult<()> {
        Self::initialize_primary(&self.db, &self.path).await
    }

    async fn initialize_primary(db: &Connection, path: &Path) -> RedisResult<()> {
        let initialize_path = path.to_path_buf();
        db.call(move |conn| {
            conn.pragma_update(None, "journal_mode", "WAL")?;
            conn.pragma_update(None, "synchronous", "NORMAL")?;
            conn.pragma_update(None, "foreign_keys", "ON")?;
            conn.busy_timeout(std::time::Duration::from_secs(5))?;
            run_schema_migrations(conn, &initialize_path)?;
            Ok::<(), StorageError>(())
        })
        .await
        .map_err(StorageError::from)
    }

    async fn initialize_readers(&self) -> RedisResult<()> {
        for reader in [&self.analytics_db, &self.auth_read_db, &self.health_db] {
            reader
                .call(|conn| {
                    conn.pragma_update(None, "query_only", true)?;
                    conn.busy_timeout(std::time::Duration::from_millis(250))?;
                    Ok::<(), StorageError>(())
                })
                .await
                .map_err(StorageError::from)?;
        }
        Ok(())
    }

    pub(crate) async fn prepare_for_system_update(&self, backup_path: &Path) -> RedisResult<()> {
        // WAL truncation and VACUUM INTO must not race any isolated reader.
        // The exclusive guard is retained by the SQLite closure even if the
        // calling task is canceled after submission.
        let backup_path = backup_path.to_path_buf();
        self.call_exclusive(move |conn| {
            // Keep every write made between this preflight and process shutdown
            // durable even if the package manager has to terminate the service.
            conn.pragma_update(None, "synchronous", "FULL")?;
            let result = (|| {
                checkpoint_wal(conn, "TRUNCATE")?;
                verify_sqlite_integrity(conn)?;
                create_consistent_sqlite_backup(conn, &backup_path)?;
                Ok(())
            })();
            if let Err(error) = result {
                if let Err(restore_error) = conn.pragma_update(None, "synchronous", "NORMAL") {
                    return Err(storage_error(format!(
                        "{error}; failed to restore SQLite synchronous mode: {restore_error}"
                    )));
                }
                return Err(error);
            }
            Ok(())
        })
        .await
    }

    pub(crate) async fn cancel_system_update(&self) -> RedisResult<()> {
        self.call(|conn| {
            conn.pragma_update(None, "synchronous", "NORMAL")?;
            Ok(())
        })
        .await
    }

    pub(crate) async fn checkpoint_for_shutdown(&self) -> RedisResult<()> {
        self.call_exclusive(|conn| {
            checkpoint_wal(conn, "TRUNCATE")?;
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
            let cutoff = now_ms();
            let expired_typed_shadow_keys = {
                let mut statement = tx.prepare(
                    "SELECT key FROM kv_keys
                     WHERE expires_at_ms IS NOT NULL AND expires_at_ms <= ?1",
                )?;
                statement
                    .query_map(params![cutoff], |row| row.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()?
            };
            let deleted = tx.execute(
                "DELETE FROM kv_keys WHERE expires_at_ms IS NOT NULL AND expires_at_ms <= ?1",
                params![cutoff],
            )?;
            sync_typed_mobility_tx(
                &tx,
                TypedMobilitySyncScope::from_keys(expired_typed_shadow_keys),
            )?;
            tx.commit()?;
            Ok(deleted)
        })
        .await
    }

    pub(crate) async fn delete_security_state_atomically(
        &self,
        password_key: &str,
        session_prefix: &str,
        backoff_prefix: &str,
    ) -> RedisResult<(bool, usize, usize)> {
        let password_key = password_key.to_string();
        let session_pattern = format!("{}%", escape_like_pattern(session_prefix));
        let backoff_pattern = format!("{}%", escape_like_pattern(backoff_prefix));
        self.call(move |conn| {
            let tx = immediate_transaction(conn)?;
            purge_expired_all_tx(&tx)?;
            let password_exists = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM kv_keys WHERE key = ?1)",
                [&password_key],
                |row| row.get::<_, bool>(0),
            )?;
            let session_keys = keys_matching_pattern_tx(&tx, &session_pattern)?;
            let backoff_keys = keys_matching_pattern_tx(&tx, &backoff_pattern)?;
            tx.execute("DELETE FROM kv_keys WHERE key = ?1", [&password_key])?;
            tx.execute(
                "DELETE FROM kv_keys WHERE key LIKE ?1 ESCAPE '\\'",
                [&session_pattern],
            )?;
            tx.execute(
                "DELETE FROM kv_keys WHERE key LIKE ?1 ESCAPE '\\'",
                [&backoff_pattern],
            )?;
            let session_count = session_keys.len();
            let backoff_count = backoff_keys.len();
            sync_typed_mobility_tx(
                &tx,
                TypedMobilitySyncScope::from_keys(session_keys.into_iter().chain(backoff_keys)),
            )?;
            tx.commit()?;
            Ok((password_exists, session_count, backoff_count))
        })
        .await
    }

    pub(crate) async fn install_password_and_delete_security_state_atomically(
        &self,
        password_key: &str,
        password_json: &str,
        session_prefix: &str,
        backoff_prefix: &str,
    ) -> RedisResult<bool> {
        let password_key = password_key.to_string();
        let password_json = password_json.to_string();
        let session_pattern = format!("{}%", escape_like_pattern(session_prefix));
        let backoff_pattern = format!("{}%", escape_like_pattern(backoff_prefix));
        self.call(move |conn| {
            let tx = immediate_transaction(conn)?;
            purge_expired_tx(&tx, &password_key)?;
            if key_kind_tx(&tx, &password_key)?.is_some() {
                // A concurrent initializer won while the caller was hashing.
                // In particular, do not invalidate sessions it has created.
                return Ok(false);
            }
            purge_expired_all_tx(&tx)?;
            let session_keys = keys_matching_pattern_tx(&tx, &session_pattern)?;
            let backoff_keys = keys_matching_pattern_tx(&tx, &backoff_pattern)?;
            set_string_tx(&tx, &password_key, &password_json, None)?;
            tx.execute(
                "DELETE FROM kv_keys WHERE key LIKE ?1 ESCAPE '\\'",
                [&session_pattern],
            )?;
            tx.execute(
                "DELETE FROM kv_keys WHERE key LIKE ?1 ESCAPE '\\'",
                [&backoff_pattern],
            )?;
            sync_typed_mobility_tx(
                &tx,
                TypedMobilitySyncScope::from_keys(session_keys.into_iter().chain(backoff_keys)),
            )?;
            tx.commit()?;
            Ok(true)
        })
        .await
    }

    pub(crate) async fn call<T, F>(&self, f: F) -> RedisResult<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut rusqlite::Connection) -> RedisResult<T> + Send + 'static,
    {
        // Keep cancellation outside tokio-rusqlite's internal queue. Once a
        // closure is submitted it cannot be recalled by dropping the caller's
        // future; moving both guards into the closure ensures a timed-out auth
        // request cannot release admission until that submitted work is truly
        // finished. Waiters canceled before admission are never submitted.
        let admission = self.primary_admission.clone();
        let (permit, wait_ms) = match admission.clone().try_acquire_owned() {
            Ok(permit) => (permit, 0),
            Err(TryAcquireError::NoPermits) => {
                let waiter = self.primary_metrics.begin_wait();
                let permit = admission
                    .acquire_owned()
                    .await
                    .map_err(|_| storage_error("primary sqlite executor admission is closed"))?;
                (permit, waiter.admit())
            }
            Err(TryAcquireError::Closed) => {
                return Err(storage_error("primary sqlite executor admission is closed"));
            }
        };
        let execution = self.primary_metrics.begin_execution(wait_ms);
        self.db
            .call(move |conn| {
                let _permit = permit;
                let _execution = execution;
                f(conn)
            })
            .await
            .map_err(StorageError::from)
    }

    pub(super) async fn call_exclusive<T, F>(&self, f: F) -> RedisResult<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut rusqlite::Connection) -> RedisResult<T> + Send + 'static,
    {
        let checkpoint_guard = self.checkpoint_gate.clone().write_owned().await;
        self.call(move |conn| {
            let _checkpoint_guard = checkpoint_guard;
            f(conn)
        })
        .await
    }

    async fn call_reader<T, F>(
        &self,
        reader: &Connection,
        admission: Arc<Semaphore>,
        f: F,
    ) -> RedisResult<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut rusqlite::Connection) -> RedisResult<T> + Send + 'static,
    {
        // tokio-rusqlite cannot retract a closure once submitted. Admission
        // stays outside its internal queue, while both guards move into the
        // closure so caller cancellation cannot release them prematurely.
        let permit = admission
            .acquire_owned()
            .await
            .map_err(|_| storage_error("sqlite reader admission is closed"))?;
        let checkpoint_guard = self.checkpoint_gate.clone().read_owned().await;
        reader
            .call(move |conn| {
                let _permit = permit;
                let _checkpoint_guard = checkpoint_guard;
                f(conn)
            })
            .await
            .map_err(StorageError::from)
    }

    pub(super) async fn call_analytics<T, F>(&self, f: F) -> RedisResult<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut rusqlite::Connection) -> RedisResult<T> + Send + 'static,
    {
        self.call_reader(&self.analytics_db, self.analytics_admission.clone(), f)
            .await
    }

    pub(crate) async fn call_auth_read<T, F>(&self, f: F) -> RedisResult<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut rusqlite::Connection) -> RedisResult<T> + Send + 'static,
    {
        self.call_reader(&self.auth_read_db, self.auth_read_admission.clone(), f)
            .await
    }

    pub(crate) async fn ping(&self) -> RedisResult<()> {
        self.call_reader(&self.health_db, self.health_admission.clone(), |conn| {
            conn.query_row("SELECT 1", [], |row| row.get::<_, i64>(0))?;
            Ok::<(), StorageError>(())
        })
        .await
    }

    pub(crate) async fn get_live_string_auth(
        &self,
        key: String,
        now_ms: i64,
    ) -> RedisResult<Option<String>> {
        self.call_auth_read(move |conn| {
            conn.query_row(
                "SELECT strings.value
                 FROM kv_strings AS strings
                 JOIN kv_keys AS keys ON keys.key = strings.key
                 WHERE strings.key = ?1
                   AND (keys.expires_at_ms IS NULL OR keys.expires_at_ms > ?2)",
                params![key, now_ms],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(Into::into)
        })
        .await
    }

    /// Read cache documents without TTL cleanup or primary-writer admission.
    /// Each submitted closure handles at most 256 keys and observes a single
    /// read transaction. Cancellation can stop between batches.
    pub(crate) async fn get_live_strings_analytics(
        &self,
        keys: Vec<String>,
    ) -> RedisResult<Vec<Option<String>>> {
        const BATCH_SIZE: usize = 256;
        let mut values = Vec::with_capacity(keys.len());
        let mut keys = keys.into_iter();
        loop {
            let batch = keys.by_ref().take(BATCH_SIZE).collect::<Vec<_>>();
            if batch.is_empty() {
                break;
            }
            let result = self
                .call_analytics(move |conn| {
                    let tx = conn.transaction()?;
                    // Admission can wait behind a long analytics query. TTLs
                    // must be evaluated when this batch actually executes.
                    let now_ms = crate::time_utils::now_ms();
                    let values = {
                        let mut statement = tx.prepare_cached(
                            "SELECT strings.value
                             FROM kv_strings AS strings
                             JOIN kv_keys AS keys ON keys.key = strings.key
                             WHERE strings.key = ?1 AND keys.kind = 'string'
                               AND (keys.expires_at_ms IS NULL OR keys.expires_at_ms > ?2)",
                        )?;
                        batch
                            .iter()
                            .map(|key| {
                                statement
                                    .query_row(params![key, now_ms], |row| row.get::<_, String>(0))
                                    .optional()
                            })
                            .collect::<Result<Vec<_>, _>>()?
                    };
                    tx.commit()?;
                    Ok(values)
                })
                .await?;
            values.extend(result);
        }
        Ok(values)
    }

    pub(crate) fn primary_queue_status(&self) -> PrimaryQueueStatus {
        self.primary_metrics.status()
    }
}
