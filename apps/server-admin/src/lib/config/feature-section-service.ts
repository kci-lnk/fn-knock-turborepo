import {
  type TerminalFeatureConfig,
  normalizeTerminalFeatureConfig,
} from "../terminal-shared";
import { normalizeSSHSecurityConfig } from "../ssh-security/config";
import type { SSHSecurityConfig } from "../ssh-security/types";
import type { AutoHttpsConfig } from "../auto-https-redirect";
import {
  normalizeAppearanceConfig,
  type AppearanceConfig,
} from "../../../../../packages/admin-shared/src/utils/appearance";
import {
  DEFAULT_CAPTCHA_SETTINGS,
  normalizeAuthCredentialSettings,
  normalizeAuthCredentialSettingsPatch,
  normalizeAutoHttpsConfig,
  normalizeCaptchaSettings,
  normalizeDashboardDisplayConfig,
  normalizeFnosNetworkTuningConfig,
  normalizeFnosPortIconHijackConfig,
  normalizeFnosShareBypassConfig,
  normalizeGatewayCrawlerBlockerConfig,
  normalizeGatewayHostResponseConfig,
  normalizeGatewayLoggingSettings,
  normalizeGatewayPortalConfig,
  normalizeGatewayProxyHeadersConfig,
  normalizeGatewayVisibilityConfig,
  normalizeIpLocationApiConfig,
  normalizeProtocolMappingFeatureConfig,
  normalizeReverseProxyThrottleConfig,
  normalizeSmartConnectConfig,
} from "./app-config";
import {
  normalizeScanDiscoveryConfig,
  normalizeWAFConfig,
} from "./normalizers";
import {
  DEFAULT_IP_LOCATION_API_CONFIG,
  DEFAULT_PROTOCOL_MAPPING_FEATURE_CONFIG,
} from "./defaults";
import type { ConfigRuntimeStateStore } from "./runtime-state-store";
import type { ConfigSectionStore } from "./section-store";
import type {
  AppConfig,
  AuthCredentialSettings,
  CaptchaSettings,
  DashboardDisplayConfig,
  FnosNetworkTuningConfig,
  FnosPortIconHijackConfig,
  FnosShareBypassConfig,
  GatewayCrawlerBlockerConfig,
  GatewayHostResponseConfig,
  GatewayHostResponseRuntimeState,
  GatewayLoggingSettings,
  GatewayPortalConfig,
  GatewayProxyHeadersConfig,
  GatewayProxyHeadersRuntimeState,
  GatewayVisibilityConfig,
  GatewayVisibilityRuntimeState,
  IpLocationApiConfig,
  ProtocolMappingFeatureConfig,
  ReverseProxyThrottleConfig,
  ReverseProxyTrustedIPRuntimeState,
  ScanDiscoveryConfig,
  SmartConnectConfig,
  SmartConnectRuntimeState,
  WAFConfig,
} from "./types";

type ConfigAccess = {
  getConfig: () => Promise<AppConfig>;
  saveConfig: (config: AppConfig) => Promise<void>;
};

const CAPTCHA_SETTINGS_KEY = "fn_knock:captcha:settings";
const IP_LOCATION_API_SETTINGS_KEY = "fn_knock:ip-location-api:settings";
const PROTOCOL_MAPPING_FEATURE_KEY = "fn_knock:protocol-mapping:feature";

export class ConfigFeatureSectionService {
  constructor(
    private readonly access: ConfigAccess,
    private readonly sections: ConfigSectionStore,
    private readonly runtimeStates: ConfigRuntimeStateStore,
  ) {}

  async getProtocolMappingFeatureConfig(): Promise<ProtocolMappingFeatureConfig> {
    return this.sections.readJson(
      PROTOCOL_MAPPING_FEATURE_KEY,
      normalizeProtocolMappingFeatureConfig,
      () => DEFAULT_PROTOCOL_MAPPING_FEATURE_CONFIG,
    );
  }

