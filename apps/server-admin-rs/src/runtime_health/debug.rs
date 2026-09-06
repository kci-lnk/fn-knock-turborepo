//! Explicit, bounded diagnostics. Normal health polling never starts a capture.
use std::{
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, Instant},
};

use axum::{extract::State, response::Response};
use serde::{Deserialize, Serialize};
use serde_json::{Map, json};
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

use super::{
    debug_resources::{MemoryDetails, ResourceSample, ResourceSampler, collect_memory_details},
    operations::{OperationRecorder, OperationSnapshot},
};
use crate::{app_version::APP_LOCAL_VERSION, response, state::AppState, time_utils};

const CAPTURE_SECONDS: u64 = 60;
const SAMPLE_INTERVAL_MS: u64 = 1_000;
const MAX_SAMPLES: usize = 61;
const MEMORY_REFRESH_INTERVAL: Duration = Duration::from_secs(3);

pub(crate) fn debug_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_debug))
        .routes(routes!(start_capture, stop_capture))
        .routes(routes!(refresh_memory))
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CaptureStatus {
    Idle,
    Running,
    Completed,
    Stopped,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub(crate) struct DebugSample {
    pub at: String,
    pub elapsed_ms: u64,
    pub resource: ResourceSample,
    pub queue_depth: u64,
    pub active_operation_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub(crate) struct DebugCapture {
    #[schema(required = true)]
    pub id: Option<String>,
    pub status: CaptureStatus,
    #[schema(required = true)]
    pub started_at: Option<String>,
    #[schema(required = true)]
    pub finished_at: Option<String>,
    pub elapsed_ms: u64,
    pub duration_seconds: u64,
    pub sample_interval_ms: u64,
    pub samples: Vec<DebugSample>,
    pub operations: OperationSnapshot,
    pub errors: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub(crate) struct DebugProcess {
    pub pid: u32,
    pub version: String,
    pub os: String,
    pub arch: String,
    pub logical_cpus: usize,
    pub uptime_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub(crate) struct DebugQueue {
    pub queue_depth: u64,
    pub queue_depth_peak: u64,
    pub queue_wait_ms: u64,
    pub queue_wait_peak_ms: u64,
    pub active_operation_ms: u64,
    pub canceled_operations: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub(crate) struct RuntimeDebugReportData {
    pub schema_version: u32,
    pub generated_at: String,
    pub process: DebugProcess,
    pub capture: DebugCapture,
    #[schema(required = true)]
    pub memory: Option<MemoryDetails>,
    pub memory_refreshing: bool,
    pub queue: DebugQueue,
}

struct DebugState {
    generation: u64,
    id: Option<String>,
    status: CaptureStatus,
    started: Option<Instant>,
    started_at: Option<String>,
    finished_at: Option<String>,
    elapsed_ms: u64,
    samples: Vec<DebugSample>,
    errors: Vec<String>,
    cancel: CancellationToken,
    memory: Option<MemoryDetails>,
    memory_refreshed: Option<Instant>,
}

impl Default for DebugState {
    fn default() -> Self {
        Self {
            generation: 0,
            id: None,
            status: CaptureStatus::Idle,
            started: None,
            started_at: None,
            finished_at: None,
            elapsed_ms: 0,
            samples: Vec::new(),
            errors: Vec::new(),
            cancel: CancellationToken::new(),
            memory: None,
            memory_refreshed: None,
        }
    }
}

struct DebugInner {
    state: Mutex<DebugState>,
    sample_gate: Arc<Semaphore>,
    memory_gate: Arc<Semaphore>,
}

#[derive(Clone)]
pub(crate) struct DebugController {
    inner: Arc<DebugInner>,
}

impl Default for DebugController {
    fn default() -> Self {
        Self {
            inner: Arc::new(DebugInner {
                state: Mutex::new(DebugState::default()),
                sample_gate: Arc::new(Semaphore::new(1)),
                memory_gate: Arc::new(Semaphore::new(1)),
            }),
        }
    }
}

impl DebugController {
    fn lock(&self) -> MutexGuard<'_, DebugState> {
        self.inner
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }

    pub(crate) fn report(&self, state: &AppState) -> RuntimeDebugReportData {
        let debug = self.lock();
        let queue = state.storage.store.primary_queue_status();
        RuntimeDebugReportData {
            schema_version: 1,
            generated_at: time_utils::now_iso(),
            process: DebugProcess {
                pid: std::process::id(),
                version: APP_LOCAL_VERSION.to_string(),
                os: std::env::consts::OS.to_string(),
                arch: std::env::consts::ARCH.to_string(),
                logical_cpus: std::thread::available_parallelism().map_or(1, usize::from),
                uptime_ms: elapsed_ms(state.runtime_health.inner.process_started),
            },
            capture: DebugCapture {
                id: debug.id.clone(),
                status: debug.status.clone(),
                started_at: debug.started_at.clone(),
                finished_at: debug.finished_at.clone(),
                elapsed_ms: if debug.status == CaptureStatus::Running {
                    debug.started.map_or(0, elapsed_ms)
                } else {
                    debug.elapsed_ms
                },
                duration_seconds: CAPTURE_SECONDS,
                sample_interval_ms: SAMPLE_INTERVAL_MS,
                samples: debug.samples.clone(),
                operations: state.storage.store.diagnostics().snapshot(),
                errors: debug.errors.clone(),
            },
            memory: debug.memory.clone(),
            memory_refreshing: self.inner.memory_gate.available_permits() == 0,
            queue: DebugQueue {
                queue_depth: queue.queue_depth,
                queue_depth_peak: queue.queue_depth_peak,
                queue_wait_ms: queue.queue_wait_ms,
                queue_wait_peak_ms: queue.queue_wait_peak_ms,
                active_operation_ms: queue.active_operation_ms,
                canceled_operations: queue.canceled_operations,
            },
        }
    }

    fn start(&self, state: &AppState) {
        let recorder = state.storage.store.diagnostics();
        let mut debug = self.lock();
        if debug.status == CaptureStatus::Running || state.shutdown.is_cancelled() {
            return;
        }
        let generation = recorder.start();
        let cancel = CancellationToken::new();
        debug.generation = generation;
        debug.id = Some(uuid::Uuid::new_v4().to_string());
        debug.status = CaptureStatus::Running;
        debug.started = Some(Instant::now());
        debug.started_at = Some(time_utils::now_iso());
        debug.finished_at = None;
        debug.elapsed_ms = 0;
        debug.samples = Vec::with_capacity(MAX_SAMPLES);
        debug.errors.clear();
        debug.cancel = cancel.clone();
        let started = debug.started.unwrap_or_else(Instant::now);
        drop(debug);
        let controller = self.clone();
        let capture_state = state.clone();
        // The finalizer is created before registration, so a refused spawn or
        // background-task abort still disables expensive instrumentation.
        let finalizer = CaptureFinalizer {
            controller: self.clone(),
            recorder,
            generation,
        };
        state.spawn_background("runtime-debug-capture", async move {
            let _finalizer = finalizer;
            controller
                .run_capture(capture_state, generation, started, cancel)
                .await;
        });
        state.runtime_health.operational_log(
            "INFO",
            "management",
            "debug_capture_started",
            "manual_diagnostic",
            Map::new(),
        );
    }

    fn finish(&self, recorder: &OperationRecorder, generation: u64, status: CaptureStatus) -> bool {
        let mut debug = self.lock();
        if debug.generation != generation || debug.status != CaptureStatus::Running {
            return false;
        }
        recorder.stop(generation);
        debug.elapsed_ms = debug.started.map_or(0, elapsed_ms);
        debug.finished_at = Some(time_utils::now_iso());
        debug.status = status;
        debug.cancel.cancel();
        true
    }

    fn stop(&self, state: &AppState) {
        let generation = self.lock().generation;
        if self.finish(
            &state.storage.store.diagnostics(),
            generation,
            CaptureStatus::Stopped,
        ) {
            state.runtime_health.operational_log(
                "INFO",
                "management",
                "debug_capture_stopped",
                "manual_diagnostic",
                Map::new(),
            );
        }
    }

    async fn run_capture(
        &self,
        state: AppState,
        generation: u64,
        started: Instant,
        cancel: CancellationToken,
    ) {
        let mut sampler = ResourceSampler::new();
        let mut timer = tokio::time::interval(Duration::from_millis(SAMPLE_INTERVAL_MS));
        timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let deadline =
            tokio::time::Instant::from_std(started + Duration::from_secs(CAPTURE_SECONDS));
        let mut status = CaptureStatus::Completed;
        loop {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => { status = CaptureStatus::Stopped; break; }
                _ = state.shutdown.cancelled() => { status = CaptureStatus::Stopped; break; }
                _ = tokio::time::sleep_until(deadline) => break,
                _ = timer.tick() => {}
            }
            // Cancellation cannot interrupt an OS read already in progress.
            // Keep a shared permit with that read across capture generations.
            let Ok(permit) = self.inner.sample_gate.clone().try_acquire_owned() else {
                let mut debug = self.lock();
                if debug.generation == generation
                    && debug.status == CaptureStatus::Running
                    && !debug
                        .errors
                        .iter()
                        .any(|code| code == "resource_sampler_busy")
                {
                    debug.errors.push("resource_sampler_busy".to_string());
                }
                continue;
            };
            let work = tokio::task::spawn_blocking(move || {
                let _permit = permit;
                let sample = sampler.sample();
                (sampler, sample)
            });
            let result = tokio::select! {
                biased;
                _ = cancel.cancelled() => { status = CaptureStatus::Stopped; break; }
                _ = state.shutdown.cancelled() => { status = CaptureStatus::Stopped; break; }
                _ = tokio::time::sleep_until(deadline) => break,
                result = work => result,
            };
            match result {
                Ok((next_sampler, resource)) => {
                    sampler = next_sampler;
                    let queue = state.storage.store.primary_queue_status();
                    let mut debug = self.lock();
                    if debug.generation != generation || debug.status != CaptureStatus::Running {
                        return;
                    }
                    if debug.samples.len() < MAX_SAMPLES {
                        debug.samples.push(DebugSample {
                            at: resource.collected_at.clone(),
                            elapsed_ms: elapsed_ms(started),
                            resource,
                            queue_depth: queue.queue_depth,
                            active_operation_ms: queue.active_operation_ms,
                        });
                    }
                }
                Err(_) => {
                    let mut debug = self.lock();
                    if debug.generation == generation && debug.status == CaptureStatus::Running {
                        debug.errors.push("resource_sample_failed".to_string());
                    }
                    status = CaptureStatus::Stopped;
                    break;
                }
            }
        }
        if self.finish(
            &state.storage.store.diagnostics(),
            generation,
            status.clone(),
        ) {
            state.runtime_health.operational_log(
                "INFO",
                "management",
                "debug_capture_finished",
                "manual_diagnostic",
                Map::from_iter([("status".to_string(), json!(status))]),
            );
        }
    }

    async fn refresh_memory(&self) {
        let Ok(permit) = self.inner.memory_gate.clone().try_acquire_owned() else {
            return;
        };
        if self
            .lock()
            .memory_refreshed
            .is_some_and(|at| at.elapsed() < MEMORY_REFRESH_INTERVAL)
        {
            return;
        }
        let controller = self.clone();
        // Publish inside the bounded blocking operation, even if the requesting
        // browser disconnects. The permit remains held until collection ends.
        let _ = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let memory = collect_memory_details();
            let mut debug = controller.lock();
            debug.memory = Some(memory);
            debug.memory_refreshed = Some(Instant::now());
        })
        .await;
    }
}

struct CaptureFinalizer {
    controller: DebugController,
    recorder: Arc<OperationRecorder>,
    generation: u64,
}

impl Drop for CaptureFinalizer {
    fn drop(&mut self) {
        self.controller
            .finish(&self.recorder, self.generation, CaptureStatus::Stopped);
    }
}

fn elapsed_ms(start: Instant) -> u64 {
    start.elapsed().as_millis().min(u64::MAX as u128) as u64
}

#[utoipa::path(get, path = "/api/admin/runtime-health/debug", tag = "runtime-health",
    operation_id = "get_api_admin_runtime_health_debug",
    responses((status = 200, description = "Cached runtime resource and operation diagnostics")))]
