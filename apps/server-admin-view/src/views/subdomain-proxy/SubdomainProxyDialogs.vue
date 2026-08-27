<script setup lang="ts">
import StaleHostMappingsCleanupDialog from "@/components/StaleHostMappingsCleanupDialog.vue";
import SubdomainActionConfirmDialog from "./SubdomainActionConfirmDialog.vue";
import SubdomainAvailabilityDialog from "./SubdomainAvailabilityDialog.vue";
import SubdomainDeleteDialog from "./SubdomainDeleteDialog.vue";
import SubdomainDiscoverDialog from "./SubdomainDiscoverDialog.vue";
import SubdomainMappingDialog from "./SubdomainMappingDialog.vue";
import SubdomainTargetOptimizationDialog from "./SubdomainTargetOptimizationDialog.vue";
import type { SubdomainProxyDialogsController } from "./useSubdomainProxyPage";

const props = defineProps<{ controller: SubdomainProxyDialogsController }>();
const {
  allMappings,
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
  closeDeleteDialog,
  closeDialog,
  closeToggleDialog,
  composedPreviewHost,
  configStore,
  confirmDelete,
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
  shouldShowPortalDisabledTooltip,
  showDiscoverHostColumn,
  showToolbar,
  setStaleCleanupDialogRef,
  stopDiscoverScan,
  t,
  toggleDialogConfirmLabel,
  toggleDialogConfirmVariant,
  toggleDialogDescription,
  toggleDialogTitle,
  toggleDiscoverSettings,
  targetOptimization,
  triggerScan,
  updateMappingBasicAuth,
  updateMappingForm,
  visibilityEditor,
} = props.controller;
</script>

<template>
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
      :groups="hostMappingGroups"
      :handle-focus-in="handleMappingDialogFocusIn"
      :handle-input-mode-change="handleMappingInputModeChange"
      :handle-portal-disabled-tooltip-open-change="
        handlePortalDisabledTooltipOpenChange
      "
      :handle-portal-disabled-tooltip-trigger-click="
        handlePortalDisabledTooltipTriggerClick
      "
      :is-gateway-advanced-loading="isGatewayAdvancedLoading"
      :icon-editor="iconEditor"
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
      :visibility-editor="visibilityEditor"
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
      v-model:group-id="discoverGroupId"
      :ref="setDiscoverDialogRef"
      :open="isDiscoverDialogOpen"
      :domain="savedRootDomain"
      :groups="hostMappingGroups"
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
    :ref="setStaleCleanupDialogRef"
    :mappings="allMappings"
    :save-mappings="configStore.saveHostMappings"
    :is-auth-service-target="isAuthServiceTarget"
  />

  <SubdomainTargetOptimizationDialog :model="targetOptimization" />
</template>