  async updateProtocolMappingFeatureConfig(
    patch: Partial<ProtocolMappingFeatureConfig>,
  ): Promise<ProtocolMappingFeatureConfig> {
    return this.sections.patchJson(
      PROTOCOL_MAPPING_FEATURE_KEY,
      patch as Record<string, unknown>,
      normalizeProtocolMappingFeatureConfig,
      () => DEFAULT_PROTOCOL_MAPPING_FEATURE_CONFIG,
    );
  }

  async getFnosShareBypassConfig(): Promise<FnosShareBypassConfig> {
    return this.sections.read(
      (config) => config.fnos_share_bypass,
      normalizeFnosShareBypassConfig,
    );
  }

  async getFnosPortIconHijackConfig(): Promise<FnosPortIconHijackConfig> {
    return this.sections.read(
      (config) => config.fnos_port_icon_hijack,
      normalizeFnosPortIconHijackConfig,
    );
  }

  async getFnosNetworkTuningConfig(): Promise<FnosNetworkTuningConfig> {
    return this.sections.read(
      (config) => config.fnos_network_tuning,
      normalizeFnosNetworkTuningConfig,
    );
  }

  async getGatewayLoggingConfig(): Promise<GatewayLoggingSettings> {
    return this.sections.read(
      (config) => config.gateway_logging,
      normalizeGatewayLoggingSettings,
    );
  }

  async getWAFConfig(): Promise<WAFConfig> {
    return this.sections.read((config) => config.waf, normalizeWAFConfig);
  }

  async getReverseProxyThrottleConfig(): Promise<ReverseProxyThrottleConfig> {
    return this.sections.read(
      (config) => config.reverse_proxy_throttle,
      normalizeReverseProxyThrottleConfig,
    );
  }

  async getGatewayVisibilityConfig(): Promise<GatewayVisibilityConfig> {
    return this.sections.read(
      (config) => config.gateway_visibility,
      normalizeGatewayVisibilityConfig,
    );
  }

  async getGatewayProxyHeadersConfig(): Promise<GatewayProxyHeadersConfig> {
    return this.sections.read(
      (config) => config.gateway_proxy_headers,
      normalizeGatewayProxyHeadersConfig,
    );
  }

  async getGatewayHostResponseConfig(): Promise<GatewayHostResponseConfig> {
    return this.sections.read(
      (config) => config.gateway_host_response,
      normalizeGatewayHostResponseConfig,
    );
  }

  async getGatewayCrawlerBlockerConfig(): Promise<GatewayCrawlerBlockerConfig> {
    return this.sections.read(
      (config) => config.gateway_crawler_blocker,
      normalizeGatewayCrawlerBlockerConfig,
    );
  }

  async getGatewayPortalConfig(): Promise<GatewayPortalConfig> {
    return this.sections.read(
      (config) => config.gateway_portal,
      normalizeGatewayPortalConfig,
    );
  }

  async getAppearanceConfig(): Promise<AppearanceConfig> {
    return this.sections.read(
      (config) => config.appearance,
      normalizeAppearanceConfig,
    );
  }

  async getDashboardDisplayConfig(): Promise<DashboardDisplayConfig> {
    return this.sections.read(
      (config) => config.dashboard_display,
      normalizeDashboardDisplayConfig,
    );
  }

  async getAutoHttpsConfig(): Promise<AutoHttpsConfig> {
    return this.sections.read(
      (config) => config.auto_https,
      normalizeAutoHttpsConfig,
    );
  }

  async getGatewayVisibilityRuntimeState(): Promise<GatewayVisibilityRuntimeState> {
    return this.runtimeStates.getGatewayVisibilityRuntimeState();
  }

  async getGatewayProxyHeadersRuntimeState(): Promise<GatewayProxyHeadersRuntimeState> {
    return this.runtimeStates.getGatewayProxyHeadersRuntimeState();
  }

  async getGatewayHostResponseRuntimeState(): Promise<GatewayHostResponseRuntimeState> {
    return this.runtimeStates.getGatewayHostResponseRuntimeState();
  }

