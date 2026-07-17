<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed } from "vue";
import { useI18n } from "vue-i18n";
import { onBeforeRouteLeave } from "vue-router";
import { Settings2 } from "lucide-vue-next";
import { DDNSAPI } from "../lib/api";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import DocsLinkButton from "@/components/DocsLinkButton.vue";
import { toast } from "@admin-shared/utils/toast";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import type { DDNSNetworkInterfacePayload } from "../lib/api";
import { useConfigStore } from "../store/config";
import { isAnySubdomainRoutingMode } from "../lib/reverse-proxy-submode";
import { docsUrls } from "../lib/docs";
import {
  normalizeTargetConfigValues,
  validateDDNSCommonConfig,
  type DDNSValidationIssue,
  type Provider,
  type TargetDialogState,
} from "./ddns-management/model";
import { useDDNSFieldState } from "./ddns-management/useDDNSFieldState";
import { useDDNSDomainField } from "./ddns-management/useDDNSDomainField";
import { useDDNSAddressSourceState } from "./ddns-management/useDDNSAddressSourceState";
import DDNSClearPrimaryConfigDialog from "./ddns-management/DDNSClearPrimaryConfigDialog.vue";
import DDNSExtraTargetsCard from "./ddns-management/DDNSExtraTargetsCard.vue";
import DDNSLogsCard from "./ddns-management/DDNSLogsCard.vue";
import DDNSPrimaryConfigCard from "./ddns-management/DDNSPrimaryConfigCard.vue";
import DDNSPublicCheckDialog from "./ddns-management/DDNSPublicCheckDialog.vue";
import DDNSStatusCard from "./ddns-management/DDNSStatusCard.vue";
import DDNSTargetDialog from "./ddns-management/DDNSTargetDialog.vue";
import DDNSUpdateIntervalDialog from "./ddns-management/DDNSUpdateIntervalDialog.vue";
import { useDDNSTargetActions } from "./ddns-management/useDDNSTargetActions";
import { useDDNSTargetDialogState } from "./ddns-management/useDDNSTargetDialogState";
import { useDDNSPrimaryConfigActions } from "./ddns-management/useDDNSPrimaryConfigActions";
import { useDDNSPolling } from "./ddns-management/useDDNSPolling";
import { useDDNSResourceLoading } from "./ddns-management/useDDNSResourceLoading";
import { useDDNSSettingsDialogs } from "./ddns-management/useDDNSSettingsDialogs";
import { useDDNSStatus } from "./ddns-management/useDDNSStatus";
import { useDDNSCredentialTransferHint } from "./ddns-management/useDDNSCredentialTransferHint";
import { useDDNSPrimaryConfigState } from "./ddns-management/useDDNSPrimaryConfigState";
import { useDDNSStatusPresentation } from "./ddns-management/useDDNSStatusPresentation";

const { t, locale } = useI18n();

// ─── State ─────────────────────────────────────────────────────
const isInitialized = ref(false);
const configStore = useConfigStore();
const selectedProvider = ref<string>("");
const providers = ref<Provider[]>([]);
const providerConfig = ref<Record<string, string>>({});
const savedProviderConfig = ref<Record<string, string>>({});
const networkInterfaces = ref<DDNSNetworkInterfacePayload[]>([]);
const showTargetDialog = ref(false);
const targetDialogMode = ref<"create" | "edit">("create");
const targetDialogState = ref<TargetDialogState>({
  id: null,
  name: "",
  enabled: true,
  provider: "",
  config: normalizeTargetConfigValues({}),
});
const testingTargetId = ref("");
const deletingTargetId = ref("");
const togglingTargetId = ref("");
const {
  applyStatus,
  defaultPublicCheckSources,
  enabled,
  httpTransport,
  publicDnsProvider,
  lastCheck,
  lastIP,
  selectionAnchor,
  publicCheckSources,
  savedProvider,
  statusIpSource,
  statusNetworkInterface,
  statusUpdateScope,
  targetSummaries,
  updateIntervalMinutes,
} = useDDNSStatus({
  selectedProvider,
});

