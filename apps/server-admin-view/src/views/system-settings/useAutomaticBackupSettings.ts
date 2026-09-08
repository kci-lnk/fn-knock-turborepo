import {
  computed,
  onBeforeUnmount,
  onMounted,
  reactive,
  ref,
  useId,
} from "vue";
import { useI18n } from "vue-i18n";
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

import {
  defaultBackupEmail,
  backupEmailPayload,
  isBackupEmailValid,
  type BackupEmailForm,
} from "@/lib/backup-email";

export function useAutomaticBackupSettings(
  emailOnly = false,
  onFilesChanged: () => void = () => {},
) {
  const emailForm = ref<BackupEmailForm>(defaultBackupEmail());

  const { locale, t } = useI18n();
  const a11yId = useId();
  const details = ref<AutomaticBackupDetails | null>(null);
  const isLoading = ref(false);
  const isSaving = ref(false);
  const loadErrorMessage = ref("");
  const saveErrorMessage = ref("");
  let refreshTimer: number | null = null;
  let disposed = false;
  let statusGeneration = 0;
  let statusPolling = false;

  async function refreshStatus(): Promise<AutomaticBackupDetails | undefined> {
    if (disposed || statusPolling || isSaving.value || isLoading.value) return;
    statusPolling = true;
    const generation = statusGeneration;
    try {
      const next = await MaintenanceAPI.getAutomaticBackupDetails();
      if (disposed || generation !== statusGeneration) return;
      if (details.value) details.value.status = next.status;
      return next;
    } finally {
      statusPolling = false;
    }
  }

  const form = reactive({
    enabled: false,
    interval_hours: 24,
    retention_days: 7,
  });

  const isValid = computed(() =>
    emailOnly
      ? isBackupEmailValid(emailForm.value)
      : isAutomaticBackupConfigValid(form.interval_hours, form.retention_days),
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
    if (!config) return false;
    return emailOnly
      ? JSON.stringify(backupEmailPayload(emailForm.value)) !==
          JSON.stringify(
            backupEmailPayload(config.email ?? defaultBackupEmail()),
          )
      : form.enabled !== config.enabled ||
          form.interval_hours !== config.interval_hours ||
          form.retention_days !== config.retention_days;
  });

  function applyDetails(value: AutomaticBackupDetails) {
    if (disposed) return;
    details.value = value;
    const email = value.config.email ?? defaultBackupEmail();
    emailForm.value = {
      ...email,
      smtp: { ...email.smtp },
      to_addresses: [...email.to_addresses],
    };
    form.enabled = value.config.enabled;
    form.interval_hours = value.config.interval_hours;
    form.retention_days = value.config.retention_days;
  }

  async function load() {
    statusGeneration += 1;
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
      !emailOnly && form.enabled && details.value?.config.enabled !== true;
    statusGeneration += 1;
    isSaving.value = true;
    saveErrorMessage.value = "";
    try {
      // Email editing must not overwrite backup scheduling changed on another page.
      const schedule = emailOnly
        ? (await MaintenanceAPI.getAutomaticBackupDetails()).config
        : form;
      applyDetails(
        await MaintenanceAPI.updateAutomaticBackupConfig({
          ...(emailOnly ? { email: backupEmailPayload(emailForm.value) } : {}),
          enabled: schedule.enabled,
          interval_hours: schedule.interval_hours,
          retention_days: schedule.retention_days,
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
    if (disposed) return;
    if (refreshTimer !== null) window.clearTimeout(refreshTimer);
    if (attempt >= AUTOMATIC_BACKUP_RESULT_POLL_LIMIT) return;
    refreshTimer = window.setTimeout(async () => {
      try {
        const next = await refreshStatus();
        if (
          next &&
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
            onFilesChanged();
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

  let emailRefreshTimer: ReturnType<typeof setInterval> | undefined;
  onMounted(() => {
    void load();
    emailRefreshTimer = setInterval(async () => {
      if (!details.value || isSaving.value || isLoading.value) return;
      try {
        await refreshStatus();
      } catch {
        /* Retry on the next status poll. */
      }
    }, 5000);
  });
  onBeforeUnmount(() => {
    disposed = true;
    statusGeneration += 1;
    if (emailRefreshTimer) clearInterval(emailRefreshTimer);
    if (refreshTimer !== null) window.clearTimeout(refreshTimer);
  });

  return {
    t,
    a11yId,
    details,
    form,
    emailForm,
    isLoading,
    isSaving,
    isDirty,
    isValid,
    intervalIsInvalid,
    retentionIsInvalid,
    requestErrorMessage,
    formatDate,
    load,
    reset,
    save,
  };
}
