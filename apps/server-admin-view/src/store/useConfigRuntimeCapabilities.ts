import { computed, type Ref } from "vue";
import {
  getEffectiveRuntimeCapabilities,
  getEffectiveRuntimeProfile,
} from "@runtime-debug";
import type { AppConfig } from "../types";
import { isProtectedAdminPanelDeploymentTarget } from "../lib/admin-panel-runtime";
import { canUseFnosConnectWafForRuntime } from "../lib/fnos-connect-waf";

export const useConfigRuntimeCapabilities = (
  config: Ref<AppConfig | null>,
) => {
  const runtimeProfile = computed(() =>
    getEffectiveRuntimeProfile(config.value?.runtime_profile),
  );
  const capabilities = computed(() =>
    getEffectiveRuntimeCapabilities(config.value?.capabilities),
  );
  const isDockerDeployment = computed(
    () => runtimeProfile.value?.is_docker === true,
  );
  const isFpkDeployment = computed(
    () => runtimeProfile.value?.deployment_target === "fpk",
  );
  const isFpkLiteDeployment = computed(
    () => runtimeProfile.value?.deployment_target === "fpk-lite",
  );
  const isOpenWrtDeployment = computed(
    () => runtimeProfile.value?.deployment_target === "openwrt",
  );
  const isLinuxDeployment = computed(
    () => runtimeProfile.value?.deployment_target === "linux",
  );
  const isSynologyDeployment = computed(
    () => runtimeProfile.value?.deployment_target === "synology",
  );
  const isWindowsDeployment = computed(
    () => runtimeProfile.value?.deployment_target === "windows",
  );
  const isProtectedAdminPanelDeployment = computed(() =>
    isProtectedAdminPanelDeploymentTarget(
      runtimeProfile.value?.deployment_target,
    ),
  );
  const canUseDirectMode = computed(
    () => capabilities.value?.direct_mode_available === true,
  );
  const canManageHostFirewall = computed(
    () => capabilities.value?.host_firewall_available === true,
  );
  const canUseSmartConnect = computed(
    () => capabilities.value?.smart_connect_available === true,
  );
  const canUseFnosCertificateSync = computed(
    () => capabilities.value?.fnos_certificate_sync_available === true,
  );
  const canSelfUpdate = computed(
    () => capabilities.value?.self_update_available === true,
  );
  const canSyncSystemClock = computed(
    () => capabilities.value?.system_clock_sync_available === true,
  );
  const canUseTerminal = computed(
    () => capabilities.value?.terminal_available === true,
  );
  const canUseDeepMonitor = computed(
    () => capabilities.value?.deep_monitor_available === true,
  );
  const canUseAutoHttps = computed(
    () => capabilities.value?.auto_https_available !== false,
  );
  const canUseFnosNetworkTuning = computed(
    () => capabilities.value?.fnos_network_tuning_available !== false,
  );
  const canUseFnosConnectWaf = computed(() =>
    canUseFnosConnectWafForRuntime(runtimeProfile.value, capabilities.value),
  );
  const hasSharedRoot = computed(
    () => capabilities.value?.shared_root_available === true,
  );
  const canUseAcme = computed(
    () => capabilities.value?.acme_available !== false,
  );
  const isAcmeResourceRequired = computed(
    () => capabilities.value?.acme_resource_required === true,
  );
  const canUseCloudflared = computed(
    () => capabilities.value?.cloudflared_available !== false,
  );
  const canUseFrpc = computed(
    () => capabilities.value?.frpc_available !== false,
  );
  const canUseSshSecurity = computed(
    () =>
      capabilities.value?.ssh_security_available ??
      capabilities.value?.host_firewall_available ??
      false,
  );
  const canUseSystemResourceMonitor = computed(
    () => capabilities.value?.system_resource_monitor_available !== false,
  );
  const isDesktopUpdateManaged = computed(
    () => capabilities.value?.desktop_update_managed === true,
  );

  return {
    runtimeProfile,
    capabilities,
    isDockerDeployment,
    isFpkDeployment,
    isFpkLiteDeployment,
    isOpenWrtDeployment,
    isLinuxDeployment,
    isSynologyDeployment,
    isWindowsDeployment,
    isProtectedAdminPanelDeployment,
    canUseDirectMode,
    canManageHostFirewall,
    canUseSmartConnect,
    canUseFnosCertificateSync,
    canSelfUpdate,
    canSyncSystemClock,
    canUseTerminal,
    canUseDeepMonitor,
    canUseAutoHttps,
    canUseFnosNetworkTuning,
    canUseFnosConnectWaf,
    hasSharedRoot,
    canUseAcme,
    isAcmeResourceRequired,
    canUseCloudflared,
    canUseFrpc,
    canUseSshSecurity,
    canUseSystemResourceMonitor,
    isDesktopUpdateManaged,
  };
};
