<script setup lang="ts">
import {
  computed,
  onBeforeUnmount,
  onMounted,
  reactive,
  ref,
  useId,
} from "vue";
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { DatabaseBackup, Loader2, RotateCcw, Save } from "lucide-vue-next";
import { toast } from "@admin-shared/utils/toast";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import { MaintenanceAPI } from "@/lib/api/config";
import {
  AUTOMATIC_BACKUP_INTERVAL_RANGE,
  AUTOMATIC_BACKUP_RESULT_POLL_LIMIT,
  AUTOMATIC_BACKUP_RETENTION_RANGE,
  automaticBackupAttemptCompleted,
  automaticBackupAttemptSucceeded,
  isAutomaticBackupConfigValid,
} from "@/lib/automatic-backup";
import type { AutomaticBackupDetails } from "@/types";

const emit = defineEmits<{ filesChanged: [] }>();
const { locale, t } = useI18n();
const a11yId = useId();
const details = ref<AutomaticBackupDetails | null>(null);
const isLoading = ref(false);
const isSaving = ref(false);
const loadErrorMessage = ref("");
const saveErrorMessage = ref("");
let refreshTimer: number | null = null;

const form = reactive({
  enabled: false,
  interval_hours: 24,
  retention_days: 7,
});

const isValid = computed(() =>
  isAutomaticBackupConfigValid(form.interval_hours, form.retention_days),
);
const intervalIsInvalid = computed(
  () =>
    !Number.isInteger(form.interval_hours) ||
    form.interval_hours < AUTOMATIC_BACKUP_INTERVAL_RANGE.min ||
    form.interval_hours > AUTOMATIC_BACKUP_INTERVAL_RANGE.max,
);
const retentionIsInvalid = computed(
  () =>
    !Number.isInteger(form.retention_days) ||
    form.retention_days < AUTOMATIC_BACKUP_RETENTION_RANGE.min ||
    form.retention_days > AUTOMATIC_BACKUP_RETENTION_RANGE.max,
);
const requestErrorMessage = computed(
  () => saveErrorMessage.value || loadErrorMessage.value,
);
const isDirty = computed(() => {
  const config = details.value?.config;
  return (
    !!config &&
    (form.enabled !== config.enabled ||
      form.interval_hours !== config.interval_hours ||
      form.retention_days !== config.retention_days)
  );
});

function applyDetails(value: AutomaticBackupDetails) {
  details.value = value;
  form.enabled = value.config.enabled;
  form.interval_hours = value.config.interval_hours;
  form.retention_days = value.config.retention_days;
}

async function load() {
  isLoading.value = true;
  loadErrorMessage.value = "";
  try {
    applyDetails(await MaintenanceAPI.getAutomaticBackupDetails());
  } catch (error) {
    loadErrorMessage.value = extractErrorMessage(
      error,
      t("admin.maintenanceSettings.automaticLoadFailedDescription"),
    );
    toast.error(t("admin.maintenanceSettings.automaticLoadFailed"), {
      description: loadErrorMessage.value,
    });
  } finally {
    isLoading.value = false;
  }
}

function reset() {
  if (details.value) applyDetails(details.value);
}

async function save() {
  if (!isValid.value) {
    toast.error(t("admin.maintenanceSettings.automaticValidationFailed"));
    return;
  }
  const previousAttempt = details.value?.status.last_attempt_at;
  const previousSuccess = details.value?.status.last_success_at;
  const shouldWatchFirstBackup =
    form.enabled && details.value?.config.enabled !== true;
  isSaving.value = true;
  saveErrorMessage.value = "";
  try {
    applyDetails(
      await MaintenanceAPI.updateAutomaticBackupConfig({
        enabled: form.enabled,
        interval_hours: form.interval_hours,
        retention_days: form.retention_days,
      }),
    );
    toast.success(t("admin.maintenanceSettings.automaticSaved"));
    if (shouldWatchFirstBackup)
      pollForBackupResult(previousAttempt, previousSuccess, 0);
  } catch (error) {
    saveErrorMessage.value = extractErrorMessage(
      error,
      t("admin.maintenanceSettings.automaticSaveFailedDescription"),
    );
    toast.error(t("admin.maintenanceSettings.automaticSaveFailed"), {
      description: saveErrorMessage.value,
    });
  } finally {
    isSaving.value = false;
  }
}

