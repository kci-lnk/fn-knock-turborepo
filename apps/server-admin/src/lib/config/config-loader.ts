import { normalizeLocaleConfig } from "../../../../../packages/i18n/src";
import { normalizeAutoManageFirewall } from "../firewall-automation";
import { normalizeReverseProxySubmode } from "../reverse-proxy-submode";
import { normalizeSSHSecurityConfig } from "../ssh-security/config";
import { normalizeTerminalFeatureConfig } from "../terminal-shared";
import {
  DEFAULT_CONFIG,
  DEFAULT_ROUTE_PLACEHOLDER,
  DEFAULT_RUN_TYPE,
  normalizeAuthCredentialSettings,
  normalizeAppearanceConfig,
  normalizeAutoHttpsConfig,
  normalizeDashboardDisplayConfig,
  normalizeEventSystemConfig,
  normalizeFnosNetworkTuningConfig,
  normalizeFnosPortIconHijackConfig,
  normalizeFnosShareBypassConfig,
  normalizeGatewayCrawlerBlockerConfig,
  normalizeGatewayHostResponseConfig,
  normalizeGatewayLoggingSettings,
  normalizeGatewayPortalConfig,
  normalizeGatewayProxyHeadersConfig,
  normalizeGatewayVisibilityConfig,
  normalizeHostMappings,
  normalizeReverseProxyThrottleConfig,
  normalizeSmartConnectConfig,
  normalizeSSLConfig,
  normalizeStreamMappings,
  normalizeSubdomainModeConfig,
} from "./app-config";
import {
  normalizeScanDiscoveryConfig,
  normalizeWAFConfig,
} from "./normalizers";
import type { AppConfig, RunType } from "./types";

const normalizeRunType = (value: unknown): RunType =>
  value === 0 || value === 1 || value === 3 ? value : DEFAULT_RUN_TYPE;

export const normalizePersistedAppConfig = (config: AppConfig): AppConfig => {
  const normalized = { ...config };

  normalized.run_type = normalizeRunType(normalized.run_type);
  normalized.reverse_proxy_submode = normalizeReverseProxySubmode(
    normalized.reverse_proxy_submode,
  );
  normalized.auto_manage_firewall = normalizeAutoManageFirewall(
    normalized.auto_manage_firewall,
  );
  if (!normalized.default_route) {
    normalized.default_route = DEFAULT_ROUTE_PLACEHOLDER;
  }
  if (!normalized.default_tunnel) {
    normalized.default_tunnel = "frp";
  }
  normalized.host_mappings = normalizeHostMappings(normalized.host_mappings);
  normalized.stream_mappings = normalizeStreamMappings(
    normalized.stream_mappings,
  );
  normalized.subdomain_mode = normalizeSubdomainModeConfig(
    normalized.subdomain_mode,
  );
  normalized.ssl = normalizeSSLConfig(normalized.ssl);
  normalized.fnos_share_bypass = normalizeFnosShareBypassConfig(
    normalized.fnos_share_bypass,
  );
  normalized.fnos_port_icon_hijack = normalizeFnosPortIconHijackConfig(
    normalized.fnos_port_icon_hijack,
  );
  normalized.fnos_network_tuning = normalizeFnosNetworkTuningConfig(
    normalized.fnos_network_tuning,
  );
  normalized.gateway_logging = normalizeGatewayLoggingSettings(
    normalized.gateway_logging,
  );
  normalized.waf = normalizeWAFConfig(normalized.waf);
  normalized.reverse_proxy_throttle = normalizeReverseProxyThrottleConfig(
    normalized.reverse_proxy_throttle,
  );
  normalized.gateway_visibility = normalizeGatewayVisibilityConfig(
    normalized.gateway_visibility,
  );
  normalized.gateway_proxy_headers = normalizeGatewayProxyHeadersConfig(
    normalized.gateway_proxy_headers,
  );
  normalized.gateway_host_response = normalizeGatewayHostResponseConfig(
    normalized.gateway_host_response,
  );
  normalized.gateway_crawler_blocker = normalizeGatewayCrawlerBlockerConfig(
    normalized.gateway_crawler_blocker,
  );
  normalized.gateway_portal = normalizeGatewayPortalConfig(
    normalized.gateway_portal,
  );
  normalized.appearance = normalizeAppearanceConfig(normalized.appearance);
  normalized.dashboard_display = normalizeDashboardDisplayConfig(
    normalized.dashboard_display,
  );
  normalized.auto_https = normalizeAutoHttpsConfig(normalized.auto_https);
  normalized.smart_connect = normalizeSmartConnectConfig(
    normalized.smart_connect,
  );
  normalized.scan_discovery = normalizeScanDiscoveryConfig(
    normalized.scan_discovery,
  );
  normalized.auth_credential_settings = normalizeAuthCredentialSettings(
    normalized.auth_credential_settings,
    {
      legacyAutoAddWhitelistOnLogin:
        normalized.subdomain_mode?.auto_add_whitelist_on_login,
    },
  );
  normalized.event_system = normalizeEventSystemConfig(normalized.event_system);
  normalized.terminal_feature = normalizeTerminalFeatureConfig(
    normalized.terminal_feature,
  );
  normalized.ssh_security = normalizeSSHSecurityConfig(normalized.ssh_security);
  normalized.locale = normalizeLocaleConfig(normalized.locale);

  return normalized;
};

export const createDefaultAppConfig = (): AppConfig =>
  normalizePersistedAppConfig(DEFAULT_CONFIG);
