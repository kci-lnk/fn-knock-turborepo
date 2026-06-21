import {
  normalizeTerminalFeatureConfig,
} from "../terminal-shared";
import {
  normalizeLocaleConfig,
} from "../../../../../packages/i18n/src";
import { normalizeAppearanceConfig } from "../../../../../packages/admin-shared/src/utils/appearance";
import { getRuntimeCapabilities, getRuntimeProfile } from "../runtime-profile";
import type { AppConfig, ProtocolMappingFeatureConfig } from "./types";

export const buildSafeAppConfig = (
  config: AppConfig,
  protocolMappingFeature: ProtocolMappingFeatureConfig,
) => {
  const runtimeProfile = getRuntimeProfile();
  const runtimeCapabilities = getRuntimeCapabilities(runtimeProfile);
  const { ssl, ...rest } = config;

  return {
    ...rest,
    runtime_profile: runtimeProfile,
    capabilities: runtimeCapabilities,
    protocol_mapping_feature: protocolMappingFeature,
    ssl: {
      enabled: !!(ssl.cert && ssl.key),
      active_cert_id: ssl.active_cert_id || undefined,
      deployment_mode: ssl.deployment_mode || "single_active",
      certificate_count: ssl.certificates?.length || 0,
    },
    terminal_feature: normalizeTerminalFeatureConfig(config.terminal_feature),
    locale: normalizeLocaleConfig(config.locale),
    appearance: normalizeAppearanceConfig(config.appearance),
  };
};
