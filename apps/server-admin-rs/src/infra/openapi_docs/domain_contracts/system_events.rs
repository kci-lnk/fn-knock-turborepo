use std::collections::HashMap;

use serde::Serialize;
use serde_json::Value;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub(super) struct SystemEventSubjectData {
    kind: String,
    id: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SystemEventData {
    id: String,
    trace_id: Option<String>,
    #[serde(rename = "type")]
    event_type: String,
    source: String,
    level: String,
    happened_at: String,
    dedupe_key: Option<String>,
    subject: Option<SystemEventSubjectData>,
    tags: Option<Vec<String>>,
    payload: HashMap<String, Value>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SystemEventListData {
    events: Vec<SystemEventData>,
    total: i64,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SystemEventDeleteBodyData {
    ids: Vec<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SystemEventClearData {
    deleted_count: i64,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SystemEventPublishBodyData {
    trace_id: Option<String>,
    #[serde(rename = "type")]
    event_type: String,
    source: String,
    level: Option<String>,
    happened_at: Option<String>,
    dedupe_key: Option<String>,
    dedupe_ttl_seconds: Option<f64>,
    subject: Option<SystemEventSubjectData>,
    tags: Option<Vec<String>>,
    payload: HashMap<String, Value>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SystemEventPublishResultData {
    success: bool,
    skipped: bool,
    #[schema(required = true)]
    data: Option<SystemEventData>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(super) enum TraceSourceStatusData {
    Found,
    NotFound,
    Unavailable,
}

#[derive(Serialize, ToSchema)]
pub(super) struct TraceSourceStatusesData {
    gateway_logs: TraceSourceStatusData,
    waf_logs: TraceSourceStatusData,
    system_events: TraceSourceStatusData,
    notifications: TraceSourceStatusData,
}

#[derive(Serialize, ToSchema)]
pub(super) struct TraceLookupData {
    trace_id: String,
    found: bool,
    #[schema(required = true)]
    request: Option<Value>,
    #[schema(required = true)]
    waf_event: Option<Value>,
    system_events: Vec<SystemEventData>,
    notification_triggers: Vec<Value>,
    notification_deliveries: Vec<Value>,
    sources: TraceSourceStatusesData,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct LoginBackoffData {
    ip: String,
    attempts: i64,
    blocked: bool,
    retry_after: Option<i64>,
    blocked_until: Option<i64>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct LoginBackoffResetBodyData {
    ip: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct LoginBackoffResetData {}
