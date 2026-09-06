//! Bounded, opt-in operation measurements. Labels are compile-time identifiers,
//! never SQL text, request fields, or configuration values.

use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    thread::ThreadId,
    time::Instant,
};

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

const MAX_OPERATIONS: usize = 128;
const MAX_LABEL_CHARS: usize = 200;

#[derive(Clone, Debug, Default, Deserialize, Serialize, ToSchema)]
pub(crate) struct OperationSnapshot {
    pub(crate) generation: u64,
    pub(crate) active: bool,
    pub(crate) elapsed_ms: u64,
    /// Scopes omitted after the distinct operation limit was reached.
    pub(crate) dropped_operations: u64,
    pub(crate) operations: Vec<OperationStats>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, ToSchema)]
pub(crate) struct OperationStats {
    pub(crate) kind: String,
    pub(crate) label: String,
    /// Completed scopes, including failures and cancellations.
    pub(crate) calls: u64,
    pub(crate) failures: u64,
    pub(crate) cancelled: u64,
    /// Scopes still running, or unfinished when capture stopped.
    pub(crate) in_flight: u64,
    pub(crate) total_wall_ms: f64,
    pub(crate) max_wall_ms: f64,
    /// Available only for scopes measured on one SQLite execution thread.
    #[schema(required = true)]
    pub(crate) total_cpu_ms: Option<f64>,
    #[schema(required = true)]
    pub(crate) max_cpu_ms: Option<f64>,
    /// Sum of item counts explicitly supplied by the instrumented operation.
    #[schema(required = true)]
    pub(crate) rows: Option<u64>,
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
struct OperationKey {
    kind: &'static str,
    label: &'static str,
}

#[derive(Default)]
struct CaptureState {
    generation: u64,
    started: Option<Instant>,
    stopped: Option<Instant>,
    dropped_operations: u64,
    operations: HashMap<OperationKey, OperationStats>,
}

#[derive(Default)]
pub(crate) struct OperationRecorder {
    // Zero lets disabled scopes avoid locking, allocating, or reading clocks.
    active_generation: AtomicU64,
    state: Mutex<CaptureState>,
}

impl OperationRecorder {
    pub(crate) fn start(&self) -> u64 {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        let generation = state.generation.wrapping_add(1).max(1);
        *state = CaptureState {
            generation,
            started: Some(Instant::now()),
            ..CaptureState::default()
        };
        self.active_generation.store(generation, Ordering::Release);
        generation
    }

    pub(crate) fn stop(&self, generation: u64) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if state.generation == generation && state.started.is_some() && state.stopped.is_none() {
            state.stopped = Some(Instant::now());
            self.active_generation.store(0, Ordering::Release);
        }
    }

    pub(crate) fn snapshot(&self) -> OperationSnapshot {
        let state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        let mut operations = state.operations.values().cloned().collect::<Vec<_>>();
        operations.sort_by(|a, b| {
            b.total_wall_ms
                .total_cmp(&a.total_wall_ms)
                .then_with(|| a.kind.cmp(&b.kind))
                .then_with(|| a.label.cmp(&b.label))
        });
        OperationSnapshot {
            generation: state.generation,
            active: state.started.is_some() && state.stopped.is_none(),
            elapsed_ms: state.started.map_or(0, |started| {
                state
                    .stopped
                    .unwrap_or_else(Instant::now)
                    .saturating_duration_since(started)
                    .as_millis()
                    .min(u128::from(u64::MAX)) as u64
            }),
            dropped_operations: state.dropped_operations,
            operations,
        }
    }

    pub(crate) fn scope(
        self: &Arc<Self>,
        kind: &'static str,
        label: &'static str,
    ) -> OperationGuard {
        self.begin_scope(kind, label, false)
    }

    /// Call and finish this scope inside the actual SQLite closure, never
    /// around the async admission/submission future.
    pub(crate) fn scope_sqlite(
        self: &Arc<Self>,
        kind: &'static str,
        label: &'static str,
    ) -> OperationGuard {
        self.begin_scope(kind, label, true)
    }

