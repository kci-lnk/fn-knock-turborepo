import type { WAFConfig } from "./waf";
import type {
  AppearanceConfig,
  AuthCredentialSettings,
  AutoHttpsConfig,
  DashboardDisplayConfig,
  FnosConnectWafDetails,
  FnosNetworkTuningConfig,
  FnosPortIconHijackConfig,
  FnosShareBypassConfig,
  GatewayLoggingConfig,
  HostMapping,
  HostMappingGroup,
  LocaleConfig,
  ProtocolMappingFeatureConfig,
  ProxyMapping,
  ReverseProxySubmode,
  RunType,
  RuntimeCapabilities,
  RuntimeProfile,
  ScanDiscoveryConfig,
  SmartConnectConfig,
  SSLDeploymentMode,
  StreamMapping,
  SubdomainModeConfig,
  TerminalFeatureConfig,
  WOLFeatureConfig,
} from "./core";
import type {
  GatewayCrawlerBlockerConfig,
  GatewayHostResponseConfig,
  GatewayPortalConfig,
  GatewayProxyHeadersConfig,
  GatewayUnmatchedRouteConfig,
  ReverseProxyThrottleConfig,
  SSHSecurityConfig,
} from "./gateway";

export interface AppConfig {
  run_type: RunType;
  reverse_proxy_submode: ReverseProxySubmode;
  auto_manage_firewall: boolean;
  firewall_additional_ports: number[];
  runtime_profile?: RuntimeProfile;
  capabilities?: RuntimeCapabilities;
  whitelist_ips: string[];
  default_route: string;
  proxy_mappings: ProxyMapping[];
  host_mappings: HostMapping[];
  host_mapping_groups: HostMappingGroup[];
  host_mapping_grouped_view: boolean;
  stream_mappings: StreamMapping[];
  subdomain_mode: SubdomainModeConfig;
  default_tunnel?: "frp" | "cloudflared";
  fnos_share_bypass?: FnosShareBypassConfig;
  fnos_port_icon_hijack?: FnosPortIconHijackConfig;
  fnos_connect_waf?: FnosConnectWafDetails["config"];
  fnos_network_tuning?: FnosNetworkTuningConfig;
  fnos_certificate_sync?: { auto_sync_enabled: boolean };
  gateway_logging?: GatewayLoggingConfig;
  waf?: WAFConfig;
  reverse_proxy_throttle?: ReverseProxyThrottleConfig;
  gateway_proxy_headers?: GatewayProxyHeadersConfig;
  gateway_host_response?: GatewayHostResponseConfig;
  gateway_crawler_blocker?: GatewayCrawlerBlockerConfig;
  gateway_portal?: GatewayPortalConfig;
  gateway_unmatched_route?: GatewayUnmatchedRouteConfig;
  appearance?: AppearanceConfig;
  protocol_mapping_feature?: ProtocolMappingFeatureConfig;
  auto_https?: AutoHttpsConfig;
  dashboard_display?: DashboardDisplayConfig;
  smart_connect?: SmartConnectConfig;
  scan_discovery?: ScanDiscoveryConfig;
  auth_credential_settings?: AuthCredentialSettings;
  terminal_feature?: TerminalFeatureConfig;
  wol_feature?: WOLFeatureConfig;
  ssh_security?: SSHSecurityConfig;
  locale?: LocaleConfig;
  ssl: {
    enabled: boolean;
    active_cert_id?: string;
    deployment_mode?: SSLDeploymentMode;
    certificate_count?: number;
  };
  login: {
    nonce_list: string[];
    ip_backoff: Record<string, number>;
  };
}
