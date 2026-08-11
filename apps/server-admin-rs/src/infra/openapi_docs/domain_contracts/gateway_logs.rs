use serde::Serialize;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub(super) struct GatewayLoggingConfigData {
    enabled: bool,
    record_localhost: bool,
    max_days: i64,
    logs_dir: String,
    dropped_entries: u64,
    queue_size: i64,
    queue_depth: i64,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GatewayLoggingConfigUpdateData {
    enabled: bool,
    record_localhost: Option<bool>,
    max_days: i64,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GatewayLogDirectoryData {
    logs_dir: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GatewayLogDatesData {
    today: String,
    logs_dir: String,
    dates: Vec<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GatewayLogEntryData {
    time: String,
    level: String,
    method: String,
    scheme: String,
    host: String,
    path: String,
    query: String,
    request_uri: String,
    protocol: String,
    status: i32,
    duration_ms: i64,
    remote_ip: String,
    remote_addr: String,
    client_ip: String,
    user_agent: String,
    referer: String,
    logged_in: bool,
    auth_required: bool,
    auth_decision: String,
    auth_rule_group_id: String,
    auth_grant_state: String,
    auth_credential_id: String,
    auth_credential_name: String,
    auth_credential_method: String,
    auth_linked_totp_id: String,
    auth_linked_totp_name: String,
    access_mode: String,
    route_type: String,
    route_key: String,
    upstream: String,
    matched: bool,
    bytes_in: u64,
    bytes_out: u64,
    tls: bool,
    websocket: bool,
    ali_real_client_ip: String,
    eo_connecting_ip: String,
    x_forwarded_for: String,
    x_real_ip: String,
    waf_blocked: bool,
    waf_trace_id: String,
    waf_mode: String,
    waf_rule_ids: Vec<i32>,
    waf_action: String,
    waf_bundle: String,
    general_blacklist_blocked: bool,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GatewayLogEntriesData {
    date: String,
    logs_dir: String,
    available_dates: Vec<String>,
    pagination: String,
    page: i32,
    limit: i32,
    total: i32,
    cursor: String,
    next_cursor: String,
    has_more: bool,
    items: Vec<GatewayLogEntryData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GatewayLogDeleteBodyData {
    date: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GatewayLogDeleteData {
    date: String,
    logs_dir: String,
    deleted: bool,
    available_dates: Vec<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GatewayLogAnalyticsRangeData {
    from: String,
    to: String,
    timezone: String,
    granularity: String,
    available_dates: Vec<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GatewayLogAnalyticsSummaryData {
    requests: i64,
    unique_clients: i64,
    client_errors: i64,
    server_errors: i64,
    average_duration_ms: f64,
    p95_duration_ms: i64,
    bytes_in: u64,
    bytes_out: u64,
    server_error_rate: f64,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GatewayLogAnalyticsPointData {
    bucket_start: String,
    requests: i64,
    client_errors: i64,
    server_errors: i64,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GatewayLogAnalyticsBucketData {
    key: String,
    count: i64,
    share: f64,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GatewayLogAnalyticsRegionBucketData {
    key: String,
    count: i64,
    share: f64,
    #[schema(nullable = false)]
    country_code: Option<String>,
    #[schema(nullable = false)]
    province: Option<String>,
    #[schema(nullable = false)]
    city: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GatewayLogAnalyticsDimensionsData {
    paths: Vec<GatewayLogAnalyticsBucketData>,
    routes: Vec<GatewayLogAnalyticsBucketData>,
    hosts: Vec<GatewayLogAnalyticsBucketData>,
    upstreams: Vec<GatewayLogAnalyticsBucketData>,
    referrers: Vec<GatewayLogAnalyticsBucketData>,
    utm_sources: Vec<GatewayLogAnalyticsBucketData>,
    utm_mediums: Vec<GatewayLogAnalyticsBucketData>,
    utm_campaigns: Vec<GatewayLogAnalyticsBucketData>,
    devices: Vec<GatewayLogAnalyticsBucketData>,
    browsers: Vec<GatewayLogAnalyticsBucketData>,
    operating_systems: Vec<GatewayLogAnalyticsBucketData>,
    statuses: Vec<GatewayLogAnalyticsBucketData>,
    methods: Vec<GatewayLogAnalyticsBucketData>,
    latency_bands: Vec<GatewayLogAnalyticsBucketData>,
    auth_decisions: Vec<GatewayLogAnalyticsBucketData>,
    waf_actions: Vec<GatewayLogAnalyticsBucketData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GatewayLogAnalyticsGeoData {
    status: String,
    region_status: String,
    resolved_clients: i64,
    resolved_region_clients: i64,
    pending_clients: i64,
    total_clients: i64,
    coverage: f64,
    region_coverage: f64,
    refreshing: bool,
    items: Vec<GatewayLogAnalyticsBucketData>,
    regions: Vec<GatewayLogAnalyticsRegionBucketData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GatewayLogAnalyticsQualityData {
    invalid_entries: i64,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GatewayLogAnalyticsData {
    range: GatewayLogAnalyticsRangeData,
    summary: GatewayLogAnalyticsSummaryData,
    series: Vec<GatewayLogAnalyticsPointData>,
    dimensions: GatewayLogAnalyticsDimensionsData,
    geo: GatewayLogAnalyticsGeoData,
    quality: GatewayLogAnalyticsQualityData,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GatewayLogAnalyticsRefreshData {
    refreshing: bool,
}