  async getReverseProxyTrustedIPsRuntimeState(): Promise<ReverseProxyTrustedIPRuntimeState> {
    return this.runtimeStates.getReverseProxyTrustedIPsRuntimeState();
  }

  async getSmartConnectConfig(): Promise<SmartConnectConfig> {
    return this.sections.read(
      (config) => config.smart_connect,
      normalizeSmartConnectConfig,
    );
  }

  async updateSmartConnectConfig(
    patch: Partial<SmartConnectConfig>,
  ): Promise<SmartConnectConfig> {
    return this.sections.patch(
      (config) => config.smart_connect,
      (config, next) => {
        config.smart_connect = next;
      },
      patch as Record<string, unknown>,
      normalizeSmartConnectConfig,
    );
  }

  async getScanDiscoveryConfig(): Promise<ScanDiscoveryConfig> {
    return this.sections.read(
      (config) => config.scan_discovery,
      normalizeScanDiscoveryConfig,
    );
  }

  async updateScanDiscoveryConfig(
    patch: Partial<ScanDiscoveryConfig>,
  ): Promise<ScanDiscoveryConfig> {
    return this.sections.patch(
      (config) => config.scan_discovery,
      (config, next) => {
        config.scan_discovery = next;
      },
      patch as Record<string, unknown>,
      normalizeScanDiscoveryConfig,
    );
  }

  async getSmartConnectRuntimeState(): Promise<SmartConnectRuntimeState> {
    return this.runtimeStates.getSmartConnectRuntimeState();
  }

  async saveSmartConnectRuntimeState(
    nextValue: SmartConnectRuntimeState,
  ): Promise<SmartConnectRuntimeState> {
    return this.runtimeStates.saveSmartConnectRuntimeState(nextValue);
  }

  async updateFnosShareBypassConfig(
    patch: Partial<FnosShareBypassConfig>,
  ): Promise<FnosShareBypassConfig> {
    return this.sections.patch(
      (config) => config.fnos_share_bypass,
      (config, next) => {
        config.fnos_share_bypass = next;
      },
      patch as Record<string, unknown>,
      normalizeFnosShareBypassConfig,
    );
  }

  async updateFnosPortIconHijackConfig(
    patch: Partial<FnosPortIconHijackConfig>,
  ): Promise<FnosPortIconHijackConfig> {
    return this.sections.patch(
      (config) => config.fnos_port_icon_hijack,
      (config, next) => {
        config.fnos_port_icon_hijack = next;
      },
      {
        ...patch,
        updated_at: new Date().toISOString(),
      },
      normalizeFnosPortIconHijackConfig,
    );
  }

  async updateFnosNetworkTuningConfig(
    patch: Partial<FnosNetworkTuningConfig>,
  ): Promise<FnosNetworkTuningConfig> {
    return this.sections.patch(
      (config) => config.fnos_network_tuning,
      (config, next) => {
        config.fnos_network_tuning = next;
      },
      {
        ...patch,
        updated_at: new Date().toISOString(),
      },
      normalizeFnosNetworkTuningConfig,
    );
  }

  async updateGatewayLoggingConfig(
    patch: Partial<GatewayLoggingSettings>,
  ): Promise<GatewayLoggingSettings> {
    return this.sections.patch(
      (config) => config.gateway_logging,
      (config, next) => {
        config.gateway_logging = next;
      },
      patch as Record<string, unknown>,
      normalizeGatewayLoggingSettings,
    );
  }

  async updateWAFConfig(patch: Partial<WAFConfig>): Promise<WAFConfig> {
    return this.sections.patch(
      (config) => config.waf,
      (config, next) => {
        config.waf = next;
      },
      {
        ...patch,
        updated_at: new Date().toISOString(),
      },
      normalizeWAFConfig,
    );
  }

