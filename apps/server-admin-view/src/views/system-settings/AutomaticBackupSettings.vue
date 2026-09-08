<script setup lang="ts">
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import {
  DatabaseBackup,
  Loader2,
  RotateCcw,
  Save,
  Mail,
  ChevronRight,
} from "lucide-vue-next";
import { useAutomaticBackupSettings } from "./useAutomaticBackupSettings";
const emit = defineEmits<{ filesChanged: [] }>();
const {
  t,
  a11yId,
  details,
  form,
  isLoading,
  isSaving,
  isDirty,
  isValid,
  intervalIsInvalid,
  retentionIsInvalid,
  requestErrorMessage,
  formatDate,
  reset,
  save,
} = useAutomaticBackupSettings(false, () => emit("filesChanged"));
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

    <a
      href="#/system/backup-email"
      class="group mt-6 flex items-center gap-4 rounded-xl border p-4 transition-colors hover:bg-muted/40 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      data-testid="backup-email-entry"
    >
      <span class="rounded-lg bg-muted p-2.5"
        ><Mail class="h-5 w-5" aria-hidden="true"
      /></span>
      <span class="min-w-0 flex-1 space-y-1">
        <span class="flex flex-wrap items-center gap-2 font-medium">
          {{ t("admin.maintenanceSettings.emailTitle") }}
          <Badge variant="secondary">{{
            t(
              details?.config.email?.enabled
                ? "admin.maintenanceSettings.emailStateEnabled"
                : "admin.maintenanceSettings.emailStateDisabled",
            )
          }}</Badge>
        </span>
        <span class="block text-sm text-muted-foreground">{{
          t("admin.maintenanceSettings.emailEntryDescription")
        }}</span>
      </span>
      <ChevronRight
        class="h-4 w-4 shrink-0 text-muted-foreground"
        aria-hidden="true"
      />
    </a>

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
