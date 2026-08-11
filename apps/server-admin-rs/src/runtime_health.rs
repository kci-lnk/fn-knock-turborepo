pub(crate) mod routes;

use std::{
    backtrace::Backtrace,
    collections::{BTreeMap, HashMap, VecDeque},
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex as StdMutex, Once, OnceLock,
        atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tokio::sync::{Mutex, Notify, RwLock, mpsc, oneshot};

use crate::{
    app_version::APP_LOCAL_VERSION,
    events::{RuntimeEventInput, publish_runtime_event},
    go_backend::{GATEWAY_HEALTH_AUTH_BRIDGE, GATEWAY_HEALTH_DATAPLANE, GATEWAY_HEALTH_PROCESS},
    state::AppState,
    time_utils,
};

const PROBE_INTERVAL: Duration = Duration::from_secs(5);
const PROBE_TIMEOUT: Duration = Duration::from_secs(2);
const SESSION_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(60);
const STORAGE_TTL_GC_INTERVAL: Duration = Duration::from_secs(5 * 60);
const RUNTIME_STATE_TTL_SECONDS: i64 = 7 * 24 * 60 * 60;
const SESSION_KEY: &str = "fn_knock:runtime:session:management";
const GATEWAY_INSTANCE_KEY: &str = "fn_knock:runtime:last_gateway_instance";
const MAX_PENDING_EVENTS: usize = 64;
const PENDING_EVENT_TTL: Duration = Duration::from_secs(60 * 60);
const SUPERVISOR_HINT_TTL: Duration = Duration::from_secs(7 * 24 * 60 * 60);
const SUPERVISOR_TEMP_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const MAX_SUPERVISOR_HINTS: usize = 32;
const MAX_SUPERVISOR_TEMP_FILES: usize = 32;
const LOG_MAX_BYTES: u64 = 1 << 20;
const AUXILIARY_LOG_MAX_BYTES: u64 = 512 << 10;
const CRASH_MAX_BYTES: u64 = 512 << 10;
const LOG_QUEUE_SIZE: usize = 768;
const LOG_HIGH_QUEUE_SIZE: usize = 256;
const LOG_REPEAT_WINDOW: Duration = Duration::from_secs(60);
const LOG_REPEAT_TTL: Duration = Duration::from_secs(5 * 60);
const MAX_LOG_REPEAT_KEYS: usize = 1024;
const SESSION_PHASE_STARTING: u8 = 0;
const SESSION_PHASE_RUNNING: u8 = 1;
const SESSION_PHASE_STOPPED: u8 = 2;

static PANIC_INSTANCE_ID: OnceLock<String> = OnceLock::new();

pub(crate) const COMPONENT_IDS: [&str; 6] = [
    "management",
    "gateway_process",
    "gateway_dataplane",
    "auth_bridge",
    "storage",
    "config_sync",
];

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
    Blocked,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProcessState {
    Running,
    Stopped,
    Unknown,
    NotApplicable,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ComponentHealth {
    pub id: String,
    pub status: HealthStatus,
    pub process_state: ProcessState,
    pub version: Option<String>,
    pub commit: Option<String>,
    pub pid: Option<u32>,
    pub instance_id: Option<String>,
    pub started_at: Option<String>,
    pub uptime_ms: Option<u64>,
    pub last_checked_at: Option<String>,
    pub last_success_at: Option<String>,
    pub consecutive_failures: u32,
    pub reason_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_percent: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rss_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub go_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub goroutines: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heap_alloc_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heap_sys_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
}

impl ComponentHealth {
    fn unknown(id: &str) -> Self {
        Self {
            id: id.to_string(),
            status: HealthStatus::Unknown,
            process_state: if matches!(id, "management" | "gateway_process") {
                ProcessState::Unknown
            } else {
                ProcessState::NotApplicable
            },
            version: None,
            commit: None,
            pid: None,
            instance_id: None,
            started_at: None,
            uptime_ms: None,
            last_checked_at: None,
            last_success_at: None,
            consecutive_failures: 0,
            reason_code: Some("not_checked".to_string()),
            cpu_percent: None,
            rss_bytes: None,
            go_version: None,
            goroutines: None,
            heap_alloc_bytes: None,
            heap_sys_bytes: None,
            latency_ms: None,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct RuntimeLogStatus {
    pub directory: String,
    pub bytes_used: u64,
    pub dropped_info: u64,
    pub oldest_at: Option<String>,
    pub newest_at: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct RuntimeSnapshot {
    pub schema_version: u32,
    pub overall_status: HealthStatus,
    pub last_checked_at: Option<String>,
    pub components: BTreeMap<String, ComponentHealth>,
    pub logs: RuntimeLogStatus,
    pub supervisor: String,
}

#[derive(Clone)]
pub(crate) struct RuntimeHealth {
    inner: Arc<RuntimeHealthInner>,
}

struct RuntimeHealthInner {
    snapshot: RwLock<RuntimeSnapshot>,
    trackers: Mutex<BTreeMap<String, Tracker>>,
    pending_events: Mutex<VecDeque<PendingRuntimeEvent>>,
    logger: DiagnosticLogger,
    process_started: Instant,
    process_started_at: String,
    management_instance_id: String,
    seen_gateway_instance: Mutex<Option<String>>,
    supervisor_events_dir: PathBuf,
    monitor_done: Notify,
    monitor_stopped: AtomicBool,
    management_abnormal_reported: AtomicBool,
    session_phase: AtomicU8,
    session_write: Mutex<()>,
}

struct Tracker {
    health: ComponentHealth,
    recovery_successes: u32,
    incident: Option<Incident>,
}

struct Incident {
    id: String,
    started_at_ms: i64,
}

struct PendingRuntimeEvent {
    queued_at: Instant,
    input: RuntimeEventInput,
}

enum TrackerTransition {
    Failed(String),
    Recovered(Incident),
}

struct ProbeResult {
    ok: bool,
    reason_code: &'static str,
    metadata: ProbeMetadata,
}

#[derive(Default)]
struct ProbeMetadata {
    process_state: Option<ProcessState>,
    version: Option<String>,
    commit: Option<String>,
    pid: Option<u32>,
    instance_id: Option<String>,
    started_at: Option<String>,
    uptime_ms: Option<u64>,
    go_version: Option<String>,
    goroutines: Option<u64>,
    rss_bytes: Option<u64>,
    heap_alloc_bytes: Option<u64>,
    heap_sys_bytes: Option<u64>,
    latency_ms: Option<u64>,
}

#[derive(Clone)]
struct DiagnosticLogger {
    info: mpsc::Sender<Vec<u8>>,
    high: mpsc::Sender<Vec<u8>>,
    dropped_info: Arc<AtomicU64>,
    directory: PathBuf,
    repeats: Arc<StdMutex<HashMap<String, RepeatEntry>>>,
    control: mpsc::Sender<DiagnosticControl>,
    writer: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

enum DiagnosticControl {
    Flush(oneshot::Sender<()>),
    Clear(oneshot::Sender<Result<(), String>>),
    Shutdown,
}

struct RepeatEntry {
    last: Instant,
    suppressed: u64,
}

impl RuntimeHealth {
    pub(crate) fn new(data_dir: &Path, supervisor: &str) -> anyhow::Result<Self> {
        let log_directory = data_dir.join("runtime/logs");
        enforce_runtime_log_caps(&log_directory);
        let logger = DiagnosticLogger::new(log_directory.clone()).unwrap_or_else(|error| {
            tracing::warn!(%error, path = %log_directory.display(), "operational diagnostic logger is unavailable");
            DiagnosticLogger::disabled(log_directory)
        });
        let mut components = BTreeMap::new();
        let mut trackers = BTreeMap::new();
        for id in COMPONENT_IDS {
            let health = ComponentHealth::unknown(id);
            components.insert(id.to_string(), health.clone());
            trackers.insert(
                id.to_string(),
                Tracker {
                    health,
                    recovery_successes: 0,
                    incident: None,
                },
            );
        }
        let process_started_at = time_utils::now_iso();
        let management_instance_id = uuid::Uuid::new_v4().simple().to_string();
        let _ = PANIC_INSTANCE_ID.set(management_instance_id.clone());
        let log_status = logger.status();
        Ok(Self {
            inner: Arc::new(RuntimeHealthInner {
                snapshot: RwLock::new(RuntimeSnapshot {
                    schema_version: 1,
                    overall_status: HealthStatus::Unknown,
                    last_checked_at: None,
                    components,
                    logs: log_status,
                    supervisor: supervisor.to_string(),
                }),
                trackers: Mutex::new(trackers),
                pending_events: Mutex::new(VecDeque::new()),
                logger,
                process_started: Instant::now(),
                process_started_at,
                management_instance_id,
                seen_gateway_instance: Mutex::new(None),
                supervisor_events_dir: data_dir.join("runtime/supervisor-events"),
                monitor_done: Notify::new(),
                monitor_stopped: AtomicBool::new(false),
                management_abnormal_reported: AtomicBool::new(false),
                session_phase: AtomicU8::new(SESSION_PHASE_STARTING),
                session_write: Mutex::new(()),
            }),
        })
    }

    pub(crate) async fn snapshot(&self) -> RuntimeSnapshot {
        let mut snapshot = self.inner.snapshot.read().await.clone();
        snapshot.logs = self.inner.logger.status();
        snapshot
    }

    pub(crate) fn logs_dir(&self) -> PathBuf {
        self.inner.logger.directory.clone()
    }

    pub(crate) fn operational_log(
        &self,
        level: &str,
        component: &str,
        event: &str,
        reason_code: &str,
        fields: Map<String, Value>,
    ) {
        self.inner
            .logger
            .log(level, component, event, reason_code, fields);
    }

    pub(crate) async fn flush_operational_log(&self) {
        self.inner.logger.flush().await;
    }

    pub(crate) async fn shutdown_operational_log(&self, timeout: Duration) -> bool {
        self.inner.logger.shutdown(timeout).await
    }

    pub(crate) async fn clear_operational_log(&self, component: &str) -> anyhow::Result<()> {
        match component {
            "management" => self.inner.logger.clear().await,
            "gateway_process" => {
                let directory = self.inner.logger.directory.clone();
                tokio::task::spawn_blocking(move || {
                    clear_external_rotating_log(&directory.join("gateway.jsonl"))
                })
                .await
                .map_err(|error| anyhow::anyhow!("gateway log clear task failed: {error}"))??;
                Ok(())
            }
            _ => anyhow::bail!("unsupported runtime log component"),
        }
    }

    pub(crate) async fn wait_stopped(&self, timeout: Duration) {
        if self.inner.monitor_stopped.load(Ordering::Acquire) {
            return;
        }
        let _ = tokio::time::timeout(timeout, self.inner.monitor_done.notified()).await;
    }

    async fn initialize_session(&self, state: &AppState) -> anyhow::Result<()> {
        if let Some(raw) = state.storage.store.get_string_value(SESSION_KEY).await?
            && let Ok(previous) = serde_json::from_str::<Value>(&raw)
            && previous.get("state").and_then(Value::as_str) == Some("running")
        {
            self.inner
                .management_abnormal_reported
                .store(true, Ordering::Release);
            let input = RuntimeEventInput {
                event_type: "FN_EVENT_RUNTIME_ABNORMAL_EXIT",
                level: "ERROR",
                component: "management".to_string(),
                payload: json!({
                    "component": "management",
                    "incident_id": uuid::Uuid::new_v4().simple().to_string(),
                    "instance_id": previous.get("instance_id").cloned().unwrap_or(Value::Null),
                    "reason_code": "stale_running_session",
                    "last_heartbeat_at": previous.get("heartbeat_at").cloned().unwrap_or(Value::Null),
                }),
            };
            self.publish_or_buffer(state, input).await;
        }
        self.persist_session(state, "starting").await?;
        let previous_gateway = state
            .storage
            .store
            .get_string_value(GATEWAY_INSTANCE_KEY)
            .await?;
        *self.inner.seen_gateway_instance.lock().await = previous_gateway;
        Ok(())
    }

    async fn persist_session(&self, state: &AppState, session_state: &str) -> anyhow::Result<()> {
        let phase = match session_state {
            "starting" => SESSION_PHASE_STARTING,
            "running" => SESSION_PHASE_RUNNING,
            "stopped" => SESSION_PHASE_STOPPED,
            _ => anyhow::bail!("unsupported runtime session state"),
        };
        self.inner.session_phase.store(phase, Ordering::Release);
        self.persist_current_session(state).await
    }

    async fn persist_current_session(&self, state: &AppState) -> anyhow::Result<()> {
        let _write = self.inner.session_write.lock().await;
        let session_state = match self.inner.session_phase.load(Ordering::Acquire) {
            SESSION_PHASE_RUNNING => "running",
            SESSION_PHASE_STOPPED => "stopped",
            _ => "starting",
        };
        let raw = serde_json::to_string(&json!({
            "instance_id": self.inner.management_instance_id,
            "pid": std::process::id(),
            "started_at": self.inner.process_started_at,
            "heartbeat_at": time_utils::now_iso(),
            "state": session_state,
        }))?;
        state
            .storage
            .store
            .set_string_value_with_optional_ttl(SESSION_KEY, &raw, Some(RUNTIME_STATE_TTL_SECONDS))
            .await?;
        Ok(())
    }

    pub(crate) async fn mark_session_ready(&self, state: &AppState) -> anyhow::Result<()> {
        self.persist_session(state, "running").await
    }

    async fn publish_or_buffer(&self, state: &AppState, input: RuntimeEventInput) {
        match publish_runtime_event(state, input.clone()).await {
            Ok(_) => return,
            Err(error) => {
                tracing::warn!(%error, event_type = input.event_type, component = %input.component, "failed to persist runtime event; buffering transition");
                self.inner.logger.log(
                    "ERROR",
                    "storage",
                    "event_write_failed",
                    "sqlite_write_failed",
                    Map::from_iter([("result".to_string(), json!("failed"))]),
                );
            }
        }
        let mut pending = self.inner.pending_events.lock().await;
        if pending.len() == MAX_PENDING_EVENTS {
            pending.pop_front();
        }
        pending.push_back(PendingRuntimeEvent {
            queued_at: Instant::now(),
            input,
        });
    }

    async fn flush_pending(&self, state: &AppState) {
        loop {
            let next = self.inner.pending_events.lock().await.pop_front();
            let Some(next) = next else { break };
            if next.queued_at.elapsed() > PENDING_EVENT_TTL {
                continue;
            }
            if publish_runtime_event(state, next.input.clone())
                .await
                .is_err()
            {
                self.inner.pending_events.lock().await.push_front(next);
                break;
            }
        }
    }

    async fn publish_lifecycle(
        &self,
        state: &AppState,
        event_type: &'static str,
        level: &'static str,
        component: &str,
        reason_code: &str,
        instance_id: Option<&str>,
    ) {
        let mut fields = Map::new();
        if component == "management" {
            fields.insert("version".to_string(), json!(APP_LOCAL_VERSION));
            fields.insert("pid".to_string(), json!(std::process::id()));
        }
        self.inner.logger.log(
            level,
            component,
            match event_type {
                "FN_EVENT_RUNTIME_STARTED" => "started",
                "FN_EVENT_RUNTIME_STOPPED" => "stopped",
                "FN_EVENT_RUNTIME_RESTARTED" => "restarted",
                _ => "lifecycle",
            },
            reason_code,
            fields,
        );
        self.publish_or_buffer(
            state,
            RuntimeEventInput {
                event_type,
                level,
                component: component.to_string(),
                payload: json!({
                    "component": component,
                    "incident_id": uuid::Uuid::new_v4().simple().to_string(),
                    "instance_id": instance_id,
                    "supervisor": state.settings.runtime_target,
                    "reason_code": reason_code,
                }),
            },
        )
        .await;
    }

    async fn run_probe(&self, state: &AppState) {
        self.import_supervisor_hints(state).await;
        let checked_at = time_utils::now_iso();
        let storage_started = Instant::now();
        let (runtime_info, process_health, dataplane_health, auth_health, storage) = tokio::join!(
            tokio::time::timeout(PROBE_TIMEOUT, state.gateway.client.get_runtime_info()),
            tokio::time::timeout(
                PROBE_TIMEOUT,
                state.gateway.client.health_serving(GATEWAY_HEALTH_PROCESS),
            ),
            tokio::time::timeout(
                PROBE_TIMEOUT,
                state
                    .gateway
                    .client
                    .health_serving(GATEWAY_HEALTH_DATAPLANE),
            ),
            tokio::time::timeout(
                PROBE_TIMEOUT,
                state
                    .gateway
                    .client
                    .health_serving(GATEWAY_HEALTH_AUTH_BRIDGE),
            ),
            tokio::time::timeout(PROBE_TIMEOUT, state.storage.store.ping()),
        );

        let management = ProbeResult {
            ok: true,
            reason_code: "running",
            metadata: ProbeMetadata {
                process_state: Some(ProcessState::Running),
                version: Some(APP_LOCAL_VERSION.to_string()),
                commit: option_env!("FN_KNOCK_GIT_COMMIT").map(str::to_string),
                pid: Some(std::process::id()),
                instance_id: Some(self.inner.management_instance_id.clone()),
                started_at: Some(self.inner.process_started_at.clone()),
                uptime_ms: Some(self.inner.process_started.elapsed().as_millis() as u64),
                rss_bytes: current_process_rss_bytes(),
                ..ProbeMetadata::default()
            },
        };

        let runtime_value = match runtime_info {
            Ok(Ok(value)) => Some(value),
            _ => None,
        };
        let last_pid = self
            .inner
            .trackers
            .lock()
            .await
            .get("gateway_process")
            .and_then(|tracker| tracker.health.pid);
        let process_serving = matches!(process_health, Ok(Ok(true)));
        let process_metadata = runtime_value
            .as_ref()
            .map(runtime_metadata)
            .unwrap_or_else(|| ProbeMetadata {
                pid: last_pid,
                process_state: last_pid.map(|pid| match process_exists(pid) {
                    Some(true) => ProcessState::Running,
                    Some(false) => ProcessState::Stopped,
                    None => ProcessState::Unknown,
                }),
                ..ProbeMetadata::default()
            });
        let process_reason = if runtime_value.is_none() {
            match process_metadata.process_state {
                Some(ProcessState::Running) => "service_unresponsive",
                Some(ProcessState::Stopped) => "process_exited",
                _ => "runtime_info_unavailable",
            }
        } else if !process_serving {
            "grpc_health_not_serving"
        } else {
            "serving"
        };
        let gateway_process = ProbeResult {
            ok: runtime_value.is_some() && process_serving,
            reason_code: process_reason,
            metadata: process_metadata,
        };
        let gateway_dataplane = bool_probe(dataplane_health, "serving", "not_serving");
        let auth_bridge = bool_probe(auth_health, "connected", "not_connected");
        let storage = ProbeResult {
            ok: matches!(storage, Ok(Ok(()))),
            reason_code: if matches!(storage, Ok(Ok(()))) {
                "sqlite_ping_ok"
            } else {
                "sqlite_ping_failed"
            },
            metadata: ProbeMetadata {
                process_state: Some(ProcessState::NotApplicable),
                latency_ms: Some(storage_started.elapsed().as_millis() as u64),
                ..ProbeMetadata::default()
            },
        };
        let config_sync = ProbeResult {
            ok: state.gateway_config_synced(),
            reason_code: if state.gateway_config_synced() {
                "generation_synced"
            } else {
                "generation_pending"
            },
            metadata: ProbeMetadata {
                process_state: Some(ProcessState::NotApplicable),
                ..ProbeMetadata::default()
            },
        };

        self.apply_probe(state, "management", management, &checked_at)
            .await;
        self.apply_probe(state, "gateway_process", gateway_process, &checked_at)
            .await;
        let parent_unhealthy = self
            .inner
            .trackers
            .lock()
            .await
            .get("gateway_process")
            .is_some_and(|tracker| tracker.health.status == HealthStatus::Unhealthy);
        if parent_unhealthy {
            self.apply_blocked("gateway_dataplane", &checked_at).await;
            self.apply_blocked("auth_bridge", &checked_at).await;
        } else {
            self.apply_probe(state, "gateway_dataplane", gateway_dataplane, &checked_at)
                .await;
            self.apply_probe(state, "auth_bridge", auth_bridge, &checked_at)
                .await;
        }
        self.apply_probe(state, "storage", storage, &checked_at)
            .await;
        self.apply_probe(state, "config_sync", config_sync, &checked_at)
            .await;
        self.publish_snapshot(&checked_at).await;
        self.observe_gateway_instance(state).await;
        self.flush_pending(state).await;
    }

    async fn import_supervisor_hints(&self, state: &AppState) {
        let directory = &self.inner.supervisor_events_dir;
        let Ok(mut reader) = tokio::fs::read_dir(directory).await else {
            return;
        };
        let mut paths = Vec::new();
        let mut temporary_paths = Vec::new();
        while let Ok(Some(entry)) = reader.next_entry().await {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) == Some("json") {
                paths.push(path);
                if paths.len() > MAX_SUPERVISOR_HINTS * 2 {
                    cleanup_supervisor_paths(&mut paths, SUPERVISOR_HINT_TTL, MAX_SUPERVISOR_HINTS)
                        .await;
                }
            } else if path.extension().and_then(|value| value.to_str()) == Some("tmp") {
                temporary_paths.push(path);
                if temporary_paths.len() > MAX_SUPERVISOR_TEMP_FILES * 2 {
                    cleanup_supervisor_paths(
                        &mut temporary_paths,
                        SUPERVISOR_TEMP_TTL,
                        MAX_SUPERVISOR_TEMP_FILES,
                    )
                    .await;
                }
            }
        }
        cleanup_supervisor_paths(
            &mut temporary_paths,
            SUPERVISOR_TEMP_TTL,
            MAX_SUPERVISOR_TEMP_FILES,
        )
        .await;
        cleanup_supervisor_paths(&mut paths, SUPERVISOR_HINT_TTL, MAX_SUPERVISOR_HINTS).await;
        paths.sort();
        for path in paths {
            let Ok(metadata) = tokio::fs::metadata(&path).await else {
                continue;
            };
            if metadata.len() > 8 * 1024 {
                let _ = tokio::fs::remove_file(path).await;
                continue;
            }
            let Ok(raw) = tokio::fs::read(&path).await else {
                continue;
            };
            let Ok(hint) = serde_json::from_slice::<Value>(&raw) else {
                let _ = tokio::fs::remove_file(path).await;
                continue;
            };
            let component = hint
                .get("component")
                .and_then(Value::as_str)
                .filter(|component| matches!(*component, "management" | "gateway_process"));
            let event = hint.get("event").and_then(Value::as_str);
            if let (Some(component), Some("exited")) = (component, event) {
                if component == "management"
                    && self
                        .inner
                        .management_abnormal_reported
                        .swap(true, Ordering::AcqRel)
                {
                    let _ = tokio::fs::remove_file(path).await;
                    continue;
                }
                let reason_code = hint
                    .get("reason_code")
                    .and_then(Value::as_str)
                    .filter(|value| {
                        value
                            .chars()
                            .all(|char| char.is_ascii_alphanumeric() || matches!(char, '_' | '-'))
                    })
                    .unwrap_or("supervisor_exit");
                self.publish_or_buffer(
                    state,
                    RuntimeEventInput {
                        event_type: "FN_EVENT_RUNTIME_ABNORMAL_EXIT",
                        level: "ERROR",
                        component: component.to_string(),
                        payload: json!({
                            "component": component,
                            "incident_id": uuid::Uuid::new_v4().simple().to_string(),
                            "reason_code": reason_code,
                            "supervisor": state.settings.runtime_target,
                            "exit_code": hint.pointer("/fields/exit_code").cloned().unwrap_or(Value::Null),
                            "signal": hint.pointer("/fields/signal").cloned().unwrap_or(Value::Null),
                        }),
                    },
                )
                .await;
            }
            let _ = tokio::fs::remove_file(path).await;
        }
    }

    async fn apply_probe(&self, state: &AppState, id: &str, probe: ProbeResult, checked_at: &str) {
        let mut event = None;
        let mut log_transition = None;
        {
            let mut trackers = self.inner.trackers.lock().await;
            let Some(tracker) = trackers.get_mut(id) else {
                tracing::debug!(id, "runtime health tracker not registered; skipping probe");
                return;
            };
            let previous_status = tracker.health.status.clone();
            apply_metadata(&mut tracker.health, probe.metadata);
            tracker.health.last_checked_at = Some(checked_at.to_string());
            tracker.health.reason_code = Some(probe.reason_code.to_string());
            if probe.ok {
                tracker.health.last_success_at = Some(checked_at.to_string());
            }
            match advance_tracker(tracker, probe.ok) {
                Some(TrackerTransition::Failed(incident_id)) => {
                    event = Some(("FN_EVENT_RUNTIME_HEALTH_FAILED", "ERROR", incident_id, None));
                }
                Some(TrackerTransition::Recovered(incident)) => {
                    event = Some((
                        "FN_EVENT_RUNTIME_RECOVERED",
                        "INFO",
                        incident.id,
                        Some((time_utils::now_ms() - incident.started_at_ms).max(0)),
                    ));
                }
                None => {}
            }
            if tracker.health.status != previous_status {
                log_transition = Some((
                    previous_status,
                    tracker.health.status.clone(),
                    tracker
                        .health
                        .reason_code
                        .clone()
                        .unwrap_or_else(|| "unknown".to_string()),
                ));
            }
        }

        if let Some((previous, current, reason)) = log_transition {
            let level = match current {
                HealthStatus::Unhealthy => "ERROR",
                HealthStatus::Degraded | HealthStatus::Blocked => "WARN",
                _ => "INFO",
            };
            self.inner.logger.log(
                level,
                id,
                "health_transition",
                &reason,
                Map::from_iter([
                    ("previous_status".to_string(), json!(previous)),
                    ("status".to_string(), json!(current)),
                ]),
            );
        }

        if let Some((event_type, level, incident_id, duration_ms)) = event {
            let Some(health) = self
                .inner
                .trackers
                .lock()
                .await
                .get(id)
                .map(|tracker| tracker.health.clone())
            else {
                tracing::warn!(
                    id,
                    "runtime health tracker disappeared before event publish"
                );
                return;
            };
            self.publish_or_buffer(
                state,
                RuntimeEventInput {
                    event_type,
                    level,
                    component: id.to_string(),
                    payload: json!({
                        "component": id,
                        "incident_id": incident_id,
                        "instance_id": health.instance_id,
                        "reason_code": health.reason_code,
                        "supervisor": state.settings.runtime_target,
                        "duration_ms": duration_ms,
                        "pid": health.pid,
                        "process_state": health.process_state,
                        "consecutive_failures": health.consecutive_failures,
                    }),
                },
            )
            .await;
        }
    }

    async fn apply_blocked(&self, id: &str, checked_at: &str) {
        let mut trackers = self.inner.trackers.lock().await;
        let Some(tracker) = trackers.get_mut(id) else {
            tracing::debug!(
                id,
                "runtime health tracker not registered; skipping blocked state"
            );
            return;
        };
        let changed = tracker.health.status != HealthStatus::Blocked;
        tracker.health.status = HealthStatus::Blocked;
        tracker.health.process_state = ProcessState::NotApplicable;
        tracker.health.last_checked_at = Some(checked_at.to_string());
        tracker.health.reason_code = Some("gateway_process_unhealthy".to_string());
        tracker.health.consecutive_failures = 0;
        tracker.recovery_successes = 0;
        tracker.incident = None;
        drop(trackers);
        if changed {
            self.inner.logger.log(
                "WARN",
                id,
                "health_transition",
                "gateway_process_unhealthy",
                Map::from_iter([("status".to_string(), json!(HealthStatus::Blocked))]),
            );
        }
    }

    async fn publish_snapshot(&self, checked_at: &str) {
        let trackers = self.inner.trackers.lock().await;
        let components = trackers
            .iter()
            .map(|(id, tracker)| (id.clone(), tracker.health.clone()))
            .collect::<BTreeMap<_, _>>();
        let overall_status = overall_status(components.values().map(|component| &component.status));
        let mut snapshot = self.inner.snapshot.write().await;
        snapshot.overall_status = overall_status;
        snapshot.last_checked_at = Some(checked_at.to_string());
        snapshot.components = components;
        snapshot.logs = self.inner.logger.status();
    }

    async fn observe_gateway_instance(&self, state: &AppState) {
        let instance = self
            .inner
            .trackers
            .lock()
            .await
            .get("gateway_process")
            .and_then(|tracker| tracker.health.instance_id.clone());
        let Some(instance) = instance else { return };
        let mut seen = self.inner.seen_gateway_instance.lock().await;
        if seen.as_deref() == Some(instance.as_str()) {
            return;
        }
        let event_type = if seen.is_some() {
            "FN_EVENT_RUNTIME_RESTARTED"
        } else {
            "FN_EVENT_RUNTIME_STARTED"
        };
        let level = if seen.is_some() { "WARN" } else { "INFO" };
        self.publish_lifecycle(
            state,
            event_type,
            level,
            "gateway_process",
            if seen.is_some() {
                "instance_changed"
            } else {
                "instance_observed"
            },
            Some(&instance),
        )
        .await;
        if state
            .storage
            .store
            .set_string_value_with_optional_ttl(
                GATEWAY_INSTANCE_KEY,
                &instance,
                Some(RUNTIME_STATE_TTL_SECONDS),
            )
            .await
            .is_ok()
        {
            *seen = Some(instance);
        }
    }
}

async fn cleanup_supervisor_paths(paths: &mut Vec<PathBuf>, ttl: Duration, max_files: usize) {
    let mut retained = Vec::with_capacity(paths.len().min(max_files));
    for path in paths.drain(..) {
        let expired = tokio::fs::metadata(&path)
            .await
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|modified| modified.elapsed().ok())
            .is_some_and(|age| age > ttl);
        if expired {
            let _ = tokio::fs::remove_file(path).await;
        } else {
            retained.push(path);
        }
    }
    retained.sort();
    let excess = retained.len().saturating_sub(max_files);
    for path in retained.drain(..excess) {
        let _ = tokio::fs::remove_file(path).await;
    }
    *paths = retained;
}

fn advance_tracker(tracker: &mut Tracker, success: bool) -> Option<TrackerTransition> {
    if success {
        tracker.health.consecutive_failures = 0;
        if matches!(
            tracker.health.status,
            HealthStatus::Unhealthy | HealthStatus::Degraded
        ) {
            tracker.recovery_successes += 1;
            if tracker.recovery_successes >= 2 {
                tracker.health.status = HealthStatus::Healthy;
                return tracker.incident.take().map(TrackerTransition::Recovered);
            }
        } else {
            tracker.health.status = HealthStatus::Healthy;
            tracker.recovery_successes = 0;
        }
        return None;
    }

    tracker.recovery_successes = 0;
    tracker.health.consecutive_failures = tracker.health.consecutive_failures.saturating_add(1);
    if tracker.health.consecutive_failures < 3 {
        tracker.health.status = HealthStatus::Degraded;
        return None;
    }
    if tracker.health.status == HealthStatus::Unhealthy {
        return None;
    }
    tracker.health.status = HealthStatus::Unhealthy;
    let incident = Incident {
        id: uuid::Uuid::new_v4().simple().to_string(),
        started_at_ms: time_utils::now_ms(),
    };
    let id = incident.id.clone();
    tracker.incident = Some(incident);
    Some(TrackerTransition::Failed(id))
}

pub(crate) async fn start_runtime_monitor(state: AppState) -> anyhow::Result<()> {
    let runtime = state.runtime_health.clone();
    if let Err(error) = runtime.initialize_session(&state).await {
        runtime.inner.logger.log(
            "ERROR",
            "storage",
            "session_write_failed",
            "runtime_session_initialization_failed",
            Map::from_iter([("result".to_string(), json!("failed"))]),
        );
        runtime.inner.logger.flush().await;
        return Err(error);
    }
    runtime
        .publish_lifecycle(
            &state,
            "FN_EVENT_RUNTIME_STARTED",
            "INFO",
            "management",
            "process_start",
            Some(&runtime.inner.management_instance_id),
        )
        .await;
    let task_state = state.clone();
    state.spawn_background("runtime-health-monitor", async move {
        let state = task_state;
        let mut probe = tokio::time::interval(PROBE_INTERVAL);
        probe.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut heartbeat = tokio::time::interval(SESSION_HEARTBEAT_INTERVAL);
        heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut storage_gc = tokio::time::interval(STORAGE_TTL_GC_INTERVAL);
        storage_gc.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = state.shutdown.cancelled() => {
                    runtime.publish_lifecycle(
                        &state,
                        "FN_EVENT_RUNTIME_STOPPED",
                        "INFO",
                        "management",
                        "graceful_shutdown",
                        Some(&runtime.inner.management_instance_id),
                    ).await;
                    let _ = runtime.persist_session(&state, "stopped").await;
                    runtime.inner.logger.log("INFO", "management", "stopped", "graceful_shutdown", Map::new());
                    runtime.flush_pending(&state).await;
                    runtime.inner.logger.flush().await;
                    runtime.inner.monitor_stopped.store(true, Ordering::Release);
                    runtime.inner.monitor_done.notify_waiters();
                    break;
                }
                _ = probe.tick() => runtime.run_probe(&state).await,
                _ = heartbeat.tick() => {
                    if runtime.persist_current_session(&state).await.is_err() {
                        runtime.inner.logger.log(
                            "ERROR",
                            "storage",
                            "session_write_failed",
                            "runtime_session_heartbeat_failed",
                            Map::from_iter([("result".to_string(), json!("failed"))]),
                        );
                    }
                }
                _ = storage_gc.tick() => {
                    if let Err(error) = state.storage.store.purge_expired_keys().await {
                        tracing::warn!(%error, "failed to purge expired storage keys");
                        runtime.inner.logger.log(
                            "WARN",
                            "storage",
                            "ttl_gc_failed",
                            "sqlite_ttl_gc_failed",
                            Map::from_iter([("result".to_string(), json!("failed"))]),
                        );
                    }
                }
            }
        }
    });
    Ok(())
}

fn bool_probe(
    result: Result<anyhow::Result<bool>, tokio::time::error::Elapsed>,
    success_reason: &'static str,
    failure_reason: &'static str,
) -> ProbeResult {
    let ok = matches!(result, Ok(Ok(true)));
    ProbeResult {
        ok,
        reason_code: if ok { success_reason } else { failure_reason },
        metadata: ProbeMetadata {
            process_state: Some(ProcessState::NotApplicable),
            ..ProbeMetadata::default()
        },
    }
}

fn runtime_metadata(value: &Value) -> ProbeMetadata {
    ProbeMetadata {
        process_state: Some(ProcessState::Running),
        version: Some(APP_LOCAL_VERSION.to_string()),
        commit: option_env!("FN_KNOCK_GATEWAY_COMMIT").map(str::to_string),
        pid: value
            .get("pid")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok()),
        instance_id: value
            .get("instance_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        started_at: value
            .get("started_at_unix_ms")
            .and_then(Value::as_i64)
            .map(time_utils::iso_from_ms),
        uptime_ms: value.get("uptime_ms").and_then(Value::as_u64),
        go_version: value
            .get("go_version")
            .and_then(Value::as_str)
            .map(str::to_string),
        goroutines: value.get("goroutines").and_then(Value::as_u64),
        rss_bytes: value
            .get("rss_bytes")
            .and_then(Value::as_u64)
            .filter(|value| *value > 0),
        heap_alloc_bytes: value.get("heap_alloc_bytes").and_then(Value::as_u64),
        heap_sys_bytes: value.get("heap_sys_bytes").and_then(Value::as_u64),
        ..ProbeMetadata::default()
    }
}

fn apply_metadata(health: &mut ComponentHealth, metadata: ProbeMetadata) {
    if let Some(value) = metadata.process_state {
        health.process_state = value;
    }
    if metadata.version.is_some() {
        health.version = metadata.version;
    }
    if metadata.commit.is_some() {
        health.commit = metadata.commit;
    }
    if metadata.pid.is_some() {
        health.pid = metadata.pid;
    }
    if metadata.instance_id.is_some() {
        health.instance_id = metadata.instance_id;
    }
    if metadata.started_at.is_some() {
        health.started_at = metadata.started_at;
    }
    if metadata.uptime_ms.is_some() {
        health.uptime_ms = metadata.uptime_ms;
    }
    if metadata.go_version.is_some() {
        health.go_version = metadata.go_version;
    }
    if metadata.goroutines.is_some() {
        health.goroutines = metadata.goroutines;
    }
    if metadata.rss_bytes.is_some() {
        health.rss_bytes = metadata.rss_bytes;
    }
    if metadata.heap_alloc_bytes.is_some() {
        health.heap_alloc_bytes = metadata.heap_alloc_bytes;
    }
    if metadata.heap_sys_bytes.is_some() {
        health.heap_sys_bytes = metadata.heap_sys_bytes;
    }
    if metadata.latency_ms.is_some() {
        health.latency_ms = metadata.latency_ms;
    }
}

fn overall_status<'a>(statuses: impl Iterator<Item = &'a HealthStatus>) -> HealthStatus {
    let mut has_degraded = false;
    let mut has_unknown = false;
    for status in statuses {
        match status {
            HealthStatus::Unhealthy => return HealthStatus::Unhealthy,
            HealthStatus::Degraded | HealthStatus::Blocked => has_degraded = true,
            HealthStatus::Unknown => has_unknown = true,
            HealthStatus::Healthy => {}
        }
    }
    if has_degraded {
        HealthStatus::Degraded
    } else if has_unknown {
        HealthStatus::Unknown
    } else {
        HealthStatus::Healthy
    }
}

#[cfg(target_os = "linux")]
fn current_process_rss_bytes() -> Option<u64> {
    let raw = fs::read_to_string("/proc/self/statm").ok()?;
    let resident_pages = raw.split_whitespace().nth(1)?.parse::<u64>().ok()?;
    // SAFETY: sysconf is read-only and does not retain any pointers.
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    (page_size > 0).then(|| resident_pages.saturating_mul(page_size as u64))
}

#[cfg(target_os = "macos")]
fn current_process_rss_bytes() -> Option<u64> {
    let mut info = std::mem::MaybeUninit::<libc::proc_taskinfo>::zeroed();
    let size = std::mem::size_of::<libc::proc_taskinfo>();
    // SAFETY: proc_pidinfo writes at most `size` bytes into the valid buffer.
    let written = unsafe {
        libc::proc_pidinfo(
            std::process::id() as i32,
            libc::PROC_PIDTASKINFO,
            0,
            info.as_mut_ptr().cast(),
            size as i32,
        )
    };
    if written != size as i32 {
        return None;
    }
    // SAFETY: a full proc_taskinfo value was initialized above.
    Some(unsafe { info.assume_init() }.pti_resident_size)
}

#[cfg(windows)]
fn current_process_rss_bytes() -> Option<u64> {
    use windows_sys::Win32::System::{
        ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS},
        Threading::GetCurrentProcess,
    };
    let mut counters = std::mem::MaybeUninit::<PROCESS_MEMORY_COUNTERS>::zeroed();
    let size = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
    // SAFETY: the pseudo handle is valid for this process and the output buffer
    // is writable for exactly the size passed to the API.
    let ok = unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), counters.as_mut_ptr(), size) };
    (ok != 0).then(|| unsafe { counters.assume_init() }.WorkingSetSize as u64)
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn current_process_rss_bytes() -> Option<u64> {
    None
}