  async updateReverseProxyThrottleConfig(
    patch: Partial<ReverseProxyThrottleConfig>,
  ): Promise<ReverseProxyThrottleConfig> {
    return this.sections.patch(
      (config) => config.reverse_proxy_throttle,
      (config, next) => {
        config.reverse_proxy_throttle = next;
      },
      patch as Record<string, unknown>,
      normalizeReverseProxyThrottleConfig,
    );
  }

  async updateGatewayVisibilityConfig(
    nextValue: GatewayVisibilityConfig,
  ): Promise<GatewayVisibilityConfig> {
    return this.sections.replace(
      (config, next) => {
        config.gateway_visibility = next;
      },
      nextValue,
      normalizeGatewayVisibilityConfig,
    );
  }

  async updateGatewayProxyHeadersConfig(
    nextValue: GatewayProxyHeadersConfig,
  ): Promise<GatewayProxyHeadersConfig> {
    return this.sections.replace(
      (config, next) => {
        config.gateway_proxy_headers = next;
      },
      nextValue,
      normalizeGatewayProxyHeadersConfig,
    );
  }

  async updateGatewayHostResponseConfig(
    nextValue: GatewayHostResponseConfig,
  ): Promise<GatewayHostResponseConfig> {
    return this.sections.replace(
      (config, next) => {
        config.gateway_host_response = next;
      },
      nextValue,
      normalizeGatewayHostResponseConfig,
    );
  }

  async updateGatewayCrawlerBlockerConfig(
    patch: Partial<GatewayCrawlerBlockerConfig>,
  ): Promise<GatewayCrawlerBlockerConfig> {
    return this.sections.patch(
      (config) => config.gateway_crawler_blocker,
      (config, next) => {
        config.gateway_crawler_blocker = next;
      },
      {
        ...patch,
        updated_at: new Date().toISOString(),
      },
      normalizeGatewayCrawlerBlockerConfig,
    );
  }

  async updateGatewayPortalConfig(
    patch: Partial<GatewayPortalConfig>,
  ): Promise<GatewayPortalConfig> {
    return this.sections.patch(
      (config) => config.gateway_portal,
      (config, next) => {
        config.gateway_portal = next;
      },
      patch as Record<string, unknown>,
      normalizeGatewayPortalConfig,
    );
  }

  async updateAppearanceConfig(
    patch: Partial<AppearanceConfig>,
  ): Promise<AppearanceConfig> {
    return this.sections.patch(
      (config) => config.appearance,
      (config, next) => {
        config.appearance = next;
      },
      patch as Record<string, unknown>,
      normalizeAppearanceConfig,
    );
  }

  async updateDashboardDisplayConfig(
    patch: Partial<DashboardDisplayConfig>,
  ): Promise<DashboardDisplayConfig> {
    return this.sections.patch(
      (config) => config.dashboard_display,
      (config, next) => {
        config.dashboard_display = next;
      },
      patch as Record<string, unknown>,
      normalizeDashboardDisplayConfig,
    );
  }

  async updateAutoHttpsConfig(
    patch: Partial<AutoHttpsConfig>,
  ): Promise<AutoHttpsConfig> {
    return this.sections.patch(
      (config) => config.auto_https,
      (config, next) => {
        config.auto_https = next;
      },
      patch as Record<string, unknown>,
      normalizeAutoHttpsConfig,
    );
  }

  async saveGatewayVisibilityRuntimeState(
    nextValue: GatewayVisibilityRuntimeState,
  ): Promise<GatewayVisibilityRuntimeState> {
    return this.runtimeStates.saveGatewayVisibilityRuntimeState(nextValue);
  }

  async saveGatewayProxyHeadersRuntimeState(
    nextValue: GatewayProxyHeadersRuntimeState,
  ): Promise<GatewayProxyHeadersRuntimeState> {
    return this.runtimeStates.saveGatewayProxyHeadersRuntimeState(nextValue);
  }

  async saveGatewayHostResponseRuntimeState(
    nextValue: GatewayHostResponseRuntimeState,
  ): Promise<GatewayHostResponseRuntimeState> {
    return this.runtimeStates.saveGatewayHostResponseRuntimeState(nextValue);
  }

