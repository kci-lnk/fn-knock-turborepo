import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "@admin-shared/utils/toast";
import { useConfigStore } from "../../store/config";
import { isAnySubdomainRoutingMode } from "../../lib/reverse-proxy-submode";
import { ConfigAPI } from "@/lib/api/config";
import { DashboardAPI } from "@/lib/api/dashboard";
import { isDefaultDomainAvailableForBehavior } from "../../lib/gatewayUnmatchedRoute";
import type { HostMapping } from "../../types";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import {
  createDefaultModeForm,
  getMappingDisplayTitle,
  isHttpTargetUrl,
  normalizeHostLike,
  parseTargetPort,
} from "./model";
import { useAccessEntryPort } from "@/composables/useAccessEntryPort";
import { useSubdomainAvailabilityActions } from "./useSubdomainAvailabilityActions";
import { useSubdomainBatchActions } from "./useSubdomainBatchActions";
import { useSubdomainAvailabilityStatus } from "./useSubdomainAvailabilityStatus";
import { useDelayedHostPopover } from "./useDelayedHostPopover";
import { useMappingFaviconState } from "./useMappingFaviconState";
import { useMediaQueryMatch } from "@admin-shared/composables/useMediaQueryMatch";
import { useTrafficRealtime } from "./useTrafficRealtime";
import { useSubdomainTouchTooltips } from "./useSubdomainTouchTooltips";
import { useSubdomainPortDisplay } from "./useSubdomainPortDisplay";
import { useSubdomainDeleteDialog } from "./useSubdomainDeleteDialog";
import { useSubdomainMappingsView } from "./useSubdomainMappingsView";
import { useSubdomainDiscoverFlow } from "./useSubdomainDiscoverFlow";
import { useSubdomainMappingDialogController } from "./useSubdomainMappingDialogController";
import { useSubdomainMappingListActions } from "./useSubdomainMappingListActions";
import { useSubdomainModeConfig } from "./useSubdomainModeConfig";
import { useSubdomainDestructiveActions } from "./useSubdomainDestructiveActions";
import { useGatewayVisibilityStatus } from "./useGatewayVisibilityStatus";
import { useSubdomainMappingGroups } from "./useSubdomainMappingGroups";
import { useActiveDeepMonitors } from "./useActiveDeepMonitors";
import { useSubdomainNavigation } from "./useSubdomainNavigation";
import { useSubdomainProxyLifecycle } from "./useSubdomainProxyLifecycle";