pub(crate) async fn get_debug(State(state): State<AppState>) -> Response {
    response::ok(state.runtime_health.inner.debug.report(&state)).into_response()
}

#[utoipa::path(post, path = "/api/admin/runtime-health/debug/capture", tag = "runtime-health",
    operation_id = "post_api_admin_runtime_health_debug_capture",
    responses((status = 200, description = "Start or return the existing bounded 60-second capture")))]
pub(crate) async fn start_capture(State(state): State<AppState>) -> Response {
    state.runtime_health.inner.debug.start(&state);
    response::ok(state.runtime_health.inner.debug.report(&state)).into_response()
}

#[utoipa::path(delete, path = "/api/admin/runtime-health/debug/capture", tag = "runtime-health",
    operation_id = "delete_api_admin_runtime_health_debug_capture",
    responses((status = 200, description = "Stop capture and retain the collected report")))]
pub(crate) async fn stop_capture(State(state): State<AppState>) -> Response {
    state.runtime_health.inner.debug.stop(&state);
    response::ok(state.runtime_health.inner.debug.report(&state)).into_response()
}

#[utoipa::path(post, path = "/api/admin/runtime-health/debug/memory", tag = "runtime-health",
    operation_id = "post_api_admin_runtime_health_debug_memory",
    responses((status = 200, description = "Collect bounded memory counters without reclaiming memory")))]