#[cfg(unix)]
fn process_exists(pid: u32) -> Option<bool> {
    // SAFETY: signal 0 performs existence/permission validation only.
    let result = unsafe { libc::kill(pid as libc::pid_t, 0) };
    if result == 0 {
        return Some(true);
    }
    match std::io::Error::last_os_error().raw_os_error() {
        Some(libc::EPERM) => Some(true),
        Some(libc::ESRCH) => Some(false),
        _ => None,
    }
}

#[cfg(windows)]
fn process_exists(pid: u32) -> Option<bool> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, STILL_ACTIVE},
        System::Threading::{GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
    };
    // SAFETY: the returned handle is checked and closed on every path.
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return Some(false);
        }
        let mut exit_code = 0u32;
        let ok = GetExitCodeProcess(handle, &mut exit_code) != 0;
        CloseHandle(handle);
        ok.then_some(exit_code == STILL_ACTIVE as u32)
    }
}

#[cfg(not(any(unix, windows)))]
fn process_exists(_pid: u32) -> Option<bool> {
    None
}

impl DiagnosticLogger {
    fn new(directory: PathBuf) -> anyhow::Result<Self> {
        fs::create_dir_all(&directory)?;
        set_private_dir_permissions(&directory)?;
        let (info_tx, info_rx) = mpsc::channel(LOG_QUEUE_SIZE);
        let (high_tx, high_rx) = mpsc::channel(LOG_HIGH_QUEUE_SIZE);
        let (control_tx, control_rx) = mpsc::channel(4);
        let path = directory.join("management.jsonl");
        let writer = tokio::spawn(diagnostic_writer(path, info_rx, high_rx, control_rx));
        Ok(Self {
            info: info_tx,
            high: high_tx,
            dropped_info: Arc::new(AtomicU64::new(0)),
            directory,
            repeats: Arc::new(StdMutex::new(HashMap::new())),
            control: control_tx,
            writer: Arc::new(Mutex::new(Some(writer))),
        })
    }