const {
  httpTransportDraft,
  isSavingPublicCheckSources,
  isSavingUpdateInterval,
  isTestingPublicCheckSources,
  openPublicCheckDialog,
  openUpdateIntervalDialog,
  publicCheckDraft,
  publicCheckTestResults,
  publicDnsProviderDraft,
  restorePublicCheckDefaults,
  savePublicCheckSources,
  saveUpdateInterval,
  showPublicCheckDialog,
  showUpdateIntervalDialog,
  testPublicCheckSources,
  updateIntervalDraft,
} = useDDNSSettingsDialogs({
  defaultPublicCheckSources,
  httpTransport,
  providerConfig,
  publicCheckSources,
  publicDnsProvider,
  updateIntervalMinutes,
});

const { isPending: isTesting, run: runTestUpdate } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.ddns.updateFailed"), {
      description: extractErrorMessage(error, t("admin.ddns.updateFailed")),
    });
  },
});
const { isPending: isSavingTarget, run: runSaveTarget } = useAsyncAction({
  rethrow: true,
  onError: (error) => {
    toast.error(t("admin.ddns.saveTargetFailed"), {
      description: extractErrorMessage(error, t("admin.ddns.saveTargetFailed")),
    });
  },
});
const { run: runDeleteTarget } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.ddns.deleteTargetFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.ddns.deleteTargetFailed"),
      ),
    });
  },
});
const { run: runToggleTarget } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.ddns.toggleTargetFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.ddns.toggleTargetFailed"),
      ),
    });
  },
});
const { run: runTestTarget } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.ddns.testTargetFailed"), {
      description: extractErrorMessage(error, t("admin.ddns.testTargetFailed")),
    });
  },
});

const {
  enableFieldEditing,
  ensurePasswordFieldsVisible,
  fieldVisibility,
  getFieldAutocomplete,
  getFieldDomId,
  getFieldInputName,
  isFieldEditReady,
  isTargetFieldVisible,
  resetFieldEditReady,
  resetTargetFieldVisibility,
  toggleFieldVisibility,
  toggleTargetFieldVisibility,
} = useDDNSFieldState({
  selectedProvider,
  targetDialogState,
});

const extraTargets = computed(() =>
  targetSummaries.value.filter((target) => !target.isPrimary),
);

const {
  configuredNetworkInterface,
  configuredNetworkInterfaceLabel,
  currentIpSourceLabel,
  currentNetworkInterfaceLabel,
  currentUpdateScopeLabel,
  effectiveIpSource,
  effectiveUpdateScope,
  formatAddressOptionLabel,
  formatOptionLabel,
  interfaceIPv4Options,
  interfaceIPv6Options,
  isProviderIpSourceOptionDisabled,
  isProviderUpdateScopeOptionDisabled,
  resolvedNetworkInterfaces,
  selectedNetworkInterfaceDetail,
  shouldShowInterfaceAddressBlock,
  shouldShowSourceDomainBlock,
  showIPv4Status,
  showIPv6Status,
  showInterfaceIPv4Select,
  showInterfaceIPv6Select,
  showStaticIPv4Input,
  showStaticIPv6Input,
  updateConfiguredIpSource,
  updateConfiguredNetworkInterface,
} = useDDNSAddressSourceState({
  networkInterfaces,
  providerConfig,
  providers,
  selectedProvider,
  statusIpSource,
  statusNetworkInterface,
  statusUpdateScope,
  translate: (key, params) => (params ? t(key, params) : t(key)),
});

const {
  targetDialogDescription,
  targetDialogIPv4Options,
  targetDialogIPv6Options,
  targetDialogNetworkInterfaceLabel,
  targetDialogProviderDef,
  targetDialogResolvedNetworkInterfaces,
  targetDialogShouldShowDomainBlock,
  targetDialogShouldShowInterfaceBlock,
  targetDialogShouldShowStaticBlock,
  targetDialogTitle,
  targetDialogUpdateScope,
} = useDDNSTargetDialogState({
  formatAddressOptionLabel,
  mode: targetDialogMode,
  networkInterfaces,
  providers,
  state: targetDialogState,
  translate: (key, params) => (params ? t(key, params) : t(key)),
});

