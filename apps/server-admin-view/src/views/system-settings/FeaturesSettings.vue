<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
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

const isDirty = computed(() => {
  if (!settings.value) return false;
  return settings.value.enabled !== form.enabled;
});

const applyFromSettings = (data: ProtocolMappingFeatureConfig) => {
  settings.value = data;
  form.enabled = data.enabled;
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
            class="cursor-pointer text-base font-medium"
            @click="form.enabled = !form.enabled"
          >
            协议映射
          </Label>
          <div class="text-sm text-muted-foreground">
            开启后，显示“协议映射”入口并启用 TCP/UDP
            转发
          </div>
        </div>
        <Switch v-model="form.enabled" :disabled="isSaving" />
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
