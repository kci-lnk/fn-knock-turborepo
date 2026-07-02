<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Card, CardContent } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Skeleton } from "@/components/ui/skeleton";
import { toast } from "@admin-shared/utils/toast";
import { SystemAPI } from "../../lib/api";
import type {
  FnosNetworkTuningStatus,
  FnosPortIconHijackConfig,
  FnosShareBypassConfig,
  FnosNetworkTuningUpdatePayload,
} from "../../types";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { useConfigStore } from "../../store/config";

const configStore = useConfigStore();
const { t } = useI18n();
const DEFAULT_FNOS_SHARE_BYPASS_VALUES = {
  upstream_timeout_ms: 2500,
  validation_cache_ttl_seconds: 30,
  validation_lock_ttl_seconds: 5,
  session_ttl_seconds: 300,
} satisfies Omit<FnosShareBypassConfig, "enabled">;
const settings = ref<FnosShareBypassConfig | null>(null);
const form = reactive<FnosShareBypassConfig>({
  enabled: false,
  ...DEFAULT_FNOS_SHARE_BYPASS_VALUES,
});
const iconHijackSettings = ref<FnosPortIconHijackConfig | null>(null);
const iconHijackForm = reactive<FnosPortIconHijackConfig>({
  enabled: false,
  updated_at: null,
});
const networkTuningStatus = ref<FnosNetworkTuningStatus | null>(null);
const networkTuningForm = reactive({
  bbr_enabled: false,
  mtu_probing_enabled: false,
});

const { isPending: isLoading, run: runLoadSettings } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.fnosSettings.loadFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.fnosSettings.loadDescription"),
      ),
    });
  },
});
const showLoadingSkeleton = useDelayedLoading(isLoading);
const { isPending: isSaving, run: runSaveSettings } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.fnosSettings.saveFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.fnosSettings.saveDescription"),
      ),
    });
  },
});
const { isPending: isIconHijackSaving, run: runSaveIconHijackSettings } =
  useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.fnosSettings.saveFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.fnosSettings.saveIconHijackDescription"),
        ),
      });
    },
  });
const { isPending: isNetworkTuningSaving, run: runSaveNetworkTuningSettings } =
  useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.fnosSettings.saveFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.fnosSettings.saveNetworkTuningDescription"),
        ),
      });
    },
  });
const isShareBypassMode = computed(
  () =>
    configStore.config?.run_type === 1 || configStore.config?.run_type === 3,
);
const isRestrictedByRunMode = computed(
  () => configStore.config?.run_type === 0,
);
const isNetworkTuningAvailable = computed(
  () => networkTuningStatus.value?.available === true,
);
const networkTuningUnavailableText = computed(
  () =>
    networkTuningStatus.value?.blocked_reason ||
    t("admin.fnosSettings.networkTuningUnavailable"),
);

const displaySysctlValue = (value: string | null | undefined) =>
  value?.trim() || "--";

const displayList = (values: string[] | null | undefined) =>
  values && values.length > 0 ? values.join(" ") : "--";

const bbrCurrentDescription = computed(() => {
  const status = networkTuningStatus.value;
  return t("admin.fnosSettings.bbrCurrent", {
    congestion: displaySysctlValue(status?.bbr.current_congestion_control),
    qdisc: displaySysctlValue(status?.bbr.current_default_qdisc),
    available: displayList(status?.bbr.available_congestion_control),
  });
});

const bbrSupportDescription = computed(() => {
  const status = networkTuningStatus.value;
  if (!status) return "";
  return status.bbr.supported
    ? t("admin.fnosSettings.bbrSupported")
    : t("admin.fnosSettings.bbrUnsupported");
});

const mtuCurrentDescription = computed(() =>
  t("admin.fnosSettings.mtuCurrent", {
    value: displaySysctlValue(
      networkTuningStatus.value?.mtu_probing.current_value,
    ),
  }),
);

const applyFromSettings = (data: FnosShareBypassConfig) => {
  settings.value = data;
  form.enabled = data.enabled;
  form.upstream_timeout_ms = data.upstream_timeout_ms;
  form.validation_cache_ttl_seconds = data.validation_cache_ttl_seconds;
  form.validation_lock_ttl_seconds = data.validation_lock_ttl_seconds;
  form.session_ttl_seconds = data.session_ttl_seconds;
};

const applyIconHijackFromSettings = (data: FnosPortIconHijackConfig) => {
  iconHijackSettings.value = data;
  iconHijackForm.enabled = data.enabled;
  iconHijackForm.updated_at = data.updated_at;
};

const applyNetworkTuningFromStatus = (data: FnosNetworkTuningStatus) => {
  networkTuningStatus.value = data;
  networkTuningForm.bbr_enabled = data.config.bbr_enabled;
  networkTuningForm.mtu_probing_enabled = data.config.mtu_probing_enabled;
};

