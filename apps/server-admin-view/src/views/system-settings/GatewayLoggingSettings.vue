<script setup lang="ts">
import { computed, onMounted, reactive, ref, useId } from "vue";
import { useI18n } from "vue-i18n";
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
import { GatewayLogsAPI } from "@/lib/api/gateway";
import { docsUrls } from "../../lib/docs";
import GatewayLoggingDirectory from "./GatewayLoggingDirectory.vue";
import type { GatewayLoggingConfig } from "../../types";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { useConfigStore } from "../../store/config";

const a11yId = useId();

const configStore = useConfigStore();
const { t } = useI18n();
const settings = ref<GatewayLoggingConfig | null>(null);
const form = reactive<
  Pick<
    GatewayLoggingConfig,
    | "enabled"
    | "record_localhost"
    | "max_days"
    | "custom_logs_dir"
    | "max_daily_size_mb"
    | "max_total_size_mb"
  >
>({
  enabled: false,
  record_localhost: false,
  max_days: 7,
  max_daily_size_mb: 256,
  max_total_size_mb: 1024,
  custom_logs_dir: "",
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
    settings.value.record_localhost !== form.record_localhost ||
    settings.value.max_days !== Number(form.max_days) ||
    (settings.value.max_daily_size_mb ?? 256) !==
      Number(form.max_daily_size_mb) ||
    (settings.value.max_total_size_mb ?? 1024) !==
      Number(form.max_total_size_mb) ||
    settings.value.custom_logs_dir !== form.custom_logs_dir
  );
});
const droppedEntries = computed(() =>
  Math.max(0, Number(settings.value?.dropped_entries ?? 0)),
);
const formatCount = (value: number) => new Intl.NumberFormat().format(value);

