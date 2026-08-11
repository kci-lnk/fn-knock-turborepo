use serde::Serialize;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct ScanDiscoveryCapabilityData {
    cpu_cores: usize,
    #[schema(required = true)]
    total_memory_mib: Option<u64>,
    #[schema(required = true)]
    available_memory_mib: Option<u64>,
    #[schema(required = true)]
    file_descriptor_limit: Option<u64>,
    safe_concurrency: usize,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct ScanDiscoverySettingsData {
    intensity_mode: String,
    configured_level: String,
    recommended_level: String,
    effective_level: String,
    configured_concurrency: usize,
    effective_concurrency: usize,
    capability: ScanDiscoveryCapabilityData,
}

#[derive(Serialize, ToSchema)]
pub(super) struct ScanDiscoverySettingsUpdateData {
    intensity_mode: String,
    intensity_level: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct ScanDiscoveryTargetData {
    cidr: String,
    label: String,
    source: String,
    host_count: u64,
    is_automatic: bool,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct ScanDiscoveryHostCandidateData {
    address: String,
    cidr: String,
    source: String,
    recommended: bool,
    included_in_automatic_scan: bool,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct ScanDiscoveryLimitsData {
    max_cidrs: usize,
    max_hosts: u64,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct ScanDiscoveryTargetsData {
    automatic_targets: Vec<ScanDiscoveryTargetData>,
    host_candidates: Vec<ScanDiscoveryHostCandidateData>,
    custom_targets: Vec<ScanDiscoveryTargetData>,
    selected_targets: Vec<ScanDiscoveryTargetData>,
    selection_mode: String,
    selected_cidrs: Vec<String>,
    effective_cidrs: Vec<String>,
    limits: ScanDiscoveryLimitsData,
}

#[derive(Serialize, ToSchema)]
pub(super) struct ScanDiscoveryTargetsUpdateData {
    #[schema(nullable = false)]
    custom_cidrs: Option<Vec<String>>,
    #[schema(nullable = false)]
    selected_cidrs: Option<Vec<String>>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct ScanDiscoverJobBodyData {
    target_cidrs: Vec<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct ScanDiscoverProxyRuleData {
    path: String,
    rewrite_html: bool,
    use_auth: bool,
    use_root_mode: bool,
    strip_path: bool,
    target: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct ScanDiscoverServiceDetailData {
    name: String,
    label: String,
    rule: ScanDiscoverProxyRuleData,
    is_default: bool,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct ScanDiscoveredServiceData {
    service_key: String,
    host: String,
    port: u16,
    http_status: u16,
    #[schema(nullable = false)]
    requires_basic_auth: Option<bool>,
    detail: ScanDiscoverServiceDetailData,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct ScanDiscoverMetaData {
    host: String,
    total_ports_scanned: usize,
    found_services: usize,
    scanned_hosts: usize,
    scan_host_count: usize,
    #[schema(required = true)]
    scan_scope: Option<String>,
    scan_cidrs: Vec<String>,
    port_range: String,
    intensity_mode: String,
    intensity_level: String,
    recommended_level: String,
    configured_concurrency: usize,
    effective_concurrency: usize,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct ScanDiscoverResultData {
    host: String,
    total_ports_scanned: usize,
    found_services: usize,
    scanned_hosts: usize,
    scan_host_count: usize,
    #[schema(required = true)]
    scan_scope: Option<String>,
    scan_cidrs: Vec<String>,
    intensity_mode: String,
    intensity_level: String,
    recommended_level: String,
    configured_concurrency: usize,
    effective_concurrency: usize,
    services: Vec<ScanDiscoveredServiceData>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct ScanDiscoverProgressData {
    scanned_ports: usize,
    total_ports: usize,
    scanned_hosts: usize,
    total_hosts: usize,
    #[schema(nullable = false)]
    current_host: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct ScanDiscoverJobData {
    job_id: String,
    state: String,
    created_at: i64,
    updated_at: i64,
    #[schema(required = true)]
    meta: Option<ScanDiscoverMetaData>,
    #[schema(required = true)]
    progress: Option<ScanDiscoverProgressData>,
    services: Vec<ScanDiscoveredServiceData>,
    next_cursor: usize,
    #[schema(required = true)]
    result: Option<ScanDiscoverResultData>,
    #[schema(required = true)]
    error: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct HostMappingsProbeBodyData {
    #[schema(nullable = false)]
    hosts: Option<Vec<String>>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct HostMappingProbeResultData {
    host: String,
    target: String,
    status: String,
    #[schema(nullable = false)]
    http_status: Option<u16>,
    #[schema(nullable = false)]
    error: Option<String>,
    latency_ms: i64,
}

#[derive(Serialize, ToSchema)]
pub(super) struct HostMappingsProbeData {
    results: Vec<HostMappingProbeResultData>,
}
