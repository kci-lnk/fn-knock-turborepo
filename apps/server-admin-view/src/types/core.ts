import type { components as ApiContractComponents } from "@fn-knock/api-contract";
import type { GatewayVisibilitySelection } from "./gateway";

export type { AppearanceConfig } from "@frontend-core/appearance";
export type { DateTimeDisplayMode } from "@admin-shared/composables/useDateTimeDisplayState";

export interface ProxyMapping {
  path: string;
  target: string;
  rewrite_html: boolean;
  use_auth: boolean;
  use_root_mode: boolean;
  strip_path: boolean;
}

export type RunType =
  ApiContractComponents["schemas"]["RunTypeUpdateData"]["run_type"];
export type ReverseProxySubmode = NonNullable<
  ApiContractComponents["schemas"]["RunTypeUpdateData"]["reverse_proxy_submode"]
>;
export type LocaleConfig = ApiContractComponents["schemas"]["LocaleConfigData"];
export type LocaleCode = LocaleConfig["default_locale"];

export type DeploymentTarget =
  ApiContractComponents["schemas"]["PanelBootstrapData"]["deployment_target"];

export interface RuntimeProfile {
  deployment_target: DeploymentTarget;
  is_docker: boolean;
  is_linux: boolean;
  is_windows: boolean;
  is_root_process: boolean;
}

export interface RuntimeCapabilities {
  direct_mode_available: boolean;
  host_firewall_available: boolean;
  smart_connect_available: boolean;
  fnos_certificate_sync_available?: boolean;
  system_clock_sync_available: boolean;
  self_update_available: boolean;
  terminal_available: boolean;
  deep_monitor_available?: boolean;
  auto_https_available?: boolean;
  fnos_network_tuning_available?: boolean;
  fnos_connect_waf_available?: boolean;
  shared_root_available: boolean;
  acme_available?: boolean;
  acme_resource_required?: boolean;
  cloudflared_available?: boolean;
  frpc_available?: boolean;
  ssh_security_available?: boolean;
  system_resource_monitor_available?: boolean;
  desktop_update_managed?: boolean;
}

export type DockerAdminBootstrapState =
  ApiContractComponents["schemas"]["PanelBootstrapData"];

export type HostAccessMode =
  ApiContractComponents["schemas"]["SubdomainModeData"]["default_access_mode"];
export type HostProtocolMode = "auto" | "http1" | "http2";
export type HostTargetPathMode =
  ApiContractComponents["schemas"]["HostTargetPathModeData"];
export type HostServiceRole = "app" | "auth";
export type StreamMappingProtocol =
  ApiContractComponents["schemas"]["StreamMappingData"]["protocol"];

export interface HostMappingBasicAuth {
  enabled: boolean;
  username: string;
  password: string;
}

export interface DailyAvailability {
  enabled: true;
  start_time: string;
  end_time: string;
}

export type HostMappingAvailability = DailyAvailability;

export type HostVisibilityMode = "inherit" | "custom" | "disabled";

export interface HostMappingVisibility {
  mode: HostVisibilityMode;
  selections: GatewayVisibilitySelection[];
  custom_cidrs: string[];
  cidrs: string[];
}

export type AdvancedAuthConditionTarget =
  ApiContractComponents["schemas"]["AdvancedAuthConditionData"]["target"];
export type AdvancedAuthOperator =
  ApiContractComponents["schemas"]["AdvancedAuthConditionData"]["operator"];
export type AdvancedAuthConditionContract =
  ApiContractComponents["schemas"]["AdvancedAuthConditionData"];
export type AdvancedAuthCondition = Omit<
  AdvancedAuthConditionContract,
  "selections"
> & {
  selections: GatewayVisibilitySelection[];
};
type AdvancedAuthRuleGroupContract =
  ApiContractComponents["schemas"]["AdvancedAuthRuleGroupData"];
export type AdvancedAuthRuleGroup = Omit<
  AdvancedAuthRuleGroupContract,
  "conditions"
> & {
  conditions: AdvancedAuthCondition[];
};
export type AdvancedAuthConfigContract =
  ApiContractComponents["schemas"]["AdvancedAuthConfigData"];
export type AdvancedAuthConfig = Omit<AdvancedAuthConfigContract, "groups"> & {
  groups: AdvancedAuthRuleGroup[];
};

