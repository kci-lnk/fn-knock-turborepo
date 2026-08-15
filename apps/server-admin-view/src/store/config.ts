import { defineStore } from "pinia";
import { ref } from "vue";
import type {
  AppConfig,
  AppearanceConfig,
  DashboardDisplayConfig,
  HostMapping,
  HostMappingGroup,
  LocaleConfig,
  ProxyMapping,
  ReverseProxySubmode,
  RunType,
  StreamMapping,
  SubdomainModeConfig,
} from "../types";
import {
  ConfigAPI,
  STREAM_MAPPING_LEGACY_REPAIR_REQUIRED_CODE,
} from "@/lib/api/config";
import { SystemAPI } from "@/lib/api/system";
import { createSerialTaskQueue } from "../lib/serialTaskQueue";
import { applyAppearanceConfig } from "@admin-shared/composables/useAppearanceState";
import { applyDateTimeDisplayConfig } from "@admin-shared/composables/useDateTimeDisplayState";
import { hasPendingHostMappingMetadata } from "./hostMappingMetadata";
import { useConfigRuntimeCapabilities } from "./useConfigRuntimeCapabilities";

const isLegacyStreamMappingRepairConflict = (error: unknown): boolean =>
  (error as { response?: { status?: number; data?: { code?: number } } })
    ?.response?.status === 409 &&
  (error as { response?: { data?: { code?: number } } })?.response?.data
    ?.code === STREAM_MAPPING_LEGACY_REPAIR_REQUIRED_CODE;

