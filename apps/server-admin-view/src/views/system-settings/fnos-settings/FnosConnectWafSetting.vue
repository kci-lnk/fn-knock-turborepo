<script setup lang="ts">
import { computed, onMounted, ref, useId } from "vue";
import { useI18n } from "vue-i18n";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { toast } from "@admin-shared/utils/toast";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { SystemAPI } from "@/lib/api/system";
import type { FnosConnectWafDetails } from "../../../types";

const a11yId = useId();
const { t } = useI18n();
const details = ref<FnosConnectWafDetails | null>(null);

const { isPending: isLoading, run: runLoad } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.fnosSettings.connectWafLoadFailed"), {
      description: extractErrorMessage(error),
    });
  },
});
const { isPending: isSaving, run: runSave } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.fnosSettings.saveFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.fnosSettings.connectWafSaveFailed"),
      ),
    });
  },
});

const enabled = computed(() => details.value?.config.enabled === true);
const statusKey = computed(() => {
  if (!enabled.value) return "connectWafStatusDisabled";
  if (details.value?.runtime.protected) return "connectWafStatusProtected";
  if (details.value?.runtime.effective && details.value.runtime.waf_active) {
    return "connectWafStatusDetection";
  }
  if (details.value?.runtime.effective) return "connectWafStatusWafInactive";
  return "connectWafStatusDegraded";
});
const statusClass = computed(() =>
  details.value?.runtime.protected
    ? "text-emerald-600"
    : enabled.value
      ? "text-amber-600"
      : "text-zinc-500",
);

const load = async () => {
  await runLoad(async () => {
    details.value = await SystemAPI.getFnosConnectWafDetails();
  });
};

const save = async (next: boolean) => {
  if (isSaving.value) return;
  const previous = details.value;
  if (details.value) {
    details.value = {
      ...details.value,
      config: { ...details.value.config, enabled: next },
    };
  }
  const result = await runSave(
    () => SystemAPI.updateFnosConnectWafConfig(next),
    {
      onSuccess: (value) => {
        details.value = value;
        toast.success(t("admin.fnosSettings.connectWafUpdated"));
      },
    },
  );
  if (!result) details.value = previous;
};

onMounted(load);
</script>

<template>
  <div class="flex items-center justify-between bg-muted/10 p-6">
    <div class="space-y-1 pr-6">
      <Label
        :for="`${a11yId}-fnos-connect-waf`"
        class="cursor-pointer text-base font-medium"
      >
        {{ t("admin.fnosSettings.connectWafTitle") }}
      </Label>
      <div class="text-sm text-muted-foreground">
        {{ t("admin.fnosSettings.connectWafDescription") }}
      </div>
      <div v-if="details" class="text-xs leading-5" :class="statusClass">
        {{
          t(`admin.fnosSettings.${statusKey}`, {
            source: details.runtime.detected_http_port ?? "-",
            listener: details.runtime.listener_port ?? "-",
          })
        }}
      </div>
      <div
        v-if="details?.runtime.last_error"
        class="text-xs leading-5 text-destructive"
      >
        {{
          t("admin.fnosSettings.connectWafLastError", {
            message: details.runtime.last_error,
          })
        }}
      </div>
    </div>
    <Switch
      :id="`${a11yId}-fnos-connect-waf`"
      :model-value="enabled"
      :disabled="isLoading || isSaving || !details?.availability.available"
      @update:model-value="save($event === true)"
    />
  </div>
</template>
