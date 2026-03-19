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
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Skeleton } from "@/components/ui/skeleton";
import DocsLinkButton from "@/components/DocsLinkButton.vue";
import { toast } from "@admin-shared/utils/toast";
import { GatewayLogsAPI } from "../../lib/api";
import { docsUrls } from "../../lib/docs";
import type { GatewayLoggingConfig } from "../../types";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { useConfigStore } from "../../store/config";

const configStore = useConfigStore();
const settings = ref<GatewayLoggingConfig | null>(null);
const form = reactive<GatewayLoggingConfig>({
  enabled: false,
  max_days: 7,
  logs_dir: "",
});

const { isPending: isLoading, run: runLoadSettings } = useAsyncAction({
  onError: (error) => {
    toast.error("加载失败", {
      description: extractErrorMessage(error, "无法获取请求日志设置"),
    });
  },
});
const showLoadingSkeleton = useDelayedLoading(isLoading);
const { isPending: isSaving, run: runSaveSettings } = useAsyncAction({
  onError: (error) => {
    toast.error("保存失败", {
      description: extractErrorMessage(error, "请求日志设置保存失败"),
    });
  },
});

const isDirty = computed(() => {
  if (!settings.value) return false;
  return (
    settings.value.enabled !== form.enabled ||
    settings.value.max_days !== Number(form.max_days)
  );
});

const applyFromSettings = (data: GatewayLoggingConfig) => {
  settings.value = data;
  form.enabled = data.enabled;
  form.max_days = data.max_days;
  form.logs_dir = data.logs_dir || "";
};

const fetchSettings = async () => {
  await runLoadSettings(async () => {
    const data = await GatewayLogsAPI.getConfig();
    applyFromSettings(data);
  });
};

const resetForm = () => {
  if (settings.value) applyFromSettings(settings.value);
};

const saveSettings = async () => {
  await runSaveSettings(
    () =>
      GatewayLogsAPI.updateConfig({
        enabled: form.enabled,
        max_days: Math.max(1, Math.floor(Number(form.max_days) || 1)),
      }),
    {
      onSuccess: async (data) => {
        applyFromSettings(data);
        toast.success("请求日志设置已更新");
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
      <div class="flex items-start justify-between gap-3">
        <div class="space-y-1.5">
          <CardTitle class="text-md">网关请求日志</CardTitle>
          <CardDescription>
            开启后，Go 网关会在运行目录下的 <code>logs</code> 目录按天写入 JSON
            结构化请求日志，方便在后台按日期查询、搜索和查看详情。
          </CardDescription>
        </div>
        <DocsLinkButton :href="docsUrls.guides.requestLogs" />
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
            启用请求日志
          </Label>
          <div class="text-sm text-muted-foreground">
            默认关闭。开启后会记录访问者
            IP、请求地址、登录状态、响应状态、耗时和上游目标等信息。
          </div>
        </div>
        <Switch v-model="form.enabled" :disabled="isSaving" />
      </div>

      <div
        class="flex flex-col justify-between gap-4 p-6 sm:flex-row sm:items-center"
      >
        <div class="space-y-1 pr-6">
          <Label class="text-base">日志保留天数</Label>
          <div class="text-sm text-muted-foreground">
            按天切分日志文件，只保留最近 N 天。超出的旧文件会自动清理。
          </div>
        </div>
        <div class="flex items-center gap-2 shrink-0">
          <Input
            v-model.number="form.max_days"
            type="number"
            min="1"
            step="1"
            class="w-24 text-center"
            :disabled="isSaving"
          />
          <span class="w-12 text-sm text-muted-foreground">天</span>
        </div>
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
