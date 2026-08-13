use serde::Serialize;
use serde_json::Value;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CloudflaredProcessResourceData {
    #[schema(required = true)]
    sampled_at: Option<String>,
    #[schema(required = true)]
    resident_kib: Option<u64>,
    #[schema(required = true)]
    peak_resident_kib: Option<u64>,
    #[schema(required = true)]
    threads: Option<u64>,
    #[schema(required = true)]
    system_available_kib: Option<u64>,
    #[schema(required = true)]
    cgroup_oom_kill_count: Option<u64>,
    #[schema(required = true)]
    cgroup_memory_fail_count: Option<u64>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CloudflaredSupervisorFailureData {
    at: String,
    #[schema(required = true)]
    started_at: Option<String>,
    reason: String,
    #[schema(required = true)]
    exit_code: Option<i32>,
    #[schema(required = true)]
    signal: Option<i32>,
    core_dumped: bool,
    uptime_ms: u64,
    #[schema(required = true)]
    diagnosis: Option<String>,
    #[schema(required = true)]
    resources: Option<CloudflaredProcessResourceData>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CloudflaredSupervisorData {
    state: String,
    desired_running: bool,
    running: bool,
    attached: bool,
    #[schema(required = true)]
    pid: Option<u32>,
    restart_count: u64,
    consecutive_failures: u32,
    #[schema(required = true)]
    next_restart_at: Option<String>,
    #[schema(required = true)]
    started_at: Option<String>,
    #[schema(required = true)]
    stopped_at: Option<String>,
    #[schema(required = true)]
    last_failure: Option<CloudflaredSupervisorFailureData>,
    #[schema(required = true)]
    last_message: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CloudflaredRuntimeStatusData {
    running: bool,
    #[schema(required = true)]
    pid: Option<u32>,
    desired_running: bool,
    supervisor: CloudflaredSupervisorData,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CloudflaredStatusData {
    initialized: bool,
    platform: String,
    running: bool,
    #[schema(required = true)]
    pid: Option<u32>,
    desired_running: bool,
    supervisor: CloudflaredSupervisorData,
}

#[derive(Serialize, ToSchema)]
pub(super) struct CloudflaredStartData {
    pid: u32,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CloudflaredPollData {
    cursor: i64,
    reset: bool,
    logs: Vec<String>,
    status: CloudflaredRuntimeStatusData,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CloudflareTunnelSummaryData {
    id: String,
    name: String,
    status: Option<String>,
    connections: Option<usize>,
    ownership: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CloudflaredConfigData {
    mode: String,
    protocol: String,
    api_token_configured: bool,
    tunnel_token_configured: bool,
    #[schema(required = true)]
    account_id: Option<String>,
    #[schema(required = true)]
    zone_id: Option<String>,
    #[schema(required = true)]
    zone_name: Option<String>,
    #[schema(required = true)]
    root_domain: Option<String>,
    #[schema(required = true)]
    tunnel: Option<CloudflareTunnelSummaryData>,
    optimization_enabled: bool,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CloudflaredConfigUpdateData {
    protocol: Option<String>,
    token: Option<String>,
    clear_token: Option<bool>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CloudflareCredentialBodyData {
    api_token: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CloudflareConnectionData {
    #[schema(required = true)]
    account_id: Option<String>,
    #[schema(required = true)]
    zone_id: Option<String>,
    #[schema(required = true)]
    zone_name: Option<String>,
    configured_root_domain: String,
    root_domain_drift: bool,
    #[schema(required = true)]
    remote_error: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct CloudflareDnsRecordData {
    id: String,
    name: String,
    content: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct CloudflareIngressData {
    hostname: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CloudflareManagedResourcesData {
    tunnel: Option<CloudflareTunnelSummaryData>,
    wildcard_dns: Option<CloudflareDnsRecordData>,
    ingress: Option<CloudflareIngressData>,
    updated_at: Option<String>,
    optimization: Option<Value>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CloudflareOptimizationCandidateData {
    ip: String,
    median_latency_ms: f64,
    jitter_ms: f64,
    loss_ratio: f64,
    download_mbps: f64,
    score: f64,
    #[schema(required = true)]
    verified_at: Option<String>,
    source_types: Vec<String>,
    source_hostnames: Vec<String>,
    #[schema(required = true)]
    colo: Option<String>,
    #[schema(required = true)]
    cf_ray: Option<String>,
    #[schema(required = true)]
    business_hostname: Option<String>,
    #[schema(required = true)]
    business_status: Option<u16>,
    #[schema(required = true)]
    business_colo: Option<String>,
    #[schema(required = true)]
    business_cf_ray: Option<String>,
    business_validated: bool,
    selected_at: Option<String>,
    source: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CloudflareOptimizationVantageData {
    id: String,
    label: String,
    #[schema(required = true)]
    public_ip: Option<String>,
    #[schema(required = true)]
    default_colo: Option<String>,
    measured_at: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct CloudflareOptimizationBuiltinSourceData {
    id: String,
    hostname: String,
    category: String,
    enabled: bool,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CloudflareOptimizationResolverDiagnosticData {
    provider: String,
    status: String,
    success_count: usize,
    failure_count: usize,
    #[schema(required = true)]
    last_error_code: Option<String>,
    #[schema(required = true)]
    last_error_detail: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CloudflareOptimizationCandidateSourcesData {
    official_ranges: bool,
    builtins: Vec<CloudflareOptimizationBuiltinSourceData>,
    custom_hostnames: Vec<String>,
    max_custom_hostnames: usize,
    resolution_policy: String,
    publish_policy: String,
    error: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CloudflareOptimizationScanData {
    id: String,
    status: String,
    phase: String,
    progress: i64,
    created_at: String,
    #[schema(required = true)]
    started_at: Option<String>,
    #[schema(required = true)]
    completed_at: Option<String>,
    #[schema(required = true)]
    completed_at_ms: Option<i64>,
    cancel_requested: bool,
    candidates: Vec<CloudflareOptimizationCandidateData>,
    #[schema(required = true)]
    recommended_ip: Option<String>,
    #[schema(required = true)]
    vantage: Option<CloudflareOptimizationVantageData>,
    source_warnings: Vec<String>,
    resolver_diagnostics: Vec<CloudflareOptimizationResolverDiagnosticData>,
    #[schema(required = true)]
    resolution_path: Option<String>,
    candidate_source_count: Option<usize>,
    business_validation_hostname: Option<String>,
    #[schema(required = true)]
    source_fingerprint: Option<String>,
    #[schema(required = true)]
    error_code: Option<String>,
    #[schema(required = true)]
    error: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CloudflareOptimizationDomainData {
    hostname: String,
    status: String,
    #[schema(required = true)]
    hostname_status: Option<String>,
    management_mode: String,
    #[schema(required = true)]
    ssl_status: Option<String>,
    #[schema(required = true)]
    custom_hostname_id: Option<String>,
    optimized: bool,
    action_required: bool,
    cleanup_pending: bool,
    #[schema(required = true)]
    conflict_resource_id: Option<String>,
    #[schema(required = true)]
    message_code: Option<String>,
    #[schema(required = true)]
    message_detail: Option<String>,
    #[schema(required = true)]
    message: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CloudflareOptimizationFallbackOriginData {
    origin: String,
    status: String,
    errors: Option<Vec<String>>,
    ownership: Option<String>,
    updated_at: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CloudflareOptimizationCapabilityProbeData {
    status: String,
    hostname: Option<String>,
    hostname_status: Option<String>,
    ssl_status: Option<String>,
    tested_ip: Option<String>,
    tested_at: Option<String>,
    reason_code: Option<String>,
    message_code: Option<String>,
    message_detail: Option<String>,
    message: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CloudflareOptimizationScheduleData {
    full_scan_interval_days: i64,
    health_check_interval_minutes: i64,
    #[schema(required = true)]
    next_full_scan_at: Option<String>,
    #[schema(required = true)]
    last_full_scan_at: Option<String>,
    #[schema(required = true)]
    last_health_at: Option<String>,
    health_failures: i64,
    #[schema(required = true)]
    last_switch_reason: Option<String>,
    #[schema(required = true)]
    last_error: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CloudflareOptimizationStateData {
    enabled: bool,
    beta: bool,
    ipv4_only: bool,
    #[schema(required = true)]
    selected: Option<CloudflareOptimizationCandidateData>,
    fallback_active: bool,
    publish_suppressed: bool,
    #[schema(required = true)]
    origin_hostname: Option<String>,
    #[schema(required = true)]
    edge_hostname: Option<String>,
    #[schema(required = true)]
    fallback_origin: Option<CloudflareOptimizationFallbackOriginData>,
    #[schema(required = true)]
    capability_probe: Option<CloudflareOptimizationCapabilityProbeData>,
    scan_ready: bool,
    #[schema(required = true)]
    scan_readiness_error_code: Option<String>,
    candidate_sources: CloudflareOptimizationCandidateSourcesData,
    #[schema(required = true)]
    vantage: Option<CloudflareOptimizationVantageData>,
    source_warnings: Vec<String>,
    resolver_diagnostics: Vec<CloudflareOptimizationResolverDiagnosticData>,
    #[schema(required = true)]
    resolution_path: Option<String>,
    domains: Vec<CloudflareOptimizationDomainData>,
    schedule: CloudflareOptimizationScheduleData,
    scans: Vec<CloudflareOptimizationScanData>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CloudflareManagedStateData {
    mode: String,
    api_token_configured: bool,
    tunnel_token_configured: bool,
    connection: CloudflareConnectionData,
    tunnels: Vec<CloudflareTunnelSummaryData>,
    managed: CloudflareManagedResourcesData,
    optimization: CloudflareOptimizationStateData,
    permissions: Vec<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CloudflareReconcileRequestData {
    action: Option<String>,
    tunnel_mode: Option<String>,
    tunnel_id: Option<String>,
    optimization_enabled: Option<bool>,
    delete_dedicated_tunnel: Option<bool>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CloudflareReconcileApplyBodyData {
    plan_id: String,
    takeover_resource_ids: Option<Vec<String>>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CloudflareReconcileJobData {
    id: String,
    plan_id: String,
    status: String,
    phase: String,
    progress: i64,
    created_at: String,
    #[schema(required = true)]
    started_at: Option<String>,
    #[schema(required = true)]
    completed_at: Option<String>,
    #[schema(required = true)]
    error_code: Option<String>,
    #[schema(required = true)]
    error: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CloudflareReconcileCapabilityData {
    required: bool,
    #[schema(required = true)]
    readable: Option<bool>,
    #[schema(required = true)]
    write_verified: Option<bool>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CloudflareReconcileCapabilitiesData {
    zone_read: CloudflareReconcileCapabilityData,
    tunnel_edit: CloudflareReconcileCapabilityData,
    dns_edit: CloudflareReconcileCapabilityData,
    ssl_certificates_edit: CloudflareReconcileCapabilityData,
}

#[derive(Serialize, ToSchema)]
pub(super) struct CloudflareReconcileOperationData {
    id: String,
    kind: String,
    action: String,
    target: String,
    owned: bool,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CloudflareReconcileConflictRecordData {
    #[serde(rename = "type")]
    #[schema(required = true)]
    record_type: Option<String>,
    #[schema(required = true)]
    content: Option<String>,
    #[schema(required = true)]
    proxied: Option<bool>,
    owner_kind: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct CloudflareReconcileConflictDesiredData {
    #[serde(rename = "type")]
    record_type: String,
    content: String,
    proxied: bool,
}

#[derive(Serialize, ToSchema)]
pub(super) struct CloudflareReconcileConflictDetailsData {
    records: Vec<CloudflareReconcileConflictRecordData>,
    desired: CloudflareReconcileConflictDesiredData,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CloudflareReconcileConflictData {
    id: String,
    kind: String,
    target: String,
    message_code: Option<String>,
    detail: Option<String>,
    message: String,
    takeover_allowed: bool,
    details: Option<CloudflareReconcileConflictDetailsData>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CloudflareReconcilePlanData {
    plan_id: String,
    expires_at: String,
    action: String,
    root_domain: String,
    account_id: String,
    zone_id: String,
    #[schema(required = true)]
    selected_tunnel_id: Option<String>,
    remote_fingerprint: String,
    capabilities: CloudflareReconcileCapabilitiesData,
    operations: Vec<CloudflareReconcileOperationData>,
    conflicts: Vec<CloudflareReconcileConflictData>,
    warnings: Vec<String>,
    warning_codes: Vec<String>,
    can_apply: bool,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CloudflareOptimizationSourceSettingsBodyData {
    official_ranges: Option<bool>,
    builtin_ids: Option<Vec<String>>,
    custom_hostnames: Option<Vec<String>>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct CloudflareOptimizationDomainBodyData {
    mode: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CloudflareOptimizationDomainUpdateData {
    hostname: String,
    mode: String,
    cleanup_pending: bool,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CloudflareOptimizationApplyBodyData {
    scan_id: String,
    candidate_ip: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct CloudflareOptimizationApplyData {
    selected: CloudflareOptimizationCandidateData,
    #[schema(required = true)]
    state: Option<Value>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CloudflareOptimizationFallbackData {
    fallback_active: bool,
}