export const useSubdomainProxyPage = () => {
  const configStore = useConfigStore();
  const { t } = useI18n();
  const staleCleanupDialogRef = ref<{ open: () => void } | null>(null);
  const setStaleCleanupDialogRef = (instance: unknown) => {
    staleCleanupDialogRef.value = instance as { open: () => void } | null;
  };
  const isScanIntensityDialogOpen = ref(false);
  const searchQuery = ref("");
  const draggableVisibleMappings = ref<HostMapping[]>([]);
  const activeDeepMonitorHosts = useActiveDeepMonitors(
    () => configStore.canUseDeepMonitor,
  );
  const { openAdvancedAuth, openDeepMonitor, navigateToGatewayLocations } =
    useSubdomainNavigation();
  const {
    canManageNewMappings,
    canUseRootDomainSuffix,
    currentModeConfig,
    edgeClientIpProviderOptions,
    getEdgeClientIpProviderLabel,
    isModeDirty,
    isModeValid,
    isRootDomainPendingSave,
    isSavingMode,
    modeForm,
    resetModeForm,
    rootDomainValidationMessage,
    saveMode,
    savedRootDomain,
  } = useSubdomainModeConfig({
    getConfig: () => configStore.config,
    saveSubdomainMode: (next) => configStore.saveSubdomainMode(next),
    translate: (key, params) => (params ? t(key, params) : t(key)),
  });
  const {
    startTrafficRealtimePolling,
    stopTrafficRealtimePolling,
    trafficRealtimeStats,
  } = useTrafficRealtime({
    load: () => DashboardAPI.getRealtime(),
    onError: (error) => {
      console.warn("load host traffic realtime failed:", error);
    },
  });
  const isTouchInteraction = useMediaQueryMatch(
    "(hover: none), (pointer: coarse)",
  );
  const { accessEntryPort, loadAccessEntryPort } = useAccessEntryPort();
  const {
    clearCloseTimer: clearProtocolHeadersWarningCloseTimer,
    handleOpenChange: handleProtocolHeadersWarningOpenChange,
    isOpen: isProtocolHeadersWarningOpen,
    open: openProtocolHeadersWarning,
    scheduleClose: scheduleCloseProtocolHeadersWarning,
    toggle: toggleProtocolHeadersWarning,
  } = useDelayedHostPopover();
  const { isFaviconBroken, markFaviconBroken, resetFaviconErrors } =
    useMappingFaviconState();
  const {
    activeEdgeClientIpProvider,
    authServicePublicPort,
    formatAuthServiceHostWithPublicPort,
    formatHostWithAccessEntryPort,
    isEdgeClientIPModeEditable,
    omitPublicPortConfiguration,
    savedEdgeClientIpProvider,
    selectEdgeClientIpProvider,
  } = useSubdomainPortDisplay({
    accessEntryPort,
    currentModeConfig,
    getConfig: () => configStore.config,
    modeForm,
  });
  const authServicePort = computed(
    () =>
      parseTargetPort(currentModeConfig.value.auth_target) ??
      parseTargetPort(createDefaultModeForm().auth_target) ??
      7997,
  );
  const isAuthServiceTarget = (target: string): boolean =>
    isHttpTargetUrl(target) &&
    parseTargetPort(target) === authServicePort.value;
  const savedEdgeClientIpProviderLabel = computed(() =>
    savedEdgeClientIpProvider.value
      ? t("admin.subdomainProxy.edgeRealIpSummary", {
          provider: getEdgeClientIpProviderLabel(
            savedEdgeClientIpProvider.value,
          ),
        })
      : "",
  );
  const allMappings = computed(() => configStore.config?.host_mappings ?? []);
  const hostMappingGroups = computed(
    () => configStore.config?.host_mapping_groups ?? [],
  );
  const hostMappingGroupedView = computed(
    () => configStore.config?.host_mapping_grouped_view === true,
  );
  const {
    authServiceMapping,
    discoverButtonDividerClass,
    discoverButtonVariant,
    existingMappingTargets,
    filteredMappings,
    getHostTrafficSample,
    hasRegularHostMappings,
    syncDraggableVisibleMappings,
    visibleMappings,
  } = useSubdomainMappingsView({
    allMappings,
    draggableVisibleMappings,
    formatHostWithAccessEntryPort,
    groups: hostMappingGroups,
    isAuthServiceTarget,
    searchQuery,
    trafficRealtimeStats,
  });
  const isGatewayPortalEnabled = computed(
    () => configStore.config?.gateway_portal?.enabled !== false,
  );
  const isDefaultDomainAvailable = computed(() =>
    isDefaultDomainAvailableForBehavior(
      configStore.config?.gateway_unmatched_route?.behavior,
    ),
  );
  const globalWafEnabled = computed(
    () => configStore.config?.waf?.enabled === true,
  );
  const { globalVisibilityEnabled, loadGlobalVisibilityStatus } =
    useGatewayVisibilityStatus();
  const shouldShowPortalDisabledTooltip = computed(
    () => !isGatewayPortalEnabled.value,
  );
  const {
    handleMappingStatusTooltipOpenChange,
    handleMappingStatusTooltipTriggerClick,
    handlePortalDisabledTooltipOpenChange,
    handlePortalDisabledTooltipTriggerClick,
    isMappingStatusTooltipOpen,
    isPortalDisabledTooltipOpen,
  } = useSubdomainTouchTooltips({
    isTouchInteraction,
    shouldShowPortalDisabledTooltip,
  });
  const isSubdomainModeConfigured = computed(() => {
    const config = currentModeConfig.value;
    return Boolean(
      savedRootDomain.value ||
      normalizeHostLike(config.auth_host) ||
      authServiceMapping.value,
    );
  });
  const { isPending: isSavingMappings, run: runSaveMappings } = useAsyncAction({
    onError: (error) => {
      if (
        (error as { response?: { status?: number } })?.response?.status === 409
      ) {
        void configStore.loadConfig({ force: true });
      }
      toast.error(t("admin.subdomainProxy.saveFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.subdomainProxy.saveMappingFailed"),
        ),
      });
    },
  });

  const {
    moveMappingsToGroup,
    saveGroupedMappingOrder,
    saveMappingGroups,
    updateHostMappingGroupedView,
  } = useSubdomainMappingGroups({
    allMappings,
    groupedView: hostMappingGroupedView,
    groups: hostMappingGroups,
    isAuthServiceTarget,
    runSaveMappings,
    saveCatalog: (mappings, groups, groupedView) =>
      configStore.saveHostMappingCatalog(mappings, groups, groupedView),
    translate: (key) => t(key),
  });
  const {
    advanceClearAllConfirmation,
    closeDeleteDialog,
    deleteDialogConfirmLabel,
    deleteDialogDescription,
    deleteDialogState,
    deleteDialogTitle,
    handleDeleteDialogOpenChange,
    isDeleteDialogOpen,
    openClearAllConfigDialogState,
    openDeleteMappingDialog,
  } = useSubdomainDeleteDialog({
    mappingsCount: computed(() => allMappings.value.length),
    translate: (key, params) => (params ? t(key, params) : t(key)),
  });
  const {
    addAuthService,
    confirmDelete,
    isClearingAllSubdomainConfig,
    openClearAllConfigDialog,
    removeAuthService,
  } = useSubdomainDestructiveActions({
    advanceClearAllConfirmation,
    allMappings,
    authServiceMapping,
    canManageNewMappings,
    closeDeleteDialog,
    currentModeConfig,
    deleteDialogState,
    isAuthServiceTarget,
    modeForm,
    openClearAllConfigDialogState,
    runSaveMappings,
    rootDomainValidationMessage,
    savedRootDomain,
    saveHostMappings: (mappings) => configStore.saveHostMappings(mappings),
    translate: (key, params) => (params ? t(key, params) : t(key)),
  });
  const isGatewayAdvancedAvailableByMode = computed(() =>
    isAnySubdomainRoutingMode(configStore.config),
  );
  const getMappingTitleForDisplay = (mapping: HostMapping): string =>
    getMappingDisplayTitle(mapping) || t("admin.subdomainProxy.notFetched");

  const {
    formatAvailabilityWindow,
    getAvailabilityState,
    isMappingUnavailable,
    startAvailabilityClock,
    stopAvailabilityClock,
  } = useSubdomainAvailabilityStatus();

  const {
    availabilityDialogHostLabel,
    availabilityFormEnabled,
    availabilityFormEndTime,
    availabilityFormStartTime,
    availabilityValidationMessage,
    closeAvailabilityDialog,
    closeToggleDialog,
    confirmToggleMapping,
    handleAvailabilityDialogOpenChange,
    handleToggleDialogOpenChange,
    isAvailabilityDialogOpen,
    isToggleDialogOpen,
    openAvailabilityDialog,
    openToggleMappingDialog,
    saveAvailabilityDialog,
    toggleDialogConfirmLabel,
    toggleDialogConfirmVariant,
    toggleDialogDescription,
    toggleDialogTitle,
  } = useSubdomainAvailabilityActions({
    allMappings,
    formatHostWithAccessEntryPort,
    isAuthServiceTarget,
    isSavingMappings,
    runSaveMappings,
    saveHostMappings: (mappings) => configStore.saveHostMappings(mappings),
    translate: (key, params) => (params ? t(key, params) : t(key)),
  });
  const {
    availabilityFormEnabled: batchAvailabilityFormEnabled,
    availabilityFormEndTime: batchAvailabilityFormEndTime,
    availabilityFormStartTime: batchAvailabilityFormStartTime,
    availabilityValidationMessage: batchAvailabilityValidationMessage,
    batchAvailabilityOpen,
    batchMutationConfirmLabel,
    batchMutationConfirmVariant,
    batchMutationDescription,
    batchMutationTitle,
    closeBatchAvailability,
    closeBatchMutation,
    confirmBatchMutation,
    isBatchMutationOpen,
    openBatchAvailability,
    openBatchMutation,
    saveBatchAvailability,
    selectedCount: batchSelectedCount,
  } = useSubdomainBatchActions({
    allMappings,
    isAuthServiceTarget,
    isSavingMappings,
    runSaveMappings,
    saveHostMappings: (mappings) => configStore.saveHostMappings(mappings),
    translate: (key, params) => (params ? t(key, params) : t(key)),
  });

  const {
    basicAuthInjectionModel,
    basicAuthValidationMessage,
    canRefreshMappingMetadata,
    canShowBasicAuthInjection,
    clearMappingDialogKeyboardScrollTimer,
    closeDialog,
    composedPreviewHost,
    fullHostInputHint,
    gatewayHostResponseBlockedReason,
    gatewayProxyHeadersBlockedReason,
    handleDialogOpenChange,
    handleMappingDialogFocusIn,
    handleMappingDialogViewportResize,
    handleMappingInputModeChange,
    iconEditor,
    isGatewayAdvancedLoading,
    isDialogOpen,
    isMappingAuthService,
    isMappingValid,
    isMappingWebSocketTarget,
    isRefreshingMappingMetadata,
    mappingDialogContentStyle,
    mappingDialogScrollStyle,
    mappingForm,
    mappingInputLabel,
    mappingInputMode,
    mappingModeDescription,
    mappingResolvedTitle,
    mappingSubdomain,
    mappingUseAuth,
    openCreateDialog,
    openEditDialog,
    preserveHostModel,
    refreshMappingMetadata,
    saveMapping,
    sendProxyHeadersModel,
    setBasicAuthInjection,
    setMappingDialogScrollElement,
    setMappingSubdomain,
    setMappingUseAuth,
    setPreserveHost,
    setSendProxyHeaders,
    setShowToolbar,
    shouldShowProtocolHeadersWarning,
    showToolbar,
    updateMappingBasicAuth,
    updateMappingForm,
    visibilityEditor,
  } = useSubdomainMappingDialogController({
    allMappings,
    canUseRootDomainSuffix,
    getConfig: () => configStore.config,
    isAuthServiceTarget,
    isGatewayAdvancedAvailableByMode,
    resetFaviconErrors,
    runSaveMappings,
    savedRootDomain,
    saveHostMappings: configStore.saveHostMappings,
    setGatewayHostResponseDisabledHosts: (disabledHosts) => {
      if (!configStore.config) return;
      configStore.config = {
        ...configStore.config,
        gateway_host_response: {
          disabled_hosts: [...disabledHosts],
        },
      };
    },
    setGatewayProxyHeadersDisabledHosts: (disabledHosts) => {
      if (!configStore.config) return;
      configStore.config = {
        ...configStore.config,
        gateway_proxy_headers: {
          disabled_hosts: [...disabledHosts],
        },
      };
    },
    translate: (key, params) => (params ? t(key, params) : t(key)),
    visibleMappings,
  });

  const {
    discoveredData,
    discoverGroupId,
    discoverProgress,
    dismissDiscoverDialog,
    handleDiscoverDialogOpenChange,
    isAllSelected,
    isDiscoverDialogOpen,
    isDiscoverSelectionValid,
    isDiscoverSettingsOpen,
    isDiscovering,
    openDiscoverDialog,
    saveDiscoveredServices,
    selectedServices,
    setDiscoverDialogRef,
    setAllSelected,
    showDiscoverHostColumn,
    stopDiscoverScan,
    toggleDiscoverSettings,
    triggerScan,
  } = useSubdomainDiscoverFlow({
    allMappings,
    canManageNewMappings,
    existingMappingTargets,
    runSaveMappings,
    rootDomainValidationMessage,
    savedRootDomain,
    saveHostMappings: (mappings) => configStore.saveHostMappings(mappings),
    translate: (key, params) => (params ? t(key, params) : t(key)),
  });

  const {
    clearDefaultMapping,
    copyMappingHost,
    exportBookmarks,
    isExportingBookmarks,
    isRefreshingTitles,
    isSyncing,
    openGatewayLocations,
    refreshAllTitles,
    saveMappingOrder,
    setDefaultMapping,
    syncRoutes,
  } = useSubdomainMappingListActions({
    allMappings,
    downloadBookmarks: () => ConfigAPI.downloadHostMappingBookmarks(),
    draggableVisibleMappings,
    filteredMappings,
    formatHostWithAccessEntryPort,
    isAuthServiceTarget,
    isDefaultDomainAvailable,
    isSavingMappings,
    navigateToGatewayLocations,
    refreshAllHostMappingTitles: () =>
      configStore.refreshAllHostMappingTitles(),
    resetFaviconErrors,
    runSaveMappings,
    saveHostMappings: (mappings) => configStore.saveHostMappings(mappings),
    savedRootDomain,
    syncDraggableVisibleMappings,
    syncRoutesApi: () => ConfigAPI.syncRoutes(),
    translate: (key, params) => (params ? t(key, params) : t(key)),
    visibleMappings,
  });

  useSubdomainProxyLifecycle({
    clearMappingDialogKeyboardScrollTimer,
    clearProtocolHeadersWarningCloseTimer,
    filteredMappings,
    handleMappingDialogViewportResize,
    isConfigLoaded: () => Boolean(configStore.config),
    loadAccessEntryPort,
    loadConfig: () => configStore.loadConfig(),
    loadGlobalVisibilityStatus,
    startAvailabilityClock,
    startTrafficRealtimePolling,
    stopAvailabilityClock,
    stopDiscoverScan,
    stopTrafficRealtimePolling,
    syncDraggableVisibleMappings,
  });

  function openStaleCleanupDialog() {
    void staleCleanupDialogRef.value?.open();
  }

  return {
    overview: {
      activeDeepMonitorHosts,
      activeEdgeClientIpProvider,
      addAuthService,
      allMappings,
      openBatchAvailability,
      openBatchMutation,
      authServiceMapping,
      authServicePublicPort,
      canManageNewMappings,
      clearDefaultMapping,
      configStore,
      copyMappingHost,
      discoverButtonDividerClass,
      discoverButtonVariant,
      draggableVisibleMappings,
      edgeClientIpProviderOptions,
      exportBookmarks,
      filteredMappings,
      formatAuthServiceHostWithPublicPort,
      formatAvailabilityWindow,
      formatHostWithAccessEntryPort,
      getAvailabilityState,
      getHostTrafficSample,
      getMappingTitleForDisplay,
      globalVisibilityEnabled,
      globalWafEnabled,
      handleMappingStatusTooltipOpenChange,
      handleMappingStatusTooltipTriggerClick,
      handleProtocolHeadersWarningOpenChange,
      hasRegularHostMappings,
      hostMappingGroupedView,
      hostMappingGroups,
      isAuthServiceTarget,
      isClearingAllSubdomainConfig,
      isDefaultDomainAvailable,
      isDiscovering,
      isEdgeClientIPModeEditable,
      isExportingBookmarks,
      isFaviconBroken,
      isGatewayPortalEnabled,
      isMappingStatusTooltipOpen,
      isMappingUnavailable,
      isModeDirty,
      isModeValid,
      isProtocolHeadersWarningOpen,
      isRefreshingTitles,
      isRootDomainPendingSave,
      isSavingMappings,
      isSavingMode,
      isScanIntensityDialogOpen,
      isSubdomainModeConfigured,
      isSyncing,
      markFaviconBroken,
      modeForm,
      moveMappingsToGroup,
      omitPublicPortConfiguration,
      openAdvancedAuth,
      openAvailabilityDialog,
      openClearAllConfigDialog,
      openCreateDialog,
      openDeepMonitor,
      openDeleteMappingDialog,
      openDiscoverDialog,
      openEditDialog,
      openGatewayLocations,
      openProtocolHeadersWarning,
      openStaleCleanupDialog,
      openToggleMappingDialog,
      refreshAllTitles,
      removeAuthService,
      resetModeForm,
      rootDomainValidationMessage,
      saveGroupedMappingOrder,
      saveMappingGroups,
      saveMappingOrder,
      saveMode,
      savedEdgeClientIpProviderLabel,
      savedRootDomain,
      scheduleCloseProtocolHeadersWarning,
      searchQuery,
      selectEdgeClientIpProvider,
      setDefaultMapping,
      shouldShowProtocolHeadersWarning,
      syncRoutes,
      toggleProtocolHeadersWarning,
      trafficRealtimeStats,
      updateHostMappingGroupedView,
      visibleMappings,
    },
    dialogs: {
      allMappings,
      batchAvailabilityFormEnabled,
      batchAvailabilityFormEndTime,
      batchAvailabilityFormStartTime,
      batchAvailabilityOpen,
      batchAvailabilityValidationMessage,
      batchMutationConfirmLabel,
      batchMutationConfirmVariant,
      batchMutationDescription,
      batchMutationTitle,
      batchSelectedCount,
      availabilityDialogHostLabel,
      availabilityFormEnabled,
      availabilityFormEndTime,
      availabilityFormStartTime,
      availabilityValidationMessage,
      basicAuthInjectionModel,
      basicAuthValidationMessage,
      canRefreshMappingMetadata,
      canShowBasicAuthInjection,
      canUseRootDomainSuffix,
      closeAvailabilityDialog,
      closeBatchAvailability,
      closeBatchMutation,
      closeDeleteDialog,
      closeDialog,
      closeToggleDialog,
      composedPreviewHost,
      configStore,
      confirmDelete,
      confirmBatchMutation,
      confirmToggleMapping,
      deleteDialogConfirmLabel,
      deleteDialogDescription,
      deleteDialogTitle,
      discoverGroupId,
      discoverProgress,
      discoveredData,
      dismissDiscoverDialog,
      fullHostInputHint,
      gatewayHostResponseBlockedReason,
      gatewayProxyHeadersBlockedReason,
      globalWafEnabled,
      handleAvailabilityDialogOpenChange,
      handleDeleteDialogOpenChange,
      handleDialogOpenChange,
      handleDiscoverDialogOpenChange,
      handleMappingDialogFocusIn,
      handleMappingInputModeChange,
      handlePortalDisabledTooltipOpenChange,
      handlePortalDisabledTooltipTriggerClick,
      handleToggleDialogOpenChange,
      hostMappingGroups,
      iconEditor,
      isAllSelected,
      isAuthServiceTarget,
      isAvailabilityDialogOpen,
      isBatchMutationOpen,
      isClearingAllSubdomainConfig,
      isDeleteDialogOpen,
      isDialogOpen,
      isDiscoverDialogOpen,
      isDiscoverSelectionValid,
      isDiscoverSettingsOpen,
      isDiscovering,
      isGatewayAdvancedLoading,
      isMappingAuthService,
      isMappingValid,
      isMappingWebSocketTarget,
      isPortalDisabledTooltipOpen,
      isRefreshingMappingMetadata,
      isSavingMappings,
      isToggleDialogOpen,
      mappingDialogContentStyle,
      mappingDialogScrollStyle,
      mappingForm,
      mappingInputLabel,
      mappingInputMode,
      mappingModeDescription,
      mappingResolvedTitle,
      mappingSubdomain,
      mappingUseAuth,
      preserveHostModel,
      refreshMappingMetadata,
      saveAvailabilityDialog,
      saveBatchAvailability,
      saveDiscoveredServices,
      saveMapping,
      savedRootDomain,
      selectedServices,
      sendProxyHeadersModel,
      setAllSelected,
      setBasicAuthInjection,
      setDiscoverDialogRef,
      setMappingDialogScrollElement,
      setMappingSubdomain,
      setMappingUseAuth,
      setPreserveHost,
      setSendProxyHeaders,
      setShowToolbar,
      setStaleCleanupDialogRef,
      shouldShowPortalDisabledTooltip,
      showDiscoverHostColumn,
      showToolbar,
      stopDiscoverScan,
      t,
      toggleDialogConfirmLabel,
      toggleDialogConfirmVariant,
      toggleDialogDescription,
      toggleDialogTitle,
      toggleDiscoverSettings,
      triggerScan,
      updateMappingBasicAuth,
      updateMappingForm,
      visibilityEditor,
    },
  };
};

export type SubdomainProxyPageController = ReturnType<
  typeof useSubdomainProxyPage
>;
export type SubdomainProxyOverviewController =
  SubdomainProxyPageController["overview"];
export type SubdomainProxyDialogsController =
  SubdomainProxyPageController["dialogs"];
