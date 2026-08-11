use std::collections::HashMap;

use serde::Serialize;
use serde_json::Value;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub(super) struct RuntimeComponentHealthData {
    id: String,
    status: String,
    process_state: String,
    #[schema(required = true)]
    version: Option<String>,
    #[schema(required = true)]
    commit: Option<String>,
    #[schema(required = true)]
    pid: Option<u32>,
    #[schema(required = true)]
    instance_id: Option<String>,
    #[schema(required = true)]
    started_at: Option<String>,
    #[schema(required = true)]
    uptime_ms: Option<u64>,
    #[schema(required = true)]
    last_checked_at: Option<String>,
    #[schema(required = true)]
    last_success_at: Option<String>,
    consecutive_failures: u32,
    #[schema(required = true)]
    reason_code: Option<String>,
    cpu_percent: Option<f64>,
    rss_bytes: Option<u64>,
    go_version: Option<String>,
    goroutines: Option<u64>,
    heap_alloc_bytes: Option<u64>,
    heap_sys_bytes: Option<u64>,
    latency_ms: Option<u64>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct RuntimeLogStatusData {
    directory: String,
    bytes_used: u64,
    dropped_info: u64,
    #[schema(required = true)]
    oldest_at: Option<String>,
    #[schema(required = true)]
    newest_at: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct RuntimeHealthSnapshotData {
    schema_version: u32,
    overall_status: String,
    #[schema(required = true)]
    last_checked_at: Option<String>,
    components: HashMap<String, RuntimeComponentHealthData>,
    logs: RuntimeLogStatusData,
    supervisor: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct RuntimeOperationalLogEntryData {
    time: String,
    level: String,
    component: String,
    event: String,
    reason_code: Option<String>,
    fields: Option<HashMap<String, Value>>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct RuntimeComponentLogsData {
    schema_version: u32,
    component: String,
    generated_at: String,
    entries: Vec<RuntimeOperationalLogEntryData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct RuntimeLogClearData {
    component: String,
    cleared_at: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct RuntimeSystemEventSubjectData {
    kind: String,
    id: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct RuntimeSystemEventData {
    id: String,
    #[serde(rename = "type")]
    event_type: String,
    source: String,
    level: String,
    happened_at: String,
    dedupe_key: Option<String>,
    subject: Option<RuntimeSystemEventSubjectData>,
    tags: Option<Vec<String>>,
    payload: HashMap<String, Value>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct RuntimePlatformData {
    os: String,
    arch: String,
    runtime_target: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct RuntimeDiagnosticsCollectionData {
    includes: Vec<String>,
    excludes: Vec<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct TypedConfigShadowStatusData {
    phase: String,
    healthy: bool,
    mismatch_count: u64,
}

#[derive(Serialize, ToSchema)]
pub(super) struct RuntimeStorageMigrationData {
    typed_config_shadow: TypedConfigShadowStatusData,
}

#[derive(Serialize, ToSchema)]
pub(super) struct RuntimeDiagnosticsData {
    schema_version: u32,
    generated_at: String,
    version: String,
    commit: String,
    platform: RuntimePlatformData,
    runtime: RuntimeHealthSnapshotData,
    recent_runtime_events: Vec<RuntimeSystemEventData>,
    storage_migration: RuntimeStorageMigrationData,
    collection: RuntimeDiagnosticsCollectionData,
}