const {
  currentProviderDef,
  hasProviderConfig,
  hasSavedProviderConfig,
  isPrimaryConfigDirty,
  onProviderChange,
  setProviderConfigField,
} = useDDNSPrimaryConfigState({
  loadConfig: () => loadConfig(),
  providerConfig,
  providers,
  savedProvider,
  savedProviderConfig,
  selectedProvider,
  translate: (key) => t(key),
});
const {
  applyCredentialTransfer,
  credentialTransferDescription,
  credentialTransferSuggestion,
  isTransferSourceLoading,
  transferSourceScopeLabel,
} = useDDNSCredentialTransferHint({
  enableFieldEditing,
  providerConfig,
  selectedProvider,
  translate: (key, params) => (params ? t(key, params) : t(key)),
});
const {
  copyIpAddress,
  getTargetLastCheckTooltipLines,
  lastCheckTooltipLines,
  updateIntervalLabel,
} = useDDNSStatusPresentation({
  lastCheck,
  lastIP,
  locale,
  translate: (key, params) => (params ? t(key, params) : t(key)),
  updateIntervalMinutes,
});
const { initialize, isLoading, loadConfig, loadStatus } =
  useDDNSResourceLoading({
    applyStatus,
    currentProviderDef,
    ensurePasswordFieldsVisible,
    isInitialized,
    isPrimaryConfigDirty,
    networkInterfaces,
    providerConfig,
    providers,
    resetFieldEditReady,
    savedProviderConfig,
    selectedProvider,
  });
const {
  isClearingLogs,
  isTogglingEnabled,
  logLines,
  logs,
  onClearLogs,
  refresh: refreshPolling,
  start: startPolling,
  stop: stopPolling,
} = useDDNSPolling({
  applyStatus,
  enabled,
  isPrimaryConfigDirty,
});
const isEnabledSwitchDisabled = computed(
  () => isTogglingEnabled.value || isLoading.value,
);
const isSubdomainMode = computed(() =>
  isAnySubdomainRoutingMode(configStore.config),
);
const targetDialogConfig = computed({
  get: () => targetDialogState.value.config,
  set: (config: Record<string, string>) => {
    targetDialogState.value = { ...targetDialogState.value, config };
  },
});
const targetDialogProviderName = computed(
  () => targetDialogState.value.provider,
);
const {
  formatOnBlur: formatPrimaryDomainOnBlur,
  getFieldDescription: getPrimaryFieldDescription,
  normalizeForSubmit: normalizePrimaryDomainForSubmit,
} = useDDNSDomainField({
  config: providerConfig,
  includeWildcardHint: isSubdomainMode,
  providerName: selectedProvider,
  providers,
  translate: (key, params) => (params ? t(key, params) : t(key)),
});
const {
  formatOnBlur: formatTargetDomainOnBlur,
  getFieldDescription: getTargetFieldDescription,
  normalizeForSubmit: normalizeTargetDomainForSubmit,
} = useDDNSDomainField({
  config: targetDialogConfig,
  includeWildcardHint: isSubdomainMode,
  providerName: targetDialogProviderName,
  providers,
  translate: (key, params) => (params ? t(key, params) : t(key)),
});
const showValidationIssue = (issue: DDNSValidationIssue | null) => {
  if (!issue) {
    return false;
  }

  const message = issue.messageParams
    ? t(issue.messageKey, issue.messageParams)
    : t(issue.messageKey);

  if (issue.descriptionKey) {
    const description = issue.descriptionParams
      ? t(issue.descriptionKey, issue.descriptionParams)
      : t(issue.descriptionKey);
    toast.error(message, { description });
  } else {
    toast.error(message);
  }

  return true;
};