const applyFromSettings = (data: GatewayLoggingConfig) => {
  settings.value = data;
  form.enabled = data.enabled;
  form.record_localhost = data.record_localhost;
  form.max_days = data.max_days;
  form.max_daily_size_mb = data.max_daily_size_mb ?? 256;
  form.max_total_size_mb = data.max_total_size_mb ?? 1024;
  form.custom_logs_dir = data.custom_logs_dir || "";
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

const validCapacity = computed(
  () =>
    Number.isInteger(form.max_daily_size_mb) &&
    Number.isInteger(form.max_total_size_mb) &&
    form.max_daily_size_mb >= 1 &&
    form.max_total_size_mb >= form.max_daily_size_mb &&
    form.max_total_size_mb <= 1_048_576,
);
const formatMiB = (bytes: number | undefined) =>
  new Intl.NumberFormat(undefined, { maximumFractionDigits: 1 }).format(
    (bytes ?? 0) / 1048576,
  );
const saveSettings = async () => {
  if (!validCapacity.value) {
    toast.error(t("admin.gatewayLogging.invalidCapacity"));
    return;
  }
  await runSaveSettings(
    async () => {
      try {
        return await GatewayLogsAPI.updateConfig({
          custom_logs_dir: form.custom_logs_dir,
          enabled: form.enabled,
          record_localhost: form.record_localhost,
          max_days: Math.max(1, Math.floor(Number(form.max_days) || 1)),
          max_daily_size_mb: form.max_daily_size_mb,
          max_total_size_mb: form.max_total_size_mb,
        });
      } catch (error) {
        // A timeout or failed rollback may leave the gateway on another directory.
        // Refresh actual state without discarding the user's unsaved draft.
        try {
          settings.value = await GatewayLogsAPI.getConfig();
        } catch {
          if (settings.value)
            settings.value = { ...settings.value, logs_dir: "" };
        }
        throw error;
      }
    },
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
            {{ t("admin.gatewayLogging.storageDescription") }}
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
            :for="`${a11yId}-gatewayloggingsettings-1`"
            class="cursor-pointer text-base font-medium"
            @click="form.enabled = !form.enabled"
          >
            {{ t("admin.gatewayLogging.enableLabel") }}
          </Label>
          <div class="text-sm text-muted-foreground">
            {{ t("admin.gatewayLogging.enableDescription") }}
          </div>
        </div>
        <Switch
          :id="`${a11yId}-gatewayloggingsettings-1`"
          v-model="form.enabled"
          :disabled="isSaving"
        />
      </div>

      <div class="flex items-center justify-between bg-muted/10 p-6">
        <div class="space-y-1 pr-6">
          <Label
            :for="`${a11yId}-gatewayloggingsettings-2`"
            class="cursor-pointer text-base font-medium"
            @click="form.record_localhost = !form.record_localhost"
          >
            {{ t("admin.gatewayLogging.recordLocalhostLabel") }}
          </Label>
          <div class="text-sm text-muted-foreground">
            {{ t("admin.gatewayLogging.recordLocalhostDescription") }}
          </div>
        </div>
        <Switch
          :id="`${a11yId}-gatewayloggingsettings-2`"
          v-model="form.record_localhost"
          :disabled="isSaving"
        />
      </div>

      <div
        class="flex flex-col justify-between gap-4 p-6 sm:flex-row sm:items-center"
      >
        <div class="space-y-1 pr-6">
          <Label :for="`${a11yId}-gatewayloggingsettings-3`" class="text-base">
            {{ t("admin.gatewayLogging.retentionLabel") }}
          </Label>
          <div class="text-sm text-muted-foreground">
            {{ t("admin.gatewayLogging.retentionDescription") }}
          </div>
        </div>
        <div class="flex items-center gap-2 shrink-0">
          <Input
            :id="`${a11yId}-gatewayloggingsettings-3`"
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

      <div class="space-y-4 p-6">
        <p class="text-sm text-muted-foreground">
          {{ t("admin.gatewayLogging.capacityDescription") }}
        </p>
        <div class="grid gap-4 sm:grid-cols-2">
          <div
            v-for="field in ['max_daily_size_mb', 'max_total_size_mb'] as const"
            :key="field"
            class="space-y-2"
          >
            <Label :for="`${a11yId}-${field}`">{{
              t(
                `admin.gatewayLogging.${field === "max_daily_size_mb" ? "dailyCapacity" : "totalCapacity"}`,
              )
            }}</Label>
            <div class="flex items-center gap-2">
              <Input
                :id="`${a11yId}-${field}`"
                v-model.number="form[field]"
                :data-testid="field"
                type="number"
                min="1"
                max="1048576"
                step="1"
                :disabled="isSaving"
                :aria-invalid="!validCapacity"
                class="w-36"
              />
              <span class="text-sm text-muted-foreground">MiB</span>
            </div>
          </div>
        </div>
        <p v-if="!validCapacity" role="alert" class="text-sm text-destructive">
          {{ t("admin.gatewayLogging.invalidCapacity") }}
        </p>
        <p class="text-sm text-muted-foreground">
          {{
            t("admin.gatewayLogging.capacityUsage", {
              today: formatMiB(settings?.today_size_bytes),
              total: formatMiB(settings?.total_size_bytes),
            })
          }}
        </p>
        <p class="text-sm text-muted-foreground">
          {{ t("admin.gatewayLogging.retainedOnly") }}
        </p>
        <Alert
          v-if="settings?.cleanup_error || settings?.capacity_dropped_entries"
          variant="destructive"
        >
          <AlertTitle>{{
            t("admin.gatewayLogging.capacityWarning")
          }}</AlertTitle>
          <AlertDescription>
            {{
              t("admin.gatewayLogging.capacityDropped", {
                count: formatCount(settings?.capacity_dropped_entries ?? 0),
              })
            }}
            <span v-if="settings?.cleanup_error" class="block break-all">{{
              settings.cleanup_error
            }}</span>
          </AlertDescription>
        </Alert>
      </div>

      <GatewayLoggingDirectory
        v-model="form.custom_logs_dir"
        :actual-directory="settings?.logs_dir || ''"
        :default-directory="settings?.default_logs_dir || ''"
        :disabled="isSaving"
      />

      <div v-if="droppedEntries > 0" class="p-6">
        <Alert class="border-amber-200 bg-amber-50 text-amber-950">
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