  async saveReverseProxyTrustedIPsRuntimeState(
    nextValue: ReverseProxyTrustedIPRuntimeState,
  ): Promise<ReverseProxyTrustedIPRuntimeState> {
    return this.runtimeStates.saveReverseProxyTrustedIPsRuntimeState(nextValue);
  }

  async getTerminalFeatureConfig(): Promise<TerminalFeatureConfig> {
    return this.sections.read(
      (config) => config.terminal_feature,
      normalizeTerminalFeatureConfig,
    );
  }

  async getSSHSecurityConfig(): Promise<SSHSecurityConfig> {
    return this.sections.read(
      (config) => config.ssh_security,
      normalizeSSHSecurityConfig,
    );
  }

  async getAuthCredentialSettings(): Promise<AuthCredentialSettings> {
    const config = await this.access.getConfig();
    return normalizeAuthCredentialSettings(config.auth_credential_settings, {
      legacyAutoAddWhitelistOnLogin:
        config.subdomain_mode?.auto_add_whitelist_on_login,
    });
  }

  async previewAuthCredentialSettingsUpdate(
    patch: Partial<AuthCredentialSettings>,
  ): Promise<AuthCredentialSettings> {
    const config = await this.access.getConfig();
    return normalizeAuthCredentialSettingsPatch(config, patch);
  }

  async updateAuthCredentialSettings(
    patch: Partial<AuthCredentialSettings>,
  ): Promise<AuthCredentialSettings> {
    const config = await this.access.getConfig();
    const next = normalizeAuthCredentialSettingsPatch(config, patch);
    config.auth_credential_settings = next;
    await this.access.saveConfig(config);
    return next;
  }

  async updateTerminalFeatureConfig(
    patch: Partial<TerminalFeatureConfig>,
  ): Promise<TerminalFeatureConfig> {
    return this.sections.patch(
      (config) => config.terminal_feature,
      (config, next) => {
        config.terminal_feature = next;
      },
      patch as Record<string, unknown>,
      normalizeTerminalFeatureConfig,
    );
  }

  async updateSSHSecurityConfig(
    nextValue: SSHSecurityConfig,
  ): Promise<SSHSecurityConfig> {
    return this.sections.replace(
      (config, next) => {
        config.ssh_security = next;
      },
      nextValue,
      normalizeSSHSecurityConfig,
    );
  }

  async getCaptchaSettings(): Promise<CaptchaSettings> {
    return this.sections.readJson(
      CAPTCHA_SETTINGS_KEY,
      normalizeCaptchaSettings,
      () => DEFAULT_CAPTCHA_SETTINGS,
    );
  }

  async updateCaptchaSettings(
    patch: Partial<CaptchaSettings>,
  ): Promise<CaptchaSettings> {
    const current = await this.getCaptchaSettings();
    const next = normalizeCaptchaSettings({
      ...current,
      ...patch,
      turnstile: {
        ...current.turnstile,
        ...(patch.turnstile ?? {}),
      },
    });
    return this.sections.saveJson(
      CAPTCHA_SETTINGS_KEY,
      next,
      normalizeCaptchaSettings,
    );
  }

  async getIpLocationApiSettings(): Promise<IpLocationApiConfig> {
    return this.sections.readJson(
      IP_LOCATION_API_SETTINGS_KEY,
      normalizeIpLocationApiConfig,
      () => DEFAULT_IP_LOCATION_API_CONFIG,
    );
  }

  async updateIpLocationApiSettings(
    patch: Partial<IpLocationApiConfig>,
  ): Promise<IpLocationApiConfig> {
    return this.sections.patchJson(
      IP_LOCATION_API_SETTINGS_KEY,
      patch as Record<string, unknown>,
      normalizeIpLocationApiConfig,
      () => DEFAULT_IP_LOCATION_API_CONFIG,
    );
  }
}
