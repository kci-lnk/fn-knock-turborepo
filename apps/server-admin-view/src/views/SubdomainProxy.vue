<template>
  <div class="space-y-6">
    <SubdomainModeConfigCard
      v-model:auth-service-public-port="authServicePublicPort"
      v-model:edge-client-ip-enabled="modeForm.edge_client_ip_enabled"
      v-model:root-domain="modeForm.root_domain"
      :active-edge-client-ip-provider="activeEdgeClientIpProvider"
      :auth-service-mapping="authServiceMapping"
      :configured="isSubdomainModeConfigured"
      :ready="!configStore.isLoading"
      :edge-client-ip-provider-options="edgeClientIpProviderOptions"
      :format-auth-service-host="formatAuthServiceHostWithPublicPort"
      :is-edge-client-ip-mode-editable="isEdgeClientIPModeEditable"
      :is-mode-dirty="isModeDirty"
      :is-mode-valid="isModeValid"
      :is-saving-mappings="isSavingMappings"
      :is-saving-mode="isSavingMode"
      :remove-auth-service="removeAuthService"
      :reset-mode-form="resetModeForm"
      :save-mode="saveMode"
      :saved-edge-client-ip-provider-label="savedEdgeClientIpProviderLabel"
      :saved-root-domain="savedRootDomain"
      :select-edge-client-ip-provider="selectEdgeClientIpProvider"
    />

    <SubdomainMappingsCard
      v-model:draggable-mappings="draggableVisibleMappings"
      v-model:search-query="searchQuery"
      :all-mappings-count="allMappings.length"
      :auth-service-mapping="authServiceMapping"
      :can-manage-new-mappings="canManageNewMappings"
      :discover-button-divider-class="discoverButtonDividerClass"
      :discover-button-variant="discoverButtonVariant"
      :docs-href="docsUrls.guides.subdomainProxy"
      :filtered-mappings="filteredMappings"
      :format-availability-window="formatAvailabilityWindow"
      :format-host="formatHostWithAccessEntryPort"
      :get-availability-state="getAvailabilityState"
      :get-host-traffic-sample="getHostTrafficSample"
      :get-mapping-title-for-display="getMappingTitleForDisplay"
      :handle-location-rules-tooltip-open-change="
        handleLocationRulesTooltipOpenChange
      "
      :handle-location-rules-tooltip-trigger-click="
        handleLocationRulesTooltipTriggerClick
      "
      :handle-protocol-headers-warning-open-change="
        handleProtocolHeadersWarningOpenChange
      "
      :has-regular-host-mappings="hasRegularHostMappings"
      :is-auth-service-target="isAuthServiceTarget"
      :is-clearing-all-subdomain-config="isClearingAllSubdomainConfig"
      :is-config-loading="configStore.isLoading"
      :is-discovering="isDiscovering"
      :is-exporting-bookmarks="isExportingBookmarks"
      :is-favicon-broken="isFaviconBroken"
      :is-gateway-portal-enabled="isGatewayPortalEnabled"
      :is-mapping-unavailable="isMappingUnavailable"
      :is-location-rules-tooltip-open="isLocationRulesTooltipOpen"
      :is-protocol-headers-warning-open="isProtocolHeadersWarningOpen"
      :is-refreshing-titles="isRefreshingTitles"
      :is-root-domain-pending-save="isRootDomainPendingSave"
      :is-saving-mappings="isSavingMappings"
      :is-syncing="isSyncing"
      :mark-favicon-broken="markFaviconBroken"
      :open-protocol-headers-warning="openProtocolHeadersWarning"
      :saved-root-domain="savedRootDomain"
      :schedule-close-protocol-headers-warning="
        scheduleCloseProtocolHeadersWarning
      "
      :should-show-protocol-headers-warning="shouldShowProtocolHeadersWarning"
      :toggle-protocol-headers-warning="toggleProtocolHeadersWarning"
      :traffic-timestamp="trafficRealtimeStats?.timestamp ?? null"
      :visible-mappings-count="visibleMappings.length"
      @add-auth-service="addAuthService"
      @clear-default="clearDefaultMapping"
      @copy-host="copyMappingHost"
      @delete="openDeleteMappingDialog"
      @edit="openEditDialog"
      @export-bookmarks="exportBookmarks"
      @open-clear-all-config="openClearAllConfigDialog"
      @open-create="openCreateDialog"
      @open-discover="openDiscoverDialog"
      @open-availability="openAvailabilityDialog"
      @open-gateway-locations="openGatewayLocations"
      @open-stale-cleanup="openStaleCleanupDialog"
      @refresh-all-titles="refreshAllTitles"
      @save-order="saveMappingOrder"
      @set-default="setDefaultMapping"
      @sync-routes="syncRoutes"
      @toggle-enabled="openToggleMappingDialog"
    />

    <SubdomainMappingDialog
      :basic-auth-injection="basicAuthInjectionModel"
      :basic-auth-validation-message="basicAuthValidationMessage"
      :can-refresh-mapping-metadata="canRefreshMappingMetadata"
      :can-show-basic-auth-injection="canShowBasicAuthInjection"
      :can-use-root-domain-suffix="canUseRootDomainSuffix"
      :composed-preview-host="composedPreviewHost"
      :content-style="mappingDialogContentStyle"
      :full-host-input-hint="fullHostInputHint"
      :gateway-host-response-blocked-reason="gatewayHostResponseBlockedReason"
      :gateway-proxy-headers-blocked-reason="gatewayProxyHeadersBlockedReason"
      :global-waf-enabled="globalWafEnabled"
      :handle-focus-in="handleMappingDialogFocusIn"
      :handle-input-mode-change="handleMappingInputModeChange"
      :handle-portal-disabled-tooltip-open-change="
        handlePortalDisabledTooltipOpenChange
      "
      :handle-portal-disabled-tooltip-trigger-click="
        handlePortalDisabledTooltipTriggerClick
      "
      :is-gateway-advanced-loading="isGatewayAdvancedLoading"
      :is-mapping-auth-service="isMappingAuthService"
      :is-mapping-valid="isMappingValid"
      :is-mapping-web-socket-target="isMappingWebSocketTarget"
      :is-portal-disabled-tooltip-open="isPortalDisabledTooltipOpen"
      :is-refreshing-mapping-metadata="isRefreshingMappingMetadata"
      :is-saving-mappings="isSavingMappings"
      :mapping-form="mappingForm"
      :mapping-input-label="mappingInputLabel"
      :mapping-input-mode="mappingInputMode"
      :mapping-mode-description="mappingModeDescription"
      :mapping-resolved-title="mappingResolvedTitle"
      :mapping-subdomain="mappingSubdomain"
      :mapping-use-auth="mappingUseAuth"
      :open="isDialogOpen"
      :preserve-host="preserveHostModel"
      :refresh-mapping-metadata="refreshMappingMetadata"
      :saved-root-domain="savedRootDomain"
      :scroll-style="mappingDialogScrollStyle"
      :send-proxy-headers="sendProxyHeadersModel"
      :set-basic-auth-injection="setBasicAuthInjection"
      :set-mapping-subdomain="setMappingSubdomain"
      :set-mapping-use-auth="setMappingUseAuth"
      :set-preserve-host="setPreserveHost"
      :set-scroll-element="setMappingDialogScrollElement"
      :set-send-proxy-headers="setSendProxyHeaders"
      :set-show-toolbar="setShowToolbar"
      :should-show-portal-disabled-tooltip="shouldShowPortalDisabledTooltip"
      :show-toolbar="showToolbar"
      :update-mapping-basic-auth="updateMappingBasicAuth"
      :update-mapping-form="updateMappingForm"
      @close="closeDialog"
      @save="saveMapping"
      @update:open="handleDialogOpenChange"
    />

    <SubdomainDeleteDialog
      :open="isDeleteDialogOpen"
      :title="deleteDialogTitle"
      :description="deleteDialogDescription"
      :cancel-label="t('admin.subdomainProxy.cancel')"
      :confirm-label="deleteDialogConfirmLabel"
      :loading="isSavingMappings || isClearingAllSubdomainConfig"
      @update:open="handleDeleteDialogOpenChange"
      @cancel="closeDeleteDialog"
      @confirm="confirmDelete"
    />

    <SubdomainActionConfirmDialog
      :open="isToggleDialogOpen"
      :title="toggleDialogTitle"
      :description="toggleDialogDescription"
      :cancel-label="t('admin.subdomainProxy.cancel')"
      :confirm-label="toggleDialogConfirmLabel"
      :confirm-variant="toggleDialogConfirmVariant"
      :loading="isSavingMappings"
      @update:open="handleToggleDialogOpenChange"
      @cancel="closeToggleDialog"
      @confirm="confirmToggleMapping"
    />

    <SubdomainAvailabilityDialog
      :open="isAvailabilityDialogOpen"
      :host="availabilityDialogHostLabel"
      :enabled="availabilityFormEnabled"
      :start-time="availabilityFormStartTime"
      :end-time="availabilityFormEndTime"
      :loading="isSavingMappings"
      :validation-message="availabilityValidationMessage"
      @update:open="handleAvailabilityDialogOpenChange"
      @update:enabled="availabilityFormEnabled = $event"
      @update:start-time="availabilityFormStartTime = $event"
      @update:end-time="availabilityFormEndTime = $event"
      @cancel="closeAvailabilityDialog"
      @save="saveAvailabilityDialog"
    />

    <SubdomainDiscoverDialog
      :ref="setDiscoverDialogRef"
      :open="isDiscoverDialogOpen"
      :domain="savedRootDomain"
      :is-settings-open="isDiscoverSettingsOpen"
      :is-discovering="isDiscovering"
      :discover-progress="discoverProgress"
      :discovered-data="discoveredData"
      :selected-services="selectedServices"
      :is-all-selected="isAllSelected"
      :is-selection-valid="isDiscoverSelectionValid"
      :show-host-column="showDiscoverHostColumn"
      :is-saving-mappings="isSavingMappings"
      @update:open="handleDiscoverDialogOpenChange"
      @update:selected-services="selectedServices = $event"
      @toggle-settings="toggleDiscoverSettings"
      @toggle-all="setAllSelected"
      @scan="triggerScan"
      @stop-scan="stopDiscoverScan"
      @cancel="dismissDiscoverDialog"
      @save="saveDiscoveredServices"
    />

    <StaleHostMappingsCleanupDialog
      ref="staleCleanupDialogRef"
      :mappings="allMappings"
      :save-mappings="saveHostMappingsForCleanup"
      :is-auth-service-target="isAuthServiceTarget"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import StaleHostMappingsCleanupDialog from "@/components/StaleHostMappingsCleanupDialog.vue";
