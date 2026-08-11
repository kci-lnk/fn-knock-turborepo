use std::{
    collections::HashMap,
    future::Future,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use tokio::{sync::oneshot, task::AbortHandle};
use tokio_util::task::TaskTracker;

#[derive(Clone, Default)]
pub(crate) struct BackgroundTaskRegistry {
    inner: Arc<RegistryInner>,
}

#[derive(Default)]
struct RegistryInner {
    next_id: AtomicU64,
    state: Mutex<RegistryState>,
    tracker: TaskTracker,
}

#[derive(Default)]
struct RegistryState {
    closed: bool,
    tasks: HashMap<u64, TaskRecord>,
}

struct TaskRecord {
    name: &'static str,
    abort: AbortHandle,
}

struct RegistrationGuard {
    id: u64,
    inner: Arc<RegistryInner>,
}

impl Drop for RegistrationGuard {
    fn drop(&mut self) {
        self.inner
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .tasks
            .remove(&self.id);
    }
}

impl BackgroundTaskRegistry {
    pub(crate) fn spawn<F>(&self, name: &'static str, future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let _ = self.spawn_abortable(name, future);
    }

    pub(crate) fn spawn_abortable<F>(&self, name: &'static str, future: F) -> Option<AbortHandle>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        // The closed check, TaskTracker registration and task-record insert
        // share one lock with shutdown. This prevents shutdown from observing
        // an empty tracker and returning while a concurrent spawn is between
        // its closed check and tracker registration.
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if state.closed {
            tracing::warn!(
                task = name,
                "refused to start background task during shutdown"
            );
            return None;
        }

        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        let (start_tx, start_rx) = oneshot::channel();
        let guard = RegistrationGuard {
            id,
            inner: self.inner.clone(),
        };
        let handle = self.inner.tracker.spawn(async move {
            if start_rx.await.is_err() {
                return;
            }
            let _registration = guard;
            tracing::debug!(task = name, "background task started");
            future.await;
            tracing::debug!(task = name, "background task stopped");
        });
        let abort = handle.abort_handle();
        state.tasks.insert(
            id,
            TaskRecord {
                name,
                abort: abort.clone(),
            },
        );
        drop(state);
        let _ = start_tx.send(());
        Some(abort)
    }

    pub(crate) async fn shutdown(&self, deadline: Duration) -> Vec<&'static str> {
        {
            let mut state = self
                .inner
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            state.closed = true;
            self.inner.tracker.close();
        }
        if tokio::time::timeout(deadline, self.inner.tracker.wait())
            .await
            .is_ok()
        {
            return Vec::new();
        }

        let names = {
            let state = self
                .inner
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let mut names = state
                .tasks
                .values()
                .map(|task| task.name)
                .collect::<Vec<_>>();
            names.sort_unstable();
            names.dedup();
            for task in state.tasks.values() {
                task.abort.abort();
            }
            names
        };
        tracing::warn!(tasks = ?names, "background tasks exceeded shutdown deadline and were aborted");
        let _ = tokio::time::timeout(Duration::from_secs(1), self.inner.tracker.wait()).await;
        names
    }

    #[cfg(test)]
    fn active_count(&self) -> usize {
        self.inner
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .tasks
            .len()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;

    use super::*;

    #[tokio::test]
    async fn waits_for_cooperative_tasks() {
        let registry = BackgroundTaskRegistry::default();
        registry.spawn("short-task", async {});
        tokio::task::yield_now().await;
        let timed_out = registry.shutdown(Duration::from_secs(1)).await;
        assert!(timed_out.is_empty());
        assert_eq!(registry.active_count(), 0);
    }

    #[tokio::test]
    async fn abortable_tasks_remain_registered_until_their_handle_cancels_them() {
        let registry = BackgroundTaskRegistry::default();
        let abort = registry
            .spawn_abortable("replaceable-task", std::future::pending::<()>())
            .expect("registry accepts task before shutdown");
        tokio::task::yield_now().await;
        assert_eq!(registry.active_count(), 1);

        abort.abort();
        for _ in 0..10 {
            if registry.active_count() == 0 {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert_eq!(registry.active_count(), 0);
        assert!(registry.shutdown(Duration::from_secs(1)).await.is_empty());
    }

    #[tokio::test]
    async fn aborts_and_reports_tasks_that_miss_the_deadline() {
        struct DropMarker(Arc<AtomicBool>);
        impl Drop for DropMarker {
            fn drop(&mut self) {
                self.0.store(true, Ordering::Release);
            }
        }

        let registry = BackgroundTaskRegistry::default();
        let dropped = Arc::new(AtomicBool::new(false));
        let marker = DropMarker(dropped.clone());
        registry.spawn("stuck-task", async move {
            let _marker = marker;
            std::future::pending::<()>().await;
        });
        tokio::task::yield_now().await;

        let timed_out = registry.shutdown(Duration::from_millis(10)).await;
        assert_eq!(timed_out, vec!["stuck-task"]);
        assert!(dropped.load(Ordering::Acquire));
        assert_eq!(registry.active_count(), 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_spawns_cannot_escape_shutdown() {
        use tokio::sync::Barrier;

        const SPAWNERS: usize = 64;
        let registry = BackgroundTaskRegistry::default();
        let barrier = Arc::new(Barrier::new(SPAWNERS + 1));
        let mut spawners = Vec::with_capacity(SPAWNERS);

        for _ in 0..SPAWNERS {
            let registry = registry.clone();
            let barrier = barrier.clone();
            spawners.push(tokio::spawn(async move {
                barrier.wait().await;
                registry.spawn("racing-task", std::future::pending::<()>());
            }));
        }

        barrier.wait().await;
        let timed_out = registry.shutdown(Duration::from_millis(10)).await;
        for spawner in spawners {
            spawner.await.expect("spawner should finish");
        }

        assert!(
            timed_out.is_empty() || timed_out == vec!["racing-task"],
            "only tasks registered before shutdown may time out"
        );
        assert_eq!(registry.active_count(), 0);
        registry.spawn("after-shutdown", std::future::pending::<()>());
        assert_eq!(registry.active_count(), 0);
    }
}