    fn begin_scope(
        self: &Arc<Self>,
        kind: &'static str,
        label: &'static str,
        measure_cpu: bool,
    ) -> OperationGuard {
        let generation = self.active_generation.load(Ordering::Acquire);
        if generation == 0 {
            return OperationGuard::default();
        }
        let key = OperationKey { kind, label };
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if state.generation != generation || state.stopped.is_some() {
            return OperationGuard::default();
        }
        if !state.operations.contains_key(&key) {
            if state.operations.len() >= MAX_OPERATIONS {
                state.dropped_operations = state.dropped_operations.saturating_add(1);
                return OperationGuard::default();
            }
            state.operations.insert(
                key,
                OperationStats {
                    kind: bounded_label(kind),
                    label: bounded_label(label),
                    ..OperationStats::default()
                },
            );
        }
        let Some(stats) = state.operations.get_mut(&key) else {
            return OperationGuard::default();
        };
        stats.in_flight = stats.in_flight.saturating_add(1);
        drop(state);
        OperationGuard {
            active: Some(ActiveOperation {
                recorder: self.clone(),
                generation,
                key,
                started: Instant::now(),
                cpu: measure_cpu.then(thread_cpu_start).flatten(),
            }),
            outcome: None,
        }
    }

    fn complete(&self, active: &ActiveOperation, outcome: Option<(bool, Option<u64>)>) {
        if self.active_generation.load(Ordering::Acquire) != active.generation {
            return;
        }
        let wall_ms = active.started.elapsed().as_secs_f64() * 1000.0;
        let cpu_ms = active.cpu.and_then(|(thread, started)| {
            (thread == std::thread::current().id())
                .then(thread_cpu_ns)
                .flatten()
                .and_then(|finished| finished.checked_sub(started))
                .map(|elapsed| elapsed as f64 / 1_000_000.0)
        });
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if state.generation != active.generation || state.stopped.is_some() {
            return;
        }
        let Some(stats) = state.operations.get_mut(&active.key) else {
            return;
        };
        stats.in_flight = stats.in_flight.saturating_sub(1);
        stats.calls = stats.calls.saturating_add(1);
        match outcome {
            None => stats.cancelled = stats.cancelled.saturating_add(1),
            Some((success, rows)) => {
                if !success {
                    stats.failures = stats.failures.saturating_add(1);
                }
                if let Some(rows) = rows {
                    stats.rows = Some(stats.rows.unwrap_or(0).saturating_add(rows));
                }
            }
        }
        stats.total_wall_ms += wall_ms;
        stats.max_wall_ms = stats.max_wall_ms.max(wall_ms);
        if let Some(cpu_ms) = cpu_ms {
            stats.total_cpu_ms = Some(stats.total_cpu_ms.unwrap_or(0.0) + cpu_ms);
            stats.max_cpu_ms = Some(stats.max_cpu_ms.unwrap_or(0.0).max(cpu_ms));
        }
    }
}

struct ActiveOperation {
    recorder: Arc<OperationRecorder>,
    generation: u64,
    key: OperationKey,
    started: Instant,
    cpu: Option<(ThreadId, u64)>,
}

#[derive(Default)]
pub(crate) struct OperationGuard {
    active: Option<ActiveOperation>,
    outcome: Option<(bool, Option<u64>)>,
}

impl OperationGuard {
    pub(crate) fn finish(mut self, success: bool, rows: Option<u64>) {
        self.outcome = Some((success, rows));
    }
}

impl Drop for OperationGuard {
    fn drop(&mut self) {
        if let Some(active) = self.active.take() {
            active.recorder.complete(&active, self.outcome);
        }
    }
}

fn bounded_label(value: &str) -> String {
    let mut label = value
        .chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .take(MAX_LABEL_CHARS + 1)
        .collect::<String>();
    if label.chars().count() > MAX_LABEL_CHARS {
        // Keep long monomorphized type names distinguishable after truncation.
        let hash = value.bytes().fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
        });
        label = label.chars().take(MAX_LABEL_CHARS - 17).collect();
        label.push_str(&format!("…{hash:016x}"));
    }
    label
}

fn thread_cpu_start() -> Option<(ThreadId, u64)> {
    thread_cpu_ns().map(|ns| (std::thread::current().id(), ns))
}

#[cfg(unix)]
fn thread_cpu_ns() -> Option<u64> {
    let mut value = std::mem::MaybeUninit::<libc::timespec>::uninit();
    // SAFETY: clock_gettime initializes this timespec on success, and a thread
    // CPU clock measures only the calling native thread.
    if unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, value.as_mut_ptr()) } != 0 {
        return None;
    }
    // SAFETY: the successful call above initialized both fields.
    let value = unsafe { value.assume_init() };
    u64::try_from(value.tv_sec)
        .ok()?
        .checked_mul(1_000_000_000)?
        .checked_add(u64::try_from(value.tv_nsec).ok()?)
}

