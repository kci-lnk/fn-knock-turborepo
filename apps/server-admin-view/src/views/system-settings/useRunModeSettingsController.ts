import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "@admin-shared/utils/toast";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useConfigStore } from "@/store/config";
import {
  CloudflaredAPI,
  FrpcAPI,
  SystemAPI,
  type AccessEntryInfo,
  type RunModePromptPreferences,
} from "@/lib/api";
import {
  DEFAULT_REVERSE_PROXY_SUBMODE,
  resolveReverseProxySubmode,
} from "@/lib/reverse-proxy-submode";
import type { ReverseProxySubmode } from "@/types";
import { useRunModeMessages } from "./useRunModeMessages";
import { useRunModePromptConfirmation } from "./useRunModePromptConfirmation";
import { useFirewallAdditionalPorts } from "./useFirewallAdditionalPorts";

export const useRunModeSettingsController = () => {
  const configStore = useConfigStore();
  const { t } = useI18n();
  const mode = ref<0 | 1 | 3>(1);
  const autoManageFirewall = ref(true);
  const reverseProxySubmode = ref<ReverseProxySubmode>(
    DEFAULT_REVERSE_PROXY_SUBMODE,
  );
  const accessEntry = ref<AccessEntryInfo>({
    port: "7999",
    env: "GO_REPROXY_PORT",
    isDefault: true,
  });
  const {
    closeConfirmation,
    confirm: confirmRunModeChange,
    dontShowAgainChecked,
    handleConfirmDialogOpenChange,
    isConfirmDialogOpen,
    loadRunModePromptPreferences,
    pendingPromptKey,
    pendingSubmode,
    queueConfirmation,
    runModePromptPreferences,
  } = useRunModePromptConfirmation();

  const { isPending: isSaving, run: runSaveMode } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.runModeSettings.saveFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.runModeSettings.operationFailed"),
        ),
      });
    },
  });
  const { isPending: isFirewallActionPending, run: runFirewallAction } =
    useAsyncAction({
      onError: (error) => {
        toast.error(t("admin.runModeSettings.firewallActionFailed"), {
          description: extractErrorMessage(
            error,
            t("admin.runModeSettings.operationFailed"),
          ),
        });
      },
    });
  const {
    isPending: isAutoManageFirewallPending,
    run: runAutoManageFirewallUpdate,
  } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.runModeSettings.autoFirewallUpdateFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.runModeSettings.operationFailed"),
        ),
      });
    },
  });

  const canUseDirectMode = computed(() => configStore.canUseDirectMode);
  const canManageHostFirewall = computed(
    () => configStore.canManageHostFirewall,
  );
  const isDockerDeployment = computed(() => configStore.isDockerDeployment);
  const isFpkLiteDeployment = computed(() => configStore.isFpkLiteDeployment);
  const showHostFirewallUnavailableAlert = computed(
    () =>
      !canManageHostFirewall.value &&
      !configStore.isDockerDeployment &&
      !configStore.isFpkLiteDeployment,
  );
  const hostFirewallUnavailableDescription = computed(() =>
    configStore.isDockerDeployment
      ? t("admin.runModeSettings.hostFirewallUnavailableDockerDescription")
      : t("admin.runModeSettings.hostFirewallUnavailableDescription"),
  );
  const savedReverseProxySubmode = computed(() =>
    resolveReverseProxySubmode(configStore.config),
  );
  const {
    buildFirewallResetSuccessDescription,
    buildRunModeChangeSuccessDescription,
    buildUnsavedModeNotice,
    confirmDialogContent,
    formatInlineList,
    getRunModeLabel,
  } = useRunModeMessages({
    mode,
    reverseProxySubmode,
    savedReverseProxySubmode,
    accessEntry,
    pendingPromptKey,
    pendingSubmode,
  });
  const isModeUnchanged = computed(() => {
    const currentMode = configStore.config?.run_type;
    if (currentMode === undefined) return true;
    if (currentMode !== mode.value) return false;
    if (mode.value !== 1) return true;
    return savedReverseProxySubmode.value === reverseProxySubmode.value;
  });
  const {
    autoManageFirewallEnabled: firewallAdditionalPortsAutoManageEnabled,
    details: firewallAdditionalPortsDetails,
    hasUnsavedModeChanges: hasUnsavedFirewallModeChanges,
    load: loadFirewallAdditionalPorts,
    loadFailed: firewallAdditionalPortsLoadFailed,
    loading: isFirewallAdditionalPortsLoading,
    modeLabel: firewallAdditionalPortsModeLabel,
    open: isFirewallAdditionalPortsDialogOpen,
    openDialog: openFirewallAdditionalPortsDialog,
    save: saveFirewallAdditionalPorts,
    saving: isFirewallAdditionalPortsSaving,
    updateOpen: handleFirewallAdditionalPortsDialogOpenChange,
  } = useFirewallAdditionalPorts({
    canManageHostFirewall: () => canManageHostFirewall.value,
    hasUnsavedModeChanges: () => !isModeUnchanged.value,
  });
  const isBusy = computed(
    () =>
      isSaving.value ||
      isFirewallActionPending.value ||
      isAutoManageFirewallPending.value ||
      isFirewallAdditionalPortsLoading.value ||
      isFirewallAdditionalPortsSaving.value,
  );
  const selectedReverseProxySubmodeLabel = computed(() =>
    reverseProxySubmode.value === "subdomain"
      ? t("admin.runModeSettings.subdomainMapping")
      : t("admin.runModeSettings.pathMapping"),
  );
  const accessAlertTitle = computed(() => {
    if (mode.value === 0) return t("admin.runModeSettings.directAccessTitle");
    if (mode.value === 1) {
      return t("admin.runModeSettings.reverseAccessTitle", {
        submode: selectedReverseProxySubmodeLabel.value,
      });
    }
    return t("admin.runModeSettings.subdomainAccessTitle");
  });
  const accessAlertDescription = computed(() => {
    const port = accessEntry.value.port;
    if (mode.value === 0) {
      return t("admin.runModeSettings.directAccessDescription", { port });
    }
    if (mode.value === 1) {
      return reverseProxySubmode.value === "subdomain"
        ? t("admin.runModeSettings.reverseSubdomainAccessDescription", { port })
        : t("admin.runModeSettings.reversePathAccessDescription", { port });
    }
    return t("admin.runModeSettings.subdomainAccessDescription", { port });
  });

  const reset = () => {
    if (configStore.config) {
      mode.value = configStore.config.run_type;
      reverseProxySubmode.value = savedReverseProxySubmode.value;
    }
  };
  const selectReverseProxyMode = () => {
    if (mode.value !== 1) reverseProxySubmode.value = "subdomain";
    mode.value = 1;
  };
  const handleAutoManageFirewallChange = async (
    value: boolean | "indeterminate",
  ) => {
    if (!canManageHostFirewall.value || isBusy.value) return;
    const nextValue = value === true;
    const previousValue = autoManageFirewall.value;
    if (nextValue === previousValue) return;
    autoManageFirewall.value = nextValue;
    await runAutoManageFirewallUpdate(async () => {
      try {
        const next = await configStore.saveAutoManageFirewall(nextValue);
        autoManageFirewall.value = next.auto_manage_firewall;
        toast.success(
          next.auto_manage_firewall
            ? t("admin.runModeSettings.autoFirewallEnabled")
            : t("admin.runModeSettings.autoFirewallDisabled"),
          {
            description: next.auto_manage_firewall
              ? t("admin.runModeSettings.autoFirewallEnabledDescription")
              : t("admin.runModeSettings.autoFirewallDisabledDescription"),
          },
        );
      } catch (error) {
        autoManageFirewall.value = previousValue;
        throw error;
      }
    });
  };

  const ensureTunnelsStoppedForTargetMode = async (
    nextMode: 0 | 1 | 3,
    nextSubmode: ReverseProxySubmode | null,
  ) => {
    const [frpcStatus, cloudflaredStatus] = await Promise.all([
      FrpcAPI.getStatus(),
      CloudflaredAPI.getStatus(),
    ]);
    const runningTunnels = [
      frpcStatus.running
        ? { key: "frp" as const, label: "FRP", stop: () => FrpcAPI.stop() }
        : null,
      cloudflaredStatus.running
        ? {
            key: "cloudflared" as const,
            label: "Cloudflared",
            stop: () => CloudflaredAPI.stop(),
          }
        : null,
    ].filter(
      (
        item,
      ): item is {
        key: "frp" | "cloudflared";
        label: string;
        stop: () => Promise<void>;
      } => item !== null,
    );
    const tunnelsToStop = nextMode === 1 ? [] : runningTunnels;
    if (tunnelsToStop.length === 0) return;
    await Promise.all(tunnelsToStop.map((item) => item.stop()));
    toast.success(t("admin.runModeSettings.tunnelsStopped"), {
      description: t("admin.runModeSettings.tunnelsStoppedDescription", {
        names: formatInlineList(tunnelsToStop.map((item) => item.label)),
        mode: getRunModeLabel(nextMode, nextSubmode ?? undefined),
      }),
    });
  };
  const applyRunModeChange = async (
    nextMode: 0 | 1 | 3,
    nextSubmode: ReverseProxySubmode | null,
    options?: {
      promptPreferenceKey?: keyof RunModePromptPreferences | null;
      disablePrompt?: boolean;
      onSuccess?: () => void;
    },
  ) => {
    await runSaveMode(async () => {
      const successDescription = buildRunModeChangeSuccessDescription(
        nextMode,
        nextSubmode,
      );
      await ensureTunnelsStoppedForTargetMode(nextMode, nextSubmode);
      if (options?.promptPreferenceKey && options.disablePrompt) {
        runModePromptPreferences.value =
          await SystemAPI.updateRunModePromptPreferences({
            [options.promptPreferenceKey]: true,
          });
      }
      const warning = await configStore.setRunType(
        nextMode,
        nextSubmode ?? undefined,
      );
      options?.onSuccess?.();
      const notify = warning ? toast.warning : toast.success;
      notify(t("admin.runModeSettings.updated"), {
        description: warning ?? successDescription,
      });
    });
  };
  const confirmSave = () => confirmRunModeChange(applyRunModeChange);
  const save = async () => {
    if (mode.value === 0 && !canUseDirectMode.value) {
      toast.error(t("admin.runModeSettings.directUnsupportedTitle"), {
        description: t("admin.runModeSettings.directUnsupportedDescription"),
      });
      return;
    }
    const currentMode = configStore.config?.run_type;
    const currentSubmode = savedReverseProxySubmode.value;
    if (
      currentMode === undefined ||
      (currentMode === mode.value &&
        (mode.value !== 1 || currentSubmode === reverseProxySubmode.value))
    ) {
      return;
    }
    if (
      queueConfirmation({
        currentMode,
        nextMode: mode.value,
        nextSubmode: reverseProxySubmode.value,
      })
    ) {
      return;
    }
    await applyRunModeChange(
      mode.value,
      mode.value === 1 ? reverseProxySubmode.value : null,
    );
  };

  const resetFirewallBySelectedMode = async () => {
    if (!canManageHostFirewall.value) {
      toast.error(t("admin.runModeSettings.firewallUnsupportedTitle"), {
        description: hostFirewallUnavailableDescription.value,
      });
      return;
    }
    await runFirewallAction(async () => {
      const result = await SystemAPI.resetFirewallByRunType(mode.value);
      toast.success(t("admin.runModeSettings.firewallReset"), {
        description: `${buildFirewallResetSuccessDescription(
          result,
          mode.value === 1 ? reverseProxySubmode.value : null,
        )}${buildUnsavedModeNotice()}`,
      });
    });
  };
  const clearFirewallRules = async () => {
    if (!canManageHostFirewall.value) {
      toast.error(t("admin.runModeSettings.firewallUnsupportedTitle"), {
        description: hostFirewallUnavailableDescription.value,
      });
      return;
    }
    await runFirewallAction(async () => {
      const result = await SystemAPI.clearFirewall();
      toast.success(t("admin.runModeSettings.firewallCleared"), {
        description: t("admin.runModeSettings.firewallClearedDescription", {
          port: result.gatewayPort,
        }),
      });
    });
  };
  const loadAccessEntry = async () => {
    try {
      accessEntry.value = await SystemAPI.getAccessEntry();
    } catch (error) {
      console.warn("load access entry failed:", error);
    }
  };

  onMounted(() => {
    if (configStore.config) {
      mode.value = configStore.config.run_type;
      autoManageFirewall.value =
        configStore.config.auto_manage_firewall !== false;
      reverseProxySubmode.value = savedReverseProxySubmode.value;
    }
    void loadAccessEntry();
    void loadRunModePromptPreferences();
  });
  watch(
    () => ({
      runType: configStore.config?.run_type,
      submode: configStore.config?.reverse_proxy_submode,
      autoManageFirewall: configStore.config?.auto_manage_firewall,
    }),
    (
      {
        runType: nextMode,
        submode: nextSubmode,
        autoManageFirewall: nextAutoManageFirewall,
      },
      previousState,
    ) => {
      const shouldSyncRunMode =
        nextMode !== undefined &&
        (nextMode !== previousState?.runType ||
          nextSubmode !== previousState?.submode);
      if (shouldSyncRunMode) {
        mode.value = nextMode;
        reverseProxySubmode.value = savedReverseProxySubmode.value;
      }
      autoManageFirewall.value = nextAutoManageFirewall !== false;
      if (!canUseDirectMode.value && mode.value === 0) {
        mode.value = nextMode === 0 ? 1 : (nextMode ?? 1);
      }
    },
  );

  return {
    accessAlertDescription,
    accessAlertTitle,
    autoManageFirewall,
    canManageHostFirewall,
    canUseDirectMode,
    clearFirewallRules,
    closeConfirmation,
    confirmDialogContent,
    confirmSave,
    dontShowAgainChecked,
    firewallAdditionalPortsAutoManageEnabled,
    handleAutoManageFirewallChange,
    handleConfirmDialogOpenChange,
    handleFirewallAdditionalPortsDialogOpenChange,
    hasUnsavedFirewallModeChanges,
    hostFirewallUnavailableDescription,
    isBusy,
    isConfirmDialogOpen,
    isAutoManageFirewallPending,
    isDockerDeployment,
    isFpkLiteDeployment,
    isFirewallActionPending,
    isFirewallAdditionalPortsDialogOpen,
    isFirewallAdditionalPortsLoading,
    isFirewallAdditionalPortsSaving,
    isModeUnchanged,
    isSaving,
    mode,
    firewallAdditionalPortsDetails,
    firewallAdditionalPortsLoadFailed,
    firewallAdditionalPortsModeLabel,
    loadFirewallAdditionalPorts,
    openFirewallAdditionalPortsDialog,
    reset,
    resetFirewallBySelectedMode,
    reverseProxySubmode,
    save,
    saveFirewallAdditionalPorts,
    selectReverseProxyMode,
    selectedReverseProxySubmodeLabel,
    showHostFirewallUnavailableAlert,
    t,
  };
};
