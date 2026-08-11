use serde::Serialize;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub(super) struct DeepMonitorStartBodyData {
    host: String,
    #[schema(nullable = false)]
    duration_seconds: Option<i32>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DeepMonitorExtendBodyData {
    duration_seconds: i32,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DeepMonitorSessionData {
    id: String,
    host: String,
    state: String,
    started_at: String,
    deadline_at: String,
    stopped_at: String,
    stop_reason: String,
    bytes_stored: u64,
    event_count: u64,
    dropped_events: u64,
    quota_bytes: u64,
    payload_limit_bytes: u64,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DeepMonitorSessionListData {
    items: Vec<DeepMonitorSessionData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DeepMonitorEventSummaryData {
    id: String,
    session_id: String,
    sequence: u64,
    #[serde(rename = "type")]
    event_type: String,
    time: String,
    exchange_id: String,
    connection_id: String,
    host: String,
    method: String,
    path: String,
    status: i32,
    client_ip: String,
    identity: String,
    direction: String,
    payload_bytes: u64,
    truncated: bool,
    notice: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DeepMonitorEventListData {
    items: Vec<DeepMonitorEventSummaryData>,
    next_cursor: String,
    has_more: bool,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DeepMonitorPayloadRefData {
    part: String,
    observed_bytes: u64,
    captured_bytes: u64,
    truncated: bool,
    sha256: String,
    content_type: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DeepMonitorHeaderData {
    name: String,
    values: Vec<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DeepMonitorTimingData {
    total_ms: i64,
    dns_ms: i64,
    connect_ms: i64,
    tls_ms: i64,
    request_write_ms: i64,
    ttfb_ms: i64,
    upstream_read_ms: i64,
    auth_ms: i64,
    waf_ms: i64,
    route_ms: i64,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DeepMonitorWebSocketFrameData {
    direction: String,
    fin: bool,
    rsv1: bool,
    rsv2: bool,
    rsv3: bool,
    opcode: u32,
    masked: bool,
    mask_key: String,
    payload_length: u64,
    close_code: i32,
    close_reason: String,
    compressed: bool,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DeepMonitorEventData {
    #[schema(required = true)]
    summary: Option<DeepMonitorEventSummaryData>,
    scheme: String,
    protocol: String,
    request_uri: String,
    upstream: String,
    user_agent: String,
    referer: String,
    remote_addr: String,
    auth_credential_id: String,
    auth_credential_name: String,
    auth_credential_method: String,
    auth_linked_totp_id: String,
    auth_linked_totp_name: String,
    auth_decision: String,
    route_type: String,
    auth_rule_group_id: String,
    auth_grant_state: String,
    route_key: String,
    tls_version: String,
    tls_cipher: String,
    tls_server_name: String,
    tls_alpn: String,
    client_request_headers: Vec<DeepMonitorHeaderData>,
    upstream_request_headers: Vec<DeepMonitorHeaderData>,
    upstream_response_headers: Vec<DeepMonitorHeaderData>,
    client_response_headers: Vec<DeepMonitorHeaderData>,
    payloads: Vec<DeepMonitorPayloadRefData>,
    #[schema(required = true)]
    timing: Option<DeepMonitorTimingData>,
    #[schema(required = true)]
    websocket_frame: Option<DeepMonitorWebSocketFrameData>,
    websocket_subprotocol: String,
    websocket_extensions: String,
    error: String,
    waf_trace_id: String,
    waf_mode: String,
    waf_rule_ids: Vec<i32>,
    waf_action: String,
    waf_bundle: String,
    waf_blocked: bool,
    general_blacklist_blocked: bool,
    client_ip_source: String,
}