import SubdomainActionConfirmDialog from "./subdomain-proxy/SubdomainActionConfirmDialog.vue";
import SubdomainAvailabilityDialog from "./subdomain-proxy/SubdomainAvailabilityDialog.vue";
import SubdomainDeleteDialog from "./subdomain-proxy/SubdomainDeleteDialog.vue";
import SubdomainDiscoverDialog from "./subdomain-proxy/SubdomainDiscoverDialog.vue";
import SubdomainMappingDialog from "./subdomain-proxy/SubdomainMappingDialog.vue";
import SubdomainMappingsCard from "./subdomain-proxy/SubdomainMappingsCard.vue";
import SubdomainModeConfigCard from "./subdomain-proxy/SubdomainModeConfigCard.vue";
import { toast } from "@admin-shared/utils/toast";
import { useConfigStore } from "../store/config";
import { isAnySubdomainRoutingMode } from "../lib/reverse-proxy-submode";
import { ConfigAPI, DashboardAPI } from "../lib/api";
import { docsUrls } from "../lib/docs";
import type { HostMapping } from "../types";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import {
  DEFAULT_ACCESS_MODE,
  DEFAULT_AUTH_SUBDOMAIN,
  DEFAULT_PROTOCOL_MODE,
  composeHostFromSubdomain,
  createDisabledMappingBasicAuth,
  getMappingDisplayTitle,
  isHttpTargetUrl,
  normalizeHostLike,
  parseTargetPort,
  resolveDefaultAuthServiceTarget,
} from "./subdomain-proxy/model";
import { useAccessEntryPort } from "@/composables/useAccessEntryPort";
import { useSubdomainAvailabilityActions } from "./subdomain-proxy/useSubdomainAvailabilityActions";
import { useSubdomainAvailabilityStatus } from "./subdomain-proxy/useSubdomainAvailabilityStatus";
import { useDelayedHostPopover } from "./subdomain-proxy/useDelayedHostPopover";
import { useMappingFaviconState } from "./subdomain-proxy/useMappingFaviconState";
import { useMediaQueryMatch } from "@admin-shared/composables/useMediaQueryMatch";
import { useTrafficRealtime } from "./subdomain-proxy/useTrafficRealtime";
import { useSubdomainTouchTooltips } from "./subdomain-proxy/useSubdomainTouchTooltips";
import { useSubdomainPortDisplay } from "./subdomain-proxy/useSubdomainPortDisplay";
import { useSubdomainDeleteDialog } from "./subdomain-proxy/useSubdomainDeleteDialog";
import { useSubdomainMappingsView } from "./subdomain-proxy/useSubdomainMappingsView";
import { useSubdomainDiscoverFlow } from "./subdomain-proxy/useSubdomainDiscoverFlow";
import { useSubdomainMappingDialogController } from "./subdomain-proxy/useSubdomainMappingDialogController";
import { useSubdomainMappingListActions } from "./subdomain-proxy/useSubdomainMappingListActions";
import { useSubdomainModeConfig } from "./subdomain-proxy/useSubdomainModeConfig";