export type HostLocationMatch = "exact" | "prefix";
export type HostLocationAction = "proxy" | "response";

export interface HostLocationResponse {
  status: number;
  content_type: string;
  headers: Record<string, string>;
  body: string;
}

export interface HostLocation {
  path: string;
  match: HostLocationMatch;
  action: HostLocationAction;
  target: string;
  strip_path: boolean;
  rewrite_html: boolean;
  response: HostLocationResponse;
}

export interface HostMapping {
  host: string;
  sync_id?: string;
  group_id: string | null;
  target: string;
  target_path_mode: HostTargetPathMode;
  waf_enabled: boolean;
  use_auth: boolean;
  access_mode: HostAccessMode;
  suppress_toolbar: boolean;
  preserve_host: boolean;
  is_default: boolean;
  disabled: boolean;
  availability: HostMappingAvailability | null;
  visibility: HostMappingVisibility;
  protocol_mode: HostProtocolMode;
  basic_auth: HostMappingBasicAuth;
  locations: HostLocation[];
  service_role: HostServiceRole;
  title: string;
  title_override: string;
  favicon: string;
  favicon_override: string;
  advanced_auth?: AdvancedAuthConfig;
}

export interface HostMappingGroup {
  id: string;
  name: string;
}

export type HostMappingRefreshSummary =
  ApiContractComponents["schemas"]["HostMappingRefreshSummaryData"];

export type UrlMetadataPreview =
  ApiContractComponents["schemas"]["HostMappingMetadataData"];

export type StreamMapping =
  ApiContractComponents["schemas"]["StreamMappingData"];

export type PasskeyRpMode =
  ApiContractComponents["schemas"]["SubdomainModeData"]["passkey_rp_mode"];
export type PostLoginIpGrantMode =
  ApiContractComponents["schemas"]["AuthCredentialSettingsData"]["post_login_ip_grant_mode"];

export type SubdomainModeConfig =
  ApiContractComponents["schemas"]["SubdomainModeData"];

export type SSLConfig =
  ApiContractComponents["schemas"]["SslCertificateSaveBodyData"];
export type SSLCertInfo =
  ApiContractComponents["schemas"]["SslCertificateInfoData"];
export type SSLDeploymentMode =
  ApiContractComponents["schemas"]["SslDeploymentModeBodyData"]["deployment_mode"];
export type SSLCertificateSource = NonNullable<SSLConfig["source"]>;
export type SubdomainCertificateCoverage =
  ApiContractComponents["schemas"]["SslSubdomainCoverageData"];
export type SubdomainCertificateLibraryCoverage =
  ApiContractComponents["schemas"]["SslCertificateLibraryCoverageData"];
export type SSLCertificateSummary =
  ApiContractComponents["schemas"]["SslCertificateSummaryData"];
export type SSLStatus = ApiContractComponents["schemas"]["SslStatusData"];
export type SharedDataFileEntry =
  ApiContractComponents["schemas"]["SslSharedFileData"];
export type SSLSharedFilesPayload =
  ApiContractComponents["schemas"]["SslSharedFilesData"];
export type SSLCAStatus = ApiContractComponents["schemas"]["SslCaStatusData"];
export type ExternalCertificateBinding =
  ApiContractComponents["schemas"]["ExternalCertificateBindingData"];
export type ExternalCertificateBindingCredential =
  ApiContractComponents["schemas"]["ExternalCertificateBindingCredentialData"];
export type LanCertificateDeployment =
  ApiContractComponents["schemas"]["LanCertificateDeploymentData"];

export type FnosShareBypassConfig =
  ApiContractComponents["schemas"]["FnosShareBypassData"];

export type FnosPortIconHijackConfig =
  ApiContractComponents["schemas"]["FnosPortIconHijackData"];

export type FnosConnectWafDetails =
  ApiContractComponents["schemas"]["FnosConnectWafData"];

export type FnosCertificateSyncStatus =
  ApiContractComponents["schemas"]["FnosCertificateSyncItemData"]["status"];
export type FnosCertificateSyncItem =
  ApiContractComponents["schemas"]["FnosCertificateSyncItemData"];
export type FnosCertificateSyncDetails =
  ApiContractComponents["schemas"]["FnosCertificateSyncDetailsData"];
