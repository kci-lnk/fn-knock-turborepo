use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use tokio::sync::{Mutex, Notify, Semaphore};

pub struct PanelSyncRuntime {
    pub config_lock: Mutex<()>,
    connection_locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
    pub source_changed: Notify,
    pub runs_invalidated: Notify,
    pub concurrency: Semaphore,
    generation: AtomicU64,
}

impl Default for PanelSyncRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl PanelSyncRuntime {
    pub fn new() -> Self {
        Self {
            config_lock: Mutex::new(()),
            connection_locks: Mutex::new(HashMap::new()),
            source_changed: Notify::new(),
            runs_invalidated: Notify::new(),
            concurrency: Semaphore::new(2),
            generation: AtomicU64::new(0),
        }
    }

    pub async fn connection_lock(&self, id: &str) -> Arc<Mutex<()>> {
        self.connection_locks
            .lock()
            .await
            .entry(id.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    pub async fn forget_connection(&self, id: &str) {
        self.connection_locks.lock().await.remove(id);
    }

    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    pub fn invalidate_runs(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.runs_invalidated.notify_waiters();
    }
}
