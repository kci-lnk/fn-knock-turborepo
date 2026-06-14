<template>
  <div class="grid gap-5">
    <CertSourceField
      id="cert-input"
      field-key="cert"
      :label="t('shared.certForm.sslCert')"
      :value="cert"
      accept=".crt,.pem"
      :supported-file-types="['.crt', '.pem']"
      placeholder="-----BEGIN CERTIFICATE-----\n..."
      :share-name="shareName"
      :shared-files="sharedFiles"
      :shared-files-available="sharedFilesAvailable"
      :shared-files-loading="sharedFilesLoading"
      :shared-files-error="sharedFilesError"
      :shared-file-selecting="sharedFileSelecting"
      @update:value="(value) => emit('update:cert', value)"
      @request-shared-files="(payload) => emit('request-shared-files', payload)"
      @select-shared-file="(payload) => emit('select-shared-file', payload)"
    />

    <CertSourceField
      id="key-input"
      field-key="sslKey"
      :label="t('shared.certForm.privateKey')"
      :value="sslKey"
      accept=".key,.pem"
      :supported-file-types="['.key', '.pem']"
      placeholder="-----BEGIN PRIVATE KEY-----\n..."
      :share-name="shareName"
      :shared-files="sharedFiles"
      :shared-files-available="sharedFilesAvailable"
      :shared-files-loading="sharedFilesLoading"
      :shared-files-error="sharedFilesError"
      :shared-file-selecting="sharedFileSelecting"
      @update:value="(value) => emit('update:sslKey', value)"
      @request-shared-files="(payload) => emit('request-shared-files', payload)"
      @select-shared-file="(payload) => emit('select-shared-file', payload)"
    />
  </div>
</template>

<script setup lang="ts">
import { useI18n } from 'vue-i18n';
import CertSourceField from './CertSourceField.vue';

interface SharedDataFileEntry {
  name: string;
  relativePath: string;
  extension: string;
  size: number;
  modifiedAt: string;
}

defineProps<{
  cert: string;
  sslKey: string;
  shareName?: string;
  sharedFiles?: SharedDataFileEntry[];
  sharedFilesAvailable?: boolean;
  sharedFilesLoading?: boolean;
  sharedFilesError?: string;
  sharedFileSelecting?: boolean;
}>();

const { t } = useI18n();

const emit = defineEmits<{
  'update:cert': [value: string];
  'update:sslKey': [value: string];
  'request-shared-files': [payload: { field: 'cert' | 'sslKey'; force?: boolean }];
  'select-shared-file': [payload: { field: 'cert' | 'sslKey'; relativePath: string }];
}>();
</script>