    fn disabled(directory: PathBuf) -> Self {
        let (info, info_rx) = mpsc::channel(1);
        let (high, high_rx) = mpsc::channel(1);
        let (control, control_rx) = mpsc::channel(1);
        drop((info_rx, high_rx, control_rx));
        Self {
            info,
            high,
            dropped_info: Arc::new(AtomicU64::new(0)),
            directory,
            repeats: Arc::new(StdMutex::new(HashMap::new())),
            control,
            writer: Arc::new(Mutex::new(None)),
        }
    }

    fn log(
        &self,
        level: &str,
        component: &str,
        event: &str,
        reason_code: &str,
        mut fields: Map<String, Value>,
    ) {
        let key = format!("{component}\0{event}\0{reason_code}");
        let mut count = 1;
        if let Ok(mut repeats) = self.repeats.lock() {
            let now = Instant::now();
            repeats.retain(|_, entry| now.saturating_duration_since(entry.last) <= LOG_REPEAT_TTL);
            if let Some(previous) = repeats.get_mut(&key) {
                if now.saturating_duration_since(previous.last) < LOG_REPEAT_WINDOW {
                    previous.suppressed = previous.suppressed.saturating_add(1);
                    return;
                }
                count += previous.suppressed;
                *previous = RepeatEntry {
                    last: now,
                    suppressed: 0,
                };
            } else {
                if repeats.len() >= MAX_LOG_REPEAT_KEYS
                    && let Some(oldest) = repeats
                        .iter()
                        .min_by_key(|(_, entry)| entry.last)
                        .map(|(key, _)| key.clone())
                {
                    repeats.remove(&oldest);
                }
                repeats.insert(
                    key,
                    RepeatEntry {
                        last: now,
                        suppressed: 0,
                    },
                );
            }
        }
        if count > 1 {
            fields.insert("count".to_string(), json!(count));
        }
        fields.retain(|key, _| {
            matches!(
                key.as_str(),
                "version"
                    | "commit"
                    | "pid"
                    | "status"
                    | "previous_status"
                    | "duration_ms"
                    | "generation"
                    | "count"
                    | "result"
                    | "protocol_version"
            )
        });
        let record = json!({
            "time": time_utils::now_iso(),
            "level": normalize_level(level),
            "component": clean_identifier(component),
            "event": clean_identifier(event),
            "reason_code": clean_identifier(reason_code),
            "fields": fields,
        });
        let Ok(mut bytes) = serde_json::to_vec(&record) else {
            return;
        };
        bytes.push(b'\n');
        if bytes.len() > 8 * 1024 {
            return;
        }
        if matches!(normalize_level(level), "WARN" | "ERROR") {
            let _ = self.high.try_send(bytes);
        } else if self.info.try_send(bytes).is_err() {
            self.dropped_info.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn status(&self) -> RuntimeLogStatus {
        if let Ok(mut repeats) = self.repeats.lock() {
            let now = Instant::now();
            repeats.retain(|_, entry| now.saturating_duration_since(entry.last) <= LOG_REPEAT_TTL);
        }
        let files = [
            "management.jsonl.1",
            "management.jsonl",
            "gateway.jsonl.1",
            "gateway.jsonl",
            "supervisor.jsonl.1",
            "supervisor.jsonl",
            "management-crash.log",
            "gateway-crash.log",
        ];
        let mut bytes_used = 0;
        let mut oldest = None;
        let mut newest = None;
        for file in files {
            if let Ok(metadata) = fs::metadata(self.directory.join(file)) {
                bytes_used += metadata.len();
                if let Ok(modified) = metadata.modified() {
                    oldest = Some(
                        oldest.map_or(modified, |value: std::time::SystemTime| value.min(modified)),
                    );
                    newest = Some(
                        newest.map_or(modified, |value: std::time::SystemTime| value.max(modified)),
                    );
                }
            }
        }
        RuntimeLogStatus {
            directory: "runtime/logs".to_string(),
            bytes_used,
            dropped_info: self.dropped_info.load(Ordering::Relaxed),
            oldest_at: oldest.map(system_time_iso),
            newest_at: newest.map(system_time_iso),
        }
    }

    async fn flush(&self) {
        let (sender, receiver) = oneshot::channel();
        if self
            .control
            .send(DiagnosticControl::Flush(sender))
            .await
            .is_ok()
        {
            let _ = tokio::time::timeout(Duration::from_secs(2), receiver).await;
        }
    }

    async fn clear(&self) -> anyhow::Result<()> {
        let (sender, receiver) = oneshot::channel();
        tokio::time::timeout(
            Duration::from_secs(2),
            self.control.send(DiagnosticControl::Clear(sender)),
        )
        .await
        .map_err(|_| anyhow::anyhow!("diagnostic writer clear queue timed out"))?
        .map_err(|_| anyhow::anyhow!("diagnostic writer is unavailable"))?;
        let result = tokio::time::timeout(Duration::from_secs(2), receiver)
            .await
            .map_err(|_| anyhow::anyhow!("diagnostic writer clear timed out"))?
            .map_err(|_| anyhow::anyhow!("diagnostic writer stopped during clear"))?;
        result.map_err(anyhow::Error::msg)?;
        self.dropped_info.store(0, Ordering::Relaxed);
        if let Ok(mut repeats) = self.repeats.lock() {
            repeats.clear();
        }
        Ok(())
    }

    async fn shutdown(&self, timeout: Duration) -> bool {
        let Some(mut writer) = self.writer.lock().await.take() else {
            return true;
        };
        let deadline = tokio::time::Instant::now() + timeout;
        let command_sent = matches!(
            tokio::time::timeout_at(deadline, self.control.send(DiagnosticControl::Shutdown)).await,
            Ok(Ok(()))
        );
        if command_sent && tokio::time::timeout_at(deadline, &mut writer).await.is_ok() {
            return true;
        }
        writer.abort();
        let _ = writer.await;
        false
    }
}

async fn diagnostic_writer(
    path: PathBuf,
    mut info: mpsc::Receiver<Vec<u8>>,
    mut high: mpsc::Receiver<Vec<u8>>,
    mut control: mpsc::Receiver<DiagnosticControl>,
) {
    let mut writer = RotatingFile::new(path.clone(), LOG_MAX_BYTES).ok();
    loop {
        let next = tokio::select! {
            biased;
            command = control.recv() => {
                match command {
                    Some(DiagnosticControl::Flush(ack)) => {
                        while let Ok(bytes) = high.try_recv() { if let Some(writer) = writer.as_mut() { let _ = writer.write(&bytes); } }
                        while let Ok(bytes) = info.try_recv() { if let Some(writer) = writer.as_mut() { let _ = writer.write(&bytes); } }
                        if let Some(writer) = writer.as_mut() { let _ = writer.flush(); }
                        let _ = ack.send(());
                        continue;
                    }
                    Some(DiagnosticControl::Clear(ack)) => {
                        while high.try_recv().is_ok() {}
                        while info.try_recv().is_ok() {}
                        if writer.is_none() {
                            writer = RotatingFile::new(path.clone(), LOG_MAX_BYTES).ok();
                        }
                        let result = writer
                            .as_mut()
                            .ok_or_else(|| "diagnostic log file is unavailable".to_string())
                            .and_then(|writer| writer.clear().map_err(|error| error.to_string()));
                        let _ = ack.send(result);
                        continue;
                    }
                    Some(DiagnosticControl::Shutdown) => {
                        while let Ok(bytes) = high.try_recv() { if let Some(writer) = writer.as_mut() { let _ = writer.write(&bytes); } }
                        while let Ok(bytes) = info.try_recv() { if let Some(writer) = writer.as_mut() { let _ = writer.write(&bytes); } }
                        if let Some(writer) = writer.as_mut() { let _ = writer.flush(); }
                        break;
                    }
                    None => None,
                }
            },
            value = high.recv() => value,
            value = info.recv() => value,
        };
        let Some(bytes) = next else {
            while let Ok(bytes) = high.try_recv() {
                if let Some(writer) = writer.as_mut() {
                    let _ = writer.write(&bytes);
                }
            }
            while let Ok(bytes) = info.try_recv() {
                if let Some(writer) = writer.as_mut() {
                    let _ = writer.write(&bytes);
                }
            }
            break;
        };
        if writer.is_none() {
            writer = RotatingFile::new(path.clone(), LOG_MAX_BYTES).ok();
        }
        if let Some(writer) = writer.as_mut() {
            let _ = writer.write(&bytes);
        }
    }
}

pub(crate) struct RotatingFile {
    path: PathBuf,
    max_bytes: u64,
    file: File,
    size: u64,
}

impl RotatingFile {
    pub(crate) fn new(path: PathBuf, max_bytes: u64) -> std::io::Result<Self> {
        cap_file_tail(&path, max_bytes)?;
        cap_file_tail(&rotated_path(&path), max_bytes)?;
        let file = private_append_file(&path)?;
        let size = file.metadata()?.len();
        Ok(Self {
            path,
            max_bytes,
            file,
            size,
        })
    }
    pub(crate) fn write(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        let bytes = if bytes.len() as u64 > self.max_bytes {
            &bytes[bytes.len() - self.max_bytes as usize..]
        } else {
            bytes
        };
        if self.size + bytes.len() as u64 > self.max_bytes {
            self.rotate()?;
        }
        self.file.write_all(bytes)?;
        self.size += bytes.len() as u64;
        Ok(())
    }
    fn rotate(&mut self) -> std::io::Result<()> {
        self.file.flush()?;
        let previous = rotated_path(&self.path);
        let _ = fs::remove_file(&previous);
        fs::rename(&self.path, previous)?;
        self.file = private_append_file(&self.path)?;
        self.size = 0;
        Ok(())
    }
    pub(crate) fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }

    fn clear(&mut self) -> std::io::Result<()> {
        self.file.flush()?;
        self.file.set_len(0)?;
        self.file.seek(SeekFrom::Start(0))?;
        self.size = 0;
        remove_file_if_exists(&rotated_path(&self.path))
    }
}

fn clear_external_rotating_log(path: &Path) -> std::io::Result<()> {
    if let Some(directory) = path.parent() {
        fs::create_dir_all(directory)?;
        set_private_dir_permissions(directory)?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)?;
    file.flush()?;
    set_private_file_permissions(path)?;
    remove_file_if_exists(&rotated_path(path))
}

fn remove_file_if_exists(path: &Path) -> std::io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn rotated_path(path: &Path) -> PathBuf {
    path.with_file_name(format!(
        "{}.1",
        path.file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("management.jsonl")
    ))
}

fn enforce_runtime_log_caps(directory: &Path) {
    for (name, max_bytes) in [
        ("management.jsonl", LOG_MAX_BYTES),
        ("management.jsonl.1", LOG_MAX_BYTES),
        ("gateway.jsonl", LOG_MAX_BYTES),
        ("gateway.jsonl.1", LOG_MAX_BYTES),
        ("supervisor.jsonl", AUXILIARY_LOG_MAX_BYTES),
        ("supervisor.jsonl.1", AUXILIARY_LOG_MAX_BYTES),
        ("management-crash.log", CRASH_MAX_BYTES),
        ("gateway-crash.log", CRASH_MAX_BYTES),
        ("gateway-console.log", AUXILIARY_LOG_MAX_BYTES),
        ("gateway-console.log.1", AUXILIARY_LOG_MAX_BYTES),
    ] {
        let _ = cap_file_tail(&directory.join(name), max_bytes);
    }
}

fn cap_file_tail(path: &Path, max_bytes: u64) -> std::io::Result<()> {
    let Ok(metadata) = fs::metadata(path) else {
        return Ok(());
    };
    if metadata.len() <= max_bytes {
        return Ok(());
    }
    let mut source = File::open(path)?;
    source.seek(SeekFrom::End(-(max_bytes as i64)))?;
    let mut tail = Vec::with_capacity(max_bytes as usize);
    source.take(max_bytes).read_to_end(&mut tail)?;
    if let Some(newline) = tail.iter().position(|byte| *byte == b'\n')
        && newline + 1 < tail.len()
    {
        tail.drain(..=newline);
    }
    let mut target = OpenOptions::new().write(true).truncate(true).open(path)?;
    target.write_all(&tail)?;
    target.flush()?;
    set_private_file_permissions(path)
}

fn private_append_file(path: &Path) -> std::io::Result<File> {
    let file = OpenOptions::new().create(true).append(true).open(path)?;
    set_private_file_permissions(path)?;
    Ok(file)
}

fn normalize_level(level: &str) -> &'static str {
    match level {
        "ERROR" => "ERROR",
        "WARN" | "WARNING" => "WARN",
        _ => "INFO",
    }
}