const configStore = useConfigStore();
const { t } = useI18n();
const staleCleanupDialogRef = ref<InstanceType<
  typeof StaleHostMappingsCleanupDialog
> | null>(null);

const searchQuery = ref("");
const router = useRouter();
const draggableVisibleMappings = ref<HostMapping[]>([]);
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
  savedEdgeClientIpProvider,
  selectEdgeClientIpProvider,
} = useSubdomainPortDisplay({
  accessEntryPort,
  currentModeConfig,
  getConfig: () => configStore.config,
  modeForm,
});
const authServicePort = computed(
  () => parseTargetPort(currentModeConfig.value.auth_target) ?? 7997,
);
const isAuthServiceTarget = (target: string): boolean =>
  isHttpTargetUrl(target) && parseTargetPort(target) === authServicePort.value;
const savedEdgeClientIpProviderLabel = computed(() =>
  savedEdgeClientIpProvider.value
    ? t("admin.subdomainProxy.edgeRealIpSummary", {
        provider: getEdgeClientIpProviderLabel(savedEdgeClientIpProvider.value),
      })
    : "",
);
const allMappings = computed(() => configStore.config?.host_mappings ?? []);
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
  isAuthServiceTarget,
  searchQuery,
  trafficRealtimeStats,
});
const isGatewayPortalEnabled = computed(
  () => configStore.config?.gateway_portal?.enabled !== false,
);
const globalWafEnabled = computed(
  () => configStore.config?.waf?.enabled === true,
);
const shouldShowPortalDisabledTooltip = computed(
  () => !isGatewayPortalEnabled.value,
);
const {
  handleLocationRulesTooltipOpenChange,
  handleLocationRulesTooltipTriggerClick,
  handlePortalDisabledTooltipOpenChange,
  handlePortalDisabledTooltipTriggerClick,
  isLocationRulesTooltipOpen,
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
    toast.error(t("admin.subdomainProxy.saveFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.subdomainProxy.saveMappingFailed"),
      ),
    });
  },
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
} = useSubdomainMappingDialogController({
  allMappings,
  canUseRootDomainSuffix,
  getConfig: () => configStore.config,
  isAuthServiceTarget,
  isGatewayAdvancedAvailableByMode,
  resetFaviconErrors,
  runSaveMappings,
  savedRootDomain,
  saveHostMappings: (mappings) => configStore.saveHostMappings(mappings),
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
  savedRootDomain,
  saveHostMappings: (mappings) => configStore.saveHostMappings(mappings),
  translate: (key, params) => (params ? t(key, params) : t(key)),
});

