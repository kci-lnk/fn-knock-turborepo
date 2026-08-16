use serde::Serialize;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub(super) struct WolFeatureConfigData {
    enabled: bool,
}

#[derive(Serialize, ToSchema)]
pub(super) struct WolFeatureConfigUpdateData {
    #[schema(nullable = false)]
    enabled: Option<bool>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct WolLocalRelayConfigData {
    enabled: bool,
    relay_id: String,
    key_version: u32,
    listen_address: String,
    port: u16,
    broadcast_destinations: Vec<String>,
    allowed_sources: Vec<String>,
    psk_configured: bool,
    updated_at: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct WolLocalRelayRuntimeData {
    enabled: bool,
    active: bool,
    #[schema(required = true)]
    listen_address: Option<String>,
    #[schema(required = true)]
    last_error: Option<String>,
    #[schema(required = true)]
    updated_at: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct WolLocalRelayData {
    config: WolLocalRelayConfigData,
    runtime: WolLocalRelayRuntimeData,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct WolLocalRelayInputData {
    enabled: bool,
    relay_id: String,
    key_version: u32,
    listen_address: String,
    port: u16,
    broadcast_destinations: Vec<String>,
    #[schema(nullable = false)]
    allowed_sources: Option<Vec<String>>,
    psk: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct WolLocalRelayPairBodyData {
    pairing_code: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct WolRelayInputData {
    name: String,
    address: String,
    #[schema(nullable = false)]
    port: Option<u16>,
    #[schema(nullable = false)]
    enabled: Option<bool>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct WolRelayData {
    id: String,
    name: String,
    address: String,
    port: u16,
    enabled: bool,
    key_version: u32,
    psk_configured: bool,
    created_at: String,
    updated_at: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct WolRelaySummaryData {
    id: String,
    name: String,
    address: String,
    port: u16,
    enabled: bool,
    psk_configured: bool,
}

#[derive(Serialize, ToSchema)]
pub(super) struct WolRelayListData {
    total: usize,
    items: Vec<WolRelayData>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct WolBootstrapData {
    pairing_code: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct WolRelayCredentialData {
    relay: WolRelayData,
    bootstrap: WolBootstrapData,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct WolDispatchData {
    request_id: String,
    #[schema(required = true)]
    relay_id: Option<String>,
    delivery_mode: String,
    target_id: Option<String>,
    status: String,
    attempts: u8,
    latency_ms: u64,
    acknowledged_at: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct WolTargetStatusData {
    state: String,
    #[schema(required = true)]
    checked_at: Option<String>,
    #[schema(required = true)]
    last_online_at: Option<String>,
    #[schema(required = true)]
    observed_ip: Option<String>,
    #[schema(required = true)]
    last_error: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct WolIntegrationRuntimeData {
    state: String,
    #[schema(required = true)]
    last_connected_at: Option<String>,
    #[schema(required = true)]
    last_message_at: Option<String>,
    #[schema(required = true)]
    last_error: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct WolBlinkerIntegrationData {
    enabled: bool,
    bind_component: bool,
    skip_tls_verify: bool,
    credential_configured: bool,
    runtime: WolIntegrationRuntimeData,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct WolBemfaIntegrationData {
    enabled: bool,
    topic: String,
    skip_tls_verify: bool,
    credential_configured: bool,
    runtime: WolIntegrationRuntimeData,
}

#[derive(Serialize, ToSchema)]
pub(super) struct WolTargetIntegrationsData {
    blinker: WolBlinkerIntegrationData,
    bemfa: WolBemfaIntegrationData,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct WolBlinkerIntegrationInputData {
    enabled: bool,
    device_key: Option<String>,
    #[schema(nullable = false)]
    bind_component: Option<bool>,
    #[schema(nullable = false)]
    skip_tls_verify: Option<bool>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct WolBemfaIntegrationInputData {
    enabled: bool,
    private_key: Option<String>,
    #[schema(nullable = false)]
    topic: Option<String>,
    #[schema(nullable = false)]
    skip_tls_verify: Option<bool>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct WolTargetIntegrationsInputData {
    blinker: Option<WolBlinkerIntegrationInputData>,
    bemfa: Option<WolBemfaIntegrationInputData>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct WolTargetSshInputData {
    enabled: bool,
    host: String,
    #[schema(nullable = false)]
    port: Option<u16>,
    username: String,
    platform: String,
    auth_method: String,
    host_key_algorithm: String,
    host_key_fingerprint: String,
    password: Option<String>,
    private_key: Option<String>,
    private_key_passphrase: Option<String>,
    #[schema(nullable = false)]
    clear_credential: Option<bool>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct WolTargetSshData {
    enabled: bool,
    host: String,
    port: u16,
    username: String,
    platform: String,
    auth_method: String,
    host_key_algorithm: String,
    host_key_fingerprint: String,
    credential_configured: bool,
    passphrase_configured: bool,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct WolSshConnectionTestData {
    authenticated: bool,
    privilege_ready: bool,
    latency_ms: u64,
    host_key_algorithm: String,
    host_key_fingerprint: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct WolShutdownData {
    target_id: String,
    status: String,
    platform: String,
    latency_ms: u64,
    requested_at: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct WolTargetInputData {
    name: String,
    mac: String,
    relay_id: Option<String>,
    broadcast_address: Option<String>,
    ip_address: Option<String>,
    #[schema(nullable = false)]
    enabled: Option<bool>,
    integrations: Option<WolTargetIntegrationsInputData>,
    ssh: Option<WolTargetSshInputData>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct WolTargetData {
    id: String,
    name: String,
    mac: String,
    #[schema(required = true)]
    relay_id: Option<String>,
    #[schema(required = true)]
    broadcast_address: Option<String>,
    #[schema(required = true)]
    ip_address: Option<String>,
    delivery_mode: String,
    enabled: bool,
    created_at: String,
    updated_at: String,
    #[schema(required = true)]
    relay: Option<WolRelaySummaryData>,
    status: WolTargetStatusData,
    integrations: WolTargetIntegrationsData,
    ssh: WolTargetSshData,
}

#[derive(Serialize, ToSchema)]
pub(super) struct WolTargetListData {
    total: usize,
    items: Vec<WolTargetData>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct WolDiscoveryBodyData {
    #[schema(nullable = false)]
    target_cidrs: Option<Vec<String>>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct WolLocalNetworkData {
    interface_name: String,
    address: String,
    cidr: String,
    scan_cidr: String,
    broadcast_address: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct WolDiscoveredDeviceData {
    ip: String,
    mac: String,
    interface_name: String,
    broadcast_address: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct WolDiscoveryProgressData {
    scanned_hosts: usize,
    total_hosts: usize,
    found_devices: usize,
    current_host: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct WolDiscoveryResultData {
    devices: Vec<WolDiscoveredDeviceData>,
    networks: Vec<WolLocalNetworkData>,
    duration_ms: u64,
    method: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct WolDiscoveryJobData {
    job_id: String,
    state: String,
    created_at: i64,
    updated_at: i64,
    networks: Vec<WolLocalNetworkData>,
    progress: WolDiscoveryProgressData,
    devices: Vec<WolDiscoveredDeviceData>,
    next_cursor: usize,
    #[schema(required = true)]
    result: Option<WolDiscoveryResultData>,
    #[schema(required = true)]
    error: Option<String>,
}