fn clean_identifier(value: &str) -> String {
    value
        .chars()
        .filter(|char| char.is_ascii_alphanumeric() || matches!(char, '_' | '-' | '.'))
        .take(128)
        .collect()
}

fn system_time_iso(value: std::time::SystemTime) -> String {
    let millis = value
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64;
    time_utils::iso_from_ms(millis)
}

#[cfg(unix)]
fn set_private_dir_permissions(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}
#[cfg(not(unix))]
fn set_private_dir_permissions(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(unix)]
fn set_private_file_permissions(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}
#[cfg(not(unix))]
fn set_private_file_permissions(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

pub(crate) fn install_panic_hook(data_dir: &Path) {
    static INSTALL: Once = Once::new();
    let crash_path = data_dir.join("runtime/logs/management-crash.log");
    INSTALL.call_once(move || {
        if let Some(parent) = crash_path.parent() {
            let _ = fs::create_dir_all(parent);
            let _ = set_private_dir_permissions(parent);
        }
        let _ = cap_file_tail(&crash_path, CRASH_MAX_BYTES);
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let location = info
                .location()
                .map(|location| {
                    format!(
                        "{}:{}",
                        Path::new(location.file())
                            .file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or("unknown"),
                        location.line()
                    )
                })
                .unwrap_or_else(|| "unknown".to_string());
            let message = info
                .payload()
                .downcast_ref::<&str>()
                .copied()
                .or_else(|| info.payload().downcast_ref::<String>().map(String::as_str))
                .unwrap_or("non-string panic");
            let message = routes::redact_string(message);
            let backtrace = redact_backtrace(&Backtrace::force_capture().to_string());
            let record = format!(
                "time={} version={} instance={} location={} panic={}\n{}\n",
                time_utils::now_iso(),
                APP_LOCAL_VERSION,
                PANIC_INSTANCE_ID
                    .get()
                    .map(String::as_str)
                    .unwrap_or("management"),
                location,
                truncate(&message, 512),
                backtrace
            );
            let record = truncate(&record, CRASH_MAX_BYTES as usize);
            let _ = cap_crash_file(&crash_path, record.len() as u64);
            if let Ok(mut file) = private_append_file(&crash_path) {
                let _ = file.write_all(record.as_bytes());
                let _ = file.flush();
            }
            previous(info);
        }));
    });
}