const {
  isPending: isClearingAllSubdomainConfig,
  run: runClearAllSubdomainConfig,
} = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.subdomainProxy.clearFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.subdomainProxy.clearConfigFailed"),
      ),
    });
  },
});

const { isPending: isSyncing, run: runSyncRoutes } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.subdomainProxy.syncFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.subdomainProxy.syncGatewayFailed"),
      ),
    });
  },
});

const { isPending: isRefreshingTitles, run: runRefreshTitles } = useAsyncAction(
  {
    onError: (error) => {
      toast.error(t("admin.subdomainProxy.refreshFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.subdomainProxy.refreshAllTitlesFailed"),
        ),
      });
    },
  },
);

const { isPending: isExportingBookmarks, run: runExportBookmarks } =
  useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.subdomainProxy.exportFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.subdomainProxy.exportBookmarksFailed"),
        ),
      });
    },
  });

const {
  clearDefaultMapping,
  copyMappingHost,
  exportBookmarks,
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
  isSavingMappings,
  navigateToGatewayLocations: (host) => {
    void router.push({
      path: "/system/gateway-locations",
      query: { host },
    });
  },
  refreshAllHostMappingTitles: () => configStore.refreshAllHostMappingTitles(),
  resetFaviconErrors,
  runExportBookmarks,
  runRefreshTitles,
  runSaveMappings,
  runSyncRoutes,
  saveHostMappings: (mappings) => configStore.saveHostMappings(mappings),
  savedRootDomain,
  syncDraggableVisibleMappings,
  syncRoutesApi: () => ConfigAPI.syncRoutes(),
  translate: (key, params) => (params ? t(key, params) : t(key)),
  visibleMappings,
});