pub(crate) async fn refresh_memory(State(state): State<AppState>) -> Response {
    state.runtime_health.inner.debug.refresh_memory().await;
    response::ok(state.runtime_health.inner.debug.report(&state)).into_response()
}

use axum::response::IntoResponse;

#[cfg(test)]
mod tests {
    use super::*;

    async fn state() -> (tempfile::TempDir, AppState) {
        let directory = tempfile::tempdir().unwrap();
        let mut settings = crate::settings::Settings::from_env();
        settings.data_dir = directory.path().join("data");
        settings.sqlite_path = directory.path().join("fn-knock.sqlite3");
        settings.gateway_config_dir = directory.path().join("gateway");
        settings.legacy_redis_url.clear();
        settings.go_backend_grpc_addr = "127.0.0.1:1".to_string();
        settings.internal_rpc_token = "debug-test-token".to_string();
        settings.altcha_hmac_key = Some("debug-test-altcha-key".to_string());
        (directory, AppState::new(settings).await.unwrap())
    }

    async fn cleanup(state: &AppState) {
        state.shutdown.cancel();
        state
            .shutdown_background_tasks(Duration::from_secs(2))
            .await;
        state
            .runtime_health
            .shutdown_operational_log(Duration::from_secs(2))
            .await;
    }

    #[tokio::test]
    async fn reads_are_cached_and_do_not_start_diagnostics() {
        let (_directory, state) = state().await;
        let controller = &state.runtime_health.inner.debug;
        for _ in 0..3 {
            let report = controller.report(&state);
            assert_eq!(report.capture.status, CaptureStatus::Idle);
            assert!(report.capture.samples.is_empty());
            assert!(!report.capture.operations.active);
            assert!(report.memory.is_none());
        }
        cleanup(&state).await;
    }

