import { ref, onMounted, onUnmounted, computed } from "vue";
import { useI18n } from "vue-i18n";
import { onBeforeRouteLeave } from "vue-router";
import { DDNSAPI, type DDNSNetworkInterfacePayload } from "@/lib/api/ddns";
import { useConfirmationDialog } from "@admin-shared/composables/useConfirmationDialog";
import { toast } from "@admin-shared/utils/toast";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useConfigStore } from "../../store/config";
import { isAnySubdomainRoutingMode } from "../../lib/reverse-proxy-submode";
import {
  normalizeTargetConfigValues,
  validateDDNSCommonConfig,
  type DDNSValidationIssue,
  type Provider,
  type TargetDialogState,
} from "./model";
import { useDDNSFieldState } from "./useDDNSFieldState";
import { useDDNSDomainField } from "./useDDNSDomainField";
import { useDDNSAddressSourceState } from "./useDDNSAddressSourceState";
import { useDDNSTargetActions } from "./useDDNSTargetActions";
import { useDDNSTargetDialogState } from "./useDDNSTargetDialogState";
import { useDDNSPrimaryConfigActions } from "./useDDNSPrimaryConfigActions";
import { useDDNSPolling } from "./useDDNSPolling";
import { useDDNSResourceLoading } from "./useDDNSResourceLoading";
import { useDDNSSettingsDialogs } from "./useDDNSSettingsDialogs";
import { useDDNSStatus } from "./useDDNSStatus";
import { useDDNSCredentialTransferHint } from "./useDDNSCredentialTransferHint";
import { useDDNSPrimaryConfigState } from "./useDDNSPrimaryConfigState";
import { useDDNSStatusPresentation } from "./useDDNSStatusPresentation";

export const useDDNSManagementPage = () => {
  const { t, locale } = useI18n();
  let isDisposed = false;
  const {
    confirmationDialogOpen,
    confirmationDialogOptions,
    confirmPendingAction,
    handleConfirmationDialogOpenChange,
    requestConfirmation,
  } = useConfirmationDialog();

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
    confirmProviderChange: () =>
      requestConfirmation({
        confirmVariant: "destructive",
        description: t("admin.ddns.unsavedSwitchProviderConfirm"),
        title: t("common.confirm"),
      }),
    loadConfig: () => loadConfig(),
    providerConfig,
    providers,
    savedProvider,
    savedProviderConfig,
    selectedProvider,
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
    requestConfirmation({
      confirmVariant: "destructive",
      description: t("admin.ddns.unsavedLeaveConfirm"),
      title: t("common.confirm"),
    });

  const handleBeforeUnload = (event: BeforeUnloadEvent) => {
    if (!isPrimaryConfigDirty.value) return;
    event.preventDefault();
    event.returnValue = "";
  };

  onBeforeRouteLeave(() => confirmDiscardUnsavedPrimaryConfig());

  onMounted(async () => {
    window.addEventListener("beforeunload", handleBeforeUnload);
    const initialized = await initialize();
    if (initialized && !isDisposed) {
      startPolling();
    }
  });
  onUnmounted(() => {
    isDisposed = true;
    window.removeEventListener("beforeunload", handleBeforeUnload);
    stopPolling();
  });

  return {
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
  };
};

export type DDNSManagementPageController = ReturnType<
  typeof useDDNSManagementPage
>;
