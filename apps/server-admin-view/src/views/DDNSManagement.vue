<script setup lang="ts">
import { ref, watch, onMounted, onUnmounted, computed } from "vue";
import { useI18n } from "vue-i18n";
import { onBeforeRouteLeave } from "vue-router";
import { Settings2 } from "lucide-vue-next";
import { DDNSAPI } from "../lib/api";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import DocsLinkButton from "@/components/DocsLinkButton.vue";
import { toast } from "@admin-shared/utils/toast";
import { copyTextToClipboard } from "@admin-shared/utils/copyTextToClipboard";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { formatDateTimeSafe } from "@admin-shared/utils/formatDateTimeSafe";
import {
  DEFAULT_LOG_WINDOW_SIZE,
  mergePollingLogWindow,
} from "@admin-shared/utils/log-window";
import { useDnsCredentialTransfer } from "@/composables/useDnsCredentialTransfer";
import { useTargetPolling } from "../composables/useTargetPolling";
import type {
  DDNSHttpTransport,
  DDNSNetworkInterfacePayload,
  DDNSPublicCheckSourcesPayload,
  DDNSPublicCheckTestResultPayload,
  DDNSTargetSummaryPayload,
} from "../lib/api";
import { useConfigStore } from "../store/config";
import { isAnySubdomainRoutingMode } from "../lib/reverse-proxy-submode";
import { docsUrls } from "../lib/docs";
import { buildDDNSTimestampTooltipLines } from "../lib/ddns-time";
import {
  DEFAULT_DDNS_HTTP_TRANSPORT,
  DEFAULT_DDNS_UPDATE_INTERVAL_MINUTES,
  INTERFACE_IPV4_INDEX_KEY,
  INTERFACE_IPV6_INDEX_KEY,
  IP_SOURCE_KEY,
  MAX_DDNS_UPDATE_INTERVAL_MINUTES,
  MIN_DDNS_UPDATE_INTERVAL_MINUTES,
  NETWORK_INTERFACE_KEY,
  SOURCE_DOMAIN_KEY,
  STATIC_IPV4_KEY,
  STATIC_IPV6_KEY,
  UPDATE_SCOPE_KEY,
  findProviderDef,
  normalizeInterfaceAddressIndex,
  normalizeDDNSHttpTransport,
  normalizeIpSource,
  normalizeNetworkInterface,
  normalizePublicCheckSources,
  normalizeSourceDomain,
  normalizeStaticIPAddress,
  normalizeTargetConfigValues,
  normalizeUpdateIntervalMinutes,
  normalizeUpdateScope,
  parseUpdateIntervalDraft,
  validateDDNSCommonConfig,
  type DDNSValidationIssue,
  type LogEntry,
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
import { useDDNSStatus } from "./ddns-management/useDDNSStatus";

const { t, locale } = useI18n();

// ─── State ─────────────────────────────────────────────────────
const isInitialized = ref(false);
const configStore = useConfigStore();
const selectedProvider = ref<string>("");
const providers = ref<Provider[]>([]);
const providerConfig = ref<Record<string, string>>({});
const savedProviderConfig = ref<Record<string, string>>({});
const updateIntervalDraft = ref(String(DEFAULT_DDNS_UPDATE_INTERVAL_MINUTES));
const publicCheckDraft = ref<DDNSPublicCheckSourcesPayload>(
  normalizePublicCheckSources(undefined),
);
const httpTransportDraft = ref<DDNSHttpTransport>(DEFAULT_DDNS_HTTP_TRANSPORT);
const publicCheckTestResults = ref<DDNSPublicCheckTestResultPayload[]>([]);
const logs = ref<LogEntry[]>([]);
const networkInterfaces = ref<DDNSNetworkInterfacePayload[]>([]);
const showTargetDialog = ref(false);
const showUpdateIntervalDialog = ref(false);
const showPublicCheckDialog = ref(false);
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
  lastCheck,
  lastIP,
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

const { isPending: isTesting, run: runTestUpdate } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.ddns.updateFailed"), {
      description: extractErrorMessage(error, t("admin.ddns.updateFailed")),
    });
  },
});
const { isPending: isClearingLogs, run: runClearLogs } = useAsyncAction({
  onError: () => {
    toast.error(t("admin.ddns.clearLogsFailed"));
  },
});
const { isPending: isTogglingEnabled, run: runToggleEnabled } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.ddns.toggleFailed"), {
      description: extractErrorMessage(error, t("admin.ddns.toggleFailed")),
    });
  },
});
const { run: runLoadStatus } = useAsyncAction({
  onError: (error) => {
    console.error(
      "loadStatus:",
      extractErrorMessage(error, t("admin.ddns.loadStatusFailed")),
    );
  },
});
const { run: runLoadProviders } = useAsyncAction({
  onError: (error) => {
    console.error(
      "loadProviders:",
      extractErrorMessage(error, t("admin.ddns.loadProvidersFailed")),
    );
  },
});
const { run: runLoadNetworkInterfaces } = useAsyncAction({
  onError: (error) => {
    console.error(
      "loadNetworkInterfaces:",
      extractErrorMessage(error, t("admin.ddns.loadInterfacesFailed")),
    );
  },
});
const { run: runLoadConfig } = useAsyncAction({
  onError: (error) => {
    console.error(
      "loadConfig:",
      extractErrorMessage(error, t("admin.ddns.loadConfigFailed")),
    );
  },
});
const { isPending: isLoading, run: runInitialize } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.ddns.initFailed"), {
      description: extractErrorMessage(error, t("admin.ddns.initLoadFailed")),
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
const { isPending: isSavingUpdateInterval, run: runSaveUpdateInterval } =
  useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.ddns.saveIntervalFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.ddns.saveIntervalFailed"),
        ),
      });
    },
  });
