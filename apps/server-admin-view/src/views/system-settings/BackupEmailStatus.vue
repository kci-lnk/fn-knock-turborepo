<script setup lang="ts">
import { backupEmailErrorKey } from "@/lib/backup-email";
import { useI18n } from "vue-i18n";
import type { BackupEmailStatus } from "@/types";
defineProps<{ status: BackupEmailStatus }>();
const { t, locale } = useI18n();
function date(value: string | null) {
  return value
    ? new Date(value).toLocaleString(locale.value)
    : t("admin.maintenanceSettings.notAvailable");
}
</script>
<template>
  <div class="mt-4 space-y-2 text-sm" role="status" aria-live="polite">
    <p>
      {{ t("admin.maintenanceSettings.emailLastAttempt") }}:
      {{ date(status.last_attempt_at) }}
    </p>
    <p>
      {{ t("admin.maintenanceSettings.emailLastSuccess") }}:
      {{ date(status.last_success_at) }}
    </p>
    <p v-if="status.last_filename" class="break-all">
      {{ status.last_filename }}
    </p>
    <p>
      {{ t("admin.maintenanceSettings.emailPending") }}:
      {{ status.pending_count }}
    </p>
    <p v-if="status.next_retry_at">
      {{ t("admin.maintenanceSettings.emailNextRetry") }}:
      {{ date(status.next_retry_at) }}
    </p>
    <p v-if="status.last_error" class="text-destructive">
      {{ t("admin.maintenanceSettings.emailDeliveryFailed") }}:
      {{ t(backupEmailErrorKey(status.last_error)) }}
    </p>
  </div>
</template>