function pollForBackupResult(
  previousAttempt: string | null | undefined,
  previousSuccess: string | null | undefined,
  attempt: number,
) {
  if (refreshTimer !== null) window.clearTimeout(refreshTimer);
  if (attempt >= AUTOMATIC_BACKUP_RESULT_POLL_LIMIT) return;
  refreshTimer = window.setTimeout(async () => {
    try {
      const next = await MaintenanceAPI.getAutomaticBackupDetails();
      applyDetails(next);
      if (
        automaticBackupAttemptCompleted(
          previousAttempt,
          next.status.last_attempt_at,
        )
      ) {
        if (
          automaticBackupAttemptSucceeded(
            previousSuccess,
            next.status.last_success_at,
          )
        )
          emit("filesChanged");
        return;
      }
    } catch {
      // A transient status request must not stop first-backup monitoring.
    }
    pollForBackupResult(previousAttempt, previousSuccess, attempt + 1);
  }, 1000);
}

function formatDate(value: string | null | undefined) {
  if (!value) return t("admin.maintenanceSettings.notAvailable");
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat(locale.value, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}

onMounted(load);
onBeforeUnmount(() => {
  if (refreshTimer !== null) window.clearTimeout(refreshTimer);
});
</script>

<template>
  <div
    class="px-6 py-6 sm:px-8"
    data-a11y-scope="automatic-backup-settings"
    role="region"
    :aria-labelledby="`${a11yId}-title`"
    :aria-describedby="`${a11yId}-description`"
    :aria-busy="isLoading || isSaving"
  >
    <p
      :id="`${a11yId}-activity-status`"
      class="sr-only"
      role="status"
      aria-live="polite"
      aria-atomic="true"
    >
      {{
        isLoading
          ? t("admin.maintenanceSettings.automaticLoading")
          : isSaving
            ? t("admin.maintenanceSettings.automaticSaving")
            : ""
      }}
    </p>
    <div
      class="flex flex-col gap-5 lg:flex-row lg:items-start lg:justify-between"
    >
      <div class="min-w-0 flex-1 space-y-2">
        <h2
          :id="`${a11yId}-title`"
          class="flex items-center gap-2 text-sm font-medium"
        >
          <DatabaseBackup class="h-4 w-4" aria-hidden="true" />
          <span>{{ t("admin.maintenanceSettings.automaticTitle") }}</span>
        </h2>
        <p
          :id="`${a11yId}-description`"
          class="max-w-3xl text-sm text-muted-foreground"
        >
          {{ t("admin.maintenanceSettings.automaticDescription") }}
        </p>
        <p
          v-if="details?.status.directory_path"
          class="break-all text-xs text-muted-foreground"
        >
          {{ t("admin.maintenanceSettings.automaticDirectory") }}:
          <code>{{ details.status.directory_path }}</code>
        </p>
      </div>

      <div class="flex shrink-0 items-center gap-3">
        <Label :for="`${a11yId}-enabled`">
          {{ t("admin.maintenanceSettings.automaticEnabled") }}
        </Label>
        <Switch
          :id="`${a11yId}-enabled`"
          v-model="form.enabled"
          :disabled="isLoading || isSaving || !details"
          :aria-describedby="`${a11yId}-description`"
        />
      </div>
    </div>

    <p
      v-if="requestErrorMessage"
      :id="`${a11yId}-request-error`"
      class="mt-4 text-sm text-destructive"
      role="alert"
    >
      {{ requestErrorMessage }}
    </p>

    <div
      class="mt-5 grid gap-4 rounded-xl border bg-muted/[0.08] p-4 md:grid-cols-2"
    >
      <div class="space-y-2">
        <Label :for="`${a11yId}-interval`">{{
          t("admin.maintenanceSettings.automaticInterval")
        }}</Label>
        <div class="flex items-center gap-2">
          <Input
            :id="`${a11yId}-interval`"
            v-model.number="form.interval_hours"
            type="number"
            min="1"
            max="8760"
            step="1"
            :disabled="isLoading || isSaving || !details"
            :aria-invalid="intervalIsInvalid"
            :aria-describedby="`${a11yId}-interval-help`"
          />
          <span
            class="shrink-0 text-sm text-muted-foreground"
            aria-hidden="true"
            >{{ t("admin.maintenanceSettings.hoursUnit") }}</span
          >
        </div>
        <p
          :id="`${a11yId}-interval-help`"
          class="text-xs"
          :class="
            intervalIsInvalid ? 'text-destructive' : 'text-muted-foreground'
          "
          :role="intervalIsInvalid ? 'alert' : undefined"
        >
          {{
            t(
              intervalIsInvalid
                ? "admin.maintenanceSettings.automaticIntervalError"
                : "admin.maintenanceSettings.automaticIntervalHelp",
            )
          }}
        </p>
      </div>
      <div class="space-y-2">
        <Label :for="`${a11yId}-retention`">{{
          t("admin.maintenanceSettings.automaticRetention")
        }}</Label>
        <div class="flex items-center gap-2">
          <Input
            :id="`${a11yId}-retention`"
            v-model.number="form.retention_days"
            type="number"
            min="1"
            max="3650"
            step="1"
            :disabled="isLoading || isSaving || !details"
            :aria-invalid="retentionIsInvalid"
            :aria-describedby="`${a11yId}-retention-help`"
          />
          <span
            class="shrink-0 text-sm text-muted-foreground"
            aria-hidden="true"
            >{{ t("admin.maintenanceSettings.daysUnit") }}</span
          >
        </div>
        <p
          :id="`${a11yId}-retention-help`"
          class="text-xs"
          :class="
            retentionIsInvalid ? 'text-destructive' : 'text-muted-foreground'
          "
          :role="retentionIsInvalid ? 'alert' : undefined"
        >
          {{
            t(
              retentionIsInvalid
                ? "admin.maintenanceSettings.automaticRetentionError"
                : "admin.maintenanceSettings.automaticRetentionHelp",
            )
          }}
        </p>
      </div>
    </div>

    <div v-if="details" class="mt-4 space-y-2 text-xs text-muted-foreground">
      <div
        class="grid gap-2 sm:grid-cols-2"
        role="status"
        aria-live="polite"
        aria-atomic="true"
      >
        <p>
          {{ t("admin.maintenanceSettings.automaticLastSuccess") }}:
          {{ formatDate(details.status.last_success_at) }}
        </p>
        <p>
          {{ t("admin.maintenanceSettings.automaticNextBackup") }}:
          {{ formatDate(details.status.next_backup_at) }}
        </p>
      </div>
      <p
        v-if="details.status.last_error"
        class="break-words text-destructive"
        role="alert"
      >
        {{ t("admin.maintenanceSettings.automaticLastError") }}:
        {{ details.status.last_error }}
      </p>
    </div>

    <div class="mt-5 flex justify-end gap-3">
      <Button
        type="button"
        variant="outline"
        :disabled="!isDirty || isSaving"
        @click="reset"
      >
        <RotateCcw class="mr-2 h-4 w-4" aria-hidden="true" />
        {{ t("admin.maintenanceSettings.resetAutomatic") }}
      </Button>
      <Button
        type="button"
        :disabled="!isDirty || !isValid || isSaving"
        @click="save"
      >
        <Loader2
          v-if="isSaving"
          class="mr-2 h-4 w-4 animate-spin"
          aria-hidden="true"
        />
        <Save v-else class="mr-2 h-4 w-4" aria-hidden="true" />
        {{
          t(
            isSaving
              ? "admin.maintenanceSettings.automaticSaving"
              : "admin.maintenanceSettings.saveAutomatic",
          )
        }}
      </Button>
    </div>
  </div>
</template>