const {
  handleTargetDialogProviderChange,
  onDeleteExtraTarget,
  onTestExtraTarget,
  onToggleExtraTarget,
  openCreateTargetDialog,
  openEditTargetDialog,
  saveTargetDialog,
  updateTargetDialogNetworkInterface,
} = useDDNSTargetActions({
  api: {
    createTarget: (payload) => DDNSAPI.createTarget(payload),
    deleteTarget: (targetId) => DDNSAPI.deleteTarget(targetId),
    getTarget: (targetId) => DDNSAPI.getTarget(targetId),
    setTargetEnabled: (targetId, nextEnabled) =>
      DDNSAPI.setTargetEnabled(targetId, nextEnabled),
    testTarget: (targetId) => DDNSAPI.testTarget(targetId),
    updateTarget: (targetId, payload) =>
      DDNSAPI.updateTarget(targetId, payload),
  },
  deletingTargetId,
  loadStatus,
  providerConfig,
  providers,
  refreshPolling,
  resetTargetFieldVisibility,
  runDeleteTarget,
  runSaveTarget,
  runTestTarget,
  runToggleTarget,
  selectedProvider,
  showTargetDialog,
  showValidationIssue,
  targetDialogIPv4Options,
  targetDialogIPv6Options,
  targetDialogMode,
  targetDialogProviderDef,
  targetDialogState,
  targetDialogUpdateScope,
  testingTargetId,
  togglingTargetId,
  translate: (key, params) => (params ? t(key, params) : t(key)),
  normalizeDomainForSubmit: normalizeTargetDomainForSubmit,
});

function validateCommonConfig() {
  const issue = validateDDNSCommonConfig({
    config: providerConfig.value,
    ipSource: effectiveIpSource.value,
    ipv4Options: interfaceIPv4Options.value,
    ipv6Options: interfaceIPv6Options.value,
    providerName: selectedProvider.value,
    providers: providers.value,
    updateScope: effectiveUpdateScope.value,
  });

  return !showValidationIssue(issue);
}

const {
  confirmClearPrimaryConfig,
  isClearingPrimaryConfig,
  isSaving,
  onCancelPrimaryConfigEdit,
  onSaveConfig,
  onSaveConfigSilent,
  openClearPrimaryConfigDialog,
  showClearPrimaryConfigDialog,
} = useDDNSPrimaryConfigActions({
  loadConfig,
  loadStatus,
  normalizeForSubmit: normalizePrimaryDomainForSubmit,
  providerConfig,
  refreshPolling,
  resetFieldEditReady,
  savedProvider,
  savedProviderConfig,
  selectedProvider,
  translate: (key) => t(key),
  validate: validateCommonConfig,
});
const isProviderSelectDisabled = computed(
  () => isSaving.value || isTesting.value || isLoading.value,
);

async function onTest() {
  await runTestUpdate(async () => {
    const saved = await onSaveConfigSilent();
    if (!saved) {
      return;
    }
    const result = await DDNSAPI.test();
    if (result.success) {
      toast.success(t("admin.ddns.updateSuccess"));
      await loadStatus();
      return;
    }
    toast.error(t("admin.ddns.updateFailed"), { description: result.message });
  });
}

const confirmDiscardUnsavedPrimaryConfig = () =>
  !isPrimaryConfigDirty.value ||
  window.confirm(t("admin.ddns.unsavedLeaveConfirm"));

const handleBeforeUnload = (event: BeforeUnloadEvent) => {
  if (!isPrimaryConfigDirty.value) return;
  event.preventDefault();
  event.returnValue = "";
};

onBeforeRouteLeave(() => confirmDiscardUnsavedPrimaryConfig());

onMounted(async () => {
  window.addEventListener("beforeunload", handleBeforeUnload);
  const initialized = await initialize();
  if (initialized) {
    startPolling();
  }
});
onUnmounted(() => {
  window.removeEventListener("beforeunload", handleBeforeUnload);
  stopPolling();
});
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
        <Switch v-model="enabled" :disabled="isEnabledSwitchDisabled" />
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
  </div>

  <div v-else class="flex h-full items-center justify-center min-h-[400px]">
    <div
      class="animate-spin rounded-full h-8 w-8 border-b-2 border-primary"
    ></div>
  </div>
</template>
