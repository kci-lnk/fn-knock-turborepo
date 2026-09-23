use super::*;
use std::sync::Mutex;
use tokio::sync::OwnedSemaphorePermit;

/// Optional experiment pool. The default remains the original single reader.
/// Leases live in submitted SQLite closures, never in the cancellable caller.
pub(super) struct AuthReadPool {
    readers: Vec<Connection>,
    available: Mutex<Vec<usize>>,
    admission: Arc<Semaphore>,
}

struct Lease {
    pool: Arc<AuthReadPool>,
    index: usize,
    _permit: OwnedSemaphorePermit,
}

impl Drop for Lease {
    fn drop(&mut self) {
        self.pool
            .available
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(self.index);
    }
}

impl AuthReadPool {
    pub(super) async fn open(path: &Path, count: usize, first: Connection) -> RedisResult<Self> {
        let mut readers = Vec::with_capacity(count);
        readers.push(first);
        for _ in 1..count {
            let reader = Connection::open_with_flags(
                path,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
                    | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )
            .await?;
            reader
                .call(|conn| {
                    conn.pragma_update(None, "query_only", true)?;
                    conn.busy_timeout(std::time::Duration::from_millis(250))?;
                    Ok::<_, StorageError>(())
                })
                .await?;
            readers.push(reader);
        }
        Ok(Self {
            readers,
            available: Mutex::new((0..count).collect()),
            admission: Arc::new(Semaphore::new(count)),
        })
    }

    pub(super) async fn call<T, F>(
        self: &Arc<Self>,
        checkpoint_gate: Arc<RwLock<()>>,
        recorder: Arc<crate::runtime_health::operations::OperationRecorder>,
        label: &'static str,
        f: F,
    ) -> RedisResult<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut rusqlite::Connection) -> RedisResult<T> + Send + 'static,
    {
        let phase = crate::auth::diagnostics::enter("sqlite_reader_wait");
        let waiting = recorder.scope("sqlite_admission", "sqlite_auth_read");
        let permit = self
            .admission
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| storage_error("auth reader pool is closed"))?;
        let index = self
            .available
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pop()
            .ok_or_else(|| storage_error("auth reader pool has no available lease"))?;
        let lease = Lease {
            pool: self.clone(),
            index,
            _permit: permit,
        };
        waiting.finish(true, None);
        drop(phase);
        let phase = crate::auth::diagnostics::enter("sqlite_checkpoint_wait");
        let checkpoint_guard = checkpoint_gate.read_owned().await;
        drop(phase);
        let _phase = crate::auth::diagnostics::enter("sqlite_reader_execute");
        self.readers[index]
            .call(move |conn| {
                let _lease = lease;
                let _checkpoint_guard = checkpoint_guard;
                let operation = recorder.scope_sqlite("sqlite_auth_read", label);
                let result = f(conn);
                operation.finish(result.is_ok(), None);
                result
            })
            .await
            .map_err(StorageError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[tokio::test]
    async fn pooled_readers_retain_admission_and_checkpoint_guards_after_cancellation() {
        for count in [2, 4] {
            let dir = tempfile::tempdir().unwrap();
            let manager =
                ConnectionManager::open_with_auth_readers(&dir.path().join("pool.sqlite3"), count)
                    .await
                    .unwrap();
            let mut jobs = Vec::new();
            let mut releases = Vec::new();
            for _ in 0..count {
                let reader = manager.clone();
                let (started_tx, started_rx) = tokio::sync::oneshot::channel();
                let (release_tx, release_rx) = std::sync::mpsc::channel();
                jobs.push(tokio::spawn(async move {
                    reader
                        .call_auth_read(move |conn| {
                            // Every slot must really be read-only.
                            assert!(
                                conn.execute("INSERT INTO storage_meta VALUES ('x','y',0)", [])
                                    .is_err()
                            );
                            let _ = started_tx.send(());
                            release_rx
                                .recv_timeout(std::time::Duration::from_secs(5))
                                .unwrap();
                            Ok(())
                        })
                        .await
                }));
                started_rx.await.unwrap();
                releases.push(release_tx);
            }
            jobs[0].abort();
            tokio::task::yield_now().await;
            let executed = Arc::new(AtomicBool::new(false));
            let in_call = executed.clone();
            assert!(
                tokio::time::timeout(
                    std::time::Duration::from_millis(30),
                    manager.call_auth_read(move |_| {
                        in_call.store(true, Ordering::Release);
                        Ok(())
                    })
                )
                .await
                .is_err()
            );
            assert!(
                tokio::time::timeout(
                    std::time::Duration::from_millis(30),
                    manager.call_exclusive(|_| Ok(()))
                )
                .await
                .is_err()
            );
            for release in releases {
                release.send(()).unwrap();
            }
            for job in jobs {
                match job.await {
                    Ok(result) => result.unwrap(),
                    Err(error) => assert!(error.is_cancelled()),
                }
            }
            manager
                .call_auth_read(|conn| {
                    conn.query_row("SELECT 1", [], |row| row.get::<_, i64>(0))
                        .map_err(Into::into)
                })
                .await
                .unwrap();
            manager.call_exclusive(|_| Ok(())).await.unwrap();
            assert!(
                !executed.load(Ordering::Acquire),
                "canceled waiter reached SQLite"
            );
        }
    }
}
