import {
  normalizeTerminalFeatureConfig,
} from "../terminal-shared";
import { normalizeSSHSecurityConfig } from "../ssh-security/config";
import { normalizeAutoManageFirewall } from "../firewall-automation";
import { getRuntimeCapabilities, getRuntimeProfile } from "../runtime-profile";
import {
  DEFAULT_RUN_TYPE,
  normalizeAutoHttpsConfig,
  normalizeSmartConnectConfig,
} from "./app-config";
import type { AppConfig } from "./types";

export const applyRuntimeConfigConstraints = async ({
  getConfig,
  saveConfig,
}: {
  getConfig: () => Promise<AppConfig>;
  saveConfig: (config: AppConfig) => Promise<void>;
}): Promise<{
  updated: boolean;
  config: AppConfig;
  corrected: string[];
}> => {
  const config = await getConfig();
  const runtimeProfile = getRuntimeProfile();
  const capabilities = getRuntimeCapabilities(runtimeProfile);
  const corrected: string[] = [];

  if (!capabilities.direct_mode_available && config.run_type === 0) {
    config.run_type = DEFAULT_RUN_TYPE;
    corrected.push(`run_type=0 -> run_type=${DEFAULT_RUN_TYPE}`);
  }

  config.smart_connect = normalizeSmartConnectConfig(config.smart_connect);
  if (
    !capabilities.smart_connect_available &&
    config.smart_connect.enabled === true
  ) {
    config.smart_connect.enabled = false;
    corrected.push("smart_connect.enabled -> false");
  }

  config.terminal_feature = normalizeTerminalFeatureConfig(
    config.terminal_feature,
  );
  if (
    !capabilities.terminal_available &&
    config.terminal_feature.enabled === true
  ) {
    config.terminal_feature.enabled = false;
    corrected.push("terminal_feature.enabled -> false");
  }

  config.auto_https = normalizeAutoHttpsConfig(config.auto_https);
  if (
    (runtimeProfile.is_docker ||
      runtimeProfile.deployment_target === "openwrt") &&
    config.auto_https.enabled === true
  ) {
    config.auto_https.enabled = false;
    corrected.push("auto_https.enabled -> false");
  }

  config.ssh_security = normalizeSSHSecurityConfig(config.ssh_security);
  if (
    (!capabilities.host_firewall_available ||
      runtimeProfile.deployment_target === "openwrt") &&
    config.ssh_security.enabled === true
  ) {
    config.ssh_security.enabled = false;
    corrected.push("ssh_security.enabled -> false");
  }

  const normalizedAutoManageFirewall = normalizeAutoManageFirewall(
    config.auto_manage_firewall,
  );
  if (!capabilities.host_firewall_available) {
    if (normalizedAutoManageFirewall !== false) {
      corrected.push("auto_manage_firewall -> false");
    }
    config.auto_manage_firewall = false;
  } else {
    config.auto_manage_firewall = normalizedAutoManageFirewall;
  }

  if (corrected.length > 0) {
    await saveConfig(config);
  }

  return {
    updated: corrected.length > 0,
    config,
    corrected,
  };
};