const {
  isPending: isSavingPublicCheckSources,
  run: runSavePublicCheckSources,
} = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.ddns.savePublicCheckFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.ddns.savePublicCheckFailed"),
      ),
    });
  },
});
const {
  isPending: isTestingPublicCheckSources,
  run: runTestPublicCheckSources,
} = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.ddns.testPublicCheckSourcesFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.ddns.testPublicCheckSourcesFailed"),
      ),
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

const currentProviderDef = computed(() => {
  return findProviderDef(providers.value, selectedProvider.value);
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

const getTargetLastCheckTooltipLines = (target: DDNSTargetSummaryPayload) =>
  buildDDNSTimestampTooltipLines({
    updatedAt: target.lastIP.updated_at,
    checkedAt: target.lastCheck.checked_at,
    locale: String(locale.value),
    labels: {
      lastSuccessfulUpdate: t("admin.ddns.lastSuccessfulUpdate"),
      lastCheck: t("admin.ddns.lastCheck"),
      never: t("admin.ddns.never"),
    },
  });

const {
  applySuggestion: applyTransferredCredentials,
  isLoadingSource: isTransferSourceLoading,
  sourceScopeLabel: transferSourceScopeLabel,
  suggestion: credentialTransferSuggestion,
} = useDnsCredentialTransfer({
  target: "ddns",
  providerId: selectedProvider,
  targetCredentials: providerConfig,
});

const credentialTransferDescription = computed(() => {
  const suggestion = credentialTransferSuggestion.value;
  if (!suggestion) return "";

  return t("admin.ddns.credentialTransferDescription", {
    scope: transferSourceScopeLabel.value,
    bridge: suggestion.bridgeLabel,
    count: suggestion.fillableFields.length,
  });
});

const hasProviderConfig = computed(() => {
  const def = currentProviderDef.value;
  if (!def) return false;
  return def.fields.some(
    (field) => providerConfig.value[field.key]?.toString().trim() !== "",
  );
});

const hasSavedProviderConfig = computed(() => {
  const def = currentProviderDef.value;
  if (!def) return false;
  return def.fields.some(
    (field) => savedProviderConfig.value[field.key]?.toString().trim() !== "",
  );
});
const normalizePrimaryProviderConfigForComparison = (
  config: Record<string, string>,
) => {
  const normalized = normalizeTargetConfigValues(config);
  return Object.keys(normalized)
    .sort()
    .reduce<Record<string, string>>((result, key) => {
      result[key] = String(normalized[key] ?? "");
      return result;
    }, {});
};
const isPrimaryConfigValueDirty = computed(() => {
  if (!selectedProvider.value) return false;
  return (
    JSON.stringify(
      normalizePrimaryProviderConfigForComparison(providerConfig.value),
    ) !==
    JSON.stringify(
      normalizePrimaryProviderConfigForComparison(savedProviderConfig.value),
    )
  );
});
const isPrimaryProviderDirty = computed(
  () => selectedProvider.value !== savedProvider.value,
);
const isPrimaryConfigDirty = computed(
  () => isPrimaryProviderDirty.value || isPrimaryConfigValueDirty.value,
);
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
const updateIntervalLabel = computed(() =>
  t("admin.ddns.updateIntervalLabel", {
    minutes: updateIntervalMinutes.value,
  }),
);

async function loadStatus() {
  await runLoadStatus(async () => {
    const status = await DDNSAPI.getStatus();
    applyStatus(status, {
      syncProvider: !isInitialized.value || !isPrimaryConfigDirty.value,
    });
  });
}

async function loadProviders() {
  await runLoadProviders(async () => {
    const data = await DDNSAPI.getProviders();
    providers.value = data.map((p) => ({
      ...p,
      fields: p.fields.map((f) => ({
        ...f,
        type: f.type as "text" | "password" | "select",
      })),
    }));
  });
}

async function loadNetworkInterfaces() {
  await runLoadNetworkInterfaces(async () => {
    networkInterfaces.value = await DDNSAPI.getNetworkInterfaces();
  });
}

async function loadConfig() {
  if (!selectedProvider.value) {
    providerConfig.value = {};
    savedProviderConfig.value = {};
    return;
  }
  await runLoadConfig(async () => {
    const config = await DDNSAPI.getConfig(selectedProvider.value);
    const def = currentProviderDef.value;
    const merged: Record<string, string> = {
      [UPDATE_SCOPE_KEY]: normalizeUpdateScope(config[UPDATE_SCOPE_KEY]),
      [IP_SOURCE_KEY]: normalizeIpSource(config[IP_SOURCE_KEY]),
      [NETWORK_INTERFACE_KEY]: normalizeNetworkInterface(
        config[NETWORK_INTERFACE_KEY],
      ),
      [INTERFACE_IPV4_INDEX_KEY]: normalizeInterfaceAddressIndex(
        config[INTERFACE_IPV4_INDEX_KEY],
      ),
      [INTERFACE_IPV6_INDEX_KEY]: normalizeInterfaceAddressIndex(
        config[INTERFACE_IPV6_INDEX_KEY],
      ),
      [STATIC_IPV4_KEY]: normalizeStaticIPAddress(config[STATIC_IPV4_KEY]),
      [STATIC_IPV6_KEY]: normalizeStaticIPAddress(config[STATIC_IPV6_KEY]),
      [SOURCE_DOMAIN_KEY]: normalizeSourceDomain(config[SOURCE_DOMAIN_KEY]),
    };

    resetFieldEditReady();

    if (def) {
      for (const f of def.fields) {
        const val = config[f.key] ?? "";
        merged[f.key] = val;
      }
      ensurePasswordFieldsVisible(def.fields);
    }
    providerConfig.value = merged;
    savedProviderConfig.value = { ...merged };
  });
}

const ddnsPolling = useTargetPolling({
  target: "ddns",
  intervalMs: 2000,
  onData: (payload) => {
    logs.value = mergePollingLogWindow(logs.value, payload.logs as LogEntry[], {
      reset: payload.reset,
      max: DEFAULT_LOG_WINDOW_SIZE,
    });

    const status = payload.status;
    applyStatus(status, {
      syncEnabled: false,
      syncProvider: !isPrimaryConfigDirty.value,
    });
    if (enabledInitialized && status.enabled !== enabled.value) {
      enabledInitialized = false;
      enabled.value = status.enabled;
      enabledInitialized = true;
    }
  },
  onError: (error) => {
    console.error(
      "ddns poll:",
      extractErrorMessage(error, t("admin.ddns.pollStatusFailed")),
    );
  },
});

let enabledInitialized = false;
watch(enabled, async (val) => {
  if (!enabledInitialized) return;
  await runToggleEnabled(() => DDNSAPI.toggle(val), {
    onSuccess: () => {
      toast.success(val ? t("admin.ddns.enabled") : t("admin.ddns.disabled"));
    },
    onError: () => {
      enabledInitialized = false;
      enabled.value = !val;
      enabledInitialized = true;
    },
  });
});

function openUpdateIntervalDialog() {
  updateIntervalDraft.value = String(updateIntervalMinutes.value);
  showUpdateIntervalDialog.value = true;
}

async function saveUpdateInterval() {
  const next = parseUpdateIntervalDraft(updateIntervalDraft.value);
  if (next === null) {
    toast.error(t("admin.ddns.intervalInvalid"), {
      description: t("admin.ddns.intervalInvalidDescription", {
        min: MIN_DDNS_UPDATE_INTERVAL_MINUTES,
        max: MAX_DDNS_UPDATE_INTERVAL_MINUTES,
      }),
    });
    return;
  }

  await runSaveUpdateInterval(
    () => DDNSAPI.saveSettings({ updateIntervalMinutes: next }),
    {
      onSuccess: (settings) => {
        updateIntervalMinutes.value = normalizeUpdateIntervalMinutes(
          settings.updateIntervalMinutes,
        );
        updateIntervalDraft.value = String(updateIntervalMinutes.value);
        showUpdateIntervalDialog.value = false;
        toast.success(t("admin.ddns.intervalSaved"));
      },
    },
  );
}

function openPublicCheckDialog() {
  publicCheckDraft.value = normalizePublicCheckSources(
    publicCheckSources.value,
  );
  httpTransportDraft.value = httpTransport.value;
  publicCheckTestResults.value = [];
  showPublicCheckDialog.value = true;
}

function restorePublicCheckDefaults() {
  publicCheckDraft.value = normalizePublicCheckSources(
    defaultPublicCheckSources.value,
  );
  publicCheckTestResults.value = [];
}

async function savePublicCheckSources(
  nextSources: DDNSPublicCheckSourcesPayload,
  nextHttpTransport: DDNSHttpTransport,
) {
  await runSavePublicCheckSources(
    () =>
      DDNSAPI.saveSettings({
        publicCheckSources: normalizePublicCheckSources(nextSources),
        httpTransport: normalizeDDNSHttpTransport(nextHttpTransport),
      }),
    {
      onSuccess: (settings) => {
        defaultPublicCheckSources.value = normalizePublicCheckSources(
          settings.defaultPublicCheckSources,
        );
        publicCheckSources.value = normalizePublicCheckSources(
          settings.publicCheckSources,
          defaultPublicCheckSources.value,
        );
        publicCheckDraft.value = normalizePublicCheckSources(
          settings.publicCheckSources,
          defaultPublicCheckSources.value,
        );
        httpTransport.value = normalizeDDNSHttpTransport(
          settings.httpTransport,
        );
        httpTransportDraft.value = httpTransport.value;
        publicCheckTestResults.value = [];
        showPublicCheckDialog.value = false;
        toast.success(t("admin.ddns.publicCheckSaved"));
      },
    },
  );
}

async function testPublicCheckSources(
  nextSources: DDNSPublicCheckSourcesPayload,
  nextHttpTransport: DDNSHttpTransport,
) {
  const sources = normalizePublicCheckSources(
    nextSources,
    defaultPublicCheckSources.value,
  );
  if (sources.ipv4.length === 0 && sources.ipv6.length === 0) {
    publicCheckTestResults.value = [];
    toast.error(t("admin.ddns.publicCheckNoTestSourcesConfigured"));
    return;
  }

  await runTestPublicCheckSources(
    () =>
      DDNSAPI.testPublicCheckSources(sources, {
        httpTransport: normalizeDDNSHttpTransport(nextHttpTransport),
        networkInterface: normalizeNetworkInterface(
          providerConfig.value[NETWORK_INTERFACE_KEY],
        ),
      }),
    {
      onSuccess: (payload) => {
        publicCheckTestResults.value = payload.results || [];
        if (publicCheckTestResults.value.length === 0) {
          toast.error(t("admin.ddns.publicCheckNoTestSourcesConfigured"));
          return;
        }
        const hasFailures = publicCheckTestResults.value.some(
          (item) => !item.success,
        );
        if (hasFailures) {
          toast.error(t("admin.ddns.publicCheckTestCompletedWithErrors"));
        } else {
          toast.success(t("admin.ddns.publicCheckTestCompleted"));
        }
      },
    },
  );
}

async function onProviderChange(val: string) {
  if (!val || val === selectedProvider.value) return;
  if (
    isPrimaryConfigValueDirty.value &&
    !window.confirm(t("admin.ddns.unsavedSwitchProviderConfirm"))
  ) {
    return;
  }
  selectedProvider.value = val;
  await loadConfig();
}

function setProviderConfigField(key: string, value: string) {
  providerConfig.value[key] = value;
}

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
  refreshPolling: () => {
    ddnsPolling.resetCursor();
    void ddnsPolling.refresh();
  },
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
  refreshPolling: () => {
    ddnsPolling.resetCursor();
    void ddnsPolling.refresh();
  },
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

function applyCredentialTransfer() {
  const result = applyTransferredCredentials();
  if (!result) return;

  for (const key of result.appliedKeys) {
    enableFieldEditing(key);
  }

  toast.success(
    t("admin.ddns.credentialsApplied", {
      scope: transferSourceScopeLabel.value,
      count: result.count,
    }),
  );
}

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

async function onClearLogs() {
  await runClearLogs(() => DDNSAPI.clearLogs(), {
    onSuccess: () => {
      logs.value = [];
      ddnsPolling.resetCursor();
      void ddnsPolling.refresh();
      toast.success(t("admin.ddns.logsCleared"));
    },
  });
}

function formatTime(iso: string | null): string {
  return formatDateTimeSafe(iso, {
    locale: String(locale.value),
    emptyText: t("admin.ddns.never"),
  });
}

const lastCheckTooltipLines = computed(() =>
  buildDDNSTimestampTooltipLines({
    updatedAt: lastIP.value.updated_at,
    checkedAt: lastCheck.value.checked_at,
    locale: String(locale.value),
    labels: {
      lastSuccessfulUpdate: t("admin.ddns.lastSuccessfulUpdate"),
      lastCheck: t("admin.ddns.lastCheck"),
      never: t("admin.ddns.never"),
    },
  }),
);

async function copyIpAddress(
  versionLabel: "IPv4" | "IPv6",
  value: string | null,
) {
  const address = value?.trim();
  if (!address) {
    toast.error(
      t("admin.ddns.copyUnavailable", {
        version: versionLabel,
      }),
    );
    return;
  }

  try {
    await copyTextToClipboard(address);
    toast.success(
      t("admin.ddns.copySuccess", {
        version: versionLabel,
      }),
      { description: address },
    );
  } catch (error) {
    console.error("copyIpAddress:", error);
    toast.error(
      t("admin.ddns.copyFailed", {
        version: versionLabel,
      }),
      {
        description: t("admin.ddns.copyFailedDescription"),
      },
    );
  }
}

const logLines = computed(() =>
  logs.value.map((e) => {
    const tag =
      e.level === "error"
        ? t("admin.ddns.logLevelError")
        : e.level === "warn"
          ? t("admin.ddns.logLevelWarn")
          : t("admin.ddns.logLevelInfo");
    return `${tag} ${formatTime(e.time)}  ${e.message}`;
  }),
);

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
  const initialized = await runInitialize(async () => {
    await Promise.all([loadProviders(), loadStatus(), loadNetworkInterfaces()]);
    enabledInitialized = true;
    await loadConfig();
    return true;
  });
  isInitialized.value = true;
  if (initialized) {
    ddnsPolling.start();
  }
});
onUnmounted(() => {
  window.removeEventListener("beforeunload", handleBeforeUnload);
  ddnsPolling.stop();
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
