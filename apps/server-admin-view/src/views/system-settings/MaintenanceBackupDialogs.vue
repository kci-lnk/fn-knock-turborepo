<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import AutomaticBackupPicker from "./AutomaticBackupPicker.vue";
import FnosBackupPicker from "./FnosBackupPicker.vue";
import type { MaintenanceBackupController } from "./maintenance-settings-contract";

const props = defineProps<{ controller: MaintenanceBackupController }>();
const { t } = useI18n();
const {
  automaticBackupFiles,
  automaticBackupFilesError,
  backupFiles,
  backupFilesError,
  handleAutomaticFileSelect,
  handleFnosFileSelect,
  hasSelectedBackup,
  importBackup,
  isAutomaticBackupPickerOpen,
  isBackupPickerOpen,
  isImportDialogOpen,
  isImporting,
  isLoadingAutomaticBackupFiles,
  isLoadingBackupFiles,
  refreshAutomaticBackupFiles,
  refreshBackupFiles,
  selectedSummary,
  supportsSharedBackup,
} = props.controller;
</script>

<template>
<FnosBackupPicker
  v-if="supportsSharedBackup"
  v-model:open="isBackupPickerOpen"
  :files="backupFiles"
  :loading="isLoadingBackupFiles"
  :selecting="isImporting"
  :error-message="backupFilesError"
  @refresh="refreshBackupFiles"
  @select="handleFnosFileSelect"
/>

<AutomaticBackupPicker
  v-model:open="isAutomaticBackupPickerOpen"
  :files="automaticBackupFiles"
  :loading="isLoadingAutomaticBackupFiles"
  :selecting="isImporting"
  :error-message="automaticBackupFilesError"
  @refresh="refreshAutomaticBackupFiles"
  @select="handleAutomaticFileSelect"
/>

<Dialog
  :open="isImportDialogOpen"
  @update:open="isImportDialogOpen = $event"
>
  <DialogContent class="sm:max-w-[420px]">
    <DialogHeader class="space-y-2">
      <DialogTitle class="text-left">{{
        t("admin.maintenanceSettings.confirmImportTitle")
      }}</DialogTitle>
      <DialogDescription class="text-left text-sm leading-6">
        {{ t("admin.maintenanceSettings.confirmImportDescription") }}
      </DialogDescription>
    </DialogHeader>

    <div
      v-if="selectedSummary"
      class="rounded-xl border border-destructive/25 bg-destructive/5 px-4 py-3 text-sm"
    >
      <p class="font-medium text-foreground">{{ selectedSummary.name }}</p>
      <p class="mt-1 text-muted-foreground">
        {{ selectedSummary.size }} · {{ selectedSummary.sourceLabel }}
      </p>
      <p
        v-if="selectedSummary.location"
        class="mt-1 break-all text-xs text-muted-foreground"
      >
        {{ selectedSummary.location }}
      </p>
    </div>

    <DialogFooter class="gap-2">
      <Button
        variant="outline"
        :disabled="isImporting"
        @click="isImportDialogOpen = false"
      >
        {{ t("common.cancel") }}
      </Button>
      <Button
        variant="destructive"
        :disabled="isImporting || !hasSelectedBackup"
        @click="importBackup"
      >
        {{
          isImporting
            ? t("admin.maintenanceSettings.importingNow")
            : t("admin.maintenanceSettings.confirmImport")
        }}
      </Button>
    </DialogFooter>
  </DialogContent>
</Dialog>
</template>