export const useConfigStore = defineStore("config", () => {
  const config = ref<AppConfig | null>(null);
  const isLoading = ref(true);
  const isError = ref(false);
  let hostMappingsFollowUpRefreshTimer: number | null = null;
  let hostMappingsFollowUpRefreshAttempts = 0;
  let loadConfigPromise: Promise<AppConfig | null> | null = null;
  let loadConfigRequestId = 0;
  let hostMappingCatalogRevision: string | null = null;
  let hostMappingsSnapshot: HostMapping[] | null = null;
  let hostMappingGroupsSnapshot: HostMappingGroup[] | null = null;
  let hostMappingGroupedViewSnapshot: boolean | null = null;
  let hostMappingsSnapshotRequestId = 0;
  let hostMappingsSavePromise: Promise<HostMapping[]> | null = null;
  const runStreamMappingsSave = createSerialTaskQueue();

  const refreshHostMappingsOnly = async () => {
    if (hostMappingsSavePromise) {
      await hostMappingsSavePromise;
    }
    const requestId = ++hostMappingsSnapshotRequestId;
    const snapshot = await ConfigAPI.getHostMappings();
    if (requestId === hostMappingsSnapshotRequestId) {
      hostMappingsSnapshot = snapshot.mappings;
      if (config.value) {
        config.value = {
          ...config.value,
          host_mappings: snapshot.mappings,
        };
      } else {
        await loadConfig();
      }
    }
    return snapshot.mappings;
  };

  const refreshStreamMappingsOnly = async () => {
    const mappings = await ConfigAPI.getStreamMappings();
    if (config.value) {
      config.value = {
        ...config.value,
        stream_mappings: mappings,
      };
    } else {
      await loadConfig();
    }
    return mappings;
  };

  const clearHostMappingsFollowUpRefresh = () => {
    if (hostMappingsFollowUpRefreshTimer !== null) {
      window.clearTimeout(hostMappingsFollowUpRefreshTimer);
      hostMappingsFollowUpRefreshTimer = null;
    }
    hostMappingsFollowUpRefreshAttempts = 0;
  };

  const scheduleHostMappingsFollowUpRefresh = (
    mappings: HostMapping[],
    previousMappings: HostMapping[] | null = null,
  ) => {
    if (typeof window === "undefined") {
      return;
    }

    if (!hasPendingHostMappingMetadata(mappings, previousMappings)) {
      clearHostMappingsFollowUpRefresh();
      return;
    }

    clearHostMappingsFollowUpRefresh();
    hostMappingsFollowUpRefreshAttempts = 2;

    const runFollowUpRefresh = async () => {
      hostMappingsFollowUpRefreshTimer = null;

      try {
        const nextMappings = await refreshHostMappingsOnly();
        if (
          hostMappingsFollowUpRefreshAttempts > 0 &&
          hasPendingHostMappingMetadata(nextMappings, previousMappings)
        ) {
          hostMappingsFollowUpRefreshAttempts -= 1;
          hostMappingsFollowUpRefreshTimer = window.setTimeout(() => {
            void runFollowUpRefresh();
          }, 2500);
          return;
        }
      } catch (error) {
        console.error("Failed to refresh host mappings after save", error);
      }

      clearHostMappingsFollowUpRefresh();
    };

    hostMappingsFollowUpRefreshTimer = window.setTimeout(() => {
      void runFollowUpRefresh();
    }, 1800);
  };

  async function loadConfig(options: { force?: boolean } = {}) {
    if (!options.force && !config.value && loadConfigPromise) {
      return loadConfigPromise;
    }

    const requestId = loadConfigRequestId + 1;
    loadConfigRequestId = requestId;
    isLoading.value = true;
    isError.value = false;
    const request = (async () => {
      if (hostMappingsSavePromise) {
        await hostMappingsSavePromise;
      }
      const hostMappingsRequestId = ++hostMappingsSnapshotRequestId;
      return {
        snapshot: await ConfigAPI.getConfig(),
        hostMappingsRequestId,
      };
    })()
      .then(({ snapshot, hostMappingsRequestId }) => {
        let next: AppConfig = {
          ...snapshot.config,
          host_mapping_groups: snapshot.config.host_mapping_groups ?? [],
          host_mapping_grouped_view:
            snapshot.config.host_mapping_grouped_view === true,
        };
        if (requestId === loadConfigRequestId) {
          if (hostMappingsRequestId === hostMappingsSnapshotRequestId) {
            hostMappingCatalogRevision = snapshot.hostMappingCatalogRevision;
            hostMappingsSnapshot = next.host_mappings;
            hostMappingGroupsSnapshot = next.host_mapping_groups;
            hostMappingGroupedViewSnapshot = next.host_mapping_grouped_view;
          } else if (hostMappingsSnapshot) {
            next = {
              ...next,
              host_mappings: hostMappingsSnapshot,
              host_mapping_groups:
                hostMappingGroupsSnapshot ?? next.host_mapping_groups,
              host_mapping_grouped_view:
                hostMappingGroupedViewSnapshot ??
                next.host_mapping_grouped_view,
            };
          }
          config.value = next;
          applyAppearanceConfig(next.appearance);
          applyDateTimeDisplayConfig(next.dashboard_display);
        }
        return next;
      })
      .catch((e) => {
        console.error(e);
        if (requestId === loadConfigRequestId) {
          isError.value = true;
        }
        return null;
      })
      .finally(() => {
        if (requestId === loadConfigRequestId) {
          isLoading.value = false;
        }
        if (loadConfigPromise === request) {
          loadConfigPromise = null;
        }
      });

    loadConfigPromise = request;
    return loadConfigPromise;
  }

  async function saveDefaultRoute(path: string) {
    await ConfigAPI.updateDefaultRoute(path);
    if (config.value) config.value.default_route = path;
    await loadConfig({ force: true });
  }

  async function setRunType(
    type: RunType,
    reverseProxySubmode?: ReverseProxySubmode,
  ) {
    const warning = await ConfigAPI.updateRunType({
      run_type: type,
      reverse_proxy_submode: reverseProxySubmode,
    });
    if (config.value) {
      config.value.run_type = type;
      if (type === 1 && reverseProxySubmode) {
        config.value.reverse_proxy_submode = reverseProxySubmode;
      }
    }
    await loadConfig({ force: true }); // refresh to be safe
    return warning;
  }

  async function saveAutoManageFirewall(enabled: boolean) {
    const next = await ConfigAPI.updateAutoManageFirewall({
      auto_manage_firewall: enabled,
    });
    if (config.value) {
      config.value.auto_manage_firewall = next.auto_manage_firewall;
    } else {
      await loadConfig({ force: true });
    }
    return next;
  }

  async function saveProxyMappings(mappings: ProxyMapping[]) {
    await ConfigAPI.updateProxyMappings(mappings);
    await loadConfig({ force: true });
  }

  async function saveHostMappings(
    mappings: HostMapping[],
    refreshedFaviconHosts: ReadonlySet<string> = new Set(),
    refreshedTitleHosts: ReadonlySet<string> = new Set(),
    previousHosts: ReadonlyMap<string, string> = new Map(),
  ) {
    if (hostMappingsSavePromise) {
      await hostMappingsSavePromise;
    }
    const requestId = ++hostMappingsSnapshotRequestId;
    let reloadConfigAfterSave = false;
    const request = (async () => {
      const groups = config.value?.host_mapping_groups ?? [];
      const groupedView = config.value?.host_mapping_grouped_view === true;
      const snapshot = await ConfigAPI.updateHostMappingCatalog(
        mappings,
        groups,
        groupedView,
        hostMappingCatalogRevision,
        refreshedFaviconHosts,
        refreshedTitleHosts,
        previousHosts,
      );
      if (requestId === hostMappingsSnapshotRequestId) {
        hostMappingCatalogRevision = snapshot.revision;
        hostMappingsSnapshot = snapshot.mappings;
        hostMappingGroupsSnapshot = snapshot.groups;
        hostMappingGroupedViewSnapshot = snapshot.groupedView;
      }
      const nextMappings = snapshot.mappings;
      if (!config.value) {
        scheduleHostMappingsFollowUpRefresh(nextMappings);
        reloadConfigAfterSave = true;
        return nextMappings;
      }

      const previousMappings = config.value.host_mappings;
      config.value = {
        ...config.value,
        host_mappings: nextMappings,
        host_mapping_groups: snapshot.groups,
        host_mapping_grouped_view: snapshot.groupedView,
      };
      scheduleHostMappingsFollowUpRefresh(nextMappings, previousMappings);
      return nextMappings;
    })();
    hostMappingsSavePromise = request;
    let nextMappings: HostMapping[];
    try {
      nextMappings = await request;
    } finally {
      if (hostMappingsSavePromise === request) {
        hostMappingsSavePromise = null;
      }
    }
    if (reloadConfigAfterSave) {
      await loadConfig({ force: true });
    }
    return nextMappings;
  }

  async function saveHostMappingCatalog(
    mappings: HostMapping[],
    groups: HostMappingGroup[],
    groupedView = config.value?.host_mapping_grouped_view === true,
  ) {
    if (hostMappingsSavePromise) {
      await hostMappingsSavePromise;
    }
    const requestId = ++hostMappingsSnapshotRequestId;
    const request = (async () => {
      const snapshot = await ConfigAPI.updateHostMappingCatalog(
        mappings,
        groups,
        groupedView,
        hostMappingCatalogRevision,
      );
      if (requestId === hostMappingsSnapshotRequestId) {
        hostMappingCatalogRevision = snapshot.revision;
        hostMappingsSnapshot = snapshot.mappings;
        hostMappingGroupsSnapshot = snapshot.groups;
        hostMappingGroupedViewSnapshot = snapshot.groupedView;
      }
      if (config.value) {
        const previousMappings = config.value.host_mappings;
        config.value = {
          ...config.value,
          host_mappings: snapshot.mappings,
          host_mapping_groups: snapshot.groups,
          host_mapping_grouped_view: snapshot.groupedView,
        };
        scheduleHostMappingsFollowUpRefresh(
          snapshot.mappings,
          previousMappings,
        );
      } else {
        await loadConfig({ force: true });
      }
      return snapshot.mappings;
    })();
    hostMappingsSavePromise = request;
    try {
      return await request;
    } finally {
      if (hostMappingsSavePromise === request) {
        hostMappingsSavePromise = null;
      }
    }
  }

  async function refreshAllHostMappingTitles() {
    const result = await ConfigAPI.refreshAllHostMappingTitles();
    await loadConfig({ force: true });
    return result;
  }

  async function saveStreamMappings(
    update: (current: readonly StreamMapping[]) => StreamMapping[],
    options: { disableFeatureOnLegacyRepairConflict?: boolean } = {},
  ) {
    return runStreamMappingsSave(async () => {
      let current =
        config.value?.stream_mappings ?? (await ConfigAPI.getStreamMappings());
      let next = update(current);
      let protocolMappingDisabled = false;
      try {
        await ConfigAPI.updateStreamMappings(next);
      } catch (error) {
        if (
          !options.disableFeatureOnLegacyRepairConflict ||
          !isLegacyStreamMappingRepairConflict(error)
        ) {
          throw error;
        }
        await SystemAPI.updateProtocolMappingFeatureConfig({ enabled: false });
        protocolMappingDisabled = true;
        if (config.value) {
          config.value = {
            ...config.value,
            protocol_mapping_feature: {
              enabled: false,
              availability:
                config.value.protocol_mapping_feature?.availability ?? null,
            },
          };
        }
        const refreshed = await loadConfig({ force: true });
        current =
          refreshed?.stream_mappings ?? (await ConfigAPI.getStreamMappings());
        next = update(current);
        await ConfigAPI.updateStreamMappings(next);
      }
      if (config.value) {
        config.value = {
          ...config.value,
          stream_mappings: next,
          ...(protocolMappingDisabled
            ? {
                protocol_mapping_feature: {
                  enabled: false,
                  availability:
                    config.value.protocol_mapping_feature?.availability ?? null,
                },
              }
            : {}),
        };
      }
      await loadConfig({ force: true });
      return { protocolMappingDisabled };
    });
  }

  async function saveSubdomainMode(next: Partial<SubdomainModeConfig>) {
    const result = await ConfigAPI.updateSubdomainMode(next);
    await loadConfig({ force: true });
    return result;
  }

  async function saveLocaleConfig(next: LocaleConfig) {
    const result = await ConfigAPI.updateLocaleConfig(next);
    if (config.value) {
      config.value.locale = result;
    } else {
      await loadConfig({ force: true });
    }
    return result;
  }

  async function saveAppearanceConfig(next: AppearanceConfig) {
    const result = await ConfigAPI.updateAppearanceConfig(next);
    applyAppearanceConfig(result);
    if (config.value) {
      config.value.appearance = result;
    } else {
      await loadConfig({ force: true });
    }
    return result;
  }

  async function saveDashboardDisplayConfig(
    next: Partial<DashboardDisplayConfig>,
  ) {
    const result = await ConfigAPI.updateDashboardDisplayConfig(next);
    applyDateTimeDisplayConfig(result);
    if (config.value) {
      config.value.dashboard_display = result;
    } else {
      await loadConfig({ force: true });
    }
    return result;
  }

  const {
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
  } = useConfigRuntimeCapabilities(config);

  return {
    config,
    isLoading,
    isError,
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
    loadConfig,
    setRunType,
    saveAutoManageFirewall,
    saveProxyMappings,
    saveHostMappings,
    saveHostMappingCatalog,
    refreshAllHostMappingTitles,
    refreshStreamMappingsOnly,
    saveStreamMappings,
    saveSubdomainMode,
    saveLocaleConfig,
    saveAppearanceConfig,
    saveDashboardDisplayConfig,
    saveDefaultRoute,
  };
});