watch(
  filteredMappings,
  () => {
    syncDraggableVisibleMappings();
  },
  { immediate: true },
);

onMounted(async () => {
  startAvailabilityClock();

  window.visualViewport?.addEventListener(
    "resize",
    handleMappingDialogViewportResize,
  );
  window.visualViewport?.addEventListener(
    "scroll",
    handleMappingDialogViewportResize,
  );
  if (!configStore.config) {
    await configStore.loadConfig();
  }
  void loadAccessEntryPort();
  startTrafficRealtimePolling();
});

onUnmounted(() => {
  stopAvailabilityClock();
  window.visualViewport?.removeEventListener(
    "resize",
    handleMappingDialogViewportResize,
  );
  window.visualViewport?.removeEventListener(
    "scroll",
    handleMappingDialogViewportResize,
  );
  clearMappingDialogKeyboardScrollTimer();
  clearProtocolHeadersWarningCloseTimer();
  stopTrafficRealtimePolling();
  stopDiscoverScan();
});

async function addAuthService() {
  if (!canManageNewMappings.value) {
    toast.error(t("admin.subdomainProxy.cannotAddAuthService"), {
      description: !savedRootDomain.value
        ? t("admin.subdomainProxy.saveRootFirst")
        : t("admin.subdomainProxy.rootDirtyAddAuth"),
    });
    return;
  }

  if (authServiceMapping.value) {
    toast.error(t("admin.subdomainProxy.authServiceExists"), {
      description: t("admin.subdomainProxy.authServiceExistsDescription", {
        host: authServiceMapping.value.host,
      }),
    });
    return;
  }

  const host = composeHostFromSubdomain(
    DEFAULT_AUTH_SUBDOMAIN,
    savedRootDomain.value,
  );
  const target = resolveDefaultAuthServiceTarget(
    modeForm.auth_target,
    currentModeConfig.value.auth_target,
  );

  if (!host) {
    toast.error(t("admin.subdomainProxy.defaultAuthGenerateFailed"), {
      description: t("admin.subdomainProxy.confirmRootSaved"),
    });
    return;
  }

  const duplicateHost = allMappings.value.find((item) => item.host === host);
  if (duplicateHost) {
    toast.error(t("admin.subdomainProxy.defaultAuthSubdomainExists"), {
      description: t(
        "admin.subdomainProxy.defaultAuthSubdomainExistsDescription",
        { host },
      ),
    });
    return;
  }

  await runSaveMappings(async () => {
    await configStore.saveHostMappings([
      ...allMappings.value,
      {
        host,
        target,
        waf_enabled: true,
        use_auth: false,
        access_mode: DEFAULT_ACCESS_MODE,
        suppress_toolbar: false,
        preserve_host: true,
        is_default: false,
        disabled: false,
        availability: null,
        protocol_mode: DEFAULT_PROTOCOL_MODE,
        basic_auth: createDisabledMappingBasicAuth(),
        locations: [],
        service_role: "auth",
        title: "",
        title_override: "",
        favicon: "",
      },
    ]);

    toast.success(t("admin.subdomainProxy.authServiceAdded"), {
      description: `${host} -> ${target}`,
    });
  });
}

