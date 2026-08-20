#![allow(dead_code)]

use serde::Serialize;
use serde_json::{Map, Value, json};
use utoipa::{OpenApi, ToSchema};

mod acme;
mod cloudflared;
mod ddns;
mod deep_monitor;
mod external_auth;
mod frpc;
mod gateway_logs;
mod gateway_security;
mod location_services;
mod notifications;
mod panel;
mod proxy_config;
mod runtime_health;
mod scan_discovery;
mod security_core;
mod ssh_security;
mod ssl;
mod system_events;
mod system_operations;
mod terminal;
mod waf;
mod whitelist;
mod wol;

use acme::*;
use cloudflared::*;
use ddns::*;
use deep_monitor::*;
use external_auth::*;
use frpc::*;
use gateway_logs::*;
use gateway_security::*;
use location_services::*;
use notifications::*;
use panel::*;
use proxy_config::*;
use runtime_health::*;
use scan_discovery::*;
use security_core::*;
use ssh_security::*;
use ssl::*;
use system_events::*;
use system_operations::*;
use terminal::*;
use waf::*;
use whitelist::*;
use wol::*;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct AuthModeStatusData {
    mode: String,
    totp_count: usize,
    account_count: usize,
    password_configured_count: usize,
    password_missing_count: usize,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct AuthModePreviewData {
    current_mode: String,
    target_mode: String,
    totp_count: usize,
    account_count: usize,
    create_account_count: usize,
    update_account_count: usize,
    password_configured_count: usize,
    password_missing_count: usize,
    blocking_issue_count: usize,
    password_required_before_switch: Option<bool>,
    missing_source_totp_count: Option<usize>,
}

#[derive(Serialize, ToSchema)]
struct AuthCredentialSettingsData {
    session_ttl_seconds: i64,
    remember_me_ttl_seconds: i64,
    post_login_ip_grant_mode: String,
    post_login_ip_grant_ttl_seconds: Option<i64>,
    session_ip_mobility_enabled: bool,
    session_ip_mobility_window_seconds: i64,
    passkey_bind_prompt_enabled: bool,
}

#[derive(Serialize, ToSchema)]
struct AuthCredentialSettingsUpdateData {
    session_ttl_seconds: Option<i64>,
    remember_me_ttl_seconds: Option<i64>,
    post_login_ip_grant_mode: Option<String>,
    post_login_ip_grant_ttl_seconds: Option<i64>,
    session_ip_mobility_enabled: Option<bool>,
    session_ip_mobility_window_seconds: Option<i64>,
    passkey_bind_prompt_enabled: Option<bool>,
}

#[derive(Serialize, ToSchema)]
struct TotpStreamAccessData {
    protocol: String,
    listen_port: u16,
}

#[derive(Serialize, ToSchema)]
struct TotpSubdomainAccessData {
    mode: String,
    hosts: Vec<String>,
    streams: Vec<TotpStreamAccessData>,
}

#[derive(Serialize, ToSchema)]
struct AccessScopesUpdateData {
    access_scopes: Vec<String>,
}

#[derive(Serialize, ToSchema)]
struct SubdomainAccessUpdateData {
    subdomain_access: TotpSubdomainAccessData,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct AuthAccountData {
    id: String,
    username: String,
    display_name: String,
    source_totp_id: String,
    source_totp_name: String,
    created_at: String,
    updated_at: String,
    #[serde(rename = "access_scopes")]
    access_scopes: Vec<String>,
    #[serde(rename = "subdomain_access")]
    subdomain_access: TotpSubdomainAccessData,
    password_configured: bool,
    totp_configured: bool,
}

#[derive(Serialize, ToSchema)]
struct AuthAccountsData {
    accounts: Vec<AuthAccountData>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct TotpCredentialData {
    id: String,
    secret: String,
    comment: String,
    created_at: String,
    #[serde(rename = "access_scopes")]
    access_scopes: Vec<String>,
    #[serde(rename = "subdomain_access")]
    subdomain_access: TotpSubdomainAccessData,
}

#[derive(Serialize, ToSchema)]
struct TotpStatusData {
    bound: bool,
    credentials: Vec<TotpCredentialData>,
}

#[derive(Serialize, ToSchema)]
struct TotpSetupData {
    secret: String,
    uri: String,
}

#[derive(Serialize, ToSchema)]
struct CredentialImportSummaryData {
    kind: Option<String>,
    login_mode: Option<String>,
    imported: usize,
    skipped_existing_id: usize,
    skipped_existing_secret: Option<usize>,
    skipped_existing_username: Option<usize>,
    skipped_file_duplicate: usize,
    invalid: usize,
    total: usize,
    password_total: Option<usize>,
    password_imported: Option<usize>,
    password_skipped_existing: Option<usize>,
    password_skipped_missing_account: Option<usize>,
    password_skipped_file_duplicate: Option<usize>,
    password_invalid: Option<usize>,
    totp_total: Option<usize>,
    totp_imported: Option<usize>,
    totp_skipped_existing_id: Option<usize>,
    totp_skipped_existing_secret: Option<usize>,
    totp_skipped_file_duplicate: Option<usize>,
    totp_invalid: Option<usize>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct PasskeyCredentialData {
    id: String,
    totp_id: String,
    public_key: String,
    counter: u32,
    transports: Option<Vec<String>>,
    device_name: String,
    created_at: String,
    last_used_at: Option<String>,
    webauthn_credential: Option<Value>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct AuthAccountTransferData {
    id: String,
    username: String,
    display_name: String,
    source_totp_id: String,
    created_at: String,
    updated_at: String,
    #[serde(rename = "access_scopes")]
    access_scopes: Vec<String>,
    #[serde(rename = "subdomain_access")]
    subdomain_access: TotpSubdomainAccessData,
}

#[derive(Serialize, ToSchema)]
struct AuthPasswordTransferData {
    #[serde(rename = "accountId")]
    account_id: String,
    algorithm: String,
    salt: String,
    hash: String,
    n: u32,
    r: u32,
    p: u32,
    key_length: usize,
    created_at: String,
    updated_at: String,
}

#[derive(Serialize, ToSchema)]
struct CredentialTransferData {
    kind: String,
    version: u64,
    login_mode: String,
    exported_at: String,
    app_version: Option<String>,
    credentials: Option<Vec<TotpCredentialData>>,
    accounts: Option<Vec<AuthAccountTransferData>>,
    password_credentials: Option<Vec<AuthPasswordTransferData>>,
    totp_credentials: Option<Vec<TotpCredentialData>>,
}

#[derive(Serialize, ToSchema)]
struct CredentialImportBodyData {
    payload: CredentialTransferData,
}

#[derive(Serialize, ToSchema)]
struct LocaleConfigData {
    default_locale: String,
}

#[derive(Serialize, ToSchema)]
struct ApplicationConfigData {
    run_type: Option<String>,
    reverse_proxy_submode: Option<String>,
    auto_manage_firewall: Option<bool>,
    firewall_additional_ports: Option<Vec<u16>>,
    whitelist_ips: Option<Vec<String>>,
    default_route: Option<String>,
    proxy_mappings: Option<Vec<Value>>,
    host_mappings: Option<Vec<HostMappingData>>,
    host_mapping_groups: Option<Vec<HostMappingGroupData>>,
    host_mapping_grouped_view: Option<bool>,
    stream_mappings: Option<Vec<Value>>,
    locale: Option<LocaleConfigData>,
    appearance: Option<Value>,
    runtime_profile: Option<Value>,
    capabilities: Option<Value>,
    ssl: Option<Value>,
    login: Option<Value>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
enum HostTargetPathModeData {
    Entry,
    Prefix,
}

#[derive(Serialize, ToSchema)]
struct HostMappingData {
    host: String,
    sync_id: String,
    group_id: Option<String>,
    target: String,
    target_path_mode: HostTargetPathModeData,
    waf_enabled: bool,
    use_auth: bool,
    access_mode: String,
    suppress_toolbar: bool,
    preserve_host: bool,
    is_default: bool,
    disabled: bool,
    availability: Option<Value>,
    visibility: Value,
    protocol_mode: String,
    basic_auth: Value,
    locations: Vec<Value>,
    service_role: String,
    title: String,
    title_override: String,
    favicon: String,
    favicon_override: String,
    advanced_auth: Option<Value>,
}

#[derive(Serialize, ToSchema)]
struct HostMappingGroupData {
    id: String,
    name: String,
}

#[derive(Serialize, ToSchema)]
struct HostMappingCatalogData {
    mappings: Vec<HostMappingData>,
    groups: Vec<HostMappingGroupData>,
    grouped_view: bool,
    revision: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct SessionRecordData {
    id: String,
    totp_id: Option<String>,
    method: Option<String>,
    credential_id: Option<String>,
    credential_name: Option<String>,
    comment: Option<String>,
    ip: Option<String>,
    user_agent: Option<String>,
    login_time: Option<String>,
    expires_at: Option<String>,
    ip_location: Option<String>,
    mobility: Option<SessionMobilitySummaryData>,
    fnos_attachments: Option<Vec<SessionAttachmentData>>,
    trim_media_attachments: Option<Vec<SessionAttachmentData>>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct SessionMobilitySummaryData {
    has_history: bool,
    drift_count: usize,
    last_drift_at: Option<String>,
    last_drift_source: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct SessionMobilityEventData {
    version: u8,
    kind: String,
    happened_at: String,
    source: String,
    from_ip: Option<String>,
    from_ip_location: Option<String>,
    to_ip: String,
    to_ip_location: Option<String>,
}

#[derive(Serialize, ToSchema)]
struct SessionMobilityDetailsData {
    summary: SessionMobilitySummaryData,
    events: Vec<SessionMobilityEventData>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct SessionAttachmentData {
    subject_hash: String,
    current_ip: String,
    created_at: String,
    last_seen_at: String,
    expires_at: Option<String>,
}

#[derive(Serialize, ToSchema)]
struct AutomaticBackupConfigData {
    enabled: bool,
    interval_hours: i64,
    retention_days: i64,
    updated_at: Option<String>,
}

#[derive(Serialize, ToSchema)]
struct AutomaticBackupStatusData {
    directory_path: String,
    last_attempt_at: Option<String>,
    last_success_at: Option<String>,
    last_error: Option<String>,
    last_filename: Option<String>,
    next_backup_at: Option<String>,
}

#[derive(Serialize, ToSchema)]
struct AutomaticBackupDetailsData {
    config: AutomaticBackupConfigData,
    status: AutomaticBackupStatusData,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct BackupFileData {
    name: String,
    relative_path: String,
    extension: String,
    size: u64,
    modified_at: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct AutomaticBackupFilesData {
    directory_path: String,
    available: bool,
    files: Vec<BackupFileData>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct BackupDirectoryFilesData {
    share_name: String,
    available: bool,
    files: Vec<BackupFileData>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct BackupDirectoryExportData {
    filename: String,
    relative_path: String,
    file_path: String,
    size: u64,
    exported_at: String,
}

#[derive(Serialize, ToSchema)]
struct BackupImportResultData {
    cleared_keys: usize,
    imported_keys: usize,
    warnings: Vec<String>,
    synced_steps: Vec<String>,
}

#[derive(OpenApi)]
#[openapi(components(schemas(
    crate::admin_control::AuthLoginModeBody,
    crate::admin_control::AuthAccountPatchBody,
    crate::admin_control::AuthAccountCreateBody,
    crate::admin_control::AuthAccountPasswordBody,
    crate::admin_control::AuthAccountSetupBody,
    crate::admin_control::TotpBindBody,
    crate::admin_control::TotpCommentBody,
    crate::admin_control::SessionCommentBody,
    crate::proxy_config::MappingsBody,
    crate::proxy_config::HostMappingCatalogBody,
    crate::maintenance::ImportBackupBody,
    crate::maintenance::ImportBackupFromDirectoryBody,
    crate::maintenance::UpdateAutomaticBackupBody,
    AcmeCertificateInfoData,
    AcmeCertificateSummaryData,
    AcmeInstallStateData,
    AcmeStatusData,
    AcmeResourceProgressData,
    AcmeResourceStatusData,
    AcmeResourceInitializeData,
    AcmeResourceCancelData,
    AcmeClientSettingsBodyData,
    AcmeClientSettingsData,
    AcmeClientSettingsUpdateData,
    AcmeInitData,
    AcmeConfigBodyData,
    AcmeConfigData,
    AcmeApplicationBodyData,
    AcmeApplicationData,
    AcmeRuntimeLockData,
    AcmeJobData,
    AcmeApplicationMutationData,
    AcmeApplicationRequestData,
    AcmeApplicationDeleteData,
    AcmeLibrarySyncData,
    AcmeLegacyRequestBodyData,
    AcmeLegacyRequestData,
    AcmeProcessResultData,
    AcmeStopJobData,
    AcmeCredentialFieldData,
    AcmeCredentialSchemeData,
    AcmeDnsProviderData,
    AcmeLatestJobData,
    AcmeOverviewCertificateData,
    AcmeOverviewLibraryData,
    AcmeApplicationOverviewData,
    AcmeRunningJobData,
    AcmeOverviewData,
    AcmeLogAnalysisData,
    AcmeJobPollData,
    AcmeCertificateData,
    AcmeSubdomainRecommendationData,
    AcmeActionMessageData,
    AuthModeStatusData,
    AuthModePreviewData,
    AuthCredentialSettingsData,
    AuthCredentialSettingsUpdateData,
    TotpStreamAccessData,
    TotpSubdomainAccessData,
    AccessScopesUpdateData,
    SubdomainAccessUpdateData,
    AuthAccountData,
    AuthAccountsData,
    TotpCredentialData,
    TotpStatusData,
    TotpSetupData,
    CredentialImportSummaryData,
    PasskeyCredentialData,
    AuthAccountTransferData,
    AuthPasswordTransferData,
    CredentialTransferData,
    CredentialImportBodyData,
    OidcProviderCatalogItemData,
    OidcProviderCatalogData,
    OidcConnectionConfigInputData,
    OidcConnectionConfigMaskedData,
    OidcProviderCreateData,
    OidcProviderUpdateData,
    OidcProviderData,
    OidcProvidersData,
    OidcBindingData,
    OidcBindingsData,
    ExternalAuthConnectionTestData,
    ExternalAuthInvitationBodyData,
    ExternalAuthInvitationData,
    LdapProviderCatalogDefaultsData,
    LdapProviderCatalogItemData,
    LdapProviderCatalogData,
    LdapConnectionConfigInputData,
    LdapConnectionConfigMaskedData,
    LdapProviderCreateData,
    LdapProviderUpdateData,
    LdapProviderData,
    LdapProvidersData,
    LdapProviderTestBodyData,
    LdapBindingData,
    LdapBindingsData,
    LocaleConfigData,
    ApplicationConfigData,
    HostMappingData,
    HostMappingGroupData,
    HostMappingCatalogData,
    SessionRecordData,
    SessionMobilitySummaryData,
    SessionMobilityEventData,
    SessionMobilityDetailsData,
    SessionAttachmentData,
    AutomaticBackupConfigData,
    AutomaticBackupStatusData,
    AutomaticBackupDetailsData,
    BackupFileData,
    AutomaticBackupFilesData,
    BackupDirectoryFilesData,
    BackupDirectoryExportData,
    BackupImportResultData,
    WolFeatureConfigData,
    WolFeatureConfigUpdateData,
    WolLocalRelayConfigData,
    WolLocalRelayRuntimeData,
    WolLocalRelayData,
    WolLocalRelayInputData,
    WolLocalRelayPairBodyData,
    WolRelayInputData,
    WolRelayData,
    WolRelaySummaryData,
    WolRelayListData,
    WolBootstrapData,
    WolRelayCredentialData,
    WolDispatchData,
    WolTargetStatusData,
    WolIntegrationRuntimeData,
    WolBlinkerIntegrationData,
    WolBemfaIntegrationData,
    WolTargetIntegrationsData,
    WolBlinkerIntegrationInputData,
    WolBemfaIntegrationInputData,
    WolTargetIntegrationsInputData,
    WolTargetSshInputData,
    WolTargetSshData,
    WolSshConnectionTestData,
    WolShutdownData,
    WolTargetInputData,
    WolTargetData,
    WolTargetListData,
    WolDiscoveryBodyData,
    WolLocalNetworkData,
    WolDiscoveredDeviceData,
    WolDiscoveryProgressData,
    WolDiscoveryResultData,
    WolDiscoveryJobData,
    GatewayLoggingConfigData,
    GatewayLoggingConfigUpdateData,
    GatewayLogDirectoryData,
    GatewayLogDatesData,
    GatewayLogEntryData,
    GatewayLogEntriesData,
    GatewayLogDeleteBodyData,
    GatewayLogDeleteData,
    GatewayLogAnalyticsRangeData,
    GatewayLogAnalyticsSummaryData,
    GatewayLogAnalyticsPointData,
    GatewayLogAnalyticsBucketData,
    GatewayLogAnalyticsRegionBucketData,
    GatewayLogAnalyticsDimensionsData,
    GatewayLogAnalyticsGeoData,
    GatewayLogAnalyticsQualityData,
    GatewayLogAnalyticsData,
    GatewayLogAnalyticsRefreshData,
    GatewayReverseProxyThrottleData,
    GatewayReverseProxyThrottleUpdateData,
    GatewayCrawlerBlockerData,
    GatewayCrawlerBlockerUpdateData,
    GatewayPortalData,
    GatewayPortalUpdateData,
    GatewayUnmatchedRouteData,
    GatewayUnmatchedRouteUpdateData,
    GatewayProxyProtocolData,
    GatewayProxyProtocolUpdateData,
    GatewaySettingsData,
    GatewaySettingsUpdateData,
    GatewayVisibilitySelectionData,
    GatewayVisibilitySelectionInputData,
    GatewayVisibilityConfigData,
    GatewayVisibilityUpdateData,
    GatewayVisibilitySummaryData,
    GatewayVisibilityDetailsData,
    GatewayProxyHeadersConfigData,
    GatewayProxyHeadersUpdateData,
    GatewayProxyHeadersItemData,
    GatewayProxyHeadersAvailabilityData,
    GatewayProxyHeadersSummaryData,
    GatewayProxyHeadersDetailsData,
    GatewayHostResponseConfigData,
    GatewayHostResponseUpdateData,
    GatewayHostResponseItemData,
    GatewayHostResponseAvailabilityData,
    GatewayHostResponseSummaryData,
    GatewayHostResponseDetailsData,
    PanelAppearanceData,
    PanelBootstrapData,
    PanelPasswordBodyData,
    PanelLoginBodyData,
    PanelLoginRateLimitErrorData,
    WhitelistRecordData,
    WhitelistRegionInputData,
    WhitelistRegionGroupData,
    WhitelistAddBodyData,
    WhitelistRegionAddBodyData,
    WhitelistCommentBodyData,
    WhitelistAddResultData,
    WhitelistRegionAddResultData,
    WhitelistRefreshData,
    WhitelistRefreshEnvelopeData,
    SshSecurityConfigData,
    SshSecurityConfigUpdateData,
    SshSecuritySummaryData,
    SshSecurityDetailsData,
    SshLoginLogEntryData,
    SshLoginLogListData,
    SshSecurityBlockData,
    SshSecurityBlockListData,
    SshFirewallSyncData,
    SshFirewallClearData,
    SshBlocksDeleteBodyData,
    SshBlocksDeleteData,
    SecurityOverviewTotalsData,
    SecurityOverviewSeriesData,
    SecurityOverviewData,
    ScannerSettingsData,
    ScannerCidrExemptionRegionInputData,
    ScannerSettingsUpdateData,
    ScannerPathWhitelistData,
    ScannerPathWhitelistUpdateData,
    ScannerFalsePositiveBodyData,
    ScannerFalsePositiveResultData,
    ScannerBlacklistHitData,
    ScannerBlacklistRecordData,
    ScannerBlacklistListData,
    IpListBodyData,
    GeneralBlacklistAddBodyData,
    GeneralBlacklistRecordData,
    GeneralBlacklistListData,
    GeneralBlacklistMutationData,
    GeneralBlacklistStatusData,
    RuntimeComponentHealthData,
    RuntimeLogStatusData,
    RuntimeHealthSnapshotData,
    RuntimeOperationalLogEntryData,
    RuntimeComponentLogsData,
    RuntimeLogClearData,
    GatewayMemoryConfigData,
    GatewayMemoryConfigUpdateData,
    GatewayMemoryReclaimBodyData,
    GatewayMemoryReclaimData,
    RuntimeSystemEventSubjectData,
    RuntimeSystemEventData,
    RuntimePlatformData,
    RuntimeDiagnosticsCollectionData,
    TypedConfigShadowStatusData,
    RuntimeStorageMigrationData,
    RuntimeDiagnosticsData,
    CidrOperatorCapabilityData,
    CidrCapabilitiesData,
    CidrProvinceItemData,
    CidrProvinceOptionData,
    CidrProvincesData,
    CidrCityItemData,
    CidrCityOptionData,
    CidrCitiesData,
    CidrSelectorData,
    CidrSelectionData,
    CidrGroupsData,
    CidrCountsData,
    CidrLookupData,
    IpLocationBatchBodyData,
    IpLocationResultData,
    IpLocationSnapshotData,
    IpLocationBatchData,
    IpLocationApiConfigData,
    IpLocationTestUrlBodyData,
    IpLocationConnectionTestData,
    CidrConnectionTestData,
    SystemEventSubjectData,
    SystemEventData,
    SystemEventListData,
    SystemEventDeleteBodyData,
    SystemEventClearData,
    SystemEventPublishBodyData,
    SystemEventPublishResultData,
    LoginBackoffData,
    LoginBackoffResetBodyData,
    LoginBackoffResetData,
    CaptchaPowUncommonLocationData,
    CaptchaPowData,
    CaptchaTurnstileData,
    CaptchaSettingsData,
    CaptchaPowUncommonLocationUpdateData,
    CaptchaPowUpdateData,
    CaptchaTurnstileUpdateData,
    CaptchaSettingsUpdateData,
    RunTypeUpdateData,
    AutoManageFirewallUpdateData,
    AutoManageFirewallData,
    TerminalFeatureData,
    TerminalFeatureUpdateData,
    WelcomeGuideData,
    AutoHttpsConfigData,
    AutoHttpsUpdateData,
    AutoHttpsRuntimeData,
    AutoHttpsDetailsData,
    DefaultRouteData,
    DefaultRouteUpdateData,
    DefaultTunnelUpdateData,
    FirewallAdditionalPortsUpdateData,
    FirewallAdditionalPortsData,
    FirewallResetBodyData,
    FirewallResetData,
    FirewallClearData,
    SyncRoutesData,
    MaintenanceClearBodyData,
    MaintenanceClearData,
    AccessEntryData,
    SystemClockIssueData,
    SystemClockStatusData,
    SystemClockSyncResponseData,
    SystemAssetDownloadProgressData,
    CloudflaredAssetStatusData,
    FrpAssetStatusData,
    SystemAssetMutationResponseData,
    DnsmasqInstallStateData,
    DnsmasqStatusData,
    ProtocolMappingAvailabilityData,
    ProtocolMappingFeatureData,
    ProtocolMappingFeatureUpdateData,
    ProxyProtocolForceData,
    RunModePromptPreferencesData,
    RunModePromptPreferencesUpdateData,
    SmartConnectConfigData,
    SmartConnectUpdateData,
    SmartConnectRuntimeData,
    SmartConnectAvailabilityData,
    SmartConnectInstallStateData,
    SmartConnectDnsmasqData,
    SmartConnectLocalIpData,
    SmartConnectDetailsData,
    FnosShareBypassData,
    FnosShareBypassUpdateData,
    FnosPortIconHijackData,
    FnosPortIconHijackUpdateData,
    FnosNetworkTuningConfigData,
    FnosNetworkTuningKernelData,
    FnosNetworkTuningBbrData,
    FnosNetworkTuningMtuData,
    FnosNetworkTuningData,
    FnosNetworkTuningUpdateData,
    FnosConnectWafAvailabilityData,
    FnosConnectWafConfigData,
    FnosConnectWafUpdateData,
    FnosConnectWafLocalNetworksData,
    FnosConnectWafRuntimeData,
    FnosConnectWafData,
    FnosCertificateSyncConfigData,
    FnosCertificateSyncUpdateData,
    FnosCertificateSyncBodyData,
    FnosCertificateSyncSummaryData,
    FnosCertificateSyncRuntimeData,
    FnosCertificateSyncAvailabilityData,
    FnosCertificateSyncCountsData,
    FnosCertificateSyncLocalData,
    FnosCertificateSyncItemData,
    FnosCertificateSyncDetailsData,
    FnosCertificateSyncResponseData,
    ProxyMappingData,
    ProxyMappingsUpdateData,
    StreamMappingData,
    StreamMappingInputData,
    StreamMappingsUpdateData,
    SubdomainModeData,
    SubdomainModeUpdateData,
    SubdomainSslAutoSelectionData,
    SubdomainModeResponseData,
    HostMappingBasicAuthInputData,
    HostMappingBasicAuthProbeBodyData,
    HostMappingBasicAuthProbeData,
    HostMappingMetadataBodyData,
    HostMappingMetadataData,
    HostMappingRefreshSummaryData,
    AdvancedAuthConditionData,
    AdvancedAuthConditionInputData,
    AdvancedAuthRuleGroupData,
    AdvancedAuthRuleGroupInputData,
    AdvancedAuthConfigData,
    AdvancedAuthConfigInputData,
    AdvancedAuthUpdateBodyData,
    AdvancedAuthDetailsData,
    ScanDiscoveryCapabilityData,
    ScanDiscoverySettingsData,
    ScanDiscoverySettingsUpdateData,
    ScanDiscoveryTargetData,
    ScanDiscoveryHostCandidateData,
    ScanDiscoveryLimitsData,
    ScanDiscoveryTargetsData,
    ScanDiscoveryTargetsUpdateData,
    ScanDiscoverJobBodyData,
    ScanDiscoverProxyRuleData,
    ScanDiscoverServiceDetailData,
    ScanDiscoveredServiceData,
    ScanDiscoverMetaData,
    ScanDiscoverResultData,
    ScanDiscoverProgressData,
    ScanDiscoverJobData,
    HostMappingsProbeBodyData,
    HostMappingProbeResultData,
    HostMappingsProbeData,
    DeepMonitorStartBodyData,
    DeepMonitorExtendBodyData,
    DeepMonitorSessionData,
    DeepMonitorSessionListData,
    DeepMonitorEventSummaryData,
    DeepMonitorEventListData,
    DeepMonitorPayloadRefData,
    DeepMonitorHeaderData,
    DeepMonitorTimingData,
    DeepMonitorWebSocketFrameData,
    DeepMonitorEventData,
    TerminalTmuxInstallStateData,
    TerminalRuntimeStatusData,
    TerminalSessionData,
    TerminalAttachmentData,
    TerminalOutputChunkData,
    TerminalPollResultData,
    TerminalCreateSessionBodyData,
    TerminalRenameSessionBodyData,
    TerminalInputBodyData,
    TerminalResizeBodyData,
    CloudflaredProcessResourceData,
    CloudflaredSupervisorFailureData,
    CloudflaredSupervisorData,
    CloudflaredRuntimeStatusData,
    CloudflaredStatusData,
    CloudflaredStartData,
    CloudflaredPollData,
    CloudflareTunnelSummaryData,
    CloudflaredConfigData,
    CloudflaredConfigUpdateData,
    CloudflareCredentialBodyData,
    CloudflareConnectionData,
    CloudflareDnsRecordData,
    CloudflareIngressData,
    CloudflareManagedResourcesData,
    CloudflareOptimizationCandidateData,
    CloudflareOptimizationVantageData,
    CloudflareOptimizationBuiltinSourceData,
    CloudflareOptimizationResolverDiagnosticData,
    CloudflareOptimizationCandidateSourcesData,
    CloudflareOptimizationScanData,
    CloudflareOptimizationDomainData,
    CloudflareOptimizationFallbackOriginData,
    CloudflareOptimizationCapabilityProbeData,
    CloudflareOptimizationScheduleData,
    CloudflareOptimizationStateData,
    CloudflareManagedStateData,
    CloudflareReconcileRequestData,
    CloudflareReconcileApplyBodyData,
    CloudflareReconcileJobData,
    CloudflareReconcileCapabilityData,
    CloudflareReconcileCapabilitiesData,
    CloudflareReconcileOperationData,
    CloudflareReconcileConflictRecordData,
    CloudflareReconcileConflictDesiredData,
    CloudflareReconcileConflictDetailsData,
    CloudflareReconcileConflictData,
    CloudflareReconcilePlanData,
    CloudflareOptimizationScanBodyData,
    CloudflareOptimizationSourceSettingsBodyData,
    CloudflareOptimizationDomainBodyData,
    CloudflareOptimizationDomainUpdateData,
    CloudflareOptimizationApplyBodyData,
    CloudflareOptimizationApplyData,
    CloudflareOptimizationFallbackData,
    FrpcTcpItemData,
    FrpcInstanceSummaryData,
    FrpcInstanceStatusData,
    FrpcDefaultsData,
    FrpcInstancesOverviewData,
    FrpcStatusData,
    FrpcLegacyOverviewData,
    FrpcWebStatusData,
    FrpcConfigData,
    FrpcConfigUpdateData,
    FrpcStartData,
    FrpcInstanceBodyData,
    FrpcInstanceDetailData,
    FrpcPrimaryStatusData,
    FrpcPollData,
    FrpcInstancePollData,
    DdnsPublicCheckSourcesData,
    DdnsSettingsData,
    DdnsSettingsUpdateData,
    DdnsToggleBodyData,
    DdnsPublicCheckTestBodyData,
    DdnsPublicCheckTestResultData,
    DdnsPublicCheckTestResultsData,
    DdnsProviderDomainTargetsData,
    DdnsProviderCapabilitiesData,
    DdnsProviderFieldOptionData,
    DdnsProviderFieldData,
    DdnsProviderData,
    DdnsNetworkInterfaceAddressData,
    DdnsNetworkInterfaceData,
    DdnsInterfaceSelectorData,
    DdnsInterfaceSelectorPreviewBodyData,
    DdnsRejectedAddressData,
    DdnsInterfaceSelectorPreviewData,
    DdnsProviderBodyData,
    DdnsConfigData,
    DdnsConfigBodyData,
    DdnsTargetBodyData,
    DdnsTargetEnabledBodyData,
    DdnsLastIpData,
    DdnsLastCheckData,
    DdnsTargetSummaryData,
    DdnsTargetDetailData,
    DdnsTargetListData,
    DdnsStatusData,
    DdnsLogEntryData,
    DdnsTestResultData,
    DdnsTestResponseData,
    DdnsPollData,
    SslCertificateSaveBodyData,
    SslCertificateActivateBodyData,
    SslDeploymentModeBodyData,
    SslCaHostBodyData,
    SslCaHostsDeleteBodyData,
    SslCertificateInfoData,
    SslSubdomainCoverageData,
    SslCertificateLibraryCoverageData,
    SslCertificateSummaryData,
    SslGatewayCertificateData,
    SslGatewayStatusData,
    SslStatusData,
    SslSharedFileData,
    SslSharedFilesData,
    SslSharedFileContentData,
    SslCaStatusData,
    SslCertificateSaveData,
    ExternalCertificateBindingCreateBodyData,
    ExternalCertificateBindingUpdateBodyData,
    ExternalCertificateBindingData,
    ExternalCertificateBindingCredentialData,
    ExternalCertificateDeployBodyData,
    ExternalCertificateDeployData,
    NotificationFieldOptionData,
    NotificationSchemaFieldData,
    NotificationProviderCapabilitiesData,
    NotificationProviderDefinitionData,
    NotificationProviderCatalogData,
    NotificationProviderData,
    NotificationProviderDetailData,
    NotificationProviderSnapshotData,
    NotificationProviderListData,
    NotificationProviderCreateBodyData,
    NotificationProviderUpdateBodyData,
    NotificationProviderTestBodyData,
    NotificationProviderTestResultData,
    NotificationProviderTestResponseData,
    NotificationTemplateData,
    NotificationDeliveryPolicyData,
    NotificationTargetData,
    NotificationTargetInputData,
    NotificationRuleData,
    NotificationRuleCreateBodyData,
    NotificationRuleUpdateBodyData,
    NotificationRuleListData,
    NotificationMessageFactData,
    NotificationMessageActionData,
    NotificationMessageData,
    NotificationTriggerData,
    NotificationTriggerListData,
    NotificationDeliveryData,
    NotificationDeliveryListData,
    NotificationDeliveryClearBodyData,
    NotificationDeliveryClearData,
    WafConfigData,
    WafConfigUpdateData,
    WafStatusData,
    WafManifestRuleData,
    WafManifestRulesDescriptionData,
    WafRemoteManifestData,
    WafRuleFileData,
    WafRuleFileContentData,
    WafSystemSyncStateData,
    WafSystemDetailsData,
    WafCustomDetailsData,
    WafDetailsData,
    WafMatchedVariableData,
    WafRuleMatchData,
    WafInterruptionData,
    WafEventData,
    WafDrainResultData,
    WafLogEntriesData,
    WafLogDeleteData,
    WafRuleToggleBodyData,
    WafUploadFileData,
    WafUploadBodyData,
    WafLogDeleteBodyData,
    DashboardDisplayData,
    DashboardDisplayUpdateData,
    DashboardStatsNowData,
    DashboardStatsTotalsData,
    DashboardStatsErrorsData,
    DashboardChartTooltipData,
    DashboardChartLegendData,
    DashboardChartAxisData,
    DashboardChartSeriesData,
    DashboardEchartsData,
    DashboardTrafficData,
    DashboardStatsData,
    DashboardHostTrafficData,
    DashboardRealtimeData,
    DashboardStreamTrafficData,
    DashboardActiveIpData,
    DashboardActiveIpsData,
    DashboardStreamActiveIpsData,
    UpdateLatestData,
    UpdateCheckData,
    UpdateDownloadData,
    UpdateStatusData,
    UpdateConfirmData
)))]
struct DomainSchemas;

pub(super) fn components() -> Map<String, Value> {
    let document = serde_json::to_value(DomainSchemas::openapi()).unwrap_or_default();
    let mut schemas = document
        .pointer("/components/schemas")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    for schema in ["AcmeStatusData", "AcmeInstallStateData"] {
        set_property_enum(
            &mut schemas,
            schema,
            "status",
            &["uninstalled", "installing", "installed", "error"],
        );
        set_property_metadata(&mut schemas, schema, "progress", "minimum", json!(0));
        set_property_metadata(&mut schemas, schema, "progress", "maximum", json!(100));
    }
    for schema in [
        "AcmeStatusData",
        "AcmeClientSettingsBodyData",
        "AcmeClientSettingsData",
        "AcmeClientSettingsUpdateData",
        "AcmeInitData",
    ] {
        set_property_enum(
            &mut schemas,
            schema,
            "certificateAuthority",
            &["zerossl", "letsencrypt"],
        );
    }
    set_property_metadata(
        &mut schemas,
        "AcmeClientSettingsBodyData",
        "accountEmail",
        "format",
        json!("email"),
    );
    set_property_enum(
        &mut schemas,
        "AcmeResourceProgressData",
        "status",
        &[
            "idle",
            "downloading",
            "verifying",
            "completed",
            "cancelled",
            "error",
        ],
    );
    set_property_metadata(
        &mut schemas,
        "AcmeResourceProgressData",
        "percent",
        "minimum",
        json!(0),
    );
    set_property_metadata(
        &mut schemas,
        "AcmeResourceProgressData",
        "percent",
        "maximum",
        json!(100),
    );
    for schema in ["AcmeJobData", "AcmeRunningJobData"] {
        set_property_enum(
            &mut schemas,
            schema,
            "status",
            &["queued", "running", "succeeded", "failed", "stopped"],
        );
        set_property_metadata(&mut schemas, schema, "progress", "minimum", json!(0));
        set_property_metadata(&mut schemas, schema, "progress", "maximum", json!(100));
    }
    set_property_enum(
        &mut schemas,
        "AcmeJobData",
        "method",
        &["dns", "http", "https"],
    );
    for schema in ["AcmeJobData", "AcmeLatestJobData"] {
        set_property_enum(
            &mut schemas,
            schema,
            "trigger",
            &["manual_request", "auto_renew"],
        );
    }
    set_property_enum(
        &mut schemas,
        "AcmeLatestJobData",
        "status",
        &[
            "idle",
            "queued",
            "running",
            "succeeded",
            "failed",
            "stopped",
        ],
    );
    set_property_enum(
        &mut schemas,
        "AcmeApplicationData",
        "latestJobStatus",
        &[
            "idle",
            "queued",
            "running",
            "succeeded",
            "failed",
            "stopped",
        ],
    );
    set_property_enum(
        &mut schemas,
        "AcmeApplicationData",
        "latestJobTrigger",
        &["manual_request", "auto_renew"],
    );
    set_property_enum(
        &mut schemas,
        "AcmeRuntimeLockData",
        "reason",
        &["manual_request", "auto_renew"],
    );
    set_property_enum(
        &mut schemas,
        "AcmeLogAnalysisData",
        "reason",
        &[
            "dns_credentials_invalid",
            "dns_credentials_invalid_email",
            "dns_api_rate_limited",
            "acme_frequency_limited",
            "unknown",
        ],
    );
    set_property_enum(
        &mut schemas,
        "AcmeSubdomainRecommendationData",
        "mode",
        &["wildcard_parent", "single_host", "manual"],
    );
    set_property_enum(
        &mut schemas,
        "AcmeLegacyRequestBodyData",
        "method",
        &["dns"],
    );
    for schema in [
        "AcmeConfigBodyData",
        "AcmeApplicationBodyData",
        "AcmeLegacyRequestBodyData",
    ] {
        set_property_metadata(&mut schemas, schema, "domains", "minItems", json!(1));
        replace_property_schema(
            &mut schemas,
            schema,
            "credentials",
            json!({
                "type": ["object", "null"],
                "additionalProperties": {
                    "oneOf": [
                        { "type": "string" },
                        { "type": "number" },
                        { "type": "boolean" }
                    ]
                },
                "writeOnly": true,
                "description": "Provider credentials; authenticated read models expose the normalized stored values separately."
            }),
        );
        if let Some(schema) = schemas.get_mut(schema).and_then(Value::as_object_mut) {
            schema.insert(
                "allOf".to_string(),
                json!([{
                    "oneOf": [
                        {
                            "type": "object",
                            "required": ["dnsType"],
                            "properties": {
                                "dnsType": { "type": "string", "minLength": 1 }
                            }
                        },
                        {
                            "type": "object",
                            "required": ["provider"],
                            "properties": {
                                "provider": { "type": "string", "minLength": 1 }
                            }
                        }
                    ]
                }]),
            );
        }
    }
    for (schema, property) in [
        ("AcmeCertificateInfoData", "validFrom"),
        ("AcmeCertificateInfoData", "validTo"),
        ("AcmeStatusData", "certificateAuthorityUpdatedAt"),
        ("AcmeClientSettingsData", "updatedAt"),
        ("AcmeClientSettingsUpdateData", "updatedAt"),
        ("AcmeConfigData", "updatedAt"),
        ("AcmeApplicationData", "createdAt"),
        ("AcmeApplicationData", "updatedAt"),
        ("AcmeApplicationData", "latestJobAt"),
        ("AcmeRuntimeLockData", "startedAt"),
        ("AcmeRuntimeLockData", "heartbeatAt"),
        ("AcmeRuntimeLockData", "expiresAt"),
        ("AcmeJobData", "createdAt"),
        ("AcmeJobData", "startedAt"),
        ("AcmeJobData", "finishedAt"),
        ("AcmeLatestJobData", "createdAt"),
        ("AcmeApplicationOverviewData", "createdAt"),
        ("AcmeApplicationOverviewData", "updatedAt"),
    ] {
        set_property_metadata(&mut schemas, schema, property, "format", json!("date-time"));
    }

    // These enums are validated by the handlers while the existing request
    // structs intentionally retain String for whitespace-compatible parsing.
    set_property_enum(
        &mut schemas,
        "AuthLoginModeBody",
        "mode",
        &["totp", "password"],
    );
    set_property_enum(
        &mut schemas,
        "LocaleConfigData",
        "default_locale",
        &["zh-CN", "zh-Hant", "en", "ko-KR", "ja-JP"],
    );
    set_property_enum(
        &mut schemas,
        "AuthCredentialSettingsData",
        "post_login_ip_grant_mode",
        &["disabled", "custom", "follow_session"],
    );
    set_property_enum(
        &mut schemas,
        "AuthCredentialSettingsUpdateData",
        "post_login_ip_grant_mode",
        &["disabled", "custom", "follow_session"],
    );
    set_property_enum(
        &mut schemas,
        "TotpSubdomainAccessData",
        "mode",
        &["all", "custom"],
    );
    set_property_enum(
        &mut schemas,
        "TotpStreamAccessData",
        "protocol",
        &["tcp", "udp"],
    );
    set_array_item_enum(
        &mut schemas,
        "AccessScopesUpdateData",
        "access_scopes",
        &["docker_admin_panel"],
    );
    for schema in [
        "AuthAccountData",
        "TotpCredentialData",
        "AuthAccountTransferData",
    ] {
        set_array_item_enum(
            &mut schemas,
            schema,
            "access_scopes",
            &["docker_admin_panel"],
        );
    }
    for schema in [
        "OidcProviderCatalogItemData",
        "OidcProviderCreateData",
        "OidcProviderData",
        "OidcBindingData",
    ] {
        set_property_enum(
            &mut schemas,
            schema,
            "type",
            &["fnknock_qq", "google", "microsoft", "github", "custom_oidc"],
        );
    }
    for schema in ["OidcProviderCatalogItemData", "OidcProviderData"] {
        set_property_enum(
            &mut schemas,
            schema,
            "protocol",
            &["oidc", "oauth2_profile"],
        );
    }
    for schema in [
        "LdapProviderCatalogItemData",
        "LdapProviderCreateData",
        "LdapProviderData",
    ] {
        set_property_enum(
            &mut schemas,
            schema,
            "type",
            &["openldap", "active_directory", "custom"],
        );
    }
    set_property_enum(
        &mut schemas,
        "OidcBindingData",
        "provider_type",
        &["fnknock_qq", "google", "microsoft", "github", "custom_oidc"],
    );
    set_property_enum(
        &mut schemas,
        "LdapBindingData",
        "provider_type",
        &["openldap", "active_directory", "custom"],
    );
    set_property_enum(&mut schemas, "LdapProviderData", "protocol", &["ldap"]);
    for schema in [
        "LdapProviderCatalogDefaultsData",
        "LdapConnectionConfigInputData",
        "LdapConnectionConfigMaskedData",
    ] {
        set_property_enum(&mut schemas, schema, "transport", &["ldaps", "starttls"]);
        set_property_enum(&mut schemas, schema, "bind_mode", &["search", "direct"]);
    }
    set_property_enum(
        &mut schemas,
        "LdapConnectionConfigMaskedData",
        "service_bind_password",
        &["", "********"],
    );
    set_property_enum(
        &mut schemas,
        "OidcConnectionConfigMaskedData",
        "client_secret",
        &["", "********", "[configured]"],
    );
    for schema in ["OidcProviderData", "LdapProviderData"] {
        set_property_enum(
            &mut schemas,
            schema,
            "last_test_status",
            &["idle", "success", "failed"],
        );
    }
    set_property_metadata(
        &mut schemas,
        "OidcConnectionConfigInputData",
        "client_secret",
        "writeOnly",
        json!(true),
    );
    set_property_metadata(
        &mut schemas,
        "LdapConnectionConfigInputData",
        "service_bind_password",
        "writeOnly",
        json!(true),
    );
    set_property_metadata(
        &mut schemas,
        "LdapProviderTestBodyData",
        "password",
        "writeOnly",
        json!(true),
    );
    for (schema, property) in [
        ("WolLocalRelayInputData", "psk"),
        ("WolLocalRelayPairBodyData", "pairingCode"),
        ("WolBlinkerIntegrationInputData", "deviceKey"),
        ("WolBemfaIntegrationInputData", "privateKey"),
        ("WolTargetSshInputData", "password"),
        ("WolTargetSshInputData", "privateKey"),
        ("WolTargetSshInputData", "privateKeyPassphrase"),
    ] {
        set_property_metadata(&mut schemas, schema, property, "writeOnly", json!(true));
    }
    set_property_enum(
        &mut schemas,
        "WolDispatchData",
        "deliveryMode",
        &["local", "relay"],
    );
    set_property_enum(
        &mut schemas,
        "WolDispatchData",
        "status",
        &["ready", "broadcasted"],
    );
    set_property_enum(
        &mut schemas,
        "WolTargetData",
        "deliveryMode",
        &["local", "relay"],
    );
    for schema in ["WolTargetSshInputData", "WolTargetSshData"] {
        set_property_enum(
            &mut schemas,
            schema,
            "platform",
            &["linux", "macos", "windows"],
        );
        set_property_enum(
            &mut schemas,
            schema,
            "authMethod",
            &["password", "privateKey"],
        );
    }
    set_property_enum(&mut schemas, "WolShutdownData", "status", &["accepted"]);
    set_property_enum(
        &mut schemas,
        "WolTargetStatusData",
        "state",
        &["online", "offline", "unknown"],
    );
    set_property_enum(
        &mut schemas,
        "WolIntegrationRuntimeData",
        "state",
        &[
            "disabled",
            "credential_missing",
            "connecting",
            "connected",
            "reconnecting",
            "error",
        ],
    );
    set_property_enum(
        &mut schemas,
        "WolDiscoveryJobData",
        "state",
        &["queued", "running", "completed", "cancelled", "failed"],
    );
    for schema in [
        "GatewayVisibilitySelectionData",
        "GatewayVisibilitySelectionInputData",
    ] {
        set_property_enum(&mut schemas, schema, "operator", &["电信", "联通", "移动"]);
    }
    for schema in ["GatewayPortalData", "GatewayPortalUpdateData"] {
        set_property_enum(&mut schemas, schema, "display_style", &["domain", "title"]);
        set_property_enum(&mut schemas, schema, "icon_drag_mode", &["corners", "free"]);
        set_property_enum(&mut schemas, schema, "version", &["v1", "v2"]);
    }
    for schema in [
        "GatewayUnmatchedRouteData",
        "GatewayUnmatchedRouteUpdateData",
    ] {
        set_property_enum(
            &mut schemas,
            schema,
            "behavior",
            &["error_page", "reset_connection"],
        );
        set_property_enum(
            &mut schemas,
            schema,
            "upstream_error_detail",
            &["less", "more", "reset_connection"],
        );
    }
    set_property_enum(
        &mut schemas,
        "GatewayLogEntriesData",
        "pagination",
        &["page", "cursor"],
    );
    set_property_enum(
        &mut schemas,
        "GatewayLogAnalyticsRangeData",
        "granularity",
        &["hour", "6h", "day"],
    );
    for property in ["status", "region_status"] {
        set_property_enum(
            &mut schemas,
            "GatewayLogAnalyticsGeoData",
            property,
            &["complete", "resolving", "partial"],
        );
    }
    set_property_enum(
        &mut schemas,
        "PanelAppearanceData",
        "theme_color_preset",
        &["default", "hermes_orange", "prussian_blue", "dynamic_white"],
    );
    set_property_enum(
        &mut schemas,
        "PanelBootstrapData",
        "deployment_target",
        &[
            "fpk", "fpk-lite", "docker", "openwrt", "linux", "macos", "synology", "windows", "dev",
        ],
    );
    set_property_enum(
        &mut schemas,
        "PanelBootstrapData",
        "auth_source",
        &["panel_session", "reauth_session"],
    );
    set_property_metadata(
        &mut schemas,
        "PanelBootstrapData",
        "session_expires_at",
        "format",
        json!("date-time"),
    );
    for schema in ["PanelPasswordBodyData", "PanelLoginBodyData"] {
        set_property_metadata(&mut schemas, schema, "password", "writeOnly", json!(true));
        set_property_metadata(
            &mut schemas,
            schema,
            "password",
            "description",
            json!("6-128 UTF-8 bytes, no whitespace, with at least one ASCII letter and digit"),
        );
    }
    set_property_metadata(
        &mut schemas,
        "PanelLoginRateLimitErrorData",
        "success",
        "const",
        json!(false),
    );
    for schema in ["WhitelistRecordData", "WhitelistAddBodyData"] {
        set_property_enum(&mut schemas, schema, "targetType", &["ip", "cidr", "cname"]);
        set_property_enum(&mut schemas, schema, "source", &["manual", "auto"]);
    }
    set_property_enum(
        &mut schemas,
        "WhitelistRecordData",
        "status",
        &["active", "pending", "expired", "deleted"],
    );
    set_property_enum(
        &mut schemas,
        "WhitelistRecordData",
        "resolveStatus",
        &["pending", "resolved", "empty", "error"],
    );
    set_property_enum(
        &mut schemas,
        "WhitelistRegionInputData",
        "operator",
        &["电信", "联通", "移动"],
    );
    set_property_enum(
        &mut schemas,
        "WhitelistRegionGroupData",
        "source",
        &["manual"],
    );
    set_property_enum(
        &mut schemas,
        "WhitelistRegionGroupData",
        "status",
        &["active", "expired", "deleted"],
    );
    set_property_metadata(
        &mut schemas,
        "WhitelistAddBodyData",
        "ip",
        "description",
        json!("IPv4/IPv6 address, CIDR range, or CNAME according to targetType"),
    );
    for key in ["minimum", "maximum"] {
        set_property_metadata(
            &mut schemas,
            "WhitelistAddBodyData",
            "checkIntervalMinutes",
            key,
            if key == "minimum" {
                json!(1)
            } else {
                json!(1440)
            },
        );
    }
    for schema in ["SshSecurityConfigData", "SshSecurityConfigUpdateData"] {
        set_property_enum(
            &mut schemas,
            schema,
            "block_duration_unit",
            &["minute", "hour", "day", "month"],
        );
    }
    set_property_enum(
        &mut schemas,
        "SshSecuritySummaryData",
        "log_source",
        &["journal", "auth.log", "unavailable"],
    );
    set_property_enum(
        &mut schemas,
        "SshLoginLogEntryData",
        "outcome",
        &["success", "failure"],
    );
    set_property_enum(&mut schemas, "SshLoginLogEntryData", "service", &["sshd"]);
    set_property_enum(
        &mut schemas,
        "SshLoginLogEntryData",
        "source",
        &["journal", "auth.log"],
    );
    set_property_enum(
        &mut schemas,
        "SshSecurityBlockData",
        "reason",
        &["failed_login_threshold", "cidr_not_allowed"],
    );
    set_property_enum(
        &mut schemas,
        "SshSecurityBlockData",
        "remove_reason",
        &["manual", "expired", "disabled"],
    );
    for (schema, property) in [
        ("SshSecurityConfigData", "configured_at"),
        ("SshSecurityConfigData", "updated_at"),
        ("SshSecuritySummaryData", "updated_at"),
        ("SshLoginLogEntryData", "happened_at"),
        ("SshSecurityBlockData", "blocked_at"),
        ("SshSecurityBlockData", "expires_at"),
        ("SshSecurityBlockData", "sample_log_time"),
        ("SshSecurityBlockData", "removed_at"),
    ] {
        set_property_metadata(&mut schemas, schema, property, "format", json!("date-time"));
    }
    set_property_enum(
        &mut schemas,
        "ScannerCidrExemptionRegionInputData",
        "operator",
        &["电信", "联通", "移动"],
    );
    for schema in ["GeneralBlacklistAddBodyData", "GeneralBlacklistRecordData"] {
        set_property_enum(
            &mut schemas,
            schema,
            "source",
            &["manual", "request_log", "active_ip", "waf_log"],
        );
    }
    for property in ["failedLogins", "blockedScanners", "wafEvents"] {
        set_pair_array_items(&mut schemas, "SecurityOverviewSeriesData", property);
    }
    for schema in ["RuntimeComponentHealthData", "RuntimeHealthSnapshotData"] {
        set_property_enum(
            &mut schemas,
            schema,
            if schema == "RuntimeComponentHealthData" {
                "status"
            } else {
                "overall_status"
            },
            &["healthy", "degraded", "unhealthy", "unknown", "blocked"],
        );
    }
    set_property_enum(
        &mut schemas,
        "RuntimeComponentHealthData",
        "process_state",
        &["running", "stopped", "unknown", "not_applicable"],
    );
    set_property_enum(
        &mut schemas,
        "RuntimeComponentHealthData",
        "id",
        &[
            "management",
            "gateway_process",
            "gateway_dataplane",
            "auth_bridge",
            "storage",
            "config_sync",
        ],
    );
    for schema in ["RuntimeComponentLogsData", "RuntimeLogClearData"] {
        set_property_enum(
            &mut schemas,
            schema,
            "component",
            &["management", "gateway_process"],
        );
    }
    set_property_enum(
        &mut schemas,
        "RuntimeOperationalLogEntryData",
        "level",
        &["INFO", "WARN", "ERROR"],
    );
    set_property_enum(
        &mut schemas,
        "RuntimeSystemEventData",
        "source",
        &["RUNTIME_MONITOR"],
    );
    set_property_enum(
        &mut schemas,
        "RuntimeSystemEventData",
        "level",
        &["INFO", "WARN", "ERROR", "CRITICAL"],
    );
    set_property_enum(
        &mut schemas,
        "RuntimeSystemEventData",
        "type",
        &[
            "FN_EVENT_RUNTIME_STARTED",
            "FN_EVENT_RUNTIME_STOPPED",
            "FN_EVENT_RUNTIME_RESTARTED",
            "FN_EVENT_RUNTIME_HEALTH_FAILED",
            "FN_EVENT_RUNTIME_RECOVERED",
            "FN_EVENT_RUNTIME_ABNORMAL_EXIT",
        ],
    );
    set_property_enum(
        &mut schemas,
        "RuntimeSystemEventSubjectData",
        "kind",
        &["COMPONENT"],
    );
    set_property_enum(
        &mut schemas,
        "TypedConfigShadowStatusData",
        "phase",
        &["typed_primary"],
    );
    for (schema, property) in [
        ("RuntimeComponentHealthData", "started_at"),
        ("RuntimeComponentHealthData", "last_checked_at"),
        ("RuntimeComponentHealthData", "last_success_at"),
        ("RuntimeHealthSnapshotData", "last_checked_at"),
        ("RuntimeLogStatusData", "oldest_at"),
        ("RuntimeLogStatusData", "newest_at"),
        ("RuntimeComponentLogsData", "generated_at"),
        ("RuntimeOperationalLogEntryData", "time"),
        ("RuntimeLogClearData", "cleared_at"),
        ("RuntimeSystemEventData", "happened_at"),
        ("RuntimeDiagnosticsData", "generated_at"),
    ] {
        set_property_metadata(&mut schemas, schema, property, "format", json!("date-time"));
    }
    set_property_enum(
        &mut schemas,
        "CidrCapabilitiesData",
        "source",
        &["online", "custom"],
    );
    set_array_item_enum(
        &mut schemas,
        "CidrOperatorCapabilityData",
        "operators",
        &["电信", "联通", "移动"],
    );
    set_property_enum(
        &mut schemas,
        "CidrSelectionData",
        "operator",
        &["电信", "联通", "移动"],
    );
    set_property_enum(
        &mut schemas,
        "IpLocationSnapshotData",
        "status",
        &[
            "idle",
            "queued",
            "processing",
            "success",
            "failed",
            "skipped",
        ],
    );
    set_property_enum(
        &mut schemas,
        "IpLocationResultData",
        "version",
        &["ipv4", "ipv6"],
    );
    for property in ["ip_lookup_mode", "cidr_mode"] {
        set_property_enum(
            &mut schemas,
            "IpLocationApiConfigData",
            property,
            &["online", "custom"],
        );
    }
    for schema in ["SystemEventData", "SystemEventPublishBodyData"] {
        set_property_enum(
            &mut schemas,
            schema,
            "type",
            crate::events::SYSTEM_EVENT_TYPES,
        );
        set_property_enum(
            &mut schemas,
            schema,
            "source",
            crate::events::SYSTEM_EVENT_SOURCES,
        );
        set_property_enum(
            &mut schemas,
            schema,
            "level",
            crate::events::SYSTEM_EVENT_LEVELS,
        );
    }
    set_property_enum(
        &mut schemas,
        "SystemEventSubjectData",
        "kind",
        crate::events::SYSTEM_EVENT_SUBJECT_KINDS,
    );
    set_property_metadata(
        &mut schemas,
        "SystemEventPublishResultData",
        "success",
        "const",
        json!(true),
    );
    set_property_metadata(
        &mut schemas,
        "LoginBackoffData",
        "retryAfter",
        "description",
        json!("Seconds until another login attempt is allowed"),
    );
    set_property_metadata(
        &mut schemas,
        "LoginBackoffData",
        "blockedUntil",
        "description",
        json!("Unix timestamp in milliseconds when the login block expires"),
    );
    for schema in ["CaptchaSettingsData", "CaptchaSettingsUpdateData"] {
        set_property_enum(&mut schemas, schema, "provider", &["pow", "turnstile"]);
        set_property_enum(&mut schemas, schema, "widget_mode", &["normal"]);
    }
    for schema in [
        "CaptchaPowData",
        "CaptchaPowUpdateData",
        "CaptchaPowUncommonLocationData",
        "CaptchaPowUncommonLocationUpdateData",
    ] {
        for property in ["base_max_number", "max_number"] {
            set_property_metadata(
                &mut schemas,
                schema,
                property,
                "minimum",
                json!(crate::runtime_config::POW_MIN_MAX_NUMBER),
            );
            set_property_metadata(
                &mut schemas,
                schema,
                property,
                "maximum",
                json!(crate::runtime_config::POW_MAX_MAX_NUMBER),
            );
            set_property_metadata(
                &mut schemas,
                schema,
                property,
                "multipleOf",
                json!(crate::runtime_config::POW_MAX_NUMBER_STEP),
            );
        }
    }
    for schema in ["CaptchaTurnstileData", "CaptchaTurnstileUpdateData"] {
        set_property_metadata(
            &mut schemas,
            schema,
            "secret_key",
            "format",
            json!("password"),
        );
        set_property_metadata(
            &mut schemas,
            schema,
            "secret_key",
            "description",
            json!("Sensitive value available only through the authenticated management API"),
        );
    }
    set_property_metadata(
        &mut schemas,
        "RunTypeUpdateData",
        "run_type",
        "enum",
        json!([0, 1, 3]),
    );
    set_property_enum(
        &mut schemas,
        "RunTypeUpdateData",
        "reverse_proxy_submode",
        &["path", "subdomain"],
    );
    set_property_enum(
        &mut schemas,
        "TerminalFeatureData",
        "resume_backend",
        &["tmux"],
    );
    for schema in ["TerminalFeatureData", "TerminalFeatureUpdateData"] {
        set_property_metadata(&mut schemas, schema, "max_sessions", "minimum", json!(1));
        set_property_metadata(&mut schemas, schema, "max_sessions", "maximum", json!(12));
        set_property_metadata(
            &mut schemas,
            schema,
            "idle_timeout_seconds",
            "minimum",
            json!(60),
        );
        set_property_metadata(
            &mut schemas,
            schema,
            "idle_timeout_seconds",
            "maximum",
            json!(7 * 24 * 60 * 60),
        );
    }
    set_property_metadata(
        &mut schemas,
        "WelcomeGuideData",
        "completed_at",
        "format",
        json!("date-time"),
    );
    set_property_enum(
        &mut schemas,
        "AutoHttpsRuntimeData",
        "status",
        &["disabled", "active", "error"],
    );
    set_property_enum(
        &mut schemas,
        "AutoHttpsRuntimeData",
        "redirect_scheme",
        &["https"],
    );
    set_property_metadata(
        &mut schemas,
        "AutoHttpsRuntimeData",
        "listen_port",
        "const",
        json!(80),
    );
    for property in ["last_error_at", "updated_at"] {
        set_property_metadata(
            &mut schemas,
            "AutoHttpsRuntimeData",
            property,
            "format",
            json!("date-time"),
        );
    }
    set_property_enum(
        &mut schemas,
        "DefaultTunnelUpdateData",
        "tunnel",
        &["frp", "cloudflared"],
    );
    set_property_metadata(
        &mut schemas,
        "FirewallAdditionalPortsUpdateData",
        "ports",
        "maxItems",
        json!(crate::runtime_config::MAX_FIREWALL_ADDITIONAL_PORTS),
    );
    set_property_metadata(
        &mut schemas,
        "FirewallAdditionalPortsUpdateData",
        "ports",
        "uniqueItems",
        json!(true),
    );
    set_array_item_metadata(
        &mut schemas,
        "FirewallAdditionalPortsUpdateData",
        "ports",
        "minimum",
        json!(1),
    );
    set_array_item_metadata(
        &mut schemas,
        "FirewallAdditionalPortsUpdateData",
        "ports",
        "maximum",
        json!(65535),
    );
    set_property_metadata(
        &mut schemas,
        "FirewallAdditionalPortsData",
        "runType",
        "enum",
        json!([0, 1, 3]),
    );
    for (schema, property) in [
        ("FirewallResetBodyData", "run_type"),
        ("FirewallResetData", "runType"),
    ] {
        set_property_metadata(&mut schemas, schema, property, "enum", json!([0, 1, 3]));
    }
    for schema in ["FirewallResetData", "FirewallClearData"] {
        set_property_metadata(&mut schemas, schema, "gatewayPort", "minimum", json!(1));
        set_property_metadata(
            &mut schemas,
            schema,
            "gatewayPort",
            "maximum",
            json!(65_535),
        );
    }
    set_property_metadata(
        &mut schemas,
        "MaintenanceClearBodyData",
        "confirmation",
        "minLength",
        json!(1),
    );
    set_property_metadata(
        &mut schemas,
        "AccessEntryData",
        "env",
        "enum",
        json!(["GO_REPROXY_PORT", "FRP_REMOTE_PORT"]),
    );
    set_property_metadata(
        &mut schemas,
        "SystemClockIssueData",
        "code",
        "enum",
        json!(["timezone_mismatch", "time_mismatch"]),
    );
    set_property_metadata(
        &mut schemas,
        "SystemClockStatusData",
        "expectedTimeZone",
        "const",
        json!("Asia/Shanghai"),
    );
    set_property_metadata(
        &mut schemas,
        "SystemClockStatusData",
        "driftThresholdMs",
        "const",
        json!(90_000),
    );
    set_property_metadata(
        &mut schemas,
        "SystemClockSyncResponseData",
        "success",
        "const",
        json!(true),
    );
    set_property_enum(
        &mut schemas,
        "SystemAssetDownloadProgressData",
        "status",
        &["idle", "downloading", "completed", "error"],
    );
    for (schema, property) in [
        ("SystemAssetDownloadProgressData", "percent"),
        ("DnsmasqInstallStateData", "progress"),
    ] {
        set_property_metadata(&mut schemas, schema, property, "minimum", json!(0));
        set_property_metadata(&mut schemas, schema, property, "maximum", json!(100));
    }
    set_property_enum(
        &mut schemas,
        "CloudflaredAssetStatusData",
        "platform",
        &[
            "darwin-amd64",
            "darwin-arm64",
            "linux-amd64",
            "linux-386",
            "linux-arm64",
            "linux-armhf",
            "linux-arm",
            "windows-amd64",
            "windows-386",
            "unsupported",
        ],
    );
    set_property_enum(
        &mut schemas,
        "FrpAssetStatusData",
        "platform",
        &[
            "darwin-amd64",
            "darwin-arm64",
            "linux-amd64",
            "linux-arm64",
            "linux-arm",
            "unsupported",
        ],
    );
    set_property_metadata(
        &mut schemas,
        "SystemAssetMutationResponseData",
        "success",
        "const",
        json!(true),
    );
    set_property_enum(
        &mut schemas,
        "DnsmasqInstallStateData",
        "status",
        &["uninstalled", "installing", "installed", "error"],
    );
    set_property_enum(
        &mut schemas,
        "TerminalTmuxInstallStateData",
        "status",
        &["uninstalled", "installing", "installed", "error"],
    );
    set_property_metadata(
        &mut schemas,
        "TerminalTmuxInstallStateData",
        "progress",
        "minimum",
        json!(0),
    );
    set_property_metadata(
        &mut schemas,
        "TerminalTmuxInstallStateData",
        "progress",
        "maximum",
        json!(100),
    );
    for (schema, property) in [
        ("TerminalTmuxInstallStateData", "detectionSource"),
        ("TerminalRuntimeStatusData", "tmuxDetectionSource"),
    ] {
        set_property_metadata(
            &mut schemas,
            schema,
            property,
            "enum",
            json!(["env-path", "absolute-path", null]),
        );
    }
    set_property_metadata(
        &mut schemas,
        "TerminalRuntimeStatusData",
        "httpPollingAvailable",
        "const",
        json!(true),
    );
    set_property_enum(
        &mut schemas,
        "TerminalSessionData",
        "status",
        &["created", "attached", "detached", "stopped", "error"],
    );
    for (property, minimum, maximum) in [("cols", 20, 400), ("rows", 8, 200)] {
        set_property_metadata(
            &mut schemas,
            "TerminalSessionData",
            property,
            "minimum",
            json!(minimum),
        );
        set_property_metadata(
            &mut schemas,
            "TerminalSessionData",
            property,
            "maximum",
            json!(maximum),
        );
    }
    set_property_metadata(
        &mut schemas,
        "TerminalSessionData",
        "resume_backend",
        "const",
        json!("tmux"),
    );
    set_property_metadata(
        &mut schemas,
        "TerminalAttachmentData",
        "transport",
        "const",
        json!("http-polling"),
    );
    set_property_metadata(
        &mut schemas,
        "TerminalOutputChunkData",
        "cursor",
        "minimum",
        json!(0),
    );
    set_property_metadata(
        &mut schemas,
        "TerminalCreateSessionBodyData",
        "cols",
        "default",
        json!(120),
    );
    set_property_metadata(
        &mut schemas,
        "TerminalCreateSessionBodyData",
        "rows",
        "default",
        json!(32),
    );
    set_property_metadata(
        &mut schemas,
        "TerminalRenameSessionBodyData",
        "title",
        "minLength",
        json!(1),
    );
    set_property_metadata(
        &mut schemas,
        "TerminalInputBodyData",
        "dataBase64",
        "format",
        json!("byte"),
    );
    for schema in ["SslCertificateSaveBodyData", "SslCertificateSummaryData"] {
        set_property_enum(
            &mut schemas,
            schema,
            "source",
            &["manual", "acme", "ca", "external"],
        );
    }
    for property in ["cert", "key"] {
        set_property_metadata(
            &mut schemas,
            "SslCertificateSaveBodyData",
            property,
            "minLength",
            json!(1),
        );
    }
    set_property_metadata(
        &mut schemas,
        "SslCertificateSaveBodyData",
        "key",
        "writeOnly",
        json!(true),
    );
    for property in ["cert", "key"] {
        set_property_metadata(
            &mut schemas,
            "ExternalCertificateDeployBodyData",
            property,
            "minLength",
            json!(1),
        );
    }
    set_property_metadata(
        &mut schemas,
        "ExternalCertificateDeployBodyData",
        "key",
        "writeOnly",
        json!(true),
    );
    set_property_metadata(
        &mut schemas,
        "ExternalCertificateBindingCredentialData",
        "token",
        "writeOnly",
        json!(true),
    );
    let external_certificate_providers = &["certd", "acme_sh", "lego", "certbot"];
    set_property_enum(
        &mut schemas,
        "ExternalCertificateBindingCreateBodyData",
        "provider",
        external_certificate_providers,
    );
    set_property_enum(
        &mut schemas,
        "ExternalCertificateBindingData",
        "provider",
        external_certificate_providers,
    );
    set_property_enum(
        &mut schemas,
        "ExternalCertificateBindingData",
        "setup_kind",
        &["webhook", "deploy_hook"],
    );
    set_property_enum(
        &mut schemas,
        "ExternalCertificateBindingData",
        "public_deploy_status",
        &["ready", "auth_host_unconfigured", "https_required"],
    );
    set_property_enum(
        &mut schemas,
        "ExternalCertificateBindingData",
        "last_result",
        &["success", "failed", "superseded"],
    );
    set_property_metadata(
        &mut schemas,
        "SslCertificateSaveBodyData",
        "key",
        "format",
        json!("password"),
    );
    set_property_metadata(
        &mut schemas,
        "SslCertificateSaveBodyData",
        "activate",
        "description",
        json!("Activate after saving unless explicitly false"),
    );
    for (schema, property) in [
        ("SslCertificateActivateBodyData", "id"),
        ("SslCaHostBodyData", "value"),
        ("SslCaHostsDeleteBodyData", "value"),
        ("SslCertificateSaveData", "id"),
    ] {
        set_property_metadata(&mut schemas, schema, property, "minLength", json!(1));
    }
    set_property_metadata(
        &mut schemas,
        "SslCaHostsDeleteBodyData",
        "all",
        "description",
        json!(
            "When true, clear the entire CA host list. An omitted body remains a successful no-op for compatibility."
        ),
    );
    for (schema, property) in [
        ("SslDeploymentModeBodyData", "deployment_mode"),
        ("SslCertificateLibraryCoverageData", "deployment_mode"),
        ("SslGatewayStatusData", "deployment_mode"),
        ("SslStatusData", "deploymentMode"),
        ("SslStatusData", "configuredDeploymentMode"),
    ] {
        set_property_enum(
            &mut schemas,
            schema,
            property,
            &["single_active", "multi_sni"],
        );
    }
    for schema in [
        "SslSubdomainCoverageData",
        "SslCertificateLibraryCoverageData",
    ] {
        set_property_enum(
            &mut schemas,
            schema,
            "status",
            &["ready", "partial", "missing"],
        );
    }
    for (schema, property) in [
        ("SslCertificateSummaryData", "created_at"),
        ("SslCertificateSummaryData", "updated_at"),
        ("SslSharedFileData", "modifiedAt"),
    ] {
        set_property_metadata(&mut schemas, schema, property, "format", json!("date-time"));
    }
    set_property_metadata(
        &mut schemas,
        "SslSharedFileData",
        "size",
        "minimum",
        json!(0),
    );
    for property in ["validFrom", "validTo"] {
        set_property_metadata(
            &mut schemas,
            "SslCertificateInfoData",
            property,
            "description",
            json!("OpenSSL-compatible UTC certificate date string"),
        );
    }
    for schema in ["CloudflaredStatusData", "CloudflaredSupervisorData"] {
        if schema == "CloudflaredStatusData" {
            set_property_enum(
                &mut schemas,
                schema,
                "platform",
                &[
                    "darwin-amd64",
                    "darwin-arm64",
                    "linux-amd64",
                    "linux-386",
                    "linux-arm64",
                    "linux-armhf",
                    "linux-arm",
                    "windows-amd64",
                    "windows-386",
                    "unsupported",
                ],
            );
        } else {
            set_property_enum(
                &mut schemas,
                schema,
                "state",
                &["stopped", "starting", "running", "backoff"],
            );
        }
    }
    for schema in ["CloudflaredConfigData", "CloudflaredConfigUpdateData"] {
        set_property_enum(&mut schemas, schema, "protocol", &["auto", "http2", "quic"]);
    }
    set_property_enum(
        &mut schemas,
        "CloudflaredConfigData",
        "mode",
        &["manual", "managed"],
    );
    for (schema, property) in [
        ("CloudflaredConfigUpdateData", "token"),
        ("CloudflareCredentialBodyData", "apiToken"),
    ] {
        set_property_metadata(&mut schemas, schema, property, "writeOnly", json!(true));
    }
    set_property_metadata(
        &mut schemas,
        "CloudflareCredentialBodyData",
        "apiToken",
        "minLength",
        json!(1),
    );
    for (schema, property) in [
        ("CloudflaredStartData", "pid"),
        ("CloudflaredPollData", "cursor"),
        ("CloudflaredSupervisorData", "restartCount"),
        ("CloudflaredSupervisorData", "consecutiveFailures"),
        ("CloudflaredSupervisorFailureData", "uptimeMs"),
        ("CloudflaredProcessResourceData", "residentKib"),
        ("CloudflaredProcessResourceData", "peakResidentKib"),
        ("CloudflaredProcessResourceData", "threads"),
        ("CloudflaredProcessResourceData", "systemAvailableKib"),
        ("CloudflaredProcessResourceData", "cgroupOomKillCount"),
        ("CloudflaredProcessResourceData", "cgroupMemoryFailCount"),
    ] {
        set_property_metadata(&mut schemas, schema, property, "minimum", json!(0));
    }
    set_property_metadata(
        &mut schemas,
        "CloudflaredStartData",
        "pid",
        "minimum",
        json!(1),
    );
    for (schema, property) in [
        ("CloudflaredProcessResourceData", "sampledAt"),
        ("CloudflaredSupervisorFailureData", "at"),
        ("CloudflaredSupervisorFailureData", "startedAt"),
        ("CloudflaredSupervisorData", "nextRestartAt"),
        ("CloudflaredSupervisorData", "startedAt"),
        ("CloudflaredSupervisorData", "stoppedAt"),
        ("CloudflareManagedResourcesData", "updatedAt"),
        ("CloudflareOptimizationCandidateData", "verifiedAt"),
        ("CloudflareOptimizationCandidateData", "selectedAt"),
        ("CloudflareOptimizationVantageData", "measuredAt"),
        ("CloudflareOptimizationScanData", "createdAt"),
        ("CloudflareOptimizationScanData", "startedAt"),
        ("CloudflareOptimizationScanData", "completedAt"),
        ("CloudflareReconcileJobData", "createdAt"),
        ("CloudflareReconcileJobData", "startedAt"),
        ("CloudflareReconcileJobData", "completedAt"),
        ("CloudflareOptimizationFallbackOriginData", "updatedAt"),
        ("CloudflareOptimizationCapabilityProbeData", "testedAt"),
        ("CloudflareOptimizationScheduleData", "nextFullScanAt"),
        ("CloudflareOptimizationScheduleData", "lastFullScanAt"),
        ("CloudflareOptimizationScheduleData", "lastHealthAt"),
        ("CloudflareReconcilePlanData", "expiresAt"),
    ] {
        set_property_metadata(&mut schemas, schema, property, "format", json!("date-time"));
    }
    for schema in ["CloudflareManagedStateData", "CloudflaredConfigData"] {
        set_property_enum(&mut schemas, schema, "mode", &["manual", "managed"]);
    }
    set_property_metadata(
        &mut schemas,
        "CloudflareOptimizationCandidateSourcesData",
        "maxCustomHostnames",
        "const",
        json!(16),
    );
    set_property_metadata(
        &mut schemas,
        "CloudflareOptimizationCandidateSourcesData",
        "resolutionPolicy",
        "const",
        json!("verified-multi-doh-fallback-v1"),
    );
    set_property_enum(
        &mut schemas,
        "CloudflareOptimizationResolverDiagnosticData",
        "provider",
        &["cloudflare", "google", "dnspod", "alidns"],
    );
    set_property_enum(
        &mut schemas,
        "CloudflareOptimizationResolverDiagnosticData",
        "status",
        &["healthy", "degraded", "unavailable"],
    );
    for schema_name in [
        "CloudflareOptimizationScanData",
        "CloudflareOptimizationStateData",
    ] {
        set_property_enum(
            &mut schemas,
            schema_name,
            "resolutionPath",
            &[
                "multi-doh",
                "official-ranges",
                "current-candidate",
                "preferred-ip",
                "unavailable",
            ],
        );
    }
    for (schema, property) in [
        ("CloudflareOptimizationScanBodyData", "preferredIp"),
        ("CloudflareOptimizationScanData", "preferredIp"),
    ] {
        set_property_metadata(&mut schemas, schema, property, "format", json!("ipv4"));
    }
    set_property_metadata(
        &mut schemas,
        "CloudflareOptimizationCandidateSourcesData",
        "publishPolicy",
        "const",
        json!("extract-ip-only"),
    );
    for (property, value) in [("beta", json!(true)), ("ipv4Only", json!(true))] {
        set_property_metadata(
            &mut schemas,
            "CloudflareOptimizationStateData",
            property,
            "const",
            value,
        );
    }
    for (property, value) in [
        ("fullScanIntervalDays", json!(7)),
        ("healthCheckIntervalMinutes", json!(15)),
    ] {
        set_property_metadata(
            &mut schemas,
            "CloudflareOptimizationScheduleData",
            property,
            "const",
            value,
        );
    }
    set_property_enum(
        &mut schemas,
        "CloudflareOptimizationScanData",
        "status",
        &["queued", "running", "completed", "failed", "cancelled"],
    );
    set_property_metadata(
        &mut schemas,
        "CloudflareOptimizationScanData",
        "progress",
        "minimum",
        json!(0),
    );
    set_property_metadata(
        &mut schemas,
        "CloudflareOptimizationScanData",
        "progress",
        "maximum",
        json!(100),
    );
    set_property_enum(
        &mut schemas,
        "CloudflareReconcileJobData",
        "status",
        &["queued", "running", "succeeded", "failed", "interrupted"],
    );
    set_property_metadata(
        &mut schemas,
        "CloudflareReconcileJobData",
        "progress",
        "minimum",
        json!(0),
    );
    set_property_metadata(
        &mut schemas,
        "CloudflareReconcileJobData",
        "progress",
        "maximum",
        json!(100),
    );
    for property in [
        "medianLatencyMs",
        "jitterMs",
        "lossRatio",
        "downloadMbps",
        "score",
    ] {
        set_property_metadata(
            &mut schemas,
            "CloudflareOptimizationCandidateData",
            property,
            "minimum",
            json!(0),
        );
    }
    set_property_metadata(
        &mut schemas,
        "CloudflareOptimizationCandidateData",
        "lossRatio",
        "maximum",
        json!(1),
    );
    set_property_metadata(
        &mut schemas,
        "CloudflareOptimizationCandidateData",
        "ip",
        "format",
        json!("ipv4"),
    );
    set_property_metadata(
        &mut schemas,
        "CloudflareOptimizationCandidateData",
        "businessStatus",
        "minimum",
        json!(100),
    );
    set_property_metadata(
        &mut schemas,
        "CloudflareOptimizationCandidateData",
        "businessStatus",
        "maximum",
        json!(599),
    );
    set_property_enum(
        &mut schemas,
        "CloudflareOptimizationDomainData",
        "managementMode",
        &["optimize", "external"],
    );
    for schema in [
        "CloudflareOptimizationDomainBodyData",
        "CloudflareOptimizationDomainUpdateData",
    ] {
        set_property_enum(&mut schemas, schema, "mode", &["optimize", "external"]);
    }
    set_property_enum(
        &mut schemas,
        "CloudflareOptimizationCapabilityProbeData",
        "status",
        &[
            "pending",
            "awaiting-candidate",
            "probe-failed",
            "compatible",
            "unsupported",
        ],
    );
    for (schema, property) in [
        ("CloudflareOptimizationSourceSettingsBodyData", "builtinIds"),
        (
            "CloudflareOptimizationSourceSettingsBodyData",
            "customHostnames",
        ),
        ("CloudflareReconcileApplyBodyData", "takeoverResourceIds"),
    ] {
        set_property_metadata(&mut schemas, schema, property, "uniqueItems", json!(true));
    }
    set_property_metadata(
        &mut schemas,
        "CloudflareOptimizationSourceSettingsBodyData",
        "customHostnames",
        "maxItems",
        json!(16),
    );
    for schema in [
        "CloudflareReconcileRequestData",
        "CloudflareReconcilePlanData",
    ] {
        set_property_enum(&mut schemas, schema, "action", &["apply", "cleanup"]);
    }
    set_property_enum(
        &mut schemas,
        "CloudflareReconcileRequestData",
        "tunnelMode",
        &["dedicated", "existing"],
    );
    set_property_enum(
        &mut schemas,
        "CloudflareReconcileOperationData",
        "action",
        &[
            "create",
            "update",
            "delete",
            "keep",
            "keep-deleted",
            "fallback",
            "probe",
            "recover",
        ],
    );
    set_property_enum(
        &mut schemas,
        "CloudflareReconcileConflictRecordData",
        "ownerKind",
        &["current-instance", "other-fn-knock-instance", "external"],
    );
    set_property_metadata(
        &mut schemas,
        "CloudflareReconcilePlanData",
        "remoteFingerprint",
        "pattern",
        json!("^[0-9a-f]{64}$"),
    );
    for schema in ["FrpcInstancesOverviewData", "FrpcStatusData"] {
        set_property_enum(
            &mut schemas,
            schema,
            "platform",
            &[
                "darwin-amd64",
                "darwin-arm64",
                "linux-amd64",
                "linux-arm64",
                "linux-arm",
                "unsupported",
            ],
        );
    }
    for schema in ["FrpcInstanceStatusData", "FrpcPrimaryStatusData"] {
        set_property_metadata(
            &mut schemas,
            schema,
            "id",
            "pattern",
            json!("^[A-Za-z0-9-]{1,80}$"),
        );
        for property in ["createdAt", "updatedAt", "startedAt", "stoppedAt"] {
            set_property_metadata(&mut schemas, schema, property, "format", json!("date-time"));
        }
        set_property_metadata(&mut schemas, schema, "pid", "minimum", json!(1));
    }
    for schema in ["FrpcConfigUpdateData", "FrpcInstanceBodyData"] {
        set_property_metadata(&mut schemas, schema, "content", "writeOnly", json!(true));
        set_property_metadata(
            &mut schemas,
            schema,
            "content",
            "description",
            json!("FRPC TOML configuration; may contain authentication credentials"),
        );
    }
    for schema in ["FrpcConfigData", "FrpcInstanceDetailData"] {
        set_property_metadata(
            &mut schemas,
            schema,
            "content",
            "description",
            json!("Authenticated management response containing the FRPC TOML configuration"),
        );
    }
    set_property_metadata(&mut schemas, "FrpcStartData", "pid", "minimum", json!(1));
    set_property_metadata(&mut schemas, "FrpcStatusData", "pid", "minimum", json!(1));
    for schema in ["FrpcPollData", "FrpcInstancePollData"] {
        set_property_metadata(&mut schemas, schema, "cursor", "minimum", json!(0));
    }
    for (schema, property) in [
        ("FrpcInstancesOverviewData", "total"),
        ("FrpcInstancesOverviewData", "extraCount"),
        ("FrpcInstancesOverviewData", "runningCount"),
        ("FrpcStatusData", "total"),
        ("FrpcStatusData", "running_count"),
    ] {
        set_property_metadata(&mut schemas, schema, property, "minimum", json!(0));
    }
    set_property_metadata(
        &mut schemas,
        "FrpcInstancesOverviewData",
        "extraCount",
        "maximum",
        json!(20),
    );
    set_property_metadata(
        &mut schemas,
        "FrpcInstancesOverviewData",
        "primaryInstanceId",
        "const",
        json!("primary"),
    );
    for schema in ["DdnsSettingsData", "DdnsStatusData"] {
        set_property_enum(&mut schemas, schema, "httpTransport", &["curl", "node"]);
        set_property_enum(
            &mut schemas,
            schema,
            "publicDnsProvider",
            &["none", "alidns", "tencent", "cloudflare", "google"],
        );
    }
    for schema in ["DdnsSettingsUpdateData", "DdnsPublicCheckTestBodyData"] {
        set_property_enum(
            &mut schemas,
            schema,
            "httpTransport",
            &["curl", "node", "fetch"],
        );
        set_property_enum(
            &mut schemas,
            schema,
            "publicDnsProvider",
            &["none", "alidns", "tencent", "cloudflare", "google"],
        );
    }
    for schema in [
        "DdnsSettingsData",
        "DdnsSettingsUpdateData",
        "DdnsStatusData",
    ] {
        set_property_metadata(
            &mut schemas,
            schema,
            "updateIntervalMinutes",
            "minimum",
            json!(crate::ddns::MIN_DDNS_UPDATE_INTERVAL_MINUTES),
        );
        set_property_metadata(
            &mut schemas,
            schema,
            "updateIntervalMinutes",
            "maximum",
            json!(crate::ddns::MAX_DDNS_UPDATE_INTERVAL_MINUTES),
        );
    }
    for property in ["ipv4", "ipv6"] {
        set_array_item_metadata(
            &mut schemas,
            "DdnsPublicCheckSourcesData",
            property,
            "format",
            json!("uri"),
        );
    }
    set_property_enum(
        &mut schemas,
        "DdnsPublicCheckTestResultData",
        "family",
        &["ipv4", "ipv6"],
    );
    set_property_metadata(
        &mut schemas,
        "DdnsPublicCheckTestResultData",
        "url",
        "format",
        json!("uri"),
    );
    set_property_metadata(
        &mut schemas,
        "DdnsPublicCheckTestResultData",
        "status",
        "minimum",
        json!(100),
    );
    set_property_metadata(
        &mut schemas,
        "DdnsPublicCheckTestResultData",
        "status",
        "maximum",
        json!(599),
    );
    let ddns_provider_names = &[
        "alidns",
        "baiducloud",
        "cloudflare",
        "dnshe",
        "dnspod",
        "duckdns",
        "dynu",
        "dynv6",
        "edgeone_cname",
        "edgeone",
        "esa",
        "godaddy",
        "huaweicloud",
        "noip",
        "porkbun",
        "tencentcloud",
    ];
    set_property_enum(
        &mut schemas,
        "DdnsProviderData",
        "name",
        ddns_provider_names,
    );
    for schema in ["DdnsProviderBodyData", "DdnsTargetBodyData"] {
        replace_property_schema(
            &mut schemas,
            schema,
            "provider",
            json!({
                "oneOf": [
                    { "type": "string", "enum": ddns_provider_names },
                    {
                        "type": "string",
                        "pattern": r"^\s*(?:alidns|baiducloud|cloudflare|dnshe|dnspod|duckdns|dynu|dynv6|edgeone_cname|edgeone|esa|godaddy|huaweicloud|noip|porkbun|tencentcloud)\s*$"
                    }
                ],
                "description": "Provider identifier. Surrounding whitespace remains accepted for compatibility."
            }),
        );
    }
    set_property_enum(
        &mut schemas,
        "DdnsProviderCapabilitiesData",
        "addressMode",
        &["dual_stack", "single_address"],
    );
    set_array_item_enum(
        &mut schemas,
        "DdnsProviderCapabilitiesData",
        "ipSources",
        &["public", "interface", "static", "domain"],
    );
    set_property_enum(
        &mut schemas,
        "DdnsProviderDomainTargetsData",
        "mode",
        &["single", "single_or_wildcard_root_pair"],
    );
    set_property_enum(
        &mut schemas,
        "DdnsProviderDomainTargetsData",
        "rootField",
        &["root_domain", "site_name"],
    );
    replace_property_schema(
        &mut schemas,
        "DdnsProviderDomainTargetsData",
        "rootField",
        json!({ "type": "string", "enum": ["root_domain", "site_name"] }),
    );
    replace_property_schema(
        &mut schemas,
        "DdnsProviderCapabilitiesData",
        "addressMode",
        json!({ "type": "string", "enum": ["dual_stack", "single_address"] }),
    );
    replace_property_schema(
        &mut schemas,
        "DdnsProviderCapabilitiesData",
        "ipSources",
        json!({
            "type": "array",
            "items": {
                "type": "string",
                "enum": ["public", "interface", "static", "domain"]
            }
        }),
    );
    replace_property_schema(
        &mut schemas,
        "DdnsProviderCapabilitiesData",
        "domainTargets",
        schema_ref("DdnsProviderDomainTargetsData"),
    );
    replace_property_schema(
        &mut schemas,
        "DdnsProviderData",
        "capabilities",
        schema_ref("DdnsProviderCapabilitiesData"),
    );
    for property in ["placeholder", "description"] {
        replace_property_schema(
            &mut schemas,
            "DdnsProviderFieldData",
            property,
            json!({ "type": "string" }),
        );
    }
    replace_property_schema(
        &mut schemas,
        "DdnsProviderFieldData",
        "required",
        json!({ "type": "boolean" }),
    );
    replace_property_schema(
        &mut schemas,
        "DdnsProviderFieldData",
        "options",
        json!({
            "type": "array",
            "items": schema_ref("DdnsProviderFieldOptionData")
        }),
    );
    for schema in [
        "DdnsNetworkInterfaceAddressData",
        "DdnsInterfaceSelectorPreviewBodyData",
    ] {
        set_property_enum(&mut schemas, schema, "family", &["ipv4", "ipv6"]);
    }
    for schema in [
        "DdnsNetworkInterfaceAddressData",
        "DdnsNetworkInterfaceData",
    ] {
        set_property_enum(&mut schemas, schema, "source", &["runtime", "docker_host"]);
    }
    set_property_metadata(
        &mut schemas,
        "DdnsInterfaceSelectorData",
        "version",
        "const",
        json!(1),
    );
    set_property_enum(
        &mut schemas,
        "DdnsInterfaceSelectorData",
        "mode",
        &["auto", "rules"],
    );
    set_property_enum(
        &mut schemas,
        "DdnsInterfaceSelectorPreviewData",
        "reason",
        &["current", "preferred", "ranked", "no_match"],
    );
    set_array_item_enum(
        &mut schemas,
        "DdnsInterfaceSelectorPreviewData",
        "warnings",
        &["multiple_matches", "status_unknown"],
    );
    let secret_config_schema = json!({
        "type": "object",
        "description": "Provider-specific DDNS configuration; may contain credentials",
        "additionalProperties": { "type": "string" },
        "writeOnly": true
    });
    schemas.insert(
        "DdnsConfigData".to_string(),
        json!({
            "type": "object",
            "description": "Authenticated management response containing provider-specific DDNS configuration",
            "additionalProperties": { "type": "string" }
        }),
    );
    replace_property_schema(
        &mut schemas,
        "DdnsConfigBodyData",
        "config",
        secret_config_schema.clone(),
    );
    replace_property_schema(
        &mut schemas,
        "DdnsTargetBodyData",
        "config",
        secret_config_schema,
    );
    replace_property_schema(
        &mut schemas,
        "DdnsTargetDetailData",
        "config",
        json!({
            "type": "object",
            "description": "Authenticated management response containing provider-specific DDNS configuration",
            "additionalProperties": { "type": "string" }
        }),
    );
    for schema in ["DdnsTargetSummaryData", "DdnsTargetDetailData"] {
        set_property_metadata(
            &mut schemas,
            schema,
            "id",
            "pattern",
            json!("^[A-Za-z0-9-]{1,80}$"),
        );
        for property in ["createdAt", "updatedAt"] {
            set_property_metadata(&mut schemas, schema, property, "format", json!("date-time"));
        }
        set_property_enum(
            &mut schemas,
            schema,
            "updateScope",
            &["dual_stack", "ipv6_only", "ipv4_only"],
        );
    }
    for schema in ["DdnsStatusData", "DdnsTestResultData"] {
        set_property_enum(
            &mut schemas,
            schema,
            "ipSource",
            &["public", "interface", "static", "domain"],
        );
    }
    set_property_enum(
        &mut schemas,
        "DdnsStatusData",
        "updateScope",
        &["dual_stack", "ipv6_only", "ipv4_only"],
    );
    set_property_enum(
        &mut schemas,
        "DdnsTestResultData",
        "source",
        &["public", "interface", "static", "domain"],
    );
    set_property_enum(
        &mut schemas,
        "DdnsLastCheckData",
        "outcome",
        &["updated", "noop", "skipped", "error"],
    );
    for schema in ["DdnsLastIpData", "DdnsLastCheckData"] {
        for property in ["updated_at", "checked_at"] {
            set_property_metadata(&mut schemas, schema, property, "format", json!("date-time"));
        }
    }
    for (schema, property) in [
        ("DdnsTargetListData", "total"),
        ("DdnsTargetListData", "extraCount"),
        ("DdnsTargetListData", "enabledExtraCount"),
        ("DdnsStatusData", "extraTargetCount"),
        ("DdnsStatusData", "enabledExtraTargetCount"),
        ("DdnsPollData", "cursor"),
    ] {
        set_property_metadata(&mut schemas, schema, property, "minimum", json!(0));
    }
    set_property_enum(
        &mut schemas,
        "DdnsLogEntryData",
        "level",
        &["info", "warn", "error"],
    );
    set_property_metadata(
        &mut schemas,
        "DdnsLogEntryData",
        "time",
        "format",
        json!("date-time"),
    );
    set_property_metadata(
        &mut schemas,
        "DdnsTestResponseData",
        "success",
        "const",
        json!(true),
    );
    for schema in ["WafConfigData", "WafConfigUpdateData"] {
        for property in ["paranoia_level", "executing_paranoia_level"] {
            set_property_metadata(&mut schemas, schema, property, "minimum", json!(1));
            set_property_metadata(&mut schemas, schema, property, "maximum", json!(4));
        }
    }
    set_property_metadata(
        &mut schemas,
        "WafConfigData",
        "mode",
        "const",
        json!("blocking"),
    );
    set_property_metadata(
        &mut schemas,
        "WafConfigData",
        "active_bundle_id",
        "const",
        json!("local"),
    );
    for (property, value) in [
        ("inbound_anomaly_threshold", json!(5)),
        ("outbound_anomaly_threshold", json!(4)),
        ("request_body_access", json!(true)),
        ("response_body_access", json!(false)),
    ] {
        set_property_metadata(&mut schemas, "WafConfigData", property, "const", value);
    }
    for (property, minimum, maximum) in [
        ("request_body_limit_bytes", 1_024, 128 * 1_024 * 1_024),
        (
            "request_body_in_memory_limit_bytes",
            1_024,
            128 * 1_024 * 1_024,
        ),
        ("log_retention_days", 1, 365),
        ("drain_interval_seconds", 1, 60),
    ] {
        set_property_metadata(
            &mut schemas,
            "WafConfigData",
            property,
            "minimum",
            json!(minimum),
        );
        set_property_metadata(
            &mut schemas,
            "WafConfigData",
            property,
            "maximum",
            json!(maximum),
        );
    }
    for schema in ["WafStatusData", "WafEventData"] {
        set_property_enum(
            &mut schemas,
            schema,
            "mode",
            &["off", "detection", "blocking"],
        );
    }
    for schema in ["WafRuleFileData", "WafRuleFileContentData"] {
        set_property_enum(&mut schemas, schema, "source", &["system", "custom"]);
        set_property_metadata(&mut schemas, schema, "size_bytes", "minimum", json!(0));
        set_property_metadata(
            &mut schemas,
            schema,
            "updated_at",
            "format",
            json!("date-time"),
        );
    }
    for (schema, property) in [
        ("WafConfigData", "updated_at"),
        ("WafStatusData", "loaded_at"),
        ("WafSystemSyncStateData", "synced_at"),
        ("WafSystemSyncStateData", "packaging_time"),
        ("WafSystemSyncStateData", "commit_date"),
        ("WafSystemDetailsData", "manifest_cached_at"),
        ("WafSystemDetailsData", "manifest_last_checked_at"),
        ("WafEventData", "time"),
    ] {
        set_property_metadata(&mut schemas, schema, property, "format", json!("date-time"));
    }
    for property in ["packagingTime", "commitDate"] {
        set_property_metadata(
            &mut schemas,
            "WafRemoteManifestData",
            property,
            "format",
            json!("date-time"),
        );
    }
    set_property_metadata(
        &mut schemas,
        "WafStatusData",
        "pending_events",
        "minimum",
        json!(0),
    );
    set_property_enum(
        &mut schemas,
        "WafRuleToggleBodyData",
        "source",
        &["system", "custom"],
    );
    set_property_metadata(
        &mut schemas,
        "WafRuleToggleBodyData",
        "filenames",
        "uniqueItems",
        json!(true),
    );
    set_property_metadata(
        &mut schemas,
        "WafUploadBodyData",
        "files",
        "minItems",
        json!(1),
    );
    set_property_metadata(
        &mut schemas,
        "WafUploadFileData",
        "filename",
        "pattern",
        json!(r"(?i)\.conf$"),
    );
    set_property_metadata(
        &mut schemas,
        "WafUploadFileData",
        "content_base64",
        "format",
        json!("byte"),
    );
    set_property_metadata(
        &mut schemas,
        "WafUploadFileData",
        "content_base64",
        "description",
        json!("Base64-encoded UTF-8 rule content; decoded size is limited to 1 MiB"),
    );
    for schema in ["WafLogEntriesData", "WafLogDeleteData"] {
        set_property_metadata(&mut schemas, schema, "date", "format", json!("date"));
        set_array_item_metadata(
            &mut schemas,
            schema,
            "available_dates",
            "format",
            json!("date"),
        );
    }
    set_property_metadata(
        &mut schemas,
        "WafLogDeleteBodyData",
        "date",
        "format",
        json!("date"),
    );
    for property in ["drained", "remaining"] {
        set_property_metadata(
            &mut schemas,
            "WafDrainResultData",
            property,
            "minimum",
            json!(0),
        );
    }
    set_property_enum(
        &mut schemas,
        "WafDrainResultData",
        "skipped_reason",
        &["waf_disabled"],
    );
    let notification_provider_types = [
        "wxpusher",
        "serverchan",
        "pushplus",
        "wecom",
        "dingtalk",
        "feishu",
        "email",
        "webhook",
        "pushdeer",
        "harmonyosmeow",
        "magicpush",
        "bark",
        "telegram",
    ];
    for schema in [
        "NotificationProviderDefinitionData",
        "NotificationProviderData",
        "NotificationProviderDetailData",
        "NotificationProviderSnapshotData",
        "NotificationProviderCreateBodyData",
        "NotificationProviderTestBodyData",
    ] {
        set_property_enum(&mut schemas, schema, "type", &notification_provider_types);
    }
    set_property_enum(
        &mut schemas,
        "NotificationDeliveryData",
        "provider_type",
        &notification_provider_types,
    );
    set_property_enum(
        &mut schemas,
        "NotificationSchemaFieldData",
        "type",
        &["string", "number", "boolean", "select", "json"],
    );
    for schema in [
        "NotificationProviderData",
        "NotificationProviderDetailData",
        "NotificationProviderSnapshotData",
    ] {
        set_property_enum(
            &mut schemas,
            schema,
            "last_test_status",
            &["idle", "success", "failed"],
        );
    }
    for schema in [
        "NotificationProviderCreateBodyData",
        "NotificationProviderUpdateBodyData",
        "NotificationProviderTestBodyData",
    ] {
        set_property_metadata(
            &mut schemas,
            schema,
            "connection_config",
            "writeOnly",
            json!(true),
        );
        set_property_metadata(
            &mut schemas,
            schema,
            "connection_config",
            "description",
            json!(
                "Provider-specific configuration; catalog fields marked sensitive contain secrets"
            ),
        );
    }
    set_property_metadata(
        &mut schemas,
        "NotificationProviderDetailData",
        "connection_config",
        "description",
        json!("Unmasked provider configuration returned only by the authenticated detail endpoint"),
    );
    for schema in ["NotificationTargetData", "NotificationTargetInputData"] {
        set_property_enum(
            &mut schemas,
            schema,
            "template_override_mode",
            &["inherit", "custom"],
        );
    }
    for schema in [
        "NotificationRuleData",
        "NotificationRuleCreateBodyData",
        "NotificationRuleUpdateBodyData",
    ] {
        set_property_enum(
            &mut schemas,
            schema,
            "event_type",
            crate::events::SYSTEM_EVENT_TYPES,
        );
        set_property_enum(
            &mut schemas,
            schema,
            "group_by",
            &["GLOBAL", "IP", "SESSION", "SUBJECT", "HOSTNAME", "PROVIDER"],
        );
        set_property_enum(
            &mut schemas,
            schema,
            "message_template_mode",
            &["default", "custom"],
        );
        set_array_item_enum(
            &mut schemas,
            schema,
            "event_level_filter",
            crate::events::SYSTEM_EVENT_LEVELS,
        );
        set_array_item_enum(
            &mut schemas,
            schema,
            "event_source_filter",
            crate::events::SYSTEM_EVENT_SOURCES,
        );
        for (property, minimum, maximum) in [
            ("window_seconds", 1, 86_400),
            ("threshold_count", 1, 9_999),
            ("cooldown_seconds", 0, 86_400),
        ] {
            set_property_metadata(&mut schemas, schema, property, "minimum", json!(minimum));
            set_property_metadata(&mut schemas, schema, property, "maximum", json!(maximum));
        }
    }
    for schema in [
        "NotificationRuleCreateBodyData",
        "NotificationRuleUpdateBodyData",
    ] {
        set_property_metadata(&mut schemas, schema, "targets", "minItems", json!(1));
    }
    for (property, minimum, maximum) in [
        ("timeout_seconds", 1, 30),
        ("max_attempts", 1, 10),
        ("backoff_seconds", 5, 3_600),
    ] {
        set_property_metadata(
            &mut schemas,
            "NotificationDeliveryPolicyData",
            property,
            "minimum",
            json!(minimum),
        );
        set_property_metadata(
            &mut schemas,
            "NotificationDeliveryPolicyData",
            property,
            "maximum",
            json!(maximum),
        );
    }
    set_property_enum(
        &mut schemas,
        "NotificationMessageData",
        "severity",
        &["info", "warn", "error", "critical"],
    );
    set_property_enum(
        &mut schemas,
        "NotificationTriggerData",
        "status",
        &["created", "fanout_done", "partially_failed", "completed"],
    );
    set_property_enum(
        &mut schemas,
        "NotificationDeliveryData",
        "status",
        &[
            "queued", "sending", "success", "failed", "gave_up", "skipped",
        ],
    );
    set_property_enum(
        &mut schemas,
        "NotificationDeliveryClearBodyData",
        "status",
        &[
            "queued", "sending", "success", "failed", "gave_up", "skipped",
        ],
    );
    for (schema, property) in [
        ("NotificationProviderData", "created_at"),
        ("NotificationProviderData", "updated_at"),
        ("NotificationProviderData", "last_test_at"),
        ("NotificationProviderDetailData", "created_at"),
        ("NotificationProviderDetailData", "updated_at"),
        ("NotificationProviderDetailData", "last_test_at"),
        ("NotificationProviderSnapshotData", "created_at"),
        ("NotificationProviderSnapshotData", "updated_at"),
        ("NotificationProviderSnapshotData", "last_test_at"),
        ("NotificationTargetData", "created_at"),
        ("NotificationTargetData", "updated_at"),
        ("NotificationRuleData", "created_at"),
        ("NotificationRuleData", "updated_at"),
        ("NotificationRuleData", "last_triggered_at"),
        ("NotificationMessageData", "occurred_at"),
        ("NotificationTriggerData", "created_at"),
        ("NotificationDeliveryData", "triggered_at"),
        ("NotificationDeliveryData", "sent_at"),
        ("NotificationDeliveryData", "next_retry_at"),
    ] {
        set_property_metadata(&mut schemas, schema, property, "format", json!("date-time"));
    }
    for schema in [
        "NotificationTriggerListData",
        "NotificationDeliveryListData",
        "NotificationDeliveryClearData",
    ] {
        let property = if schema == "NotificationDeliveryClearData" {
            "deleted_count"
        } else {
            "total"
        };
        set_property_metadata(&mut schemas, schema, property, "minimum", json!(0));
    }
    set_property_metadata(
        &mut schemas,
        "ProtocolMappingAvailabilityData",
        "enabled",
        "const",
        json!(true),
    );
    for property in ["start_time", "end_time"] {
        set_property_metadata(
            &mut schemas,
            "ProtocolMappingAvailabilityData",
            property,
            "pattern",
            json!(r"^(?:[01]\d|2[0-3]):[0-5]\d$"),
        );
    }
    set_property_enum(
        &mut schemas,
        "SmartConnectInstallStateData",
        "status",
        &["uninstalled", "installing", "installed", "error"],
    );
    set_property_metadata(
        &mut schemas,
        "SmartConnectInstallStateData",
        "progress",
        "minimum",
        json!(0),
    );
    set_property_metadata(
        &mut schemas,
        "SmartConnectInstallStateData",
        "progress",
        "maximum",
        json!(100),
    );
    set_property_metadata(
        &mut schemas,
        "SmartConnectLocalIpData",
        "value",
        "format",
        json!("ipv4"),
    );
    for property in ["last_sync_at", "updated_at"] {
        for schema in [
            "SmartConnectRuntimeData",
            "FnosPortIconHijackData",
            "FnosNetworkTuningConfigData",
            "FnosConnectWafConfigData",
            "FnosConnectWafRuntimeData",
        ] {
            set_property_metadata(&mut schemas, schema, property, "format", json!("date-time"));
        }
    }
    for (property, minimum, maximum) in [
        ("upstream_timeout_ms", 500, 15_000),
        ("validation_cache_ttl_seconds", 5, 300),
        ("validation_lock_ttl_seconds", 1, 30),
        ("session_ttl_seconds", 30, 3_600),
    ] {
        for schema in ["FnosShareBypassData", "FnosShareBypassUpdateData"] {
            set_property_metadata(&mut schemas, schema, property, "minimum", json!(minimum));
            set_property_metadata(&mut schemas, schema, property, "maximum", json!(maximum));
        }
    }
    set_property_enum(
        &mut schemas,
        "FnosNetworkTuningData",
        "blocked_reason_code",
        &["lite", "deployment", "platform", "permission"],
    );
    set_property_enum(
        &mut schemas,
        "FnosConnectWafAvailabilityData",
        "reason_code",
        &["standard_fpk_required"],
    );
    set_property_enum(
        &mut schemas,
        "FnosCertificateSyncItemData",
        "status",
        &[
            "unmatched",
            "up_to_date",
            "syncable",
            "source_invalid",
            "target_invalid",
            "protected",
            "sync_failed",
        ],
    );
    set_array_item_metadata(
        &mut schemas,
        "FnosCertificateSyncBodyData",
        "target_ids",
        "pattern",
        json!(r"^[+-]?\d+$"),
    );
    for schema in ["StreamMappingData", "StreamMappingInputData"] {
        set_property_enum(&mut schemas, schema, "protocol", &["tcp", "udp"]);
        set_property_metadata(&mut schemas, schema, "listen_port", "minimum", json!(1));
        set_property_metadata(&mut schemas, schema, "listen_port", "maximum", json!(65535));
    }
    for schema in [
        "SubdomainModeData",
        "SubdomainModeUpdateData",
        "SubdomainModeResponseData",
    ] {
        set_property_enum(
            &mut schemas,
            schema,
            "default_access_mode",
            &["login_first", "strict_whitelist"],
        );
        set_property_enum(
            &mut schemas,
            schema,
            "passkey_rp_mode",
            &["auth_host", "parent_domain"],
        );
        set_property_metadata(
            &mut schemas,
            schema,
            "root_domain",
            "pattern",
            json!(r"^[^*]*$"),
        );
        for property in ["public_http_port", "public_https_port"] {
            set_property_metadata(&mut schemas, schema, property, "minimum", json!(0));
            set_property_metadata(&mut schemas, schema, property, "maximum", json!(65535));
        }
        for property in [
            "auth_cache_ttl_seconds",
            "auth_cache_unauthorized_ttl_seconds",
        ] {
            set_property_metadata(&mut schemas, schema, property, "minimum", json!(0));
        }
    }
    set_property_metadata(
        &mut schemas,
        "HostMappingBasicAuthInputData",
        "password",
        "writeOnly",
        json!(true),
    );
    set_property_metadata(
        &mut schemas,
        "HostMappingBasicAuthProbeData",
        "httpStatus",
        "minimum",
        json!(100),
    );
    set_property_metadata(
        &mut schemas,
        "HostMappingBasicAuthProbeData",
        "httpStatus",
        "maximum",
        json!(599),
    );
    for schema in [
        "AdvancedAuthConditionData",
        "AdvancedAuthConditionInputData",
    ] {
        set_property_enum(
            &mut schemas,
            schema,
            "target",
            &[
                "source_ip",
                "source_region",
                "url_path",
                "request_header",
                "query_parameter",
                "http_method",
            ],
        );
        set_property_enum(
            &mut schemas,
            schema,
            "operator",
            &[
                "equals",
                "not_equals",
                "in_cidr",
                "not_in_cidr",
                "in",
                "not_in",
                "exists",
                "not_exists",
                "prefix",
                "not_prefix",
                "contains",
                "not_contains",
                "starts_with",
                "not_starts_with",
                "ends_with",
                "not_ends_with",
                "regex",
                "not_regex",
            ],
        );
        for property in ["values", "selections"] {
            set_property_metadata(&mut schemas, schema, property, "maxItems", json!(256));
        }
    }
    for schema in [
        "AdvancedAuthRuleGroupData",
        "AdvancedAuthRuleGroupInputData",
    ] {
        set_property_metadata(&mut schemas, schema, "conditions", "maxItems", json!(16));
    }
    for schema in ["AdvancedAuthConfigData", "AdvancedAuthConfigInputData"] {
        set_property_metadata(
            &mut schemas,
            schema,
            "idle_ttl_seconds",
            "minimum",
            json!(300),
        );
        set_property_metadata(
            &mut schemas,
            schema,
            "idle_ttl_seconds",
            "maximum",
            json!(2_592_000),
        );
        set_property_metadata(
            &mut schemas,
            schema,
            "max_lifetime_seconds",
            "minimum",
            json!(300),
        );
        set_property_metadata(
            &mut schemas,
            schema,
            "max_lifetime_seconds",
            "maximum",
            json!(31_536_000),
        );
        set_property_metadata(&mut schemas, schema, "groups", "maxItems", json!(16));
    }
    for (schema, property) in [
        ("AdvancedAuthConfigData", "compiled_at"),
        ("AdvancedAuthConditionData", "resolved_at"),
    ] {
        set_property_metadata(&mut schemas, schema, property, "format", json!("date-time"));
    }
    set_property_enum(
        &mut schemas,
        "ScanDiscoverySettingsData",
        "intensityMode",
        &["auto", "manual"],
    );
    set_property_enum(
        &mut schemas,
        "ScanDiscoverySettingsUpdateData",
        "intensity_mode",
        &["auto", "manual"],
    );
    set_property_enum(
        &mut schemas,
        "ScanDiscoverySettingsUpdateData",
        "intensity_level",
        &["low", "medium", "high", "extreme"],
    );
    for property in ["configuredLevel", "recommendedLevel", "effectiveLevel"] {
        set_property_enum(
            &mut schemas,
            "ScanDiscoverySettingsData",
            property,
            &["low", "medium", "high", "extreme"],
        );
    }
    for property in ["configuredConcurrency", "effectiveConcurrency"] {
        set_property_metadata(
            &mut schemas,
            "ScanDiscoverySettingsData",
            property,
            "minimum",
            json!(1),
        );
    }
    for property in ["cpuCores", "safeConcurrency"] {
        set_property_metadata(
            &mut schemas,
            "ScanDiscoveryCapabilityData",
            property,
            "minimum",
            json!(1),
        );
    }
    set_property_enum(
        &mut schemas,
        "ScanDiscoveryTargetData",
        "source",
        &[
            "docker",
            "loopback",
            "interface",
            "mapping",
            "custom",
            "saved",
        ],
    );
    set_property_enum(
        &mut schemas,
        "ScanDiscoveryHostCandidateData",
        "source",
        &["configured", "proxy", "request_host"],
    );
    set_property_enum(
        &mut schemas,
        "ScanDiscoveryTargetsData",
        "selectionMode",
        &["automatic", "custom"],
    );
    for schema in ["ScanDiscoveryTargetsUpdateData", "ScanDiscoverJobBodyData"] {
        for property in ["custom_cidrs", "selected_cidrs", "target_cidrs"] {
            set_property_metadata(&mut schemas, schema, property, "maxItems", json!(16));
        }
    }
    set_property_metadata(
        &mut schemas,
        "ScanDiscoverJobBodyData",
        "target_cidrs",
        "minItems",
        json!(1),
    );
    for schema in ["ScanDiscoverMetaData", "ScanDiscoverResultData"] {
        set_property_enum(&mut schemas, schema, "intensityMode", &["auto", "manual"]);
        for property in ["intensityLevel", "recommendedLevel"] {
            set_property_enum(
                &mut schemas,
                schema,
                property,
                &["low", "medium", "high", "extreme"],
            );
        }
        for property in ["configuredConcurrency", "effectiveConcurrency"] {
            set_property_metadata(&mut schemas, schema, property, "minimum", json!(1));
        }
    }
    set_property_enum(
        &mut schemas,
        "ScanDiscoverJobData",
        "state",
        &["queued", "running", "completed", "cancelled", "failed"],
    );
    set_property_metadata(
        &mut schemas,
        "ScanDiscoverJobData",
        "jobId",
        "format",
        json!("uuid"),
    );
    set_property_enum(
        &mut schemas,
        "HostMappingProbeResultData",
        "status",
        &["online", "stale", "unsupported"],
    );
    replace_property_schema(
        &mut schemas,
        "DeepMonitorStartBodyData",
        "duration_seconds",
        json!({
            "oneOf": [
                { "type": "integer", "format": "int32", "const": 0 },
                { "type": "integer", "format": "int32", "minimum": 300, "maximum": 7_200 }
            ]
        }),
    );
    set_property_metadata(
        &mut schemas,
        "DeepMonitorExtendBodyData",
        "duration_seconds",
        "minimum",
        json!(300),
    );
    set_property_metadata(
        &mut schemas,
        "DeepMonitorExtendBodyData",
        "duration_seconds",
        "maximum",
        json!(7_200),
    );
    set_property_metadata(
        &mut schemas,
        "DeepMonitorStartBodyData",
        "host",
        "minLength",
        json!(1),
    );
    set_property_enum(
        &mut schemas,
        "DeepMonitorSessionData",
        "state",
        &[
            "active",
            "stopped",
            "expired",
            "quota_exceeded",
            "overload",
            "io_error",
            "aborted_restart",
        ],
    );
    for property in ["started_at", "deadline_at"] {
        set_property_metadata(
            &mut schemas,
            "DeepMonitorSessionData",
            property,
            "format",
            json!("date-time"),
        );
    }
    set_property_enum(
        &mut schemas,
        "DeepMonitorEventSummaryData",
        "type",
        &["http_exchange", "ws_open", "ws_frame", "monitor_notice"],
    );
    set_property_metadata(
        &mut schemas,
        "DeepMonitorEventSummaryData",
        "time",
        "format",
        json!("date-time"),
    );
    set_property_metadata(
        &mut schemas,
        "DeepMonitorEventSummaryData",
        "status",
        "minimum",
        json!(0),
    );
    set_property_metadata(
        &mut schemas,
        "DeepMonitorEventSummaryData",
        "status",
        "maximum",
        json!(599),
    );
    set_property_metadata(
        &mut schemas,
        "DeepMonitorPayloadRefData",
        "sha256",
        "pattern",
        json!("^[0-9a-f]{64}$"),
    );
    for property in [
        "total_ms",
        "dns_ms",
        "connect_ms",
        "tls_ms",
        "request_write_ms",
        "ttfb_ms",
        "upstream_read_ms",
        "auth_ms",
        "waf_ms",
        "route_ms",
    ] {
        set_property_metadata(
            &mut schemas,
            "DeepMonitorTimingData",
            property,
            "minimum",
            json!(0),
        );
    }
    set_property_metadata(
        &mut schemas,
        "DeepMonitorWebSocketFrameData",
        "opcode",
        "maximum",
        json!(15),
    );
    set_property_metadata(
        &mut schemas,
        "DeepMonitorWebSocketFrameData",
        "mask_key",
        "pattern",
        json!("^([0-9a-f]{8})?$"),
    );
    for property in ["port", "httpStatus"] {
        set_property_metadata(
            &mut schemas,
            "ScanDiscoveredServiceData",
            property,
            "minimum",
            json!(if property == "port" { 1 } else { 100 }),
        );
        set_property_metadata(
            &mut schemas,
            "ScanDiscoveredServiceData",
            property,
            "maximum",
            json!(if property == "port" { 65_535 } else { 599 }),
        );
    }
    set_property_metadata(
        &mut schemas,
        "HostMappingProbeResultData",
        "httpStatus",
        "minimum",
        json!(100),
    );
    set_property_metadata(
        &mut schemas,
        "HostMappingProbeResultData",
        "httpStatus",
        "maximum",
        json!(599),
    );
    for property in ["detected_http_port", "listener_port"] {
        set_property_metadata(
            &mut schemas,
            "FnosConnectWafRuntimeData",
            property,
            "minimum",
            json!(1),
        );
        set_property_metadata(
            &mut schemas,
            "FnosConnectWafRuntimeData",
            property,
            "maximum",
            json!(65535),
        );
    }
    for schema in ["DashboardDisplayData", "DashboardDisplayUpdateData"] {
        set_property_enum(
            &mut schemas,
            schema,
            "date_time_display_mode",
            &["human_friendly", "full"],
        );
        set_array_item_enum(
            &mut schemas,
            schema,
            "sidebar_menu_order",
            crate::system::dashboard::DEFAULT_SIDEBAR_MENU_ORDER,
        );
    }
    set_property_enum(
        &mut schemas,
        "DashboardChartTooltipData",
        "trigger",
        &["axis"],
    );
    set_property_enum(
        &mut schemas,
        "DashboardChartAxisData",
        "type",
        &["time", "value"],
    );
    set_property_enum(&mut schemas, "DashboardChartSeriesData", "type", &["line"]);
    set_property_enum(
        &mut schemas,
        "UpdateDownloadData",
        "status",
        &[
            "idle",
            "downloading",
            "verifying",
            "downloaded",
            "installing",
            "error",
        ],
    );
    for (schema, property) in [
        ("DashboardActiveIpData", "last_seen_at"),
        ("UpdateConfirmData", "completedAt"),
    ] {
        set_property_metadata(&mut schemas, schema, property, "format", json!("date-time"));
    }
    for (schema, property) in [
        ("DashboardRealtimeData", "timestamp"),
        ("DashboardActiveIpsData", "timestamp"),
        ("DashboardStreamActiveIpsData", "timestamp"),
        ("UpdateCheckData", "lastCheckedAt"),
    ] {
        set_property_metadata(
            &mut schemas,
            schema,
            property,
            "description",
            json!("Unix timestamp in milliseconds"),
        );
    }
    schemas
}

fn set_property_enum(
    schemas: &mut Map<String, Value>,
    schema: &str,
    property: &str,
    values: &[&str],
) {
    let Some(property_schema) = schemas
        .get_mut(schema)
        .and_then(Value::as_object_mut)
        .and_then(|schema| schema.get_mut("properties"))
        .and_then(Value::as_object_mut)
        .and_then(|properties| properties.get_mut(property))
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    property_schema.insert("enum".to_string(), json!(values));
}

fn set_array_item_enum(
    schemas: &mut Map<String, Value>,
    schema: &str,
    property: &str,
    values: &[&str],
) {
    let Some(item_schema) = schemas
        .get_mut(schema)
        .and_then(Value::as_object_mut)
        .and_then(|schema| schema.get_mut("properties"))
        .and_then(Value::as_object_mut)
        .and_then(|properties| properties.get_mut(property))
        .and_then(Value::as_object_mut)
        .and_then(|property| property.get_mut("items"))
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    item_schema.insert("enum".to_string(), json!(values));
}

fn set_array_item_metadata(
    schemas: &mut Map<String, Value>,
    schema: &str,
    property: &str,
    key: &str,
    value: Value,
) {
    let Some(item_schema) = schemas
        .get_mut(schema)
        .and_then(Value::as_object_mut)
        .and_then(|schema| schema.get_mut("properties"))
        .and_then(Value::as_object_mut)
        .and_then(|properties| properties.get_mut(property))
        .and_then(Value::as_object_mut)
        .and_then(|property| property.get_mut("items"))
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    item_schema.insert(key.to_string(), value);
}

fn set_pair_array_items(schemas: &mut Map<String, Value>, schema: &str, property: &str) {
    let Some(property_schema) = schemas
        .get_mut(schema)
        .and_then(Value::as_object_mut)
        .and_then(|schema| schema.get_mut("properties"))
        .and_then(Value::as_object_mut)
        .and_then(|properties| properties.get_mut(property))
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    property_schema.insert(
        "items".to_string(),
        json!({
            "type": "array",
            "prefixItems": [
                { "type": "integer", "format": "int64" },
                { "type": "integer", "format": "int64" }
            ],
            "items": false,
            "minItems": 2,
            "maxItems": 2
        }),
    );
}

fn set_property_metadata(
    schemas: &mut Map<String, Value>,
    schema: &str,
    property: &str,
    key: &str,
    value: Value,
) {
    let Some(property_schema) = schemas
        .get_mut(schema)
        .and_then(Value::as_object_mut)
        .and_then(|schema| schema.get_mut("properties"))
        .and_then(Value::as_object_mut)
        .and_then(|properties| properties.get_mut(property))
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    property_schema.insert(key.to_string(), value);
}

fn replace_property_schema(
    schemas: &mut Map<String, Value>,
    schema: &str,
    property: &str,
    value: Value,
) {
    let Some(property_schema) = schemas
        .get_mut(schema)
        .and_then(Value::as_object_mut)
        .and_then(|schema| schema.get_mut("properties"))
        .and_then(Value::as_object_mut)
        .and_then(|properties| properties.get_mut(property))
    else {
        return;
    };
    *property_schema = value;
}

#[derive(Clone, Copy)]
enum ResponseSchema {
    Ref(&'static str),
    NullableRef(&'static str),
    Array(&'static str),
    StringArray,
    OptionalStringArray,
    RawJson(&'static str),
    DirectJson(&'static str),
    Binary,
    PemAttachment,
    HtmlAttachment,
    DiagnosticsZip,
    ZipAttachment,
    BinaryPayload,
    EventStream,
    Envelope,
}

#[derive(Clone, Copy)]
struct DomainOperation {
    method: &'static str,
    path: &'static str,
    request: Option<&'static str>,
    response: ResponseSchema,
}

const OPERATIONS: &[DomainOperation] = &[
    DomainOperation {
        method: "get",
        path: "/api/admin/cloudflared/status",
        request: None,
        response: ResponseSchema::Ref("CloudflaredStatusData"),
    },
    DomainOperation {
        method: "get",
        path: "/api/admin/cloudflared/config",
        request: None,
        response: ResponseSchema::Ref("CloudflaredConfigData"),
    },
    DomainOperation {
        method: "post",
        path: "/api/admin/cloudflared/config",
        request: Some("CloudflaredConfigUpdateData"),
        response: ResponseSchema::Envelope,
    },
    DomainOperation {
        method: "post",
        path: "/api/admin/cloudflared/start",
        request: None,
        response: ResponseSchema::Ref("CloudflaredStartData"),
    },
    DomainOperation {
        method: "post",
        path: "/api/admin/cloudflared/stop",
        request: None,
        response: ResponseSchema::Envelope,
    },
    DomainOperation {
        method: "get",
        path: "/api/admin/cloudflared/logs",
        request: None,
        response: ResponseSchema::StringArray,
    },
    DomainOperation {
        method: "delete",
        path: "/api/admin/cloudflared/logs",
        request: None,
        response: ResponseSchema::Envelope,
    },
    DomainOperation {
        method: "get",
        path: "/api/admin/cloudflared/poll",
        request: None,
        response: ResponseSchema::Ref("CloudflaredPollData"),
    },
    DomainOperation {
        method: "put",
        path: "/api/admin/cloudflared/cloudflare/credential",
        request: Some("CloudflareCredentialBodyData"),
        response: ResponseSchema::Ref("CloudflareManagedStateData"),
    },
    DomainOperation {
        method: "delete",
        path: "/api/admin/cloudflared/cloudflare/credential",
        request: None,
        response: ResponseSchema::Envelope,
    },
    DomainOperation {
        method: "get",
        path: "/api/admin/cloudflared/cloudflare/state",
        request: None,
        response: ResponseSchema::Ref("CloudflareManagedStateData"),
    },
    DomainOperation {
        method: "post",
        path: "/api/admin/cloudflared/reconcile/preview",
        request: Some("CloudflareReconcileRequestData"),
        response: ResponseSchema::Ref("CloudflareReconcilePlanData"),
    },
    DomainOperation {
        method: "post",
        path: "/api/admin/cloudflared/reconcile/apply",
        request: Some("CloudflareReconcileApplyBodyData"),
        response: ResponseSchema::Ref("CloudflareReconcileJobData"),
    },
    DomainOperation {
        method: "get",
        path: "/api/admin/cloudflared/reconcile/jobs/active",
        request: None,
        response: ResponseSchema::Ref("CloudflareReconcileJobData"),
    },
    DomainOperation {
        method: "get",
        path: "/api/admin/cloudflared/reconcile/jobs/{id}",
        request: None,
        response: ResponseSchema::Ref("CloudflareReconcileJobData"),
    },
    DomainOperation {
        method: "get",
        path: "/api/admin/cloudflared/reconcile/jobs/by-plan/{plan_id}",
        request: None,
        response: ResponseSchema::Ref("CloudflareReconcileJobData"),
    },
    DomainOperation {
        method: "post",
        path: "/api/admin/cloudflared/optimization/scans",
        request: Some("?CloudflareOptimizationScanBodyData"),
        response: ResponseSchema::Ref("CloudflareOptimizationScanData"),
    },
    DomainOperation {
        method: "get",
        path: "/api/admin/cloudflared/optimization/scans/{id}",
        request: None,
        response: ResponseSchema::Ref("CloudflareOptimizationScanData"),
    },
    DomainOperation {
        method: "delete",
        path: "/api/admin/cloudflared/optimization/scans/{id}",
        request: None,
        response: ResponseSchema::Envelope,
    },
    DomainOperation {
        method: "post",
        path: "/api/admin/cloudflared/optimization/apply",
        request: Some("CloudflareOptimizationApplyBodyData"),
        response: ResponseSchema::Ref("CloudflareOptimizationApplyData"),
    },
    DomainOperation {
        method: "post",
        path: "/api/admin/cloudflared/optimization/fallback",
        request: None,
        response: ResponseSchema::Ref("CloudflareOptimizationFallbackData"),
    },
    DomainOperation {
        method: "put",
        path: "/api/admin/cloudflared/optimization/settings",
        request: Some("CloudflareOptimizationSourceSettingsBodyData"),
        response: ResponseSchema::Ref("CloudflareOptimizationCandidateSourcesData"),
    },
    DomainOperation {
        method: "put",
        path: "/api/admin/cloudflared/optimization/domains/{hostname}",
        request: Some("CloudflareOptimizationDomainBodyData"),
        response: ResponseSchema::Ref("CloudflareOptimizationDomainUpdateData"),
    },
    DomainOperation {
        method: "get",
        path: "/api/admin/frpc/status",
        request: None,
        response: ResponseSchema::Ref("FrpcStatusData"),
    },
    DomainOperation {
        method: "get",
        path: "/api/admin/frpc/overview",
        request: None,
        response: ResponseSchema::Ref("FrpcLegacyOverviewData"),
    },
    DomainOperation {
        method: "get",
        path: "/api/admin/frpc/web-status",
        request: None,
        response: ResponseSchema::Ref("FrpcWebStatusData"),
    },
    DomainOperation {
        method: "get",
        path: "/api/admin/frpc/config",
        request: None,
        response: ResponseSchema::Ref("FrpcConfigData"),
    },
    DomainOperation {
        method: "post",
        path: "/api/admin/frpc/config",
        request: Some("FrpcConfigUpdateData"),
        response: ResponseSchema::Envelope,
    },
    DomainOperation {
        method: "post",
        path: "/api/admin/frpc/start",
        request: None,
        response: ResponseSchema::Ref("FrpcStartData"),
    },
    DomainOperation {
        method: "post",
        path: "/api/admin/frpc/stop",
        request: None,
        response: ResponseSchema::Envelope,
    },
    DomainOperation {
        method: "get",
        path: "/api/admin/frpc/logs",
        request: None,
        response: ResponseSchema::StringArray,
    },
    DomainOperation {
        method: "delete",
        path: "/api/admin/frpc/logs",
        request: None,
        response: ResponseSchema::Envelope,
    },
    DomainOperation {
        method: "get",
        path: "/api/admin/frpc/poll",
        request: None,
        response: ResponseSchema::Ref("FrpcPollData"),
    },
    DomainOperation {
        method: "get",
        path: "/api/admin/frpc/instances",
        request: None,
        response: ResponseSchema::Ref("FrpcInstancesOverviewData"),
    },
    DomainOperation {
        method: "post",
        path: "/api/admin/frpc/instances",
        request: Some("FrpcInstanceBodyData"),
        response: ResponseSchema::Ref("FrpcInstanceStatusData"),
    },
    DomainOperation {
        method: "post",
        path: "/api/admin/frpc/instances/draft",
        request: None,
        response: ResponseSchema::Ref("FrpcConfigData"),
    },
    DomainOperation {
        method: "get",
        path: "/api/admin/frpc/instances/{id}",
        request: None,
        response: ResponseSchema::Ref("FrpcInstanceDetailData"),
    },
    DomainOperation {
        method: "put",
        path: "/api/admin/frpc/instances/{id}",
        request: Some("FrpcInstanceBodyData"),
        response: ResponseSchema::Ref("FrpcInstanceStatusData"),
    },
    DomainOperation {
        method: "delete",
        path: "/api/admin/frpc/instances/{id}",
        request: None,
        response: ResponseSchema::Envelope,
    },
    DomainOperation {
        method: "post",
        path: "/api/admin/frpc/instances/{id}/start",
        request: None,
        response: ResponseSchema::Ref("FrpcStartData"),
    },
    DomainOperation {
        method: "post",
        path: "/api/admin/frpc/instances/{id}/stop",
        request: None,
        response: ResponseSchema::Envelope,
    },
    DomainOperation {
        method: "post",
        path: "/api/admin/frpc/instances/{id}/restart",
        request: None,
        response: ResponseSchema::Ref("FrpcStartData"),
    },
    DomainOperation {
        method: "get",
        path: "/api/admin/frpc/instances/{id}/logs",
        request: None,
        response: ResponseSchema::StringArray,
    },
    DomainOperation {
        method: "delete",
        path: "/api/admin/frpc/instances/{id}/logs",
        request: None,
        response: ResponseSchema::Envelope,
    },
    DomainOperation {
        method: "get",
        path: "/api/admin/frpc/instances/{id}/poll",
        request: None,
        response: ResponseSchema::Ref("FrpcInstancePollData"),
    },
    DomainOperation {
        method: "delete",
        path: "/api/admin/acme",
        request: None,
        response: ResponseSchema::Ref("AcmeInstallStateData"),
    },
    DomainOperation {
        method: "get",
        path: "/api/admin/acme/status",
        request: None,
        response: ResponseSchema::Ref("AcmeStatusData"),
    },
    DomainOperation {
        method: "get",
        path: "/api/admin/acme/resource/status",
        request: None,
        response: ResponseSchema::Ref("AcmeResourceStatusData"),
    },
    DomainOperation {
        method: "post",
        path: "/api/admin/acme/resource/initialize",
        request: None,
        response: ResponseSchema::Ref("AcmeResourceInitializeData"),
    },
    DomainOperation {
        method: "post",
        path: "/api/admin/acme/resource/cancel",
        request: None,
        response: ResponseSchema::Ref("AcmeResourceCancelData"),
    },
    DomainOperation {
        method: "delete",
        path: "/api/admin/acme/resource",
        request: None,
        response: ResponseSchema::Envelope,
    },
    DomainOperation {
        method: "get",
        path: "/api/admin/acme/overview",
        request: None,
        response: ResponseSchema::Ref("AcmeOverviewData"),
    },
    DomainOperation {
        method: "get",
        path: "/api/admin/acme/dns-providers",
        request: None,
        response: ResponseSchema::Array("AcmeDnsProviderData"),
    },
    DomainOperation {
        method: "get",
        path: "/api/admin/acme/subdomain-recommendation",
        request: None,
        response: ResponseSchema::Ref("AcmeSubdomainRecommendationData"),
    },
    DomainOperation {
        method: "post",
        path: "/api/admin/acme/init",
        request: None,
        response: ResponseSchema::Ref("AcmeInitData"),
    },
    DomainOperation {
        method: "post",
        path: "/api/admin/acme/client-settings",
        request: Some("AcmeClientSettingsBodyData"),
        response: ResponseSchema::Ref("AcmeClientSettingsUpdateData"),
    },
    DomainOperation {
        method: "get",
        path: "/api/admin/acme/config",
        request: None,
        response: ResponseSchema::NullableRef("AcmeConfigData"),
    },
    DomainOperation {
        method: "post",
        path: "/api/admin/acme/config",
        request: Some("AcmeConfigBodyData"),
        response: ResponseSchema::Ref("AcmeConfigData"),
    },
    DomainOperation {
        method: "get",
        path: "/api/admin/acme/applications",
        request: None,
        response: ResponseSchema::Array("AcmeApplicationData"),
    },
    DomainOperation {
        method: "post",
        path: "/api/admin/acme/applications",
        request: Some("AcmeApplicationBodyData"),
        response: ResponseSchema::Ref("AcmeApplicationMutationData"),
    },
    DomainOperation {
        method: "get",
        path: "/api/admin/acme/applications/{id}",
        request: None,
        response: ResponseSchema::Ref("AcmeApplicationData"),
    },
    DomainOperation {
        method: "patch",
        path: "/api/admin/acme/applications/{id}",
        request: Some("AcmeApplicationBodyData"),
        response: ResponseSchema::Ref("AcmeApplicationMutationData"),
    },
    DomainOperation {
        method: "delete",
        path: "/api/admin/acme/applications/{id}",
        request: None,
        response: ResponseSchema::Ref("AcmeApplicationDeleteData"),
    },
    DomainOperation {
        method: "delete",
        path: "/api/admin/acme/applications/{id}/certificate",
        request: None,
        response: ResponseSchema::Envelope,
    },
    DomainOperation {
        method: "post",
        path: "/api/admin/acme/applications/{id}/library/sync",
        request: None,
        response: ResponseSchema::Ref("AcmeLibrarySyncData"),
    },
    DomainOperation {
        method: "post",
        path: "/api/admin/acme/applications/{id}/deploy",
        request: None,
        response: ResponseSchema::DirectJson("AcmeActionMessageData"),
    },
    DomainOperation {
        method: "post",
        path: "/api/admin/acme/applications/{id}/request",
        request: None,
        response: ResponseSchema::Ref("AcmeApplicationRequestData"),
    },
    DomainOperation {
        method: "post",
        path: "/api/admin/acme/request",
        request: Some("AcmeLegacyRequestBodyData"),
        response: ResponseSchema::Ref("AcmeLegacyRequestData"),
    },
    DomainOperation {
        method: "post",
        path: "/api/admin/acme/jobs/active/stop",
        request: None,
        response: ResponseSchema::Ref("AcmeStopJobData"),
    },
    DomainOperation {
        method: "get",
        path: "/api/admin/acme/jobs/{id}",
        request: None,
        response: ResponseSchema::Ref("AcmeJobData"),
    },
    DomainOperation {
        method: "get",
        path: "/api/admin/acme/jobs/{id}/logs",
        request: None,
        response: ResponseSchema::StringArray,
    },
    DomainOperation {
        method: "get",
        path: "/api/admin/acme/jobs/{id}/poll",
        request: None,
        response: ResponseSchema::Ref("AcmeJobPollData"),
    },
    DomainOperation {
        method: "get",
        path: "/api/admin/acme/certs/{domain}",
        request: None,
        response: ResponseSchema::Ref("AcmeCertificateData"),
    },
    DomainOperation {
        method: "delete",
        path: "/api/admin/acme/certs/{domain}",
        request: None,
        response: ResponseSchema::Envelope,
    },
    DomainOperation {
        method: "get",
        path: "/api/admin/acme/certs/{domain}/download",
        request: None,
        response: ResponseSchema::ZipAttachment,
    },
    DomainOperation {
        method: "post",
        path: "/api/admin/acme/certs/{domain}/deploy",
        request: None,
        response: ResponseSchema::DirectJson("AcmeActionMessageData"),
    },
];

pub(super) fn apply(paths: &mut Map<String, Value>) -> usize {
    let mut applied = 0;
    for contract in OPERATIONS {
        let path = paths
            .entry(contract.path.to_string())
            .or_insert_with(|| Value::Object(Map::new()))
            .as_object_mut()
            .expect("domain path is generated as an object");
        let operation = path
            .entry(contract.method.to_string())
            .or_insert_with(|| {
                json!({
                    "operationId": super::operation_id(contract.method, contract.path),
                    "summary": format!("{} {}", contract.method.to_ascii_uppercase(), contract.path),
                    "tags": [super::route_tag(contract.path)],
                    "parameters": super::path_parameters(contract.path),
                    "responses": {
                        "default": {
                            "description": "Standard fn-knock error response",
                            "content": {
                                "application/json": {
                                    "schema": schema_ref("ApiErrorEnvelope")
                                }
                            }
                        }
                    }
                })
            })
            .as_object_mut()
            .expect("domain operation is generated as an object");

        operation.insert(
            "x-fn-knock-contract-source".to_string(),
            json!("utoipa-domain"),
        );
        if let Some(request) = contract.request {
            let (request, required) = request
                .strip_prefix('?')
                .map_or((request, true), |request| (request, false));
            operation.insert(
                "requestBody".to_string(),
                json!({
                    "required": required,
                    "content": {
                        "application/json": {
                            "schema": schema_ref(request)
                        }
                    }
                }),
            );
        } else if matches!(contract.method, "post" | "put" | "patch") {
            operation.remove("requestBody");
        }

        let success = match contract.response {
            ResponseSchema::Ref(schema) => json_success_response(schema_ref(schema)),
            ResponseSchema::NullableRef(schema) => json_success_response(json!({
                "anyOf": [schema_ref(schema), { "type": "null" }]
            })),
            ResponseSchema::Array(schema) => json_success_response(json!({
                "type": "array",
                "items": schema_ref(schema)
            })),
            ResponseSchema::StringArray => json_success_response(json!({
                "type": "array",
                "items": { "type": "string" }
            })),
            ResponseSchema::OptionalStringArray => json!({
                "description": "Successful response; data is present when a host was removed",
                "content": {
                    "application/json": {
                        "schema": {
                            "type": "object",
                            "required": ["success"],
                            "properties": {
                                "success": { "type": "boolean", "const": true },
                                "message": { "type": ["string", "null"] },
                                "data": {
                                    "type": "array",
                                    "items": { "type": "string" }
                                }
                            },
                            "additionalProperties": true
                        }
                    }
                }
            }),
            ResponseSchema::RawJson(schema) => json!({
                "description": "JSON attachment",
                "content": {
                    "application/json": {
                        "schema": schema_ref(schema)
                    }
                }
            }),
            ResponseSchema::DirectJson(schema) => json!({
                "description": "Successful response",
                "content": {
                    "application/json": {
                        "schema": schema_ref(schema)
                    }
                }
            }),
            ResponseSchema::Binary => json!({
                "description": "Backup archive",
                "content": {
                    "application/octet-stream": {
                        "schema": { "type": "string", "format": "binary" }
                    }
                }
            }),
            ResponseSchema::PemAttachment => json!({
                "description": "PEM certificate attachment",
                "headers": {
                    "Content-Disposition": {
                        "description": "Attachment filename",
                        "schema": { "type": "string" }
                    }
                },
                "content": {
                    "application/x-pem-file": {
                        "schema": { "type": "string" }
                    }
                }
            }),
            ResponseSchema::HtmlAttachment => json!({
                "description": "HTML bookmark attachment",
                "content": {
                    "text/html": {
                        "schema": { "type": "string" }
                    }
                }
            }),
            ResponseSchema::DiagnosticsZip => json!({
                "description": "Redacted runtime diagnostics archive",
                "headers": {
                    "Content-Disposition": {
                        "description": "Attachment filename",
                        "schema": { "type": "string" }
                    }
                },
                "content": {
                    "application/zip": {
                        "schema": { "type": "string", "format": "binary" }
                    }
                }
            }),
            ResponseSchema::ZipAttachment => json!({
                "description": "ZIP archive attachment",
                "headers": {
                    "Content-Disposition": {
                        "description": "Attachment filename",
                        "schema": { "type": "string" }
                    }
                },
                "content": {
                    "application/zip": {
                        "schema": { "type": "string", "format": "binary" }
                    }
                }
            }),
            ResponseSchema::BinaryPayload => json!({
                "description": "Captured event payload",
                "headers": {
                    "Content-Disposition": {
                        "description": "Attachment disposition for streamed payloads",
                        "schema": { "type": "string" }
                    }
                },
                "content": {
                    "application/octet-stream": {
                        "schema": { "type": "string", "format": "binary" }
                    }
                }
            }),
            ResponseSchema::EventStream => json!({
                "description": "Live traffic event stream",
                "content": {
                    "text/event-stream": {
                        "schema": { "type": "string" }
                    }
                }
            }),
            ResponseSchema::Envelope => json!({
                "description": "Successful response",
                "content": {
                    "application/json": {
                        "schema": schema_ref("ApiSuccessEnvelope")
                    }
                }
            }),
        };
        operation
            .entry("responses".to_string())
            .or_insert_with(|| Value::Object(Map::new()))
            .as_object_mut()
            .expect("responses is generated as an object")
            .insert("200".to_string(), success);
        if matches!(contract.response, ResponseSchema::BinaryPayload)
            || (matches!(contract.response, ResponseSchema::ZipAttachment)
                && contract.path == "/api/admin/deep-monitor/sessions/{session_id}/download")
        {
            operation
                .get_mut("responses")
                .and_then(Value::as_object_mut)
                .expect("responses is generated as an object")
                .insert(
                    "204".to_string(),
                    json!({ "description": "The requested stream is empty" }),
                );
        }
        let query_parameters = query_parameters(contract);
        if !query_parameters.is_empty() {
            operation
                .entry("parameters".to_string())
                .or_insert_with(|| Value::Array(Vec::new()))
                .as_array_mut()
                .expect("parameters is generated as an array")
                .extend(query_parameters);
        }
        refine_path_parameters(operation, contract);
        applied += 1;
    }
    applied
}

fn schema_ref(schema: &str) -> Value {
    json!({ "$ref": format!("#/components/schemas/{schema}") })
}

fn query_parameters(contract: &DomainOperation) -> Vec<Value> {
    match (contract.method, contract.path) {
        ("get", "/api/admin/dashboard/stats") => vec![
            query_parameter(
                "rangeSec",
                json!({ "type": "integer", "minimum": 60, "maximum": 2_592_000 }),
            ),
            query_parameter("userId", json!({ "type": "string", "minLength": 1 })),
            query_parameter("host", json!({ "type": "string", "minLength": 1 })),
            query_parameter("stream", json!({ "type": "string", "minLength": 1 })),
        ],
        ("get", "/api/admin/dashboard/active-ips") => vec![required_query_parameter(
            "host",
            json!({ "type": "string", "minLength": 1 }),
        )],
        ("get", "/api/admin/dashboard/stream-active-ips") => vec![required_query_parameter(
            "stream",
            json!({ "type": "string", "minLength": 1 }),
        )],
        ("get", "/api/admin/events") => vec![
            query_parameter("page", json!({ "type": "integer", "minimum": 1 })),
            query_parameter(
                "limit",
                json!({ "type": "string", "pattern": "^[1-9][0-9]*$" }),
            ),
            query_parameter("search", json!({ "type": "string" })),
            query_parameter(
                "type",
                json!({ "type": "string", "enum": crate::events::SYSTEM_EVENT_TYPES }),
            ),
            query_parameter(
                "level",
                json!({ "type": "string", "enum": crate::events::SYSTEM_EVENT_LEVELS }),
            ),
            query_parameter(
                "source",
                json!({ "type": "string", "enum": crate::events::SYSTEM_EVENT_SOURCES }),
            ),
        ],
        ("get", "/api/admin/backoff/status") => vec![required_query_parameter(
            "ip",
            json!({ "type": "string", "minLength": 1 }),
        )],
        ("get", "/api/admin/cidr/cities") => vec![required_query_parameter(
            "province",
            json!({ "type": "string", "minLength": 1 }),
        )],
        ("get", "/api/admin/cidr/selector") => vec![query_parameter(
            "province",
            json!({ "type": "string", "minLength": 1 }),
        )],
        ("get", "/api/admin/cidr/cidrs") => vec![
            required_query_parameter("province", json!({ "type": "string", "minLength": 1 })),
            query_parameter("city", json!({ "type": "string", "minLength": 1 })),
            query_parameter(
                "operator",
                json!({ "type": "string", "enum": ["电信", "联通", "移动"] }),
            ),
        ],
        ("get", "/api/admin/runtime-health/logs/{component}") => vec![query_parameter(
            "limit",
            json!({ "type": "integer", "minimum": 1, "maximum": 500 }),
        )],
        ("get", "/api/admin/security/overview") => vec![query_parameter(
            "rangeSec",
            json!({ "type": "integer", "minimum": 60, "maximum": 2_592_000 }),
        )],
        ("get", "/api/admin/scanner/blacklist") | ("get", "/api/admin/general-blacklist") => vec![
            query_parameter("page", json!({ "type": "integer", "minimum": 1 })),
            query_parameter(
                "limit",
                json!({ "type": "string", "pattern": "^[1-9][0-9]*$" }),
            ),
            query_parameter("search", json!({ "type": "string" })),
        ],
        ("get", "/api/admin/ssh-security/login-logs") => vec![
            query_parameter("page", json!({ "type": "integer", "minimum": 1 })),
            query_parameter(
                "limit",
                json!({ "type": "string", "pattern": "^[1-9][0-9]*$" }),
            ),
            query_parameter("search", json!({ "type": "string" })),
            query_parameter(
                "outcome",
                json!({ "type": "string", "enum": ["success", "failure"] }),
            ),
        ],
        ("get", "/api/admin/ssh-security/blocks") => vec![
            query_parameter("page", json!({ "type": "integer", "minimum": 1 })),
            query_parameter(
                "limit",
                json!({ "type": "string", "pattern": "^[1-9][0-9]*$" }),
            ),
            query_parameter("search", json!({ "type": "string" })),
        ],
        ("get", "/api/admin/wol/discover/jobs/{id}") => vec![query_parameter(
            "cursor",
            json!({ "type": "integer", "minimum": 0 }),
        )],
        ("get", "/api/admin/scan/discover/jobs/{job_id}") => vec![query_parameter(
            "cursor",
            json!({ "type": "integer", "minimum": 0 }),
        )],
        ("get", "/api/admin/terminal/attachments/{id}/poll") => vec![
            query_parameter(
                "cursor",
                json!({
                    "oneOf": [
                        { "type": "integer", "minimum": 0 },
                        { "type": "string", "pattern": r"^\s*[+-]?\d+" }
                    ],
                    "default": 0,
                    "description": "Byte cursor. Legacy integer-prefix strings remain accepted."
                }),
            ),
            query_parameter(
                "timeout_ms",
                json!({
                    "type": "number",
                    "default": 15_000,
                    "description": "Long-poll timeout; non-zero values are clamped to 1000–20000 ms and zero selects the default."
                }),
            ),
        ],
        ("get", "/api/admin/cloudflared/logs") => vec![query_parameter(
            "limit",
            json!({
                "oneOf": [
                    { "type": "integer" },
                    { "type": "string", "pattern": r"^\s*[+-]?\d+" }
                ],
                "default": 200,
                "description": "Number of retained lines. Legacy integer-prefix strings remain accepted; the result is clamped to 1–1000."
            }),
        )],
        ("get", "/api/admin/cloudflared/poll") => vec![query_parameter(
            "cursor",
            json!({
                "oneOf": [
                    { "type": "integer", "minimum": 0 },
                    { "type": "string", "pattern": "^[0-9]+$" }
                ],
                "description": "Last observed log sequence. Omission returns the retained buffer; stale or future cursors set reset=true."
            }),
        )],
        (
            "get",
            "/api/admin/frpc/overview"
            | "/api/admin/frpc/logs"
            | "/api/admin/frpc/instances/{id}"
            | "/api/admin/frpc/instances/{id}/logs",
        ) => vec![query_parameter(
            "limit",
            json!({
                "oneOf": [
                    { "type": "integer" },
                    { "type": "string", "pattern": r"^\s*[+-]?\d+" }
                ],
                "default": 200,
                "description": "Number of retained lines. Legacy integer-prefix strings remain accepted; the result is clamped to 1–1000."
            }),
        )],
        ("get", "/api/admin/frpc/poll" | "/api/admin/frpc/instances/{id}/poll") => {
            vec![query_parameter(
                "cursor",
                json!({
                    "oneOf": [
                        { "type": "integer", "minimum": 0 },
                        { "type": "string", "pattern": "^[0-9]+$" }
                    ],
                    "description": "Last observed log sequence. Invalid values request the retained buffer; stale or future cursors set reset=true."
                }),
            )]
        }
        ("get", "/api/admin/ddns/logs") => vec![query_parameter(
            "limit",
            json!({
                "oneOf": [
                    { "type": "integer" },
                    { "type": "string", "pattern": r"^\s*[+-]?\d+" }
                ],
                "default": 200,
                "description": "Number of retained entries. Legacy integer-prefix strings remain accepted; the result is clamped to 1–1000."
            }),
        )],
        ("get", "/api/admin/ddns/poll") => vec![query_parameter(
            "cursor",
            json!({
                "oneOf": [
                    { "type": "integer", "minimum": 0 },
                    { "type": "string", "pattern": "^[0-9]+$" }
                ],
                "description": "Last observed log sequence. Invalid values request the retained buffer; stale or future cursors set reset=true."
            }),
        )],
        ("get", "/api/admin/acme/jobs/{id}/poll") => vec![
            query_parameter(
                "limit",
                json!({
                    "oneOf": [
                        { "type": "number" },
                        { "type": "string" }
                    ],
                    "default": 500,
                    "description": "Log line limit. JavaScript-number-compatible values are floored and clamped to 1–1000; invalid strings select 500 and an empty string selects 1."
                }),
            ),
            query_parameter(
                "order",
                json!({
                    "type": "string",
                    "default": "desc",
                    "description": "Only asc selects chronological order; other legacy values select desc."
                }),
            ),
        ],
        ("get", "/api/admin/ssl/shared-files/content") => vec![required_query_parameter(
            "path",
            json!({ "type": "string", "minLength": 1 }),
        )],
        ("get", "/api/admin/waf/logs") => vec![
            query_parameter("date", json!({ "type": "string", "format": "date" })),
            query_parameter("trace_id", json!({ "type": "string", "minLength": 1 })),
            query_parameter("search", json!({ "type": "string" })),
            query_parameter("host", json!({ "type": "string" })),
            query_parameter("client_ip", json!({ "type": "string" })),
            query_parameter(
                "rule_id",
                json!({
                    "type": "string",
                    "pattern": r"^\s*[+-]?\d+",
                    "description": "Rule ID. Legacy integer-prefix strings remain accepted."
                }),
            ),
            query_parameter("route_type", json!({ "type": "string" })),
            query_parameter(
                "mode",
                json!({ "type": "string", "enum": ["off", "detection", "blocking"] }),
            ),
            query_parameter(
                "cursor",
                json!({
                    "type": "string",
                    "pattern": r"^\s*[+-]?\d+",
                    "default": "0",
                    "description": "Result offset. Invalid or negative values select zero."
                }),
            ),
            query_parameter(
                "limit",
                json!({
                    "type": "string",
                    "pattern": r"^\s*[+-]?\d+",
                    "default": "50",
                    "description": "Page size. Positive values are capped at 200; other values select 50."
                }),
            ),
        ],
        ("get", "/api/admin/notifications/triggers") => vec![
            query_parameter(
                "page",
                json!({
                    "oneOf": [
                        { "type": "integer", "minimum": 1 },
                        { "type": "string", "pattern": r"^\s*[+]?[1-9]\d*" }
                    ],
                    "default": 1
                }),
            ),
            query_parameter(
                "limit",
                json!({
                    "oneOf": [
                        { "type": "integer", "minimum": 1 },
                        { "type": "string", "pattern": r"^\s*[+]?[1-9]\d*" }
                    ],
                    "default": 20,
                    "description": "Page size; positive values are capped at 100."
                }),
            ),
            query_parameter("rule_id", json!({ "type": "string", "minLength": 1 })),
            query_parameter(
                "status",
                json!({
                    "type": "string",
                    "enum": ["created", "fanout_done", "partially_failed", "completed"]
                }),
            ),
        ],
        ("get", "/api/admin/notifications/deliveries") => vec![
            query_parameter(
                "page",
                json!({
                    "oneOf": [
                        { "type": "integer", "minimum": 1 },
                        { "type": "string", "pattern": r"^\s*[+]?[1-9]\d*" }
                    ],
                    "default": 1
                }),
            ),
            query_parameter(
                "limit",
                json!({
                    "oneOf": [
                        { "type": "integer", "minimum": 1 },
                        { "type": "string", "pattern": r"^\s*[+]?[1-9]\d*" }
                    ],
                    "default": 20,
                    "description": "Page size; positive values are capped at 100."
                }),
            ),
            query_parameter("rule_id", json!({ "type": "string", "minLength": 1 })),
            query_parameter("provider_id", json!({ "type": "string", "minLength": 1 })),
            query_parameter("trigger_id", json!({ "type": "string", "minLength": 1 })),
            query_parameter(
                "status",
                json!({
                    "type": "string",
                    "enum": ["queued", "sending", "success", "failed", "gave_up", "skipped"]
                }),
            ),
        ],
        ("get", "/api/admin/deep-monitor/sessions") => vec![query_parameter(
            "include_expired",
            json!({ "type": "boolean", "default": false }),
        )],
        ("get", "/api/admin/deep-monitor/sessions/{session_id}/events") => vec![
            query_parameter("cursor", json!({ "type": "string" })),
            query_parameter(
                "limit",
                json!({ "type": "integer", "minimum": 1, "maximum": 200, "default": 100 }),
            ),
            query_parameter(
                "type",
                json!({
                    "type": "string",
                    "enum": ["http_exchange", "ws_open", "ws_frame", "monitor_notice"]
                }),
            ),
            query_parameter("search", json!({ "type": "string" })),
            query_parameter(
                "direction",
                json!({ "type": "string", "enum": ["client_to_upstream", "upstream_to_client"] }),
            ),
            query_parameter("method", json!({ "type": "string" })),
            query_parameter("status", json!({ "type": "integer" })),
            query_parameter("client_ip", json!({ "type": "string" })),
            query_parameter("identity", json!({ "type": "string" })),
            query_parameter("path", json!({ "type": "string" })),
        ],
        ("get", "/api/admin/deep-monitor/sessions/{session_id}/events/{event_id}/payload") => vec![
            required_query_parameter("part", json!({ "type": "string", "minLength": 1 })),
            query_parameter(
                "offset",
                json!({ "type": "integer", "minimum": 0, "default": 0 }),
            ),
            query_parameter(
                "limit",
                json!({ "type": "integer", "minimum": 1, "maximum": 262_144 }),
            ),
        ],
        ("get", "/api/admin/deep-monitor/sessions/{session_id}/live") => vec![
            query_parameter(
                "after_sequence",
                json!({ "type": "integer", "minimum": 0, "default": 0 }),
            ),
            json!({
                "name": "Last-Event-ID",
                "in": "header",
                "required": false,
                "schema": { "type": "integer", "minimum": 0 }
            }),
        ],
        ("get", "/api/admin/gateway-logs/entries") => vec![
            query_parameter("date", json!({ "type": "string", "format": "date" })),
            query_parameter(
                "pagination",
                json!({ "type": "string", "enum": ["page", "cursor"] }),
            ),
            query_parameter("page", json!({ "type": "integer", "minimum": 1 })),
            query_parameter(
                "limit",
                json!({ "type": "string", "pattern": "^[1-9][0-9]*$" }),
            ),
            query_parameter("cursor", json!({ "type": "string" })),
            query_parameter("search", json!({ "type": "string" })),
            query_parameter("status", json!({ "type": "string" })),
            query_parameter(
                "logged_in",
                json!({ "type": "string", "enum": ["true", "false"] }),
            ),
            query_parameter("credential", json!({ "type": "string" })),
            query_parameter(
                "waf_status",
                json!({ "type": "string", "enum": ["has_waf", "none"] }),
            ),
        ],
        ("get" | "post", "/api/admin/gateway-logs/analytics") => vec![
            query_parameter("from", json!({ "type": "string", "format": "date" })),
            query_parameter("to", json!({ "type": "string", "format": "date" })),
        ],
        _ => Vec::new(),
    }
}

fn refine_path_parameters(operation: &mut Map<String, Value>, contract: &DomainOperation) {
    let refinements: &[(&str, Value)] = match contract.path {
        "/api/admin/runtime-health/logs/{component}" => &[(
            "component",
            json!({
                "type": "string",
                "enum": ["management", "gateway_process"]
            }),
        )],
        "/api/admin/waf/rules/{source}/{filename}" => &[
            (
                "source",
                json!({ "type": "string", "enum": ["system", "custom"] }),
            ),
            (
                "filename",
                json!({ "type": "string", "pattern": r"(?i)\.conf$" }),
            ),
        ],
        "/api/admin/waf/custom/{filename}" => &[(
            "filename",
            json!({ "type": "string", "pattern": r"(?i)\.conf$" }),
        )],
        "/api/admin/cloudflared/optimization/scans/{id}" => {
            &[("id", json!({ "type": "string", "format": "uuid" }))]
        }
        "/api/admin/cloudflared/optimization/domains/{hostname}" => {
            &[("hostname", json!({ "type": "string", "minLength": 1 }))]
        }
        "/api/admin/frpc/instances/{id}"
        | "/api/admin/frpc/instances/{id}/start"
        | "/api/admin/frpc/instances/{id}/stop"
        | "/api/admin/frpc/instances/{id}/restart"
        | "/api/admin/frpc/instances/{id}/logs"
        | "/api/admin/frpc/instances/{id}/poll" => &[(
            "id",
            json!({ "type": "string", "pattern": "^[A-Za-z0-9-]{1,80}$" }),
        )],
        "/api/admin/ddns/config/{provider}" if contract.method == "post" => &[(
            "provider",
            json!({
                "oneOf": [
                    {
                        "type": "string",
                        "enum": [
                            "alidns", "baiducloud", "cloudflare", "dnshe", "dnspod",
                            "duckdns", "dynu", "dynv6", "edgeone_cname", "edgeone",
                            "esa", "godaddy", "huaweicloud", "noip", "porkbun",
                            "tencentcloud"
                        ]
                    },
                    {
                        "type": "string",
                        "pattern": r"^\s*(?:alidns|baiducloud|cloudflare|dnshe|dnspod|duckdns|dynu|dynv6|edgeone_cname|edgeone|esa|godaddy|huaweicloud|noip|porkbun|tencentcloud)\s*$"
                    }
                ]
            }),
        )],
        "/api/admin/ddns/config/{provider}" => &[(
            "provider",
            json!({
                "type": "string",
                "minLength": 1,
                "description": "Configuration lookup key. Unknown providers return an empty authenticated configuration object."
            }),
        )],
        "/api/admin/ddns/targets/{id}"
        | "/api/admin/ddns/targets/{id}/enabled"
        | "/api/admin/ddns/targets/{id}/test" => &[(
            "id",
            json!({ "type": "string", "pattern": "^[A-Za-z0-9-]{1,80}$" }),
        )],
        "/api/admin/acme/applications/{id}"
        | "/api/admin/acme/applications/{id}/certificate"
        | "/api/admin/acme/applications/{id}/library/sync"
        | "/api/admin/acme/applications/{id}/deploy"
        | "/api/admin/acme/applications/{id}/request" => &[(
            "id",
            json!({
                "type": "string",
                "minLength": 1,
                "description": "Opaque application identifier; legacy non-UUID identifiers remain accepted."
            }),
        )],
        "/api/admin/acme/jobs/{id}"
        | "/api/admin/acme/jobs/{id}/logs"
        | "/api/admin/acme/jobs/{id}/poll" => &[(
            "id",
            json!({
                "type": "string",
                "minLength": 1,
                "description": "Opaque job identifier; current jobs use UUIDs and legacy identifiers remain accepted."
            }),
        )],
        "/api/admin/acme/certs/{domain}"
        | "/api/admin/acme/certs/{domain}/download"
        | "/api/admin/acme/certs/{domain}/deploy" => &[(
            "domain",
            json!({
                "type": "string",
                "minLength": 1,
                "description": "Primary certificate domain; wildcard names are accepted when URL-encoded."
            }),
        )],
        "/api/admin/ssl/certificates/{id}" => {
            &[("id", json!({ "type": "string", "minLength": 1 }))]
        }
        _ => return,
    };
    let Some(parameters) = operation
        .get_mut("parameters")
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    for (name, schema) in refinements {
        if let Some(parameter) = parameters.iter_mut().find(|parameter| {
            parameter.get("name") == Some(&json!(name))
                && parameter.get("in") == Some(&json!("path"))
        }) {
            parameter["schema"] = schema.clone();
        }
    }
}

fn query_parameter(name: &str, schema: Value) -> Value {
    json!({
        "name": name,
        "in": "query",
        "required": false,
        "schema": schema,
    })
}

fn required_query_parameter(name: &str, schema: Value) -> Value {
    let mut parameter = query_parameter(name, schema);
    parameter["required"] = json!(true);
    parameter
}

fn json_success_response(data: Value) -> Value {
    json!({
        "description": "Successful response",
        "content": {
            "application/json": {
                "schema": {
                    "type": "object",
                    "required": ["success", "data"],
                    "properties": {
                        "success": { "type": "boolean", "const": true },
                        "message": { "type": ["string", "null"] },
                        "data": data
                    },
                    "additionalProperties": true
                }
            }
        }
    })
}

#[cfg(test)]
pub(super) fn expected_operation_count() -> usize {
    OPERATIONS.len()
}