export type FnosCertificateSyncSummary =
  ApiContractComponents["schemas"]["FnosCertificateSyncSummaryData"];
export type FnosCertificateSyncResponse =
  ApiContractComponents["schemas"]["FnosCertificateSyncResponseData"];

export type FnosNetworkTuningConfig =
  ApiContractComponents["schemas"]["FnosNetworkTuningConfigData"];

export type FnosNetworkTuningKernelState =
  ApiContractComponents["schemas"]["FnosNetworkTuningKernelData"];

export type FnosNetworkTuningStatus =
  ApiContractComponents["schemas"]["FnosNetworkTuningData"];

export type FnosNetworkTuningUpdatePayload =
  ApiContractComponents["schemas"]["FnosNetworkTuningUpdateData"];

export type GatewayLoggingConfig =
  ApiContractComponents["schemas"]["GatewayLoggingConfigData"];

export * from "./waf";

export type IpLocationLookupStatus =
  ApiContractComponents["schemas"]["IpLocationSnapshotData"]["status"];
export type IpLocationSnapshot =
  ApiContractComponents["schemas"]["IpLocationSnapshotData"];
export type IpLocationBatchPayload =
  ApiContractComponents["schemas"]["IpLocationBatchData"];

export type ProtocolMappingFeatureConfig =
  ApiContractComponents["schemas"]["ProtocolMappingFeatureData"];

export type AutoHttpsConfig =
  ApiContractComponents["schemas"]["AutoHttpsConfigData"];

export type AutoHttpsRuntimeStatus =
  ApiContractComponents["schemas"]["AutoHttpsRuntimeData"]["status"];

export type AutoHttpsRuntimeState =
  ApiContractComponents["schemas"]["AutoHttpsRuntimeData"];

export type AutoHttpsDetails =
  ApiContractComponents["schemas"]["AutoHttpsDetailsData"];

export type SidebarNavItemId =
  ApiContractComponents["schemas"]["DashboardDisplayData"]["sidebar_menu_order"][number];

export type DeepMonitorSession =
  ApiContractComponents["schemas"]["DeepMonitorSessionData"];
export type DeepMonitorEventSummary =
  ApiContractComponents["schemas"]["DeepMonitorEventSummaryData"];
export type DeepMonitorPayloadRef =
  ApiContractComponents["schemas"]["DeepMonitorPayloadRefData"];
export type DeepMonitorHeader =
  ApiContractComponents["schemas"]["DeepMonitorHeaderData"];
export type DeepMonitorTiming =
  ApiContractComponents["schemas"]["DeepMonitorTimingData"];
export type DeepMonitorWebSocketFrame =
  ApiContractComponents["schemas"]["DeepMonitorWebSocketFrameData"];
export type DeepMonitorEvent =
  ApiContractComponents["schemas"]["DeepMonitorEventData"];

export type DashboardDisplayConfig =
  ApiContractComponents["schemas"]["DashboardDisplayData"];

export type SmartConnectConfig =
  ApiContractComponents["schemas"]["SmartConnectConfigData"];

export interface ScanDiscoveryConfig {
  custom_cidrs: string[];
  selected_cidrs: string[];
}

export type SmartConnectRuntimeState =
  ApiContractComponents["schemas"]["SmartConnectRuntimeData"];

export type DnsmasqInstallStatus =
  ApiContractComponents["schemas"]["DnsmasqInstallStateData"]["status"];

export type DnsmasqInstallState =
  ApiContractComponents["schemas"]["DnsmasqInstallStateData"];

export type DnsmasqStatus =
  ApiContractComponents["schemas"]["DnsmasqStatusData"];

export type SmartConnectAvailability =
  ApiContractComponents["schemas"]["SmartConnectAvailabilityData"];

export type SmartConnectLocalIpOption =
  ApiContractComponents["schemas"]["SmartConnectLocalIpData"];

export type SmartConnectDetails =
  ApiContractComponents["schemas"]["SmartConnectDetailsData"];

export type AuthCredentialSettings = Omit<
  ApiContractComponents["schemas"]["AuthCredentialSettingsData"],
  "post_login_ip_grant_ttl_seconds"