function openClearAllConfigDialog() {
  if (allMappings.value.length === 0) {
    toast.error(t("admin.subdomainProxy.noClearableMappings"));
    return;
  }

  openClearAllConfigDialogState();
}

async function removeAuthService(): Promise<boolean> {
  if (!authServiceMapping.value) {
    toast.error(t("admin.subdomainProxy.noCurrentAuthService"));
    return false;
  }

  const authHost = authServiceMapping.value.host;

  const removed = await runSaveMappings(async () => {
    await configStore.saveHostMappings(
      allMappings.value.filter((item) => !isAuthServiceTarget(item.target)),
    );

    toast.success(t("admin.subdomainProxy.authServiceDeleted"), {
      description: authHost,
    });

    return true;
  });

  return removed === true;
}

async function clearAllSubdomainConfig(): Promise<boolean> {
  const mappingsCount = allMappings.value.length;

  const cleared = await runClearAllSubdomainConfig(async () => {
    await configStore.saveHostMappings([]);

    toast.success(t("admin.subdomainProxy.allCleared"), {
      description:
        mappingsCount > 0
          ? t("admin.subdomainProxy.clearedMappingsDescription", {
              count: mappingsCount,
            })
          : t("admin.subdomainProxy.modeConfigKept"),
    });

    return true;
  });

  return cleared === true;
}

async function removeMapping(host: string): Promise<boolean> {
  const target = allMappings.value.find((item) => item.host === host);
  if (!target) return false;

  const removed = await runSaveMappings(async () => {
    await configStore.saveHostMappings(
      allMappings.value.filter((item) => item.host !== host),
    );
    toast.success(t("admin.subdomainProxy.mappingDeleted"));

    return true;
  });

  return removed === true;
}

async function confirmDelete() {
  const target = deleteDialogState.value;
  if (!target) return;

  if (target.kind === "clear_all") {
    if (advanceClearAllConfirmation()) {
      return;
    }

    const cleared = await clearAllSubdomainConfig();
    if (cleared) {
      closeDeleteDialog();
    }
    return;
  }

  const removed = await removeMapping(target.host);

  if (removed) {
    closeDeleteDialog();
  }
}

function openStaleCleanupDialog() {
  void staleCleanupDialogRef.value?.open();
}

const saveHostMappingsForCleanup = async (mappings: HostMapping[]) => {
  await configStore.saveHostMappings(mappings);
};
</script>
