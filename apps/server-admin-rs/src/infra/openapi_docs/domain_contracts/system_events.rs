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