    #[tokio::test]
    async fn repeated_start_is_idempotent_and_old_work_cannot_enter_new_capture() {
        let (_directory, state) = state().await;
        let controller = &state.runtime_health.inner.debug;
        let recorder = state.storage.store.diagnostics();
        controller.start(&state);
        let first = controller.report(&state);
        controller.start(&state);
        assert_eq!(controller.report(&state).capture.id, first.capture.id);
        let old_work = recorder.scope("task", "old-work");
        recorder
            .scope("task", "complete-work")
            .finish(true, Some(7));
        state
            .storage
            .store
            .set_json_value(
                "debug-sensitive-canary-key",
                &json!({"secret":"debug-sensitive-canary-value"}),
            )
            .await
            .unwrap();
        controller.stop(&state);
        let stopped = controller.report(&state);
        assert_eq!(stopped.capture.status, CaptureStatus::Stopped);
        assert!(!stopped.capture.operations.active);
        let encoded = serde_json::to_string(&stopped).unwrap();
        assert!(!encoded.contains("debug-sensitive-canary"));
        tokio::time::sleep(Duration::from_millis(5)).await;
        assert_eq!(
            controller.report(&state).capture.elapsed_ms,
            stopped.capture.elapsed_ms
        );
        controller.start(&state);
        assert_ne!(controller.report(&state).capture.id, first.capture.id);
        old_work.finish(true, None);
        assert!(
            !controller
                .report(&state)
                .capture
                .operations
                .operations
                .iter()
                .any(|row| row.label == "old-work")
        );
        controller.stop(&state);
        cleanup(&state).await;
    }

