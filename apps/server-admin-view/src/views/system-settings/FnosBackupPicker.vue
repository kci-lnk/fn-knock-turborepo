<script setup lang="ts">
import { useI18n } from "vue-i18n";
import DataShareFilePicker from "@admin-shared/components/common/DataShareFilePicker.vue";
import { KNOCK_BACKUP_EXTENSION } from "@admin-shared/utils/maintenanceBackup";
import type {
  BackupDirectoryFilesPayload,
  SharedDataFileEntry,
} from "@/types";

defineProps<{
  files: BackupDirectoryFilesPayload;
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
    :title="t('admin.maintenanceSettings.pickerTitle')"
    :description="t('admin.maintenanceSettings.pickerDescription')"
    :directory-label="t('admin.maintenanceSettings.pickerDirectoryLabel')"
    :files="files.files"
    :supported-file-types="[KNOCK_BACKUP_EXTENSION]"
    :available="files.available"
    :loading="loading"
    :selecting="selecting"
    :error-message="errorMessage"
    :alert-title="t('admin.maintenanceSettings.pickerAlertTitle')"
    :available-description="
      t('admin.maintenanceSettings.pickerAvailableDescription')
    "
    :unavailable-description="
      t('admin.maintenanceSettings.pickerUnavailableDescription')
    "
    :empty-title="t('admin.maintenanceSettings.pickerEmptyTitle')"
    :empty-description="t('admin.maintenanceSettings.pickerEmptyDescription')"
    :confirm-text="t('admin.maintenanceSettings.pickerConfirmText')"
    @refresh="emit('refresh')"
    @select="emit('select', $event)"
  />
</template>
