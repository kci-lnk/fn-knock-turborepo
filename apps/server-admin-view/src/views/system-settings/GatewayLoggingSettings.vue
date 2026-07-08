<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";
import { TriangleAlert } from "lucide-vue-next";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
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
import FloatingActionDock from "@admin-shared/components/common/FloatingActionDock.vue";
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
const { t } = useI18n();
const settings = ref<GatewayLoggingConfig | null>(null);
const form = reactive<Pick<GatewayLoggingConfig, "enabled" | "max_days" | "logs_dir">>({
  enabled: false,
  max_days: 7,
  logs_dir: "",
});

const { isPending: isLoading, run: runLoadSettings } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.gatewayLogging.loadFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.gatewayLogging.loadDescription"),
      ),
    });
  },
});
const showLoadingSkeleton = useDelayedLoading(isLoading);
const { isPending: isSaving, run: runSaveSettings } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.gatewayLogging.saveFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.gatewayLogging.saveDescription"),
      ),
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
const droppedEntries = computed(() =>
  Math.max(0, Number(settings.value?.dropped_entries ?? 0)),
);
const queueSize = computed(() =>
  Math.max(0, Number(settings.value?.queue_size ?? 0)),
);
const queueDepth = computed(() =>
  Math.max(0, Number(settings.value?.queue_depth ?? 0)),
);
const queueUsageLabel = computed(() =>
  queueSize.value > 0 ? `${queueDepth.value}/${queueSize.value}` : "0/0",
);
const formatCount = (value: number) => new Intl.NumberFormat().format(value);

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
        toast.success(t("admin.gatewayLogging.updated"));
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
          <CardTitle class="text-md">
            {{ t("admin.gatewayLogging.title") }}
          </CardTitle>
          <CardDescription>
            {{ t("admin.gatewayLogging.descriptionPrefix") }}
            <code>logs</code>
            {{ t("admin.gatewayLogging.descriptionSuffix") }}
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
            {{ t("admin.gatewayLogging.enableLabel") }}
          </Label>
          <div class="text-sm text-muted-foreground">
            {{ t("admin.gatewayLogging.enableDescription") }}
          </div>
        </div>
        <Switch v-model="form.enabled" :disabled="isSaving" />
      </div>

      <div
        class="flex flex-col justify-between gap-4 p-6 sm:flex-row sm:items-center"
      >
        <div class="space-y-1 pr-6">
          <Label class="text-base">
            {{ t("admin.gatewayLogging.retentionLabel") }}
          </Label>
          <div class="text-sm text-muted-foreground">
            {{ t("admin.gatewayLogging.retentionDescription") }}
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
          <span class="w-12 text-sm text-muted-foreground">{{
            t("admin.gatewayLogging.daysUnit")
          }}</span>
        </div>
      </div>

      <div v-if="settings" class="space-y-4 p-6">
        <div class="space-y-1">
          <Label class="text-base">
            {{ t("admin.gatewayLogging.runtimeLabel") }}
          </Label>
          <div class="text-sm text-muted-foreground">
            {{
              t("admin.gatewayLogging.runtimeDescription", {
                queue: queueUsageLabel,
                dropped: formatCount(droppedEntries),
              })
            }}
          </div>
        </div>
        <Alert
          v-if="droppedEntries > 0"
          class="border-amber-200 bg-amber-50 text-amber-950"
        >
          <TriangleAlert class="mt-0.5 h-4 w-4" />
          <AlertTitle>
            {{ t("admin.gatewayLogging.dropWarningTitle") }}
          </AlertTitle>
          <AlertDescription class="text-sm leading-6 text-amber-900">
            {{
              t("admin.gatewayLogging.dropWarningDescription", {
                count: formatCount(droppedEntries),
              })
            }}
          </AlertDescription>
        </Alert>
      </div>

      <FloatingActionDock
        :active="isDirty"
        inline-class="flex items-center justify-end gap-3 p-6"
      >
        <template #inline>
          <Button
            variant="outline"
            :disabled="!isDirty || isSaving"
            @click="resetForm"
          >
            {{ t("admin.gatewayLogging.reset") }}
          </Button>
          <Button :disabled="!isDirty || isSaving" @click="saveSettings">
            {{ t("admin.gatewayLogging.saveSettings") }}
          </Button>
        </template>
      </FloatingActionDock>
    </CardContent>
  </Card>
</template>