const fetchSettings = async () => {
  await runLoadSettings(async () => {
    const [shareBypass, iconHijack, networkTuning] = await Promise.all([
      SystemAPI.getFnosShareBypassConfig(),
      SystemAPI.getFnosPortIconHijackConfig(),
      SystemAPI.getFnosNetworkTuningStatus(),
    ]);
    applyFromSettings(shareBypass);
    applyIconHijackFromSettings(iconHijack);
    applyNetworkTuningFromStatus(networkTuning);
  });
};

const saveShareBypassEnabled = async (nextValue: boolean) => {
  if (!isShareBypassMode.value || isSaving.value) {
    if (!isShareBypassMode.value) {
      toast.error(t("admin.fnosSettings.unavailableTitle"), {
        description: t("admin.fnosSettings.unavailableDescription"),
      });
    }
    return;
  }

  const previousSettings = settings.value;
  form.enabled = nextValue;

  const result = await runSaveSettings(
    () =>
      SystemAPI.updateFnosShareBypassConfig({
        enabled: nextValue,
        ...DEFAULT_FNOS_SHARE_BYPASS_VALUES,
      }),
    {
      onSuccess: (data) => {
        applyFromSettings(data);
        toast.success(t("admin.fnosSettings.shareBypassUpdated"));
      },
    },
  );

  if (!result && previousSettings) {
    applyFromSettings(previousSettings);
  }
};

const saveIconHijackEnabled = async (nextValue: boolean) => {
  if (isIconHijackSaving.value) {
    return;
  }

  const previousSettings = iconHijackSettings.value;
  iconHijackForm.enabled = nextValue;

  const result = await runSaveIconHijackSettings(
    () =>
      SystemAPI.updateFnosPortIconHijackConfig({
        enabled: nextValue,
      }),
    {
      onSuccess: (data) => {
        applyIconHijackFromSettings(data);
        toast.success(t("admin.fnosSettings.iconHijackUpdated"));
      },
    },
  );

  if (!result && previousSettings) {
    applyIconHijackFromSettings(previousSettings);
  }
};

const saveNetworkTuning = async (
  patch: FnosNetworkTuningUpdatePayload,
  successKey: string,
) => {
  if (!isNetworkTuningAvailable.value || isNetworkTuningSaving.value) {
    if (!isNetworkTuningAvailable.value) {
      toast.error(t("admin.fnosSettings.unavailableTitle"), {
        description: networkTuningUnavailableText.value,
      });
    }
    return;
  }

  const previousStatus = networkTuningStatus.value;
  if (patch.bbr_enabled !== undefined) {
    networkTuningForm.bbr_enabled = patch.bbr_enabled;
  }
  if (patch.mtu_probing_enabled !== undefined) {
    networkTuningForm.mtu_probing_enabled = patch.mtu_probing_enabled;
  }

  const result = await runSaveNetworkTuningSettings(
    () => SystemAPI.updateFnosNetworkTuningConfig(patch),
    {
      onSuccess: (data) => {
        applyNetworkTuningFromStatus(data);
        toast.success(t(successKey));
      },
    },
  );

  if (!result && previousStatus) {
    applyNetworkTuningFromStatus(previousStatus);
  }
};

const toggleShareBypass = () => {
  void saveShareBypassEnabled(!form.enabled);
};

const toggleIconHijack = () => {
  void saveIconHijackEnabled(!iconHijackForm.enabled);
};

const toggleBbr = () => {
  void saveNetworkTuning(
    { bbr_enabled: !networkTuningForm.bbr_enabled },
    "admin.fnosSettings.bbrUpdated",
  );
};

const toggleMtuProbing = () => {
  void saveNetworkTuning(
    { mtu_probing_enabled: !networkTuningForm.mtu_probing_enabled },
    "admin.fnosSettings.mtuUpdated",
  );
};

onMounted(fetchSettings);
</script>

