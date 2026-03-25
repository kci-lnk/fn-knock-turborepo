<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from "vue";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import { toast } from "@admin-shared/utils/toast";
import { SystemAPI } from "../../lib/api";
import type { ProtocolMappingFeatureConfig } from "../../types";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { useConfigStore } from "../../store/config";

const configStore = useConfigStore();
const settings = ref<ProtocolMappingFeatureConfig | null>(null);
const form = reactive<ProtocolMappingFeatureConfig>({
  enabled: false,
});
const runTypeLabelMap = {
  0: "直连模式",
  1: "反代模式",
  3: "子域模式",
} as const;

const { isPending: isLoading, run: runLoadSettings } = useAsyncAction({
  onError: (error) => {
    toast.error("加载失败", {
      description: extractErrorMessage(error, "无法获取功能设置"),
    });
  },
});
const showLoadingSkeleton = useDelayedLoading(isLoading);
const { isPending: isSaving, run: runSaveSettings } = useAsyncAction({
  onError: (error) => {
    toast.error("保存失败", {
      description: extractErrorMessage(error, "功能设置保存失败"),
    });
  },
});
const isProtocolMappingAvailable = computed(
  () => configStore.config?.run_type === 3,
);
const currentRunTypeLabel = computed(() => {
  const runType = configStore.config?.run_type;
  if (runType === 0 || runType === 1 || runType === 3) {
    return runTypeLabelMap[runType];
  }
  return "当前模式";
});
const protocolMappingDisabledReason = computed(() => {
  if (isProtocolMappingAvailable.value) return "";
  return `仅子域模式可开启，当前为${currentRunTypeLabel.value}。`;
});

const isDirty = computed(() => {
  if (!settings.value) return false;
  return settings.value.enabled !== form.enabled;
});

const syncFormEnabledWithAvailability = () => {
  if (!settings.value) return;
  form.enabled = isProtocolMappingAvailable.value
    ? settings.value.enabled
    : false;
};

const applyFromSettings = (data: ProtocolMappingFeatureConfig) => {
  settings.value = data;
  syncFormEnabledWithAvailability();
};

const fetchSettings = async () => {
  await runLoadSettings(async () => {
    const data = await SystemAPI.getProtocolMappingFeatureConfig();
    applyFromSettings(data);
  });
};

const resetForm = () => {
  if (settings.value) applyFromSettings(settings.value);
};

const toggleEnabled = () => {
  if (!isProtocolMappingAvailable.value) return;
  form.enabled = !form.enabled;
};

const saveSettings = async () => {
  await runSaveSettings(
    () =>
      SystemAPI.updateProtocolMappingFeatureConfig({
        enabled: form.enabled,
      }),
    {
      onSuccess: async (data) => {
        applyFromSettings(data);
        toast.success("功能设置已更新");
        await configStore.loadConfig();
      },
    },
  );
};

onMounted(fetchSettings);

watch(
  () => configStore.config?.run_type,
  () => {
    syncFormEnabledWithAvailability();
  },
);
</script>

<template>
  <Card>
    <CardHeader>
      <div class="space-y-1.5">
        <CardTitle class="text-md">功能开关</CardTitle>
        <CardDescription>
          控制可选功能的启用状态。
        </CardDescription>
      </div>
    </CardHeader>

    <CardContent v-if="isLoading && showLoadingSkeleton" class="border-t p-0">
      <div class="space-y-4 p-6">
        <Skeleton class="h-6 w-1/3" />
        <Skeleton class="h-4 w-2/3" />
      </div>
    </CardContent>

    <CardContent v-else-if="!isLoading" class="border-t p-0 divide-y">
      <div class="flex items-center justify-between bg-muted/10 p-6">
        <div class="space-y-1 pr-6">
          <Label
            class="text-base font-medium"
            :class="
              isProtocolMappingAvailable
                ? 'cursor-pointer'
                : 'cursor-not-allowed text-zinc-500'
            "
            @click="toggleEnabled"
          >
            协议映射
          </Label>
          <div
            class="text-sm"
            :class="
              isProtocolMappingAvailable
                ? 'text-muted-foreground'
                : 'text-zinc-500'
            "
          >
            开启后，显示“协议映射”入口并启用 TCP/UDP
            转发
          </div>
          <div
            v-if="!isProtocolMappingAvailable"
            class="text-xs leading-5 text-zinc-500"
          >
            {{ protocolMappingDisabledReason }}
          </div>
        </div>
        <Switch
          :model-value="isProtocolMappingAvailable ? form.enabled : false"
          :disabled="!isProtocolMappingAvailable || isSaving"
          @update:model-value="form.enabled = $event === true"
        />
      </div>

      <div class="flex items-center justify-end gap-3 p-6">
        <Button
          variant="outline"
          :disabled="!isDirty || isSaving"
          @click="resetForm"
        >
          重置
        </Button>
        <Button :disabled="!isDirty || isSaving" @click="saveSettings">
          保存设置
        </Button>
      </div>
    </CardContent>
  </Card>
</template>