#[cfg(not(unix))]
fn thread_cpu_ns() -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_operation_scopes_do_not_lock_allocate_or_retain_recorder() {
        let recorder = Arc::new(OperationRecorder::default());
        let state_lock = recorder.state.lock().unwrap();
        let scope = recorder.scope("task", "disabled");
        let sqlite = recorder.scope_sqlite("sqlite_primary", "disabled");
        assert!(scope.active.is_none());
        assert!(sqlite.active.is_none());
        assert_eq!(Arc::strong_count(&recorder), 1);
        drop(state_lock);
        assert!(recorder.snapshot().operations.is_empty());
    }

    #[test]
    fn operation_capture_counts_success_error_cancel_and_freezes_at_stop() {
        let recorder = Arc::new(OperationRecorder::default());
        let generation = recorder.start();
        recorder.scope("task", "round").finish(true, Some(2));
        recorder.scope("task", "round").finish(false, Some(3));
        drop(recorder.scope("task", "round"));
        let unfinished = recorder.scope("task", "round");
        recorder.stop(generation);
        let stopped = recorder.snapshot();
        let stats = &stopped.operations[0];
        assert!(!stopped.active);
        assert_eq!(
            (
                stats.calls,
                stats.failures,
                stats.cancelled,
                stats.in_flight
            ),
            (3, 1, 1, 1)
        );
        assert_eq!(stats.rows, Some(5));
        assert_eq!(stats.total_cpu_ms, None);
        unfinished.finish(true, Some(100));
        std::thread::sleep(std::time::Duration::from_millis(3));
        assert_eq!(
            serde_json::to_value(recorder.snapshot()).unwrap(),
            serde_json::to_value(stopped).unwrap()
        );
    }

    #[test]
    fn operation_capture_generations_isolate_old_guards_and_stop_requests() {
        let recorder = Arc::new(OperationRecorder::default());
        let old_generation = recorder.start();
        let old_scope = recorder.scope("task", "old");
        let generation = recorder.start();
        recorder.stop(old_generation);
        old_scope.finish(false, Some(99));
        recorder.scope("task", "new").finish(true, None);
        let snapshot = recorder.snapshot();
        assert!(snapshot.active);
        assert_eq!(snapshot.generation, generation);
        assert_eq!(snapshot.operations.len(), 1);
        assert_eq!(snapshot.operations[0].label, "new");
        assert_eq!(snapshot.operations[0].calls, 1);
        assert_eq!(snapshot.operations[0].failures, 0);
    }

    #[test]
    fn operation_capture_bounds_labels_and_overflow_without_losing_known_operations() {
        let recorder = Arc::new(OperationRecorder::default());
        recorder.start();
        for index in 0..MAX_OPERATIONS + 3 {
            let label = Box::leak(format!("operation-{index}").into_boxed_str());
            recorder.scope("task", label).finish(true, None);
        }
        recorder.scope("task", "operation-0").finish(true, None);
        let snapshot = recorder.snapshot();
        assert_eq!(snapshot.operations.len(), MAX_OPERATIONS);
        assert_eq!(snapshot.dropped_operations, 3);
        assert_eq!(
            snapshot
                .operations
                .iter()
                .find(|stats| stats.label == "operation-0")
                .unwrap()
                .calls,
            2
        );
        let long = "界".repeat(MAX_LABEL_CHARS + 1);
        let truncated = bounded_label(&long);
        assert_eq!(truncated.chars().count(), MAX_LABEL_CHARS);
        assert_ne!(truncated, bounded_label(&(long + "x")));
        assert_eq!(bounded_label("name\nwith\tcontrols"), "name with controls");
    }

    #[test]
    fn sqlite_operation_cpu_clock_is_optional_and_never_used_across_threads() {
        let recorder = Arc::new(OperationRecorder::default());
        recorder.start();
        let scope = recorder.scope_sqlite("sqlite_primary", "same-thread");
        for value in 0..10_000 {
            std::hint::black_box(value * 3);
        }
        scope.finish(true, None);
        let moved = recorder.scope_sqlite("sqlite_primary", "different-thread");
        std::thread::scope(|threads| {
            threads.spawn(move || moved.finish(true, None));
        });
        let snapshot = recorder.snapshot();
        let same = snapshot
            .operations
            .iter()
            .find(|stats| stats.label == "same-thread")
            .unwrap();
        if thread_cpu_ns().is_some() {
            assert!(same.total_cpu_ms.is_some_and(|cpu| cpu >= 0.0));
        }
        let moved = snapshot
            .operations
            .iter()
            .find(|stats| stats.label == "different-thread")
            .unwrap();
        assert_eq!(moved.total_cpu_ms, None);
    }
}