<template>
  <Card>
    <CardContent v-if="isLoading && showLoadingSkeleton" class="p-0">
      <div class="space-y-4 p-6">
        <Skeleton class="h-6 w-1/3" />
        <Skeleton class="h-4 w-2/3" />
        <Skeleton class="h-6 w-1/3" />
        <Skeleton class="h-4 w-2/3" />
        <Skeleton class="h-6 w-1/3" />
        <Skeleton class="h-4 w-2/3" />
        <Skeleton class="h-6 w-1/3" />
        <Skeleton class="h-4 w-2/3" />
      </div>
    </CardContent>

    <CardContent v-else-if="!isLoading" class="p-0 divide-y">
      <div class="flex items-center justify-between bg-muted/10 p-6">
        <div class="space-y-1 pr-6">
          <Label
            class="text-base font-medium"
            :class="
              isShareBypassMode
                ? 'cursor-pointer'
                : 'cursor-not-allowed text-zinc-500'
            "
            @click="toggleShareBypass"
          >
            {{ t("admin.fnosSettings.shareBypassTitle") }}
          </Label>
          <div
            class="text-sm"
            :class="
              isShareBypassMode ? 'text-muted-foreground' : 'text-zinc-500'
            "
          >
            {{ t("admin.fnosSettings.shareBypassDescription") }}
          </div>
          <div
            v-if="isRestrictedByRunMode"
            class="text-xs leading-5 text-zinc-500"
          >
            {{ t("admin.fnosSettings.shareBypassDirectUnavailable") }}
          </div>
        </div>
        <Switch
          :model-value="isShareBypassMode ? form.enabled : false"
          :disabled="!isShareBypassMode || isSaving"
          @update:model-value="saveShareBypassEnabled($event === true)"
        />
      </div>

      <div class="flex items-center justify-between bg-muted/10 p-6">
        <div class="space-y-1 pr-6">
          <Label
            class="cursor-pointer text-base font-medium"
            @click="toggleIconHijack"
          >
            {{ t("admin.fnosSettings.iconHijackTitle") }}
          </Label>
          <div class="text-sm text-muted-foreground">
            {{ t("admin.fnosSettings.iconHijackDescriptionPrefix") }}<u>{{
              t("admin.fnosSettings.iconHijackDescriptionHighlight")
            }}</u>{{ t("admin.fnosSettings.iconHijackDescriptionSuffix") }}
          </div>
        </div>
        <Switch
          :model-value="iconHijackForm.enabled"
          :disabled="isIconHijackSaving"
          @update:model-value="saveIconHijackEnabled($event === true)"
        />
      </div>

      <div class="flex items-center justify-between bg-muted/10 p-6">
        <div class="space-y-1 pr-6">
          <Label
            class="text-base font-medium"
            :class="
              isNetworkTuningAvailable
                ? 'cursor-pointer'
                : 'cursor-not-allowed text-zinc-500'
            "
            @click="toggleBbr"
          >
            {{ t("admin.fnosSettings.bbrTitle") }}
          </Label>
          <div
            class="text-sm"
            :class="
              isNetworkTuningAvailable
                ? 'text-muted-foreground'
                : 'text-zinc-500'
            "
          >
            {{ t("admin.fnosSettings.bbrDescription") }}
          </div>
          <div class="text-xs leading-5 text-zinc-500">
            {{ bbrCurrentDescription }}
          </div>
          <div
            v-if="networkTuningStatus"
            class="text-xs leading-5"
            :class="
              networkTuningStatus.bbr.supported
                ? 'text-emerald-600'
                : 'text-amber-600'
            "
          >
            {{ bbrSupportDescription }}
          </div>
          <div
            v-if="!isNetworkTuningAvailable"
            class="text-xs leading-5 text-zinc-500"
          >
            {{ networkTuningUnavailableText }}
          </div>
          <div
            v-if="networkTuningStatus?.last_error"
            class="text-xs leading-5 text-destructive"
          >
            {{
              t("admin.fnosSettings.networkTuningLastError", {
                message: networkTuningStatus.last_error,
              })
            }}
          </div>
        </div>
        <Switch
          :model-value="networkTuningForm.bbr_enabled"
          :disabled="!isNetworkTuningAvailable || isNetworkTuningSaving"
          @update:model-value="
            saveNetworkTuning(
              { bbr_enabled: $event === true },
              'admin.fnosSettings.bbrUpdated',
            )
          "
        />
      </div>

      <div class="flex items-center justify-between bg-muted/10 p-6">
        <div class="space-y-1 pr-6">
          <Label
            class="text-base font-medium"
            :class="
              isNetworkTuningAvailable
                ? 'cursor-pointer'
                : 'cursor-not-allowed text-zinc-500'
            "
            @click="toggleMtuProbing"
          >
            {{ t("admin.fnosSettings.mtuTitle") }}
          </Label>
          <div
            class="text-sm"
            :class="
              isNetworkTuningAvailable
                ? 'text-muted-foreground'
                : 'text-zinc-500'
            "
          >
            {{ t("admin.fnosSettings.mtuDescription") }}
          </div>
          <div class="text-xs leading-5 text-zinc-500">
            {{ mtuCurrentDescription }}
          </div>
          <div
            v-if="!isNetworkTuningAvailable"
            class="text-xs leading-5 text-zinc-500"
          >
            {{ networkTuningUnavailableText }}
          </div>
        </div>
        <Switch
          :model-value="networkTuningForm.mtu_probing_enabled"
          :disabled="!isNetworkTuningAvailable || isNetworkTuningSaving"
          @update:model-value="
            saveNetworkTuning(
              { mtu_probing_enabled: $event === true },
              'admin.fnosSettings.mtuUpdated',
            )
          "
        />
      </div>
    </CardContent>

    <CardContent v-else class="min-h-[160px]" aria-hidden="true" />
  </Card>
</template>
