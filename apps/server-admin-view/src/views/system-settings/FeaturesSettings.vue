<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { ChevronRight } from "lucide-vue-next";
import { toast } from "@admin-shared/utils/toast";
import { ConfigAPI, SSHSecurityAPI, SystemAPI } from "../../lib/api";
import type {
  AuthCredentialSettings,
  AutoHttpsDetails,
  DashboardDisplayConfig,
  ProtocolMappingFeatureConfig,
} from "../../types";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { useConfigStore } from "../../store/config";
import FeatureSwitchRow from "./FeatureSwitchRow.vue";

const router = useRouter();
const configStore = useConfigStore();
const { t } = useI18n();
const protocolMappingEnabled = ref(false);
const passkeyBindPromptEnabled = ref(true);
const showEntryStatusModule = ref(true);
const autoHttpsDetails = ref<AutoHttpsDetails | null>(null);
const sshSecurityEnabled = ref(false);
const sshSecurityUnavailableReason = ref("");
const runTypeLabelKeyMap = {
  0: "admin.featuresSettings.runTypes.direct",
  1: "admin.featuresSettings.runTypes.reverse",
  3: "admin.featuresSettings.runTypes.subdomain",
} as const;

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
const showSmartConnectEntry = computed(() => !configStore.isDockerDeployment);
const isDashboardDisplaySwitchDisabled = computed(
  () => isSaving.value || configStore.isLoading || configStore.isError,
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
  if (!runtime || (runtime.status !== "error" && !runtime.last_error))
    return "";
  return (
    runtime.last_error || t("admin.featuresSettings.autoHttpsListenFailed")
  );
});
const showAutoHttpsEntry = computed(
  () => !configStore.isDockerDeployment && !configStore.isOpenWrtDeployment,
);
const showSSHSecurityEntry = computed(() => configStore.canUseSshSecurity);
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

const applyDashboardDisplaySettings = (data: DashboardDisplayConfig) => {
  showEntryStatusModule.value = data.show_entry_status_module;
};

const syncDashboardDisplayFromConfig = () => {
  if (!configStore.config) {
    return;
  }

  applyDashboardDisplaySettings({
    show_entry_status_module:
      configStore.config.dashboard_display?.show_entry_status_module !== false,
  });
};