> & {
  // The normalized server response always includes this key, including null.
  post_login_ip_grant_ttl_seconds: number | null;
};

export type GatewayLogEntry =
  ApiContractComponents["schemas"]["GatewayLogEntryData"] & {
    // Presentation-only enrichment populated from the shared IP location cache.
    ipLocation?: string;
  };

export type GatewayLogDatesPayload =
  ApiContractComponents["schemas"]["GatewayLogDatesData"];

export type GatewayLogEntriesPayload = Omit<
  ApiContractComponents["schemas"]["GatewayLogEntriesData"],
  "items"
> & {
  items: GatewayLogEntry[];
};

export type GatewayLogDeletePayload =
  ApiContractComponents["schemas"]["GatewayLogDeleteData"];

export type GatewayLogAnalyticsBucket =
  ApiContractComponents["schemas"]["GatewayLogAnalyticsBucketData"];

export type GatewayLogAnalyticsRegionBucket =
  ApiContractComponents["schemas"]["GatewayLogAnalyticsRegionBucketData"];

export type GatewayLogAnalyticsPayload =
  ApiContractComponents["schemas"]["GatewayLogAnalyticsData"];

export type FnKnockBackupImportArchiveRequest =
  ApiContractComponents["schemas"]["ImportBackupBody"];

export type FnKnockBackupImportResult =
  ApiContractComponents["schemas"]["BackupImportResultData"];

export interface BackupDirectoryFilesPayload {
  shareName: string;
  available: boolean;
  files: SharedDataFileEntry[];
}

export interface AutomaticBackupConfig {
  enabled: boolean;
  interval_hours: number;
  retention_days: number;
  updated_at: string | null;
}

export interface AutomaticBackupStatus {
  directory_path: string;
  last_attempt_at: string | null;
  last_success_at: string | null;
  last_error: string | null;
  last_filename: string | null;
  next_backup_at: string | null;
}

export interface AutomaticBackupDetails {
  config: AutomaticBackupConfig;
  status: AutomaticBackupStatus;
}

export interface AutomaticBackupFilesPayload {
  directoryPath: string;
  available: boolean;
  files: SharedDataFileEntry[];
}

export type FnKnockBackupExportToDirectoryResult =
  ApiContractComponents["schemas"]["BackupDirectoryExportData"];

export type TerminalFeatureConfig =
  ApiContractComponents["schemas"]["TerminalFeatureData"];

export type WOLFeatureConfig =
  ApiContractComponents["schemas"]["WolFeatureConfigData"];

export type TerminalTmuxInstallState =
  ApiContractComponents["schemas"]["TerminalTmuxInstallStateData"];
export type TerminalTmuxDetectionSource = Exclude<
  TerminalTmuxInstallState["detectionSource"],
  null
>;
export type TerminalTmuxInstallStatus = TerminalTmuxInstallState["status"];

export type TerminalSessionRecord =
  ApiContractComponents["schemas"]["TerminalSessionData"];
export type TerminalSessionStatus = TerminalSessionRecord["status"];

export type TerminalAttachmentRecord =
  ApiContractComponents["schemas"]["TerminalAttachmentData"];
export type TerminalTransport = TerminalAttachmentRecord["transport"];

export type TerminalOutputChunk =
  ApiContractComponents["schemas"]["TerminalOutputChunkData"];

export type TerminalRuntimeStatus =
  ApiContractComponents["schemas"]["TerminalRuntimeStatusData"];

export type FirewallAdditionalPortsDetails =
  ApiContractComponents["schemas"]["FirewallAdditionalPortsData"];

export type TrafficStats =
  ApiContractComponents["schemas"]["DashboardRealtimeData"];
export type HostTrafficStats =
  ApiContractComponents["schemas"]["DashboardHostTrafficData"];
export type StreamTrafficStats =
  ApiContractComponents["schemas"]["DashboardStreamTrafficData"];
export type HostActiveIp =
  ApiContractComponents["schemas"]["DashboardActiveIpData"];
export type HostActiveIpsPayload =
  ApiContractComponents["schemas"]["DashboardActiveIpsData"];
export type StreamActiveIpsPayload =
  ApiContractComponents["schemas"]["DashboardStreamActiveIpsData"];
export type DashboardStats =
  ApiContractComponents["schemas"]["DashboardStatsData"];