fn cap_crash_file(path: &Path, incoming_bytes: u64) -> std::io::Result<()> {
    if fs::metadata(path)
        .map(|metadata| metadata.len().saturating_add(incoming_bytes) > CRASH_MAX_BYTES)
        .unwrap_or(false)
    {
        File::create(path)?;
        set_private_file_permissions(path)?;
    }
    Ok(())
}

fn redact_backtrace(value: &str) -> String {
    value
        .lines()
        .take(128)
        .map(|line| {
            line.split_whitespace()
                .map(|part| {
                    if part.starts_with('/') || (part.len() > 2 && part.as_bytes()[1] == b':') {
                        part.rsplit(['/', '\\'])
                            .next()
                            .filter(|value| !value.is_empty())
                            .unwrap_or("[path]")
                            .to_string()
                    } else {
                        part.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn truncate(value: &str, max: usize) -> &str {
    if value.len() <= max {
        return value;
    }
    let mut boundary = max;
    while !value.is_char_boundary(boundary) {
        boundary -= 1;
    }
    &value[..boundary]
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn runtime_test_state() -> (tempfile::TempDir, AppState) {
        let directory = tempfile::tempdir().unwrap();
        let mut settings = crate::settings::Settings::from_env();
        settings.data_dir = directory.path().join("data");
        settings.sqlite_path = directory.path().join("fn-knock.sqlite3");
        settings.gateway_config_dir = directory.path().join("gateway");
        settings.legacy_redis_url.clear();
        settings.go_backend_grpc_addr = "127.0.0.1:1".to_string();
        settings.internal_rpc_token = "runtime-health-test".to_string();
        settings.altcha_hmac_key = Some("runtime-health-altcha-key".to_string());
        let state = AppState::new(settings).await.unwrap();
        (directory, state)
    }

    #[test]
    fn overall_status_prioritizes_failure_and_degradation() {
        assert_eq!(
            overall_status([&HealthStatus::Healthy, &HealthStatus::Healthy].into_iter()),
            HealthStatus::Healthy
        );
        assert_eq!(
            overall_status([&HealthStatus::Healthy, &HealthStatus::Blocked].into_iter()),
            HealthStatus::Degraded
        );
        assert_eq!(
            overall_status([&HealthStatus::Degraded, &HealthStatus::Unhealthy].into_iter()),
            HealthStatus::Unhealthy
        );
    }

    #[test]
    fn backtrace_redaction_removes_absolute_paths() {
        let redacted = redact_backtrace(
            "1: fn_name at /Users/example/project/src/main.rs:42\n2: C:\\secret\\main.rs:7",
        );
        assert!(!redacted.contains("/Users/example"));
        assert!(!redacted.contains("C:\\secret"));
    }

    #[test]
    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    fn current_process_rss_is_reported() {
        assert!(current_process_rss_bytes().is_some_and(|bytes| bytes > 0));
    }

    #[test]
    fn tracker_requires_three_failures_and_two_successes() {
        let mut tracker = Tracker {
            health: ComponentHealth::unknown("storage"),
            recovery_successes: 0,
            incident: None,
        };
        assert!(advance_tracker(&mut tracker, false).is_none());
        assert_eq!(tracker.health.status, HealthStatus::Degraded);
        assert!(advance_tracker(&mut tracker, false).is_none());
        assert!(matches!(
            advance_tracker(&mut tracker, false),
            Some(TrackerTransition::Failed(_))
        ));
        assert_eq!(tracker.health.status, HealthStatus::Unhealthy);
        assert!(advance_tracker(&mut tracker, false).is_none());
        assert!(advance_tracker(&mut tracker, true).is_none());
        assert_eq!(tracker.health.status, HealthStatus::Unhealthy);
        assert!(matches!(
            advance_tracker(&mut tracker, true),
            Some(TrackerTransition::Recovered(_))
        ));
        assert_eq!(tracker.health.status, HealthStatus::Healthy);
    }

    #[test]
    fn diagnostic_log_failure_does_not_block_runtime_initialization() {
        let directory = tempfile::tempdir().unwrap();
        let blocked_data_dir = directory.path().join("data-file");
        std::fs::write(&blocked_data_dir, b"not a directory").unwrap();
        let runtime = RuntimeHealth::new(&blocked_data_dir, "test").unwrap();
        assert_eq!(runtime.logs_dir(), blocked_data_dir.join("runtime/logs"));
        runtime.operational_log(
            "INFO",
            "management",
            "test",
            "logger_unavailable",
            Map::new(),
        );
        assert_eq!(runtime.inner.logger.dropped_info.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn diagnostic_logger_bounds_queue_and_aggregates_repeats() {
        let directory = tempfile::tempdir().unwrap();
        let (info, mut info_rx) = mpsc::channel(2);
        let (high, _high_rx) = mpsc::channel(1);
        let (control, _control_rx) = mpsc::channel(1);
        let logger = DiagnosticLogger {
            info,
            high,
            dropped_info: Arc::new(AtomicU64::new(0)),
            directory: directory.path().to_path_buf(),
            repeats: Arc::new(StdMutex::new(HashMap::new())),
            control,
            writer: Arc::new(Mutex::new(None)),
        };
        logger.log("INFO", "storage", "write_failed", "sqlite_busy", Map::new());
        logger.log("INFO", "storage", "write_failed", "sqlite_busy", Map::new());
        {
            let mut repeats = logger.repeats.lock().unwrap();
            let repeat = repeats
                .get_mut("storage\0write_failed\0sqlite_busy")
                .unwrap();
            repeat.last = Instant::now() - Duration::from_secs(61);
        }
        logger.log("INFO", "storage", "write_failed", "sqlite_busy", Map::new());
        let _first = info_rx.try_recv().unwrap();
        let aggregated: Value = serde_json::from_slice(&info_rx.try_recv().unwrap()).unwrap();
        assert_eq!(
            aggregated.pointer("/fields/count").and_then(Value::as_u64),
            Some(2)
        );

        logger.log("INFO", "management", "one", "queue_test", Map::new());
        logger.log("INFO", "management", "two", "queue_test", Map::new());
        logger.log("INFO", "management", "three", "queue_test", Map::new());
        assert_eq!(logger.dropped_info.load(Ordering::Relaxed), 1);

        {
            let mut repeats = logger.repeats.lock().unwrap();
            repeats.clear();
            repeats.insert(
                "expired".to_string(),
                RepeatEntry {
                    last: Instant::now() - LOG_REPEAT_TTL - Duration::from_secs(1),
                    suppressed: 0,
                },
            );
        }
        for index in 0..MAX_LOG_REPEAT_KEYS + 100 {
            logger.log(
                "INFO",
                "management",
                "bounded_repeat",
                &format!("reason_{index}"),
                Map::new(),
            );
        }
        let repeats = logger.repeats.lock().unwrap();
        assert_eq!(repeats.len(), MAX_LOG_REPEAT_KEYS);
        assert!(!repeats.contains_key("expired"));
    }

    #[test]
    fn rotating_diagnostic_file_stays_within_two_bounded_generations() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("management.jsonl");
        let previous = path.with_file_name("management.jsonl.1");
        std::fs::write(&path, [b'z'; 128]).unwrap();
        std::fs::write(&previous, [b'p'; 128]).unwrap();
        let mut writer = RotatingFile::new(path.clone(), 64).unwrap();
        assert!(std::fs::metadata(&path).unwrap().len() <= 64);
        assert!(std::fs::metadata(&previous).unwrap().len() <= 64);
        writer.write(&[b'x'; 40]).unwrap();
        writer.write(&[b'y'; 40]).unwrap();
        writer.write(&[b'w'; 96]).unwrap();
        writer.flush().unwrap();
        for file in [path.clone(), path.with_file_name("management.jsonl.1")] {
            let metadata = std::fs::metadata(&file).unwrap();
            assert!(metadata.len() <= 64);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
            }
        }
    }

    #[tokio::test]
    async fn clearing_management_log_discards_queue_rotation_and_counters() {
        let directory = tempfile::tempdir().unwrap();
        let logger = DiagnosticLogger::new(directory.path().to_path_buf()).unwrap();
        logger.log("INFO", "management", "ready", "serving", Map::new());
        logger.log("INFO", "management", "ready", "serving", Map::new());
        logger.dropped_info.store(3, Ordering::Relaxed);
        std::fs::write(directory.path().join("management.jsonl.1"), b"old\n").unwrap();

        logger.clear().await.unwrap();

        assert_eq!(
            std::fs::metadata(directory.path().join("management.jsonl"))
                .unwrap()
                .len(),
            0
        );
        assert!(!directory.path().join("management.jsonl.1").exists());
        assert_eq!(logger.dropped_info.load(Ordering::Relaxed), 0);
        assert!(logger.repeats.lock().unwrap().is_empty());
        assert!(logger.shutdown(Duration::from_secs(2)).await);
    }

    #[tokio::test]
    async fn diagnostic_logger_shutdown_flushes_pending_records_and_is_idempotent() {
        let directory = tempfile::tempdir().unwrap();
        let logger = DiagnosticLogger::new(directory.path().to_path_buf()).unwrap();
        logger.log(
            "INFO",
            "management",
            "stopped",
            "graceful_shutdown",
            Map::new(),
        );

        assert!(logger.shutdown(Duration::from_secs(2)).await);
        assert!(logger.shutdown(Duration::from_secs(2)).await);
        let contents = std::fs::read_to_string(directory.path().join("management.jsonl")).unwrap();
        assert!(contents.contains("\"reason_code\":\"graceful_shutdown\""));
    }

    #[test]
    fn clearing_external_log_keeps_current_file_and_removes_rotation() {
        let directory = tempfile::tempdir().unwrap();
        let current = directory.path().join("gateway.jsonl");
        let previous = directory.path().join("gateway.jsonl.1");
        std::fs::write(&current, b"current\n").unwrap();
        std::fs::write(&previous, b"previous\n").unwrap();

        clear_external_rotating_log(&current).unwrap();

        assert_eq!(std::fs::metadata(&current).unwrap().len(), 0);
        assert!(!previous.exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&current).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn startup_repairs_all_fixed_runtime_log_caps() {
        let directory = tempfile::tempdir().unwrap();
        for (name, max_bytes) in [
            ("management.jsonl", LOG_MAX_BYTES),
            ("management.jsonl.1", LOG_MAX_BYTES),
            ("gateway.jsonl", LOG_MAX_BYTES),
            ("gateway.jsonl.1", LOG_MAX_BYTES),
            ("supervisor.jsonl", AUXILIARY_LOG_MAX_BYTES),
            ("supervisor.jsonl.1", AUXILIARY_LOG_MAX_BYTES),
            ("management-crash.log", CRASH_MAX_BYTES),
            ("gateway-crash.log", CRASH_MAX_BYTES),
            ("gateway-console.log", AUXILIARY_LOG_MAX_BYTES),
            ("gateway-console.log.1", AUXILIARY_LOG_MAX_BYTES),
        ] {
            std::fs::write(
                directory.path().join(name),
                vec![b'x'; max_bytes as usize + 64],
            )
            .unwrap();
        }

        enforce_runtime_log_caps(directory.path());

        for entry in std::fs::read_dir(directory.path()).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let max_bytes =
                if name.starts_with("management.jsonl") || name.starts_with("gateway.jsonl") {
                    LOG_MAX_BYTES
                } else {
                    AUXILIARY_LOG_MAX_BYTES
                };
            assert!(entry.metadata().unwrap().len() <= max_bytes);
        }
    }

    #[tokio::test]
    async fn stale_running_session_emits_abnormal_exit_but_stopped_session_does_not() {
        let (_directory, state) = runtime_test_state().await;
        state
            .storage
            .store
            .set_string_value_with_optional_ttl(
                SESSION_KEY,
                &serde_json::to_string(&json!({
                    "instance_id": "previous",
                    "state": "running",
                    "heartbeat_at": "2026-01-01T00:00:00.000Z",
                }))
                .unwrap(),
                None,
            )
            .await
            .unwrap();

        state
            .runtime_health
            .initialize_session(&state)
            .await
            .unwrap();
        let starting_session = state
            .storage
            .store
            .get_string_value(SESSION_KEY)
            .await
            .unwrap()
            .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
            .unwrap();
        assert_eq!(
            starting_session.get("state").and_then(Value::as_str),
            Some("starting")
        );
        let session_ttl = state.storage.store.ttl_seconds(SESSION_KEY).await.unwrap();
        assert!(session_ttl > 0 && session_ttl <= RUNTIME_STATE_TTL_SECONDS);
        let events = state
            .storage
            .store
            .list_system_events(1, 10, "", None, None, Some("RUNTIME_MONITOR"))
            .await
            .unwrap();
        assert_eq!(events.get("total").and_then(Value::as_i64), Some(1));
        assert_eq!(
            events.pointer("/events/0/type").and_then(Value::as_str),
            Some("FN_EVENT_RUNTIME_ABNORMAL_EXIT")
        );

        state
            .runtime_health
            .mark_session_ready(&state)
            .await
            .unwrap();
        let running_session = state
            .storage
            .store
            .get_string_value(SESSION_KEY)
            .await
            .unwrap()
            .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
            .unwrap();
        assert_eq!(
            running_session.get("state").and_then(Value::as_str),
            Some("running")
        );

        state.storage.store.clear_system_events().await.unwrap();
        state
            .runtime_health
            .persist_session(&state, "stopped")
            .await
            .unwrap();
        state
            .runtime_health
            .initialize_session(&state)
            .await
            .unwrap();
        let events = state
            .storage
            .store
            .list_system_events(1, 10, "", None, None, Some("RUNTIME_MONITOR"))
            .await
            .unwrap();
        assert_eq!(events.get("total").and_then(Value::as_i64), Some(0));
    }

    #[tokio::test]
    async fn interrupted_starting_session_is_not_reported_as_an_abnormal_exit() {
        let (_directory, state) = runtime_test_state().await;
        state
            .storage
            .store
            .set_string_value(
                SESSION_KEY,
                &serde_json::to_string(&json!({
                    "instance_id": "interrupted-start",
                    "state": "starting",
                    "heartbeat_at": "2026-01-01T00:00:00.000Z",
                }))
                .unwrap(),
            )
            .await
            .unwrap();

        state
            .runtime_health
            .initialize_session(&state)
            .await
            .unwrap();

        let events = state
            .storage
            .store
            .list_system_events(1, 10, "", None, None, Some("RUNTIME_MONITOR"))
            .await
            .unwrap();
        assert_eq!(events.get("total").and_then(Value::as_i64), Some(0));
    }

    #[tokio::test]
    async fn supervisor_exit_hint_is_imported_once_with_exit_code() {
        let (_directory, state) = runtime_test_state().await;
        let hints = state.runtime_health.inner.supervisor_events_dir.clone();
        tokio::fs::create_dir_all(&hints).await.unwrap();
        let hint = hints.join("001-gateway-exit.json");
        tokio::fs::write(
            &hint,
            serde_json::to_vec(&json!({
                "component": "gateway_process",
                "event": "exited",
                "reason_code": "unexpected_exit",
                "fields": { "exit_code": 17, "signal": null },
            }))
            .unwrap(),
        )
        .await
        .unwrap();

        state.runtime_health.import_supervisor_hints(&state).await;
        assert!(!tokio::fs::try_exists(&hint).await.unwrap());
        let events = state
            .storage
            .store
            .list_system_events(1, 10, "", None, None, Some("RUNTIME_MONITOR"))
            .await
            .unwrap();
        assert_eq!(events.get("total").and_then(Value::as_i64), Some(1));
        assert_eq!(
            events.pointer("/events/0/type").and_then(Value::as_str),
            Some("FN_EVENT_RUNTIME_ABNORMAL_EXIT")
        );
        assert_eq!(
            events
                .pointer("/events/0/payload/exit_code")
                .and_then(Value::as_i64),
            Some(17)
        );
    }

    #[tokio::test]
    async fn supervisor_temporary_hints_are_not_imported_and_are_count_bounded() {
        let (_directory, state) = runtime_test_state().await;
        let hints = state.runtime_health.inner.supervisor_events_dir.clone();
        tokio::fs::create_dir_all(&hints).await.unwrap();
        for index in 0..MAX_SUPERVISOR_TEMP_FILES + 8 {
            tokio::fs::write(
                hints.join(format!(".hint-{index:03}.tmp")),
                br#"{"component":"management","event":"exited"}"#,
            )
            .await
            .unwrap();
        }

        state.runtime_health.import_supervisor_hints(&state).await;
        let mut reader = tokio::fs::read_dir(&hints).await.unwrap();
        let mut temporary_count = 0;
        while let Some(entry) = reader.next_entry().await.unwrap() {
            if entry.path().extension().and_then(|value| value.to_str()) == Some("tmp") {
                temporary_count += 1;
            }
        }
        assert_eq!(temporary_count, MAX_SUPERVISOR_TEMP_FILES);
        let events = state
            .storage
            .store
            .list_system_events(1, 10, "", None, None, Some("RUNTIME_MONITOR"))
            .await
            .unwrap();
        assert_eq!(events.get("total").and_then(Value::as_i64), Some(0));
    }

    #[tokio::test]
    async fn gateway_instance_change_emits_one_restart_event() {
        let (_directory, state) = runtime_test_state().await;
        *state
            .runtime_health
            .inner
            .seen_gateway_instance
            .lock()
            .await = Some("old-instance".to_string());
        state
            .runtime_health
            .inner
            .trackers
            .lock()
            .await
            .get_mut("gateway_process")
            .unwrap()
            .health
            .instance_id = Some("new-instance".to_string());

        state.runtime_health.observe_gateway_instance(&state).await;
        state.runtime_health.observe_gateway_instance(&state).await;
        let instance_ttl = state
            .storage
            .store
            .ttl_seconds(GATEWAY_INSTANCE_KEY)
            .await
            .unwrap();
        assert!(instance_ttl > 0 && instance_ttl <= RUNTIME_STATE_TTL_SECONDS);
        let events = state
            .storage
            .store
            .list_system_events(1, 10, "", None, None, Some("RUNTIME_MONITOR"))
            .await
            .unwrap();
        assert_eq!(events.get("total").and_then(Value::as_i64), Some(1));
        assert_eq!(
            events.pointer("/events/0/type").and_then(Value::as_str),
            Some("FN_EVENT_RUNTIME_RESTARTED")
        );
    }

    #[test]
    fn panic_hook_captures_and_redacts_unhandled_panic() {
        const CHILD_ENV: &str = "FN_KNOCK_RUST_CRASH_TEST_CHILD";
        const DIRECTORY_ENV: &str = "FN_KNOCK_RUST_CRASH_TEST_DIR";
        if std::env::var(CHILD_ENV).as_deref() == Ok("1") {
            let directory = PathBuf::from(std::env::var(DIRECTORY_ENV).unwrap());
            install_panic_hook(&directory);
            panic!("token=rust-crash-canary at /Users/example/project/main.rs:42");
        }

        let directory = tempfile::tempdir().unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg("runtime_health::tests::panic_hook_captures_and_redacts_unhandled_panic")
            .arg("--nocapture")
            .env(CHILD_ENV, "1")
            .env(DIRECTORY_ENV, directory.path())
            .status()
            .unwrap();
        assert!(!status.success());
        let crash =
            std::fs::read_to_string(directory.path().join("runtime/logs/management-crash.log"))
                .unwrap();
        assert!(!crash.contains("rust-crash-canary"));
        assert!(!crash.contains("/Users/example"));
        assert!(crash.contains("panic=[redacted]"));
    }
}
