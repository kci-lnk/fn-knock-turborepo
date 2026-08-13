<script setup lang="ts">
import { Settings2 } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import DocsLinkButton from "@/components/DocsLinkButton.vue";
import ConfirmationDialog from "@admin-shared/components/common/ConfirmationDialog.vue";
import { docsUrls } from "../../lib/docs";
import DDNSClearPrimaryConfigDialog from "./DDNSClearPrimaryConfigDialog.vue";
import DDNSExtraTargetsCard from "./DDNSExtraTargetsCard.vue";
import DDNSLogsCard from "./DDNSLogsCard.vue";
import DDNSPrimaryConfigCard from "./DDNSPrimaryConfigCard.vue";
import DDNSPublicCheckDialog from "./DDNSPublicCheckDialog.vue";
import DDNSStatusCard from "./DDNSStatusCard.vue";
import DDNSTargetDialog from "./DDNSTargetDialog.vue";
import DDNSUpdateIntervalDialog from "./DDNSUpdateIntervalDialog.vue";
import type { DDNSManagementPageController } from "./useDDNSManagementPage";

const props = defineProps<{ controller: DDNSManagementPageController }>();
const {
  applyCredentialTransfer,
  configuredNetworkInterface,
  configuredNetworkInterfaceLabel,
  confirmClearPrimaryConfig,
  confirmPendingAction,
  confirmationDialogOpen,
  confirmationDialogOptions,
  copyIpAddress,
  credentialTransferDescription,
  credentialTransferSuggestion,
  currentIpSourceLabel,
  currentNetworkInterfaceLabel,
  currentProviderDef,
  currentUpdateScopeLabel,
  deletingTargetId,
  enableFieldEditing,
  enabled,
  extraTargets,
  fieldVisibility,
  formatOptionLabel,
  formatPrimaryDomainOnBlur,
  formatTargetDomainOnBlur,
  getFieldAutocomplete,
  getFieldDomId,
  getFieldInputName,
  getPrimaryFieldDescription,
  getTargetFieldDescription,
  getTargetLastCheckTooltipLines,
  handleConfirmationDialogOpenChange,
  handleTargetDialogProviderChange,
  hasProviderConfig,
  hasSavedProviderConfig,
  httpTransportDraft,
  interfaceIPv4Options,
  interfaceIPv6Options,
  isClearingLogs,
  isClearingPrimaryConfig,
  isEnabledSwitchDisabled,
  isFieldEditReady,
  isInitialized,
  isLoading,
  isPrimaryConfigDirty,
  isProviderIpSourceOptionDisabled,
  isProviderSelectDisabled,
  isProviderUpdateScopeOptionDisabled,
  isSaving,
  isSavingPublicCheckSources,
  isSavingTarget,
  isSavingUpdateInterval,
  isTargetFieldVisible,
  isTesting,
  isTestingPublicCheckSources,
  isTransferSourceLoading,
  lastCheck,
  lastCheckTooltipLines,
  lastIP,
  logLines,
  logs,
  onCancelPrimaryConfigEdit,
  onClearLogs,
  onDeleteExtraTarget,
  onProviderChange,
  onSaveConfig,
  onTest,
  onTestExtraTarget,
  onToggleExtraTarget,
  openClearPrimaryConfigDialog,
  openCreateTargetDialog,
  openEditTargetDialog,
  openPublicCheckDialog,
  openUpdateIntervalDialog,
  providerConfig,
  providers,
  publicCheckDraft,
  publicCheckTestResults,
  publicDnsProviderDraft,
  resolvedNetworkInterfaces,
  restorePublicCheckDefaults,
  savePublicCheckSources,
  saveTargetDialog,
  saveUpdateInterval,
  selectedNetworkInterfaceDetail,
  selectedProvider,
  selectionAnchor,
  setProviderConfigField,
  shouldShowInterfaceAddressBlock,
  shouldShowSourceDomainBlock,
  showClearPrimaryConfigDialog,
  showIPv4Status,
  showIPv6Status,
  showInterfaceIPv4Select,
  showInterfaceIPv6Select,
  showPublicCheckDialog,
  showStaticIPv4Input,
  showStaticIPv6Input,
  showTargetDialog,
  showUpdateIntervalDialog,
  t,
  targetDialogDescription,
  targetDialogIPv4Options,
  targetDialogIPv6Options,
  targetDialogNetworkInterfaceLabel,
  targetDialogProviderDef,
  targetDialogResolvedNetworkInterfaces,
  targetDialogShouldShowDomainBlock,
  targetDialogShouldShowInterfaceBlock,
  targetDialogShouldShowStaticBlock,
  targetDialogState,
  targetDialogTitle,
  targetDialogUpdateScope,
  testPublicCheckSources,
  testingTargetId,
  toggleFieldVisibility,
  toggleTargetFieldVisibility,
  togglingTargetId,
  transferSourceScopeLabel,
  updateConfiguredIpSource,
  updateConfiguredNetworkInterface,
  updateIntervalDraft,
  updateIntervalLabel,
  updateTargetDialogNetworkInterface,
} = props.controller;
</script>

