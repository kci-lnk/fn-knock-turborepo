import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { toast } from "@admin-shared/utils/toast";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import {
  applyDateTimeDisplayMode,
  normalizeDateTimeDisplayMode,
} from "@admin-shared/composables/useDateTimeDisplayState";
import { ConfigAPI } from "@/lib/api/config";
import { SSHSecurityAPI } from "@/lib/api/security";
import { SystemAPI } from "@/lib/api/system";
import type {
  AuthCredentialSettings,
  AutoHttpsDetails,
  DashboardDisplayConfig,
  DateTimeDisplayMode,
  ProtocolMappingFeatureConfig,
} from "../../types";
import { useConfigStore } from "../../store/config";
import { smartConnectFeatureEntryVisible } from "../layout/runtime-navigation";

const runTypeLabelKeyMap = {
  0: "admin.featuresSettings.runTypes.direct",
  1: "admin.featuresSettings.runTypes.reverse",
  3: "admin.featuresSettings.runTypes.subdomain",
} as const;

export function useFeaturesSettings() {
  const router = useRouter();
  const configStore = useConfigStore();
  const { t } = useI18n();
  const protocolMappingEnabled = ref(false);
  const wolEnabled = ref(false);
  const passkeyBindPromptEnabled = ref(true);
  const showEntryStatusModule = ref(true);
  const showConsoleAppList = ref(false);
  const dateTimeDisplayMode = ref<DateTimeDisplayMode>("human_friendly");
  const autoHttpsDetails = ref<AutoHttpsDetails | null>(null);
  const sshSecurityEnabled = ref(false);
  const sshSecurityUnavailableReason = ref("");

  const { isPending: isLoading, run: runLoadSettings } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.featuresSettings.loadFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.featuresSettings.loadFailedDescription"),
        ),
      });
    },
  });
  const showLoadingSkeleton = useDelayedLoading(isLoading);
  const { isPending: isSaving, run: runSaveSettings } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.featuresSettings.updateFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.featuresSettings.updateFailedDescription"),
        ),
      });
    },
  });

  const isProtocolMappingAvailable = computed(
    () => configStore.config?.run_type === 3,
  );
  const isSmartConnectAvailable = computed(
    () => configStore.canUseSmartConnect && configStore.config?.run_type === 3,
  );
  const showSmartConnectEntry = computed(() =>
    smartConnectFeatureEntryVisible({
      isFpkLiteDeployment: configStore.isFpkLiteDeployment,
      isDockerDeployment: configStore.isDockerDeployment,
      isOpenWrtDeployment: configStore.isOpenWrtDeployment,
      isSynologyDeployment: configStore.isSynologyDeployment,
    }),
  );
  const isDashboardDisplaySwitchDisabled = computed(
    () => isSaving.value || configStore.isLoading || configStore.isError,
  );
  const showConsoleAppListEntry = computed(
    () => configStore.isFpkDeployment || configStore.isFpkLiteDeployment,
  );
  const currentRunTypeLabel = computed(() => {
    const runType = configStore.config?.run_type;
    if (runType === 0 || runType === 1 || runType === 3) {
      return t(runTypeLabelKeyMap[runType]);
    }
    return t("admin.featuresSettings.runTypes.current");
  });
  const protocolMappingDisabledReason = computed(() => {
    if (isProtocolMappingAvailable.value) return "";
    return t("admin.featuresSettings.subdomainOnlyEnableReason", {
      mode: currentRunTypeLabel.value,
    });
  });
  const smartConnectDisabledReason = computed(() => {
    if (isSmartConnectAvailable.value) return "";
    if (!configStore.canUseSmartConnect) {
      return configStore.isDockerDeployment
        ? t("admin.featuresSettings.smartConnectDockerUnsupported")
        : t("admin.featuresSettings.smartConnectEnvironmentUnsupported");
    }
    return t("admin.featuresSettings.subdomainOnlyReason", {
      mode: currentRunTypeLabel.value,
    });
  });
  const autoHttpsEnabled = computed(
    () => autoHttpsDetails.value?.enabled === true,
  );
  const autoHttpsRuntimeError = computed(() => {
    const runtime = autoHttpsDetails.value?.runtime;
    if (!runtime || (runtime.status !== "error" && !runtime.last_error)) {
      return "";
    }
    return (
      runtime.last_error || t("admin.featuresSettings.autoHttpsListenFailed")
    );
  });
  const showAutoHttpsEntry = computed(
    () =>
      configStore.canUseAutoHttps &&
      !configStore.isDockerDeployment &&
      !configStore.isOpenWrtDeployment &&
      !configStore.isSynologyDeployment,
  );
  const showSSHSecurityEntry = computed(
    () => configStore.canUseSshSecurity && !configStore.isSynologyDeployment,
  );
  const isSSHSecurityAvailable = computed(
    () =>
      configStore.canManageHostFirewall && !sshSecurityUnavailableReason.value,
  );
  const sshSecurityDisabledReason = computed(() => {
    if (isSSHSecurityAvailable.value) return "";
    return (
      sshSecurityUnavailableReason.value ||
      t("admin.featuresSettings.sshFirewallUnsupported")
    );
  });

  const applyProtocolMappingSettings = (data: ProtocolMappingFeatureConfig) => {
    protocolMappingEnabled.value = data.enabled;
  };
  const applyWOLSettings = (data: { enabled: boolean }) => {
    wolEnabled.value = data.enabled;
  };
  const applyAutoHttpsDetails = (data: AutoHttpsDetails) => {
    autoHttpsDetails.value = data;
  };
  const applySSHSecurityDetails = (
    data: Awaited<ReturnType<typeof SSHSecurityAPI.getDetails>>,
  ) => {
    sshSecurityEnabled.value = data.config.enabled;
    sshSecurityUnavailableReason.value = data.summary.available
      ? ""
      : data.summary.unavailable_reason;
  };
  const applyAuthCredentialSettings = (data: AuthCredentialSettings) => {
    passkeyBindPromptEnabled.value = data.passkey_bind_prompt_enabled !== false;
  };
  const applyDashboardDisplaySettings = (
    data: Pick<
      DashboardDisplayConfig,
      | "show_entry_status_module"
      | "show_console_app_list"
      | "date_time_display_mode"
    >,
  ) => {
    showEntryStatusModule.value = data.show_entry_status_module;
    showConsoleAppList.value = data.show_console_app_list;
    dateTimeDisplayMode.value = normalizeDateTimeDisplayMode(
      data.date_time_display_mode,
    );
    applyDateTimeDisplayMode(dateTimeDisplayMode.value);
  };
  const syncDashboardDisplayFromConfig = () => {
    if (!configStore.config) return;
    applyDashboardDisplaySettings({
      show_entry_status_module:
        configStore.config.dashboard_display?.show_entry_status_module !==
        false,
      show_console_app_list:
        configStore.config.dashboard_display?.show_console_app_list === true,
      date_time_display_mode: normalizeDateTimeDisplayMode(
        configStore.config.dashboard_display?.date_time_display_mode,
      ),
    });
  };

  const fetchSettings = async () => {
    await runLoadSettings(async () => {
      const [protocolMappingSettings, authCredentialSettings, wolSettings] =
        await Promise.all([
          SystemAPI.getProtocolMappingFeatureConfig(),
          ConfigAPI.getAuthCredentialSettings(),
          ConfigAPI.getWOLFeature(),
        ]);
      applyProtocolMappingSettings(protocolMappingSettings);
      applyAuthCredentialSettings(authCredentialSettings);
      applyWOLSettings(wolSettings);

      if (showAutoHttpsEntry.value) {
        applyAutoHttpsDetails(await SystemAPI.getAutoHttpsDetails());
      } else {
        autoHttpsDetails.value = null;
      }

      if (showSSHSecurityEntry.value) {
        applySSHSecurityDetails(await SSHSecurityAPI.getDetails());
      } else {
        sshSecurityEnabled.value = false;
        sshSecurityUnavailableReason.value = "";
      }
    });
  };

  const saveProtocolMappingEnabled = async (nextValue: boolean) => {
    if (!isProtocolMappingAvailable.value || isSaving.value) return;
    const previousValue = protocolMappingEnabled.value;
    protocolMappingEnabled.value = nextValue;
    const result = await runSaveSettings(
      () =>
        SystemAPI.updateProtocolMappingFeatureConfig({ enabled: nextValue }),
      {
        onSuccess: async (data) => {
          applyProtocolMappingSettings(data);
          toast.success(t("admin.featuresSettings.updated"));
          await configStore.loadConfig();
        },
      },
    );
    if (!result) {
      protocolMappingEnabled.value = previousValue;
      await configStore.loadConfig({ force: true });
    }
  };

  const saveWOLEnabled = async (nextValue: boolean) => {
    if (isSaving.value) return;
    const previousValue = wolEnabled.value;
    wolEnabled.value = nextValue;
    const result = await runSaveSettings(
      () => ConfigAPI.updateWOLFeature({ enabled: nextValue }),
      {
        onSuccess: async (data) => {
          applyWOLSettings(data);
          toast.success(t("admin.featuresSettings.updated"));
          await configStore.loadConfig();
        },
      },
    );
    if (!result) wolEnabled.value = previousValue;
  };

  const saveShowEntryStatusModule = async (nextValue: boolean) => {
    if (isDashboardDisplaySwitchDisabled.value || !configStore.config) return;
    const previousValue = showEntryStatusModule.value;
    showEntryStatusModule.value = nextValue;
    const result = await runSaveSettings(
      () =>
        ConfigAPI.updateDashboardDisplayConfig({
          show_entry_status_module: nextValue,
        }),
      {
        onSuccess: async (data) => {
          applyDashboardDisplaySettings(data);
          toast.success(t("admin.featuresSettings.updated"));
          await configStore.loadConfig();
        },
      },
    );
    if (!result) showEntryStatusModule.value = previousValue;
  };

  const saveShowConsoleAppList = async (nextValue: boolean) => {
    if (
      isDashboardDisplaySwitchDisabled.value ||
      !showConsoleAppListEntry.value ||
      !configStore.config
    ) {
      return;
    }
    const previousValue = showConsoleAppList.value;
    showConsoleAppList.value = nextValue;
    const result = await runSaveSettings(
      () =>
        ConfigAPI.updateDashboardDisplayConfig({
          show_console_app_list: nextValue,
        }),
      {
        onSuccess: async (data) => {
          applyDashboardDisplaySettings(data);
          toast.success(t("admin.featuresSettings.updated"));
          await configStore.loadConfig();
        },
      },
    );
    if (!result) showConsoleAppList.value = previousValue;
  };

  const saveDateTimeDisplayMode = async (nextValue: DateTimeDisplayMode) => {
    if (isDashboardDisplaySwitchDisabled.value || !configStore.config) return;
    const previousValue = dateTimeDisplayMode.value;
    dateTimeDisplayMode.value = nextValue;
    applyDateTimeDisplayMode(nextValue);
    const result = await runSaveSettings(
      () =>
        ConfigAPI.updateDashboardDisplayConfig({
          date_time_display_mode: nextValue,
        }),
      {
        onSuccess: async (data) => {
          applyDashboardDisplaySettings(data);
          toast.success(t("admin.featuresSettings.updated"));
          await configStore.loadConfig();
        },
      },
    );
    if (!result) {
      dateTimeDisplayMode.value = previousValue;
      applyDateTimeDisplayMode(previousValue);
    }
  };

  const savePasskeyBindPromptEnabled = async (nextValue: boolean) => {
    if (isSaving.value) return;
    const previousValue = passkeyBindPromptEnabled.value;
    passkeyBindPromptEnabled.value = nextValue;
    const result = await runSaveSettings(
      () =>
        ConfigAPI.updateAuthCredentialSettings({
          passkey_bind_prompt_enabled: nextValue,
        }),
      {
        onSuccess: async (data) => {
          applyAuthCredentialSettings(data);
          toast.success(t("admin.featuresSettings.updated"));
          await configStore.loadConfig();
        },
      },
    );
    if (!result) passkeyBindPromptEnabled.value = previousValue;
  };

  const saveAutoHttpsEnabled = async (nextValue: boolean) => {
    if (isSaving.value) return;
    const previousValue = autoHttpsDetails.value;
    autoHttpsDetails.value = {
      enabled: nextValue,
      runtime: previousValue?.runtime ?? {
        enabled: false,
        active: false,
        status: "disabled",
        listen_host: "::",
        listen_port: 80,
        redirect_scheme: "https",
        last_error: null,
        last_error_at: null,
        updated_at: new Date().toISOString(),
      },
    };
    const result = await runSaveSettings(
      () => SystemAPI.updateAutoHttps({ enabled: nextValue }),
      {
        onSuccess: async (data) => {
          applyAutoHttpsDetails(data);
          if (data.runtime.status === "error" || data.runtime.last_error) {
            toast.error(t("admin.featuresSettings.autoHttpsStartFailed"), {
              description:
                data.runtime.last_error ||
                t("admin.featuresSettings.port80ListenFailed"),
            });
          } else {
            toast.success(t("admin.featuresSettings.updated"));
          }
          await configStore.loadConfig();
        },
      },
    );
    if (!result) autoHttpsDetails.value = previousValue;
  };

  const saveSSHSecurityEnabled = async (nextValue: boolean) => {
    if (isSaving.value || (!isSSHSecurityAvailable.value && nextValue)) return;
    const previousValue = sshSecurityEnabled.value;
    sshSecurityEnabled.value = nextValue;
    const result = await runSaveSettings(
      () => SSHSecurityAPI.updateConfig({ enabled: nextValue }),
      {
        onSuccess: async (data) => {
          applySSHSecurityDetails(data);
          toast.success(t("admin.featuresSettings.updated"));
          await configStore.loadConfig();
        },
      },
    );
    if (!result) sshSecurityEnabled.value = previousValue;
  };

  const openWebTerminal = () => {
    void router.push("/system/web-terminal");
  };

  const openSmartConnect = () => {
    if (isSmartConnectAvailable.value)
      void router.push("/system/smart-connect");
  };

  const openSidebarMenuOrder = () => {
    void router.push("/system/sidebar-menu-order");
  };

  onMounted(() => {
    syncDashboardDisplayFromConfig();
    void fetchSettings();
  });

  watch(
    () => configStore.config?.dashboard_display,
    syncDashboardDisplayFromConfig,
    { immediate: true },
  );

  watch(
    () => configStore.config?.run_type,
    (runType) => {
      if (runType === 3) {
        void fetchSettings();
      } else {
        protocolMappingEnabled.value = false;
      }
    },
  );

  return {
    autoHttpsEnabled,
    autoHttpsRuntimeError,
    dateTimeDisplayMode,
    isDashboardDisplaySwitchDisabled,
    isLoading,
    isProtocolMappingAvailable,
    isSaving,
    isSmartConnectAvailable,
    isSSHSecurityAvailable,
    openSmartConnect,
    openWebTerminal,
    openSidebarMenuOrder,
    passkeyBindPromptEnabled,
    protocolMappingDisabledReason,
    protocolMappingEnabled,
    saveAutoHttpsEnabled,
    saveDateTimeDisplayMode,
    savePasskeyBindPromptEnabled,
    saveProtocolMappingEnabled,
    saveShowConsoleAppList,
    saveShowEntryStatusModule,
    saveSSHSecurityEnabled,
    saveWOLEnabled,
    showAutoHttpsEntry,
    showConsoleAppList,
    showConsoleAppListEntry,
    showEntryStatusModule,
    showLoadingSkeleton,
    showSmartConnectEntry,
    showSSHSecurityEntry,
    smartConnectDisabledReason,
    sshSecurityDisabledReason,
    sshSecurityEnabled,
    t,
    wolEnabled,
  };
}
