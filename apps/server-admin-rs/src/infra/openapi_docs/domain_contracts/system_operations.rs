use serde::Serialize;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub(super) struct CaptchaPowUncommonLocationData {
    enabled: bool,
    max_number: i64,
}

#[derive(Serialize, ToSchema)]
pub(super) struct CaptchaPowData {
    base_max_number: i64,
    uncommon_location: CaptchaPowUncommonLocationData,
}

#[derive(Serialize, ToSchema)]
pub(super) struct CaptchaTurnstileData {
    site_key: String,
    secret_key: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct CaptchaSettingsData {
    provider: String,
    widget_mode: String,
    pow: CaptchaPowData,
    turnstile: CaptchaTurnstileData,
}

#[derive(Serialize, ToSchema)]
pub(super) struct CaptchaPowUncommonLocationUpdateData {
    enabled: Option<bool>,
    max_number: Option<i64>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct CaptchaPowUpdateData {
    base_max_number: Option<i64>,
    uncommon_location: Option<CaptchaPowUncommonLocationUpdateData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct CaptchaTurnstileUpdateData {
    site_key: Option<String>,
    secret_key: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct CaptchaSettingsUpdateData {
    provider: Option<String>,
    widget_mode: Option<String>,
    pow: Option<CaptchaPowUpdateData>,
    turnstile: Option<CaptchaTurnstileUpdateData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct RunTypeUpdateData {
    run_type: i64,
    reverse_proxy_submode: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct AutoManageFirewallUpdateData {
    auto_manage_firewall: bool,
}

#[derive(Serialize, ToSchema)]
pub(super) struct AutoManageFirewallData {
    auto_manage_firewall: bool,
}

#[derive(Serialize, ToSchema)]
pub(super) struct AutoHttpsConfigData {
    enabled: bool,
}

#[derive(Serialize, ToSchema)]
pub(super) struct AutoHttpsUpdateData {
    enabled: Option<bool>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct AutoHttpsRuntimeData {
    enabled: bool,
    active: bool,
    status: String,
    listen_host: String,
    listen_port: u16,
    redirect_scheme: String,
    #[schema(required = true)]
    last_error: Option<String>,
    #[schema(required = true)]
    last_error_at: Option<String>,
    updated_at: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct AutoHttpsDetailsData {
    enabled: bool,
    runtime: AutoHttpsRuntimeData,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DefaultRouteData {
    default_route: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DefaultRouteUpdateData {
    path: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DefaultTunnelUpdateData {
    tunnel: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct FirewallAdditionalPortsUpdateData {
    ports: Vec<i64>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct FirewallAdditionalPortsData {
    additional_ports: Vec<i64>,
    automatic_ports: Vec<i64>,
    effective_ports: Vec<i64>,
    run_type: i64,
    applied_now: bool,
}

#[derive(Serialize, ToSchema)]
pub(super) struct FirewallResetBodyData {
    run_type: i64,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct FirewallResetData {
    run_type: i64,
    gateway_port: i64,
    exempt_ports: Vec<String>,
    whitelist_synced: usize,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct FirewallClearData {
    gateway_port: i64,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SyncRoutesData {
    synced_rules: usize,
    synced_host_rules: usize,
    synced_stream_rules: usize,
    synced_gateway_logging: bool,
    synced_waf: bool,
    waf_bundle_id: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct MaintenanceClearBodyData {
    confirmation: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct MaintenanceClearData {
    cleared_keys: usize,
    gateway_reset: bool,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct AccessEntryData {
    env: String,
    port: String,
    is_default: bool,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SystemClockIssueData {
    code: String,
    title: String,
    message: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct SystemClockStatusData {
    expected_time_zone: String,
    #[schema(required = true)]
    system_time_zone: Option<String>,
    #[schema(required = true)]
    checked_at: Option<String>,
    #[schema(required = true)]
    network_source: Option<String>,
    has_remote_time: bool,
    #[schema(required = true)]
    last_check_error: Option<String>,
    #[schema(required = true)]
    system_time_ms: Option<i64>,
    #[schema(required = true)]
    remote_time_ms: Option<i64>,
    #[schema(required = true)]
    system_beijing_time: Option<String>,
    #[schema(required = true)]
    remote_beijing_time: Option<String>,
    #[schema(required = true)]
    drift_ms: Option<i64>,
    drift_threshold_ms: i64,
    time_mismatch: bool,
    timezone_mismatch: bool,
    needs_attention: bool,
    issues: Vec<SystemClockIssueData>,
    checking: bool,
    sync_in_progress: bool,
    #[schema(required = true)]
    last_sync_at: Option<String>,
    #[schema(required = true)]
    last_sync_error: Option<String>,
    #[schema(required = true)]
    sync_summary: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SystemClockSyncResponseData {
    success: bool,
    message: String,
    data: SystemClockStatusData,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SystemAssetDownloadProgressData {
    status: String,
    percent: i64,
    #[schema(required = true)]
    error: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct CloudflaredAssetStatusData {
    supported: bool,
    platform: String,
    downloaded: bool,
    installation_status: String,
    target_version: String,
    progress: SystemAssetDownloadProgressData,
}

#[derive(Serialize, ToSchema)]
pub(super) struct FrpAssetStatusData {
    supported: bool,
    platform: String,
    downloaded: bool,
    progress: SystemAssetDownloadProgressData,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SystemAssetMutationResponseData {
    success: bool,
    message: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DnsmasqInstallStateData {
    status: String,
    progress: i64,
    message: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DnsmasqStatusData {
    installed: bool,
    service_active: bool,
    initialized: bool,
    version: String,
    install_state: DnsmasqInstallStateData,
}

#[derive(Serialize, ToSchema)]
pub(super) struct ProtocolMappingAvailabilityData {
    enabled: bool,
    start_time: String,
    end_time: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct ProtocolMappingRuntimeIssueData {
    code: String,
    message: String,
    #[schema(required = true)]
    protocol: Option<String>,
    #[schema(required = true)]
    listen_port: Option<u16>,
    #[schema(required = true)]
    target: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct ProtocolMappingFeatureData {
    enabled: bool,
    #[schema(required = true)]
    availability: Option<ProtocolMappingAvailabilityData>,
    runtime_issue: Option<ProtocolMappingRuntimeIssueData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct ProtocolMappingFeatureUpdateData {
    enabled: Option<bool>,
    availability: Option<ProtocolMappingAvailabilityData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct ProxyProtocolForceData {
    proxy_protocol_force: bool,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct RunModePromptPreferencesData {
    direct_to_reverse_proxy: bool,
    reverse_proxy_to_direct: bool,
    switch_to_subdomain: bool,
    subdomain_to_reverse_proxy: bool,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct RunModePromptPreferencesUpdateData {
    direct_to_reverse_proxy: Option<bool>,
    reverse_proxy_to_direct: Option<bool>,
    switch_to_subdomain: Option<bool>,
    subdomain_to_reverse_proxy: Option<bool>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SmartConnectConfigData {
    enabled: bool,
    selected_ipv4: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SmartConnectUpdateData {
    enabled: Option<bool>,
    selected_ipv4: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SmartConnectRuntimeData {
    selected_ipv4: String,
    synced_domains: Vec<String>,
    managed_rule_count: i64,
    #[schema(required = true)]
    last_sync_at: Option<String>,
    #[schema(required = true)]
    last_sync_error: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SmartConnectAvailabilityData {
    available: bool,
    reason: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SmartConnectInstallStateData {
    status: String,
    progress: i64,
    message: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SmartConnectDnsmasqData {
    installed: bool,
    service_active: bool,
    initialized: bool,
    version: String,
    install_state: SmartConnectInstallStateData,
    runtime: SmartConnectRuntimeData,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SmartConnectLocalIpData {
    label: String,
    value: String,
    #[serde(rename = "interface")]
    interface_name: String,
    netmask: String,
    #[schema(required = true)]
    prefix: Option<u8>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SmartConnectDetailsData {
    config: SmartConnectConfigData,
    availability: SmartConnectAvailabilityData,
    dnsmasq: SmartConnectDnsmasqData,
    domains: Vec<String>,
    local_ip_options: Vec<SmartConnectLocalIpData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct FnosShareBypassData {
    enabled: bool,
    upstream_timeout_ms: i64,
    validation_cache_ttl_seconds: i64,
    validation_lock_ttl_seconds: i64,
    session_ttl_seconds: i64,
}

#[derive(Serialize, ToSchema)]
pub(super) struct FnosShareBypassUpdateData {
    enabled: Option<bool>,
    upstream_timeout_ms: Option<i64>,
    validation_cache_ttl_seconds: Option<i64>,
    validation_lock_ttl_seconds: Option<i64>,
    session_ttl_seconds: Option<i64>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct FnosPortIconHijackData {
    enabled: bool,
    #[schema(required = true)]
    updated_at: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct FnosPortIconHijackUpdateData {
    enabled: Option<bool>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct FnosNetworkTuningConfigData {
    bbr_enabled: bool,
    mtu_probing_enabled: bool,
    #[schema(required = true)]
    previous_tcp_congestion_control: Option<String>,
    #[schema(required = true)]
    previous_default_qdisc: Option<String>,
    #[schema(required = true)]
    previous_tcp_mtu_probing: Option<String>,
    #[schema(required = true)]
    updated_at: Option<String>,
    #[schema(required = true)]
    last_error: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct FnosNetworkTuningKernelData {
    #[schema(required = true)]
    tcp_congestion_control: Option<String>,
    tcp_available_congestion_control: Vec<String>,
    #[schema(required = true)]
    default_qdisc: Option<String>,
    #[schema(required = true)]
    tcp_mtu_probing: Option<String>,
    mtu_probing_supported: bool,
    bbr_module_loaded: bool,
    bbr_supported: bool,
    bbr_active: bool,
    mtu_probing_active: bool,
}

#[derive(Serialize, ToSchema)]
pub(super) struct FnosNetworkTuningBbrData {
    desired_enabled: bool,
    active: bool,
    supported: bool,
    module_loaded: bool,
    #[schema(required = true)]
    current_congestion_control: Option<String>,
    #[schema(required = true)]
    current_default_qdisc: Option<String>,
    available_congestion_control: Vec<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct FnosNetworkTuningMtuData {
    desired_enabled: bool,
    active: bool,
    supported: bool,
    #[schema(required = true)]
    current_value: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct FnosNetworkTuningData {
    available: bool,
    #[schema(required = true)]
    blocked_reason_code: Option<String>,
    #[schema(required = true)]
    blocked_reason: Option<String>,
    managed_config_path: String,
    config: FnosNetworkTuningConfigData,
    state: FnosNetworkTuningKernelData,
    bbr: FnosNetworkTuningBbrData,
    mtu_probing: FnosNetworkTuningMtuData,
    #[schema(required = true)]
    last_error: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct FnosNetworkTuningUpdateData {
    #[schema(nullable = false)]
    bbr_enabled: Option<bool>,
    #[schema(nullable = false)]
    mtu_probing_enabled: Option<bool>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct FnosConnectWafAvailabilityData {
    available: bool,
    #[schema(required = true)]
    reason_code: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct FnosConnectWafConfigData {
    enabled: bool,
    #[schema(required = true)]
    updated_at: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct FnosConnectWafUpdateData {
    enabled: bool,
}

#[derive(Serialize, ToSchema)]
pub(super) struct FnosConnectWafLocalNetworksData {
    ipv4: Vec<String>,
    ipv6: Vec<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct FnosConnectWafRuntimeData {
    effective: bool,
    protected: bool,
    #[schema(required = true)]
    detected_http_port: Option<u16>,
    #[schema(required = true)]
    listener_port: Option<u16>,
    ipv4_redirect_active: bool,
    ipv6_redirect_active: bool,
    ipv4_relay_redirect_active: bool,
    ipv6_relay_redirect_active: bool,
    ipv4_direct_redirect_active: bool,
    ipv6_direct_redirect_active: bool,
    listener_guard_active: bool,
    #[schema(required = true)]
    local_networks: Option<FnosConnectWafLocalNetworksData>,
    waf_active: bool,
    #[schema(required = true)]
    waf_mode: Option<String>,
    cgroup_path: Option<String>,
    #[schema(required = true)]
    source: Option<String>,
    #[schema(required = true)]
    last_sync_at: Option<String>,
    #[schema(required = true)]
    last_error: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct FnosConnectWafData {
    availability: FnosConnectWafAvailabilityData,
    config: FnosConnectWafConfigData,
    runtime: FnosConnectWafRuntimeData,
}

#[derive(Serialize, ToSchema)]
pub(super) struct FnosCertificateSyncConfigData {
    auto_sync_enabled: bool,
}

#[derive(Serialize, ToSchema)]
pub(super) struct FnosCertificateSyncUpdateData {
    auto_sync_enabled: bool,
}

#[derive(Serialize, ToSchema)]
pub(super) struct FnosCertificateSyncBodyData {
    #[schema(nullable = false)]
    target_ids: Option<Vec<String>>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct FnosCertificateSyncSummaryData {
    synced: usize,
    skipped: usize,
    failed: usize,
    rolled_back: bool,
}

#[derive(Serialize, ToSchema)]
pub(super) struct FnosCertificateSyncRuntimeData {
    running: bool,
    #[schema(required = true)]
    last_sync_at: Option<i64>,
    #[schema(required = true)]
    last_result: Option<FnosCertificateSyncSummaryData>,
    #[schema(required = true)]
    last_error: Option<String>,
    failed_target_ids: Vec<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct FnosCertificateSyncAvailabilityData {
    available: bool,
    #[schema(required = true)]
    reason: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct FnosCertificateSyncCountsData {
    total: usize,
    syncable: usize,
    up_to_date: usize,
}

#[derive(Serialize, ToSchema)]
pub(super) struct FnosCertificateSyncLocalData {
    id: String,
    label: String,
    #[schema(required = true)]
    valid_from: Option<i64>,
    #[schema(required = true)]
    valid_to: Option<i64>,
    #[schema(required = true)]
    fingerprint: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct FnosCertificateSyncItemData {
    target_id: String,
    domain: String,
    san: Vec<String>,
    source: String,
    renewal: bool,
    #[schema(required = true)]
    valid_from: Option<i64>,
    #[schema(required = true)]
    valid_to: Option<i64>,
    #[schema(required = true)]
    fingerprint: Option<String>,
    status: String,
    #[schema(required = true)]
    reason: Option<String>,
    #[schema(required = true)]
    local: Option<FnosCertificateSyncLocalData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct FnosCertificateSyncDetailsData {
    availability: FnosCertificateSyncAvailabilityData,
    config: FnosCertificateSyncConfigData,
    runtime: FnosCertificateSyncRuntimeData,
    summary: FnosCertificateSyncCountsData,
    certificates: Vec<FnosCertificateSyncItemData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct FnosCertificateSyncResponseData {
    summary: FnosCertificateSyncSummaryData,
    details: FnosCertificateSyncDetailsData,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DashboardDisplayData {
    show_entry_status_module: bool,
    show_console_app_list: bool,
    sidebar_menu_order: Vec<String>,
    date_time_display_mode: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DashboardDisplayUpdateData {
    show_entry_status_module: Option<bool>,
    show_console_app_list: Option<bool>,
    sidebar_menu_order: Option<Vec<String>>,
    date_time_display_mode: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct DashboardStatsNowData {
    #[schema(required = true)]
    online: Option<i64>,
    #[schema(required = true)]
    error5xx_total: Option<f64>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct DashboardStatsTotalsData {
    in_bytes: f64,
    out_bytes: f64,
    error5xx: f64,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DashboardStatsErrorsData {
    error5xx1d: f64,
    error5xx1w: f64,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DashboardChartTooltipData {
    trigger: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DashboardChartLegendData {
    data: Vec<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DashboardChartAxisData {
    #[serde(rename = "type")]
    axis_type: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct DashboardChartSeriesData {
    name: String,
    #[serde(rename = "type")]
    series_type: String,
    show_symbol: bool,
    data: Vec<Vec<f64>>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct DashboardEchartsData {
    tooltip: DashboardChartTooltipData,
    legend: DashboardChartLegendData,
    x_axis: DashboardChartAxisData,
    y_axis: DashboardChartAxisData,
    series: Vec<DashboardChartSeriesData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DashboardTrafficData {
    echarts: DashboardEchartsData,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct DashboardStatsData {
    range_sec: i64,
    now: DashboardStatsNowData,
    totals: DashboardStatsTotalsData,
    errors: DashboardStatsErrorsData,
    traffic: DashboardTrafficData,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DashboardHostTrafficData {
    host: String,
    total_in: f64,
    total_out: f64,
    error_5xx: f64,
    active_ip_count: i64,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DashboardRealtimeData {
    total_in: f64,
    total_out: f64,
    active_conns: i64,
    error_5xx: f64,
    by_host: Vec<DashboardHostTrafficData>,
    by_stream: Vec<DashboardStreamTrafficData>,
    timestamp: i64,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DashboardStreamTrafficData {
    protocol: String,
    listen_port: i64,
    key: String,
    total_in: f64,
    total_out: f64,
    error_5xx: f64,
    active_conns: i64,
    active_ip_count: i64,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DashboardActiveIpData {
    ip: String,
    last_seen_at: String,
    active_conns: i64,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DashboardActiveIpsData {
    host: String,
    window_seconds: i64,
    items: Vec<DashboardActiveIpData>,
    timestamp: i64,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DashboardStreamActiveIpsData {
    protocol: String,
    listen_port: i64,
    key: String,
    window_seconds: i64,
    items: Vec<DashboardActiveIpData>,
    timestamp: i64,
}

#[derive(Serialize, ToSchema)]
pub(super) struct UpdateLatestData {
    version: String,
    update_available: bool,
    force_update: bool,
    download_url: String,
    sha256: String,
    download_url_arm64: String,
    sha256_arm64: String,
    release_notes: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct UpdateCheckData {
    #[schema(required = true)]
    last_checked_at: Option<i64>,
    #[schema(required = true)]
    error: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct UpdateDownloadData {
    status: String,
    percent: i64,
    downloaded_bytes: i64,
    #[schema(required = true)]
    total_bytes: Option<i64>,
    #[schema(required = true)]
    error: Option<String>,
    #[schema(required = true)]
    target_version: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct UpdateStatusData {
    github_url: String,
    local_version: String,
    #[schema(required = true)]
    latest: Option<UpdateLatestData>,
    update_enabled: bool,
    has_update: bool,
    force_update: bool,
    check: UpdateCheckData,
    download: UpdateDownloadData,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct UpdateConfirmData {
    version: String,
    completed_at: String,
}
