<script setup lang="ts">
import SubdomainActionConfirmDialog from "./SubdomainActionConfirmDialog.vue";
import SubdomainAvailabilityDialog from "./SubdomainAvailabilityDialog.vue";
import type { SubdomainProxyDialogsController } from "./useSubdomainProxyPage";

const props = defineProps<{ controller: SubdomainProxyDialogsController }>();
const {
  batchAvailabilityFormEnabled,
  batchAvailabilityFormEndTime,
  batchAvailabilityFormStartTime,
  batchAvailabilityOpen,
  batchAvailabilityValidationMessage,
  batchMutationConfirmLabel,
  batchMutationConfirmVariant,
  batchMutationDescription,
  batchMutationTitle,
  batchSelectedCount,
  closeBatchAvailability,
  closeBatchMutation,
  confirmBatchMutation,
  isBatchMutationOpen,
  isSavingMappings,
  saveBatchAvailability,
  t,
} = props.controller;
</script>

<template>
  <SubdomainActionConfirmDialog
    :open="isBatchMutationOpen"
    :title="batchMutationTitle"
    :description="batchMutationDescription"
    :cancel-label="t('admin.subdomainProxy.cancel')"
    :confirm-label="batchMutationConfirmLabel"
    :confirm-variant="batchMutationConfirmVariant"
    :loading="isSavingMappings"
    @update:open="(open) => !open && closeBatchMutation()"
    @cancel="closeBatchMutation"
    @confirm="confirmBatchMutation"
  />

  <SubdomainAvailabilityDialog
    :open="batchAvailabilityOpen"
    :host="String(batchSelectedCount)"
    :title="t('admin.subdomainProxy.batchAvailabilityTitle', { count: batchSelectedCount })"
    :description="t('admin.subdomainProxy.batchAvailabilityDescription', { count: batchSelectedCount })"
    :save-label="t('admin.subdomainProxy.saveBatchAvailability')"
    :enabled="batchAvailabilityFormEnabled"
    :start-time="batchAvailabilityFormStartTime"
    :end-time="batchAvailabilityFormEndTime"
    :loading="isSavingMappings"
    :validation-message="batchAvailabilityValidationMessage"
    @update:open="(open) => !open && closeBatchAvailability()"
    @update:enabled="batchAvailabilityFormEnabled = $event"
    @update:start-time="batchAvailabilityFormStartTime = $event"
    @update:end-time="batchAvailabilityFormEndTime = $event"
    @cancel="closeBatchAvailability"
    @save="saveBatchAvailability"
  />
</template>
