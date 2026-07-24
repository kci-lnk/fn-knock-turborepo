<script setup lang="ts">
import { useI18n } from "vue-i18n";
import DataShareFilePicker from "@admin-shared/components/common/DataShareFilePicker.vue";
import { KNOCK_BACKUP_EXTENSION } from "@admin-shared/utils/maintenanceBackup";
import type {
  AutomaticBackupFilesPayload,
  SharedDataFileEntry,
} from "@/types";

defineProps<{
  files: AutomaticBackupFilesPayload;
  loading: boolean;
  selecting: boolean;
  errorMessage: string;
}>();

const open = defineModel<boolean>("open", { required: true });
const emit = defineEmits<{
  refresh: [];
  select: [file: SharedDataFileEntry];
}>();
const { t } = useI18n();
</script>

<template>
  <DataShareFilePicker
    v-model:open="open"
    :title="t('admin.maintenanceSettings.automaticPickerTitle')"
    :description="t('admin.maintenanceSettings.automaticPickerDescription')"
    :directory-label="t('admin.maintenanceSettings.automaticPickerDirectory')"
    :files="files.files"
    :supported-file-types="[KNOCK_BACKUP_EXTENSION]"
    :available="files.available"
    :loading="loading"
    :selecting="selecting"
    :error-message="errorMessage"
    :alert-title="t('admin.maintenanceSettings.automaticPickerAlertTitle')"
    :available-description="
      t('admin.maintenanceSettings.automaticPickerAvailableDescription', {
        path: files.directoryPath,
      })
    "
    :unavailable-description="
      t('admin.maintenanceSettings.automaticPickerUnavailableDescription')
    "
    :empty-title="t('admin.maintenanceSettings.automaticPickerEmptyTitle')"
    :empty-description="
      t('admin.maintenanceSettings.automaticPickerEmptyDescription')
    "
    :confirm-text="t('admin.maintenanceSettings.pickerConfirmText')"
    @refresh="emit('refresh')"
    @select="emit('select', $event)"
  />
</template>