    #[tokio::test]
    async fn elapsed_deadline_completes_and_disables_operation_recording() {
        let (_directory, state) = state().await;
        let controller = &state.runtime_health.inner.debug;
        let recorder = state.storage.store.diagnostics();
        let generation = recorder.start();
        let started = Instant::now() - Duration::from_secs(CAPTURE_SECONDS + 1);
        {
            let mut debug = controller.lock();
            debug.generation = generation;
            debug.status = CaptureStatus::Running;
            debug.started = Some(started);
        }
        controller
            .run_capture(state.clone(), generation, started, CancellationToken::new())
            .await;
        let report = controller.report(&state);
        assert_eq!(report.capture.status, CaptureStatus::Completed);
        assert!(!report.capture.operations.active);
        assert!(report.capture.samples.is_empty());
        cleanup(&state).await;
    }

    #[tokio::test]
    async fn refused_background_registration_cannot_leave_recorder_enabled() {
        let (_directory, state) = state().await;
        state
            .shutdown_background_tasks(Duration::from_secs(1))
            .await;
        state.runtime_health.inner.debug.start(&state);
        let report = state.runtime_health.inner.debug.report(&state);
        assert_eq!(report.capture.status, CaptureStatus::Stopped);
        assert!(!report.capture.operations.active);
        cleanup(&state).await;
    }

    #[tokio::test]
    async fn memory_refresh_is_cached_and_does_not_start_a_capture() {
        let (_directory, state) = state().await;
        let controller = &state.runtime_health.inner.debug;
        controller.refresh_memory().await;
        let first = controller.report(&state);
        assert!(first.memory.is_some());
        controller.refresh_memory().await;
        let second = controller.report(&state);
        assert_eq!(
            first.memory.unwrap().collected_at,
            second.memory.unwrap().collected_at
        );
        assert_eq!(second.capture.status, CaptureStatus::Idle);
        assert!(!second.memory_refreshing);
        cleanup(&state).await;
    }

    #[tokio::test]
    async fn busy_sampler_is_bounded_across_capture_generations() {
        let (_directory, state) = state().await;
        let controller = &state.runtime_health.inner.debug;
        let permit = controller
            .inner
            .sample_gate
            .clone()
            .acquire_owned()
            .await
            .unwrap();
        controller.start(&state);
        tokio::time::timeout(Duration::from_secs(2), async {
            while controller.report(&state).capture.errors.is_empty() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        controller.stop(&state);
        controller.start(&state);
        tokio::time::timeout(Duration::from_secs(2), async {
            while controller.report(&state).capture.errors.is_empty() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let report = controller.report(&state);
        assert!(report.capture.samples.is_empty());
        assert_eq!(report.capture.errors, vec!["resource_sampler_busy"]);
        controller.stop(&state);
        drop(permit);
        tokio::task::yield_now().await;
        assert_eq!(
            controller.report(&state).capture.errors,
            report.capture.errors
        );
        assert!(controller.report(&state).capture.samples.is_empty());
        cleanup(&state).await;
    }
}