<template>
  <div v-if="isInitialized && !isLoading" class="space-y-3">
    <div class="flex items-center justify-between">
      <div class="flex items-center gap-2">
        <h2 class="text-xl font-semibold">{{ t("admin.ddns.title") }}</h2>
        <Button
          variant="ghost"
          size="icon-sm"
          class="text-muted-foreground hover:text-foreground"
          :aria-label="t('admin.ddns.publicCheckSettings')"
          :title="t('admin.ddns.publicCheckSettings')"
          @click="openPublicCheckDialog"
        >
          <Settings2 class="h-4 w-4" />
        </Button>
        <DocsLinkButton :href="docsUrls.guides.ddns" />
      </div>
      <div class="flex items-center gap-3">
        <span class="text-sm text-muted-foreground">{{
          enabled ? t("admin.ddns.enabled") : t("admin.ddns.disabled")
        }}</span>
        <Switch
          v-model="enabled"
          :aria-label="t('admin.ddns.enabled')"
          :disabled="isEnabledSwitchDisabled"
        />
      </div>
    </div>

    <DDNSStatusCard
      :copy-ip-address="copyIpAddress"
      :current-ip-source-label="currentIpSourceLabel"
      :current-network-interface-label="currentNetworkInterfaceLabel"
      :current-update-scope-label="currentUpdateScopeLabel"
      :enabled="enabled"
      :last-check="lastCheck"
      :last-check-tooltip-lines="lastCheckTooltipLines"
      :last-ip="lastIP"
      :open-update-interval-dialog="openUpdateIntervalDialog"
      :show-ipv4-status="showIPv4Status"
      :show-ipv6-status="showIPv6Status"
      :update-interval-label="updateIntervalLabel"
    />

    <DDNSPrimaryConfigCard
      :configured="hasProviderConfig"
      :configured-network-interface="configuredNetworkInterface"
      :configured-network-interface-label="configuredNetworkInterfaceLabel"
      :credential-transfer-description="credentialTransferDescription"
      :credential-transfer-suggestion="credentialTransferSuggestion"
      :enable-field-editing="enableFieldEditing"
      :field-visibility="fieldVisibility"
      :format-option-label="formatOptionLabel"
      :get-field-autocomplete="getFieldAutocomplete"
      :get-field-description="getPrimaryFieldDescription"
      :get-field-dom-id="getFieldDomId"
      :get-field-input-name="getFieldInputName"
      :has-saved-provider-config="hasSavedProviderConfig"
      :interface-i-pv4-options="interfaceIPv4Options"
      :interface-i-pv6-options="interfaceIPv6Options"
      :last-ip="lastIP"
      :selection-anchor="selectionAnchor"
      :is-clearing-primary-config="isClearingPrimaryConfig"
      :is-dirty="isPrimaryConfigDirty"
      :is-field-edit-ready="isFieldEditReady"
      :is-ip-source-option-disabled="isProviderIpSourceOptionDisabled"
      :is-provider-select-disabled="isProviderSelectDisabled"
      :is-saving="isSaving"
      :is-testing="isTesting"
      :is-transfer-source-loading="isTransferSourceLoading"
      :is-update-scope-option-disabled="isProviderUpdateScopeOptionDisabled"
      :provider-config="providerConfig"
      :provider-def="currentProviderDef"
      :providers="providers"
      :ready="!isLoading"
      :resolved-network-interfaces="resolvedNetworkInterfaces"
      :selected-network-interface-detail="selectedNetworkInterfaceDetail"
      :selected-provider="selectedProvider"
      :set-field-value="setProviderConfigField"
      :format-domain-field="formatPrimaryDomainOnBlur"
      :show-interface-address-block="shouldShowInterfaceAddressBlock"
      :show-interface-i-pv4-select="showInterfaceIPv4Select"
      :show-interface-i-pv6-select="showInterfaceIPv6Select"
      :show-source-domain-block="shouldShowSourceDomainBlock"
      :show-static-i-pv4-input="showStaticIPv4Input"
      :show-static-i-pv6-input="showStaticIPv6Input"
      :toggle-field-visibility="toggleFieldVisibility"
      :transfer-source-scope-label="transferSourceScopeLabel"
      :update-ip-source="updateConfiguredIpSource"
      :update-network-interface="updateConfiguredNetworkInterface"
      @apply-credential-transfer="applyCredentialTransfer"
      @cancel="onCancelPrimaryConfigEdit"
      @clear-primary-config="openClearPrimaryConfigDialog"
      @provider-change="onProviderChange"
      @save="onSaveConfig"
      @test="onTest"
    />

    <DDNSExtraTargetsCard
      :targets="extraTargets"
      :is-saving-target="isSavingTarget"
      :testing-target-id="testingTargetId"
      :toggling-target-id="togglingTargetId"
      :deleting-target-id="deletingTargetId"
      :copy-ip-address="copyIpAddress"
      :delete-target="onDeleteExtraTarget"
      :edit-target="openEditTargetDialog"
      :get-last-check-tooltip-lines="getTargetLastCheckTooltipLines"
      :test-target="onTestExtraTarget"
      :toggle-target="onToggleExtraTarget"
      @create="openCreateTargetDialog"
    />

    <DDNSLogsCard
      :can-clear="logs.length > 0"
      :clear-logs="onClearLogs"
      :is-clearing="isClearingLogs"
      :log-lines="logLines"
    />

    <DDNSTargetDialog
      :open="showTargetDialog"
      :title="targetDialogTitle"
      :description="targetDialogDescription"
      :state="targetDialogState"
      :providers="providers"
      :provider-def="targetDialogProviderDef"
      :resolved-network-interfaces="targetDialogResolvedNetworkInterfaces"
      :network-interface-label="targetDialogNetworkInterfaceLabel"
      :should-show-static-block="targetDialogShouldShowStaticBlock"
      :should-show-domain-block="targetDialogShouldShowDomainBlock"
      :should-show-interface-block="targetDialogShouldShowInterfaceBlock"
      :update-scope="targetDialogUpdateScope"
      :ipv4-options="targetDialogIPv4Options"
      :ipv6-options="targetDialogIPv6Options"
      :is-saving="isSavingTarget"
      :format-option-label="formatOptionLabel"
      :is-update-scope-option-disabled="isProviderUpdateScopeOptionDisabled"
      :is-ip-source-option-disabled="isProviderIpSourceOptionDisabled"
      :get-field-description="getTargetFieldDescription"
      :get-field-autocomplete="getFieldAutocomplete"
      :format-domain-field="formatTargetDomainOnBlur"
      :is-field-visible="isTargetFieldVisible"
      :toggle-field-visibility="toggleTargetFieldVisibility"
      @update:open="showTargetDialog = $event"
      @update:provider="handleTargetDialogProviderChange"
      @update:network-interface="updateTargetDialogNetworkInterface"
      @confirm="saveTargetDialog"
    />

    <DDNSUpdateIntervalDialog
      v-model:draft="updateIntervalDraft"
      :open="showUpdateIntervalDialog"
      :is-saving="isSavingUpdateInterval"
      @update:open="showUpdateIntervalDialog = $event"
      @confirm="saveUpdateInterval"
    />

    <DDNSPublicCheckDialog
      v-model:draft="publicCheckDraft"
      v-model:http-transport-draft="httpTransportDraft"
      v-model:public-dns-provider-draft="publicDnsProviderDraft"
      :open="showPublicCheckDialog"
      :is-saving="isSavingPublicCheckSources"
      :is-testing="isTestingPublicCheckSources"
      :test-results="publicCheckTestResults"
      @update:open="showPublicCheckDialog = $event"
      @restore-defaults="restorePublicCheckDefaults"
      @save="savePublicCheckSources"
      @test="testPublicCheckSources"
    />

    <DDNSClearPrimaryConfigDialog
      :open="showClearPrimaryConfigDialog"
      :is-clearing="isClearingPrimaryConfig"
      @update:open="showClearPrimaryConfigDialog = $event"
      @confirm="confirmClearPrimaryConfig"
    />

    <ConfirmationDialog
      :open="confirmationDialogOpen"
      v-bind="confirmationDialogOptions"
      @update:open="handleConfirmationDialogOpenChange"
      @confirm="confirmPendingAction"
    />
  </div>

  <div v-else class="flex h-full items-center justify-center min-h-[400px]">
    <div
      class="animate-spin rounded-full h-8 w-8 border-b-2 border-primary"
    ></div>
  </div>
</template>