const fetchSettings = async () => {
  await runLoadSettings(async () => {
    const [protocolMappingSettings, authCredentialSettings] = await Promise.all(
      [
        SystemAPI.getProtocolMappingFeatureConfig(),
        ConfigAPI.getAuthCredentialSettings(),
      ],
    );
    applyProtocolMappingSettings(protocolMappingSettings);
    applyAuthCredentialSettings(authCredentialSettings);

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
  if (!isProtocolMappingAvailable.value || isSaving.value) {
    return;
  }

  const previousValue = protocolMappingEnabled.value;
  protocolMappingEnabled.value = nextValue;

  const result = await runSaveSettings(
    () =>
      SystemAPI.updateProtocolMappingFeatureConfig({
        enabled: nextValue,
      }),
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
  }
};

const saveShowEntryStatusModule = async (nextValue: boolean) => {
  if (isDashboardDisplaySwitchDisabled.value || !configStore.config) {
    return;
  }

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

  if (!result) {
    showEntryStatusModule.value = previousValue;
  }
};

const savePasskeyBindPromptEnabled = async (nextValue: boolean) => {
  if (isSaving.value) {
    return;
  }

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

  if (!result) {
    passkeyBindPromptEnabled.value = previousValue;
  }
};

const saveAutoHttpsEnabled = async (nextValue: boolean) => {
  if (isSaving.value) {
    return;
  }

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
    () =>
      SystemAPI.updateAutoHttps({
        enabled: nextValue,
      }),
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

  if (!result) {
    autoHttpsDetails.value = previousValue;
  }
};

const saveSSHSecurityEnabled = async (nextValue: boolean) => {
  if (isSaving.value || (!isSSHSecurityAvailable.value && nextValue)) {
    return;
  }

  const previousValue = sshSecurityEnabled.value;
  sshSecurityEnabled.value = nextValue;

  const result = await runSaveSettings(
    () =>
      SSHSecurityAPI.updateConfig({
        enabled: nextValue,
      }),
    {
      onSuccess: async (data) => {
        applySSHSecurityDetails(data);
        toast.success(t("admin.featuresSettings.updated"));
        await configStore.loadConfig();
      },
    },
  );

  if (!result) {
    sshSecurityEnabled.value = previousValue;
  }
};

const openSmartConnect = () => {
  if (!isSmartConnectAvailable.value) {
    return;
  }

  void router.push("/system/smart-connect");
};

onMounted(() => {
  syncDashboardDisplayFromConfig();
  void fetchSettings();
});

watch(
  () => configStore.config?.dashboard_display?.show_entry_status_module,
  () => {
    syncDashboardDisplayFromConfig();
  },
  { immediate: true },
);

watch(
  () => configStore.config?.run_type,
  (runType) => {
    if (runType === 3) {
      void fetchSettings();
      return;
    }

    protocolMappingEnabled.value = false;
  },
);
</script>

<template>
  <Card>
    <CardHeader>
      <div class="space-y-1.5">
        <CardTitle class="text-md">{{
          t("admin.featuresSettings.title")
        }}</CardTitle>
        <CardDescription>{{
          t("admin.featuresSettings.description")
        }}</CardDescription>
      </div>
    </CardHeader>

    <CardContent v-if="isLoading && showLoadingSkeleton" class="border-t p-0">
      <div class="space-y-4 p-6">
        <Skeleton class="h-6 w-1/3" />
        <Skeleton class="h-4 w-2/3" />
      </div>
    </CardContent>

    <CardContent v-else-if="!isLoading" class="border-t p-0 divide-y">
      <FeatureSwitchRow
        :title="t('admin.featuresSettings.showEntryStatusModule')"
        :description="t('admin.featuresSettings.showEntryStatusModuleHint')"
        :model-value="showEntryStatusModule"
        :disabled="isDashboardDisplaySwitchDisabled"
        @change="saveShowEntryStatusModule"
      />

      <FeatureSwitchRow
        :title="t('admin.featuresSettings.passkeyBindPrompt')"
        :description="t('admin.featuresSettings.passkeyBindPromptHint')"
        :model-value="passkeyBindPromptEnabled"
        :disabled="isSaving"
        @change="savePasskeyBindPromptEnabled"
      />

      <FeatureSwitchRow
        v-if="showAutoHttpsEntry"
        :title="t('admin.featuresSettings.autoHttps')"
        :description="t('admin.featuresSettings.autoHttpsHint')"
        :model-value="autoHttpsEnabled"
        :disabled="isSaving"
        :error="autoHttpsRuntimeError"
        @change="saveAutoHttpsEnabled"
      />

      <FeatureSwitchRow
        v-if="showSSHSecurityEntry"
        :title="t('admin.featuresSettings.sshSecurity')"
        :description="t('admin.featuresSettings.sshSecurityHint')"
        :model-value="sshSecurityEnabled"
        :available="isSSHSecurityAvailable"
        :disabled="isSaving"
        :disabled-reason="sshSecurityDisabledReason"
        @change="saveSSHSecurityEnabled"
      />

      <FeatureSwitchRow
        :title="t('admin.featuresSettings.protocolMapping')"
        :description="t('admin.featuresSettings.protocolMappingHint')"
        :model-value="protocolMappingEnabled"
        :available="isProtocolMappingAvailable"
        :disabled="isSaving"
        :disabled-reason="protocolMappingDisabledReason"
        @change="saveProtocolMappingEnabled"
      />

      <button
        v-if="showSmartConnectEntry"
        type="button"
        class="flex w-full items-center justify-between p-6 text-left transition-colors"
        :class="
          isSmartConnectAvailable
            ? 'bg-muted/5 hover:bg-muted/15'
            : 'cursor-not-allowed bg-muted/5'
        "
        :disabled="!isSmartConnectAvailable"
        @click="openSmartConnect"
      >
        <div class="space-y-1 pr-6">
          <div
            class="text-base font-medium"
            :class="
              isSmartConnectAvailable ? 'text-foreground' : 'text-zinc-500'
            "
          >
            {{ t("admin.featuresSettings.smartConnect") }}
          </div>
          <div
            class="text-sm"
            :class="
              isSmartConnectAvailable
                ? 'text-muted-foreground'
                : 'text-zinc-500'
            "
          >
            {{ t("admin.featuresSettings.smartConnectHint") }}
          </div>
          <div
            v-if="!isSmartConnectAvailable"
            class="text-xs leading-5 text-zinc-500"
          >
            {{ smartConnectDisabledReason }}
          </div>
        </div>
        <ChevronRight
          class="h-5 w-5 shrink-0"
          :class="
            isSmartConnectAvailable ? 'text-muted-foreground' : 'text-zinc-400'
          "
        />
      </button>
    </CardContent>
  </Card>
</template>
