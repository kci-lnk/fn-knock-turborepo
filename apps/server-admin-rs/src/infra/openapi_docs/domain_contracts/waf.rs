use serde::Serialize;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub(super) struct WafConfigData {
    enabled: bool,
    system_rules_auto_update_enabled: bool,
    common_location_exempt_enabled: bool,
    private_ip_exempt_enabled: bool,
    block_behavior: String,
    mode: String,
    active_bundle_id: String,
    rules_dir: String,
    paranoia_level: i64,
    executing_paranoia_level: i64,
    inbound_anomaly_threshold: i64,
    outbound_anomaly_threshold: i64,
    request_body_access: bool,
    request_body_limit_bytes: i64,
    request_body_in_memory_limit_bytes: i64,
    response_body_access: bool,
    disabled_hosts: Vec<String>,
    disabled_path_prefixes: Vec<String>,
    log_retention_days: i64,
    drain_interval_seconds: i64,
    #[schema(required = true)]
    updated_at: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct WafConfigUpdateData {
    enabled: Option<bool>,
    system_rules_auto_update_enabled: Option<bool>,
    common_location_exempt_enabled: Option<bool>,
    private_ip_exempt_enabled: Option<bool>,
    block_behavior: Option<String>,
    paranoia_level: Option<i64>,
    executing_paranoia_level: Option<i64>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct WafStatusData {
    enabled: bool,
    mode: String,
    loaded: bool,
    bundle_id: String,
    bundle_hash: String,
    loaded_at: String,
    rules_dir: String,
    pending_events: i64,
    last_error: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct WafManifestRuleData {
    filename: String,
    description: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct WafManifestRulesDescriptionData {
    rules: Option<Vec<WafManifestRuleData>>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct WafRemoteManifestData {
    rules_description: Option<WafManifestRulesDescriptionData>,
    packaging_time: Option<String>,
    zip_file: String,
    zip_hash: String,
    commit_hash: Option<String>,
    commit_date: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct WafRuleFileData {
    source: String,
    filename: String,
    description: String,
    recommended: bool,
    enabled: bool,
    size_bytes: u64,
    updated_at: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct WafRuleFileContentData {
    source: String,
    filename: String,
    description: String,
    recommended: bool,
    enabled: bool,
    size_bytes: u64,
    updated_at: String,
    content: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct WafSystemSyncStateData {
    zip_file: String,
    zip_hash: String,
    synced_at: String,
    #[schema(required = true)]
    packaging_time: Option<String>,
    #[schema(required = true)]
    commit_hash: Option<String>,
    #[schema(required = true)]
    commit_date: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct WafSystemDetailsData {
    #[schema(required = true)]
    manifest: Option<WafRemoteManifestData>,
    #[schema(required = true)]
    manifest_cached_at: Option<String>,
    #[schema(required = true)]
    manifest_last_checked_at: Option<String>,
    #[schema(required = true)]
    manifest_last_error: Option<String>,
    #[schema(required = true)]
    synced: Option<WafSystemSyncStateData>,
    update_available: bool,
    rules: Vec<WafRuleFileData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct WafCustomDetailsData {
    rules: Vec<WafRuleFileData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct WafDetailsData {
    config: WafConfigData,
    #[schema(required = true)]
    status: Option<WafStatusData>,
    rules_dir: String,
    system: WafSystemDetailsData,
    custom: WafCustomDetailsData,
}

#[derive(Serialize, ToSchema)]
pub(super) struct WafMatchedVariableData {
    variable: Option<String>,
    key: Option<String>,
    value_preview: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct WafRuleMatchData {
    id: i64,
    message: Option<String>,
    data: Option<String>,
    severity: Option<String>,
    phase: Option<i64>,
    file: Option<String>,
    line: Option<i64>,
    tags: Option<Vec<String>>,
    disruptive: bool,
    matched_variables: Option<Vec<WafMatchedVariableData>>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct WafInterruptionData {
    rule_id: Option<i64>,
    action: Option<String>,
    status: Option<i64>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct WafEventData {
    trace_id: String,
    transaction_id: Option<String>,
    time: String,
    mode: String,
    action: String,
    status: Option<i64>,
    client_ip: Option<String>,
    remote_addr: Option<String>,
    method: Option<String>,
    scheme: Option<String>,
    host: Option<String>,
    path: Option<String>,
    query: Option<String>,
    request_uri: Option<String>,
    user_agent: Option<String>,
    referer: Option<String>,
    route_type: Option<String>,
    route_key: Option<String>,
    upstream: Option<String>,
    bundle_id: Option<String>,
    bundle_hash: Option<String>,
    rule_ids: Option<Vec<i64>>,
    rules: Option<Vec<WafRuleMatchData>>,
    interruption: Option<WafInterruptionData>,
    error: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct WafDrainResultData {
    drained: i64,
    remaining: i64,
    skipped_reason: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct WafLogEntriesData {
    date: String,
    available_dates: Vec<String>,
    cursor: String,
    next_cursor: String,
    has_more: bool,
    limit: i64,
    total: i64,
    items: Vec<WafEventData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct WafLogDeleteData {
    date: String,
    deleted: bool,
    available_dates: Vec<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct WafRuleToggleBodyData {
    source: Option<String>,
    filenames: Option<Vec<String>>,
    enabled: bool,
}

#[derive(Serialize, ToSchema)]
pub(super) struct WafUploadFileData {
    filename: String,
    content_base64: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct WafUploadBodyData {
    files: Vec<WafUploadFileData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct WafLogDeleteBodyData {
    date: Option<String>,
}
