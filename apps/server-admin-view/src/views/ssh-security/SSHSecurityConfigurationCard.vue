<script setup lang="ts">
import { Save } from "lucide-vue-next";
import ConfigCollapsibleCard from "@admin-shared/components/ConfigCollapsibleCard.vue";
import { Button } from "@/components/ui/button";
import SSHSecurityActionsMenu from "./SSHSecurityActionsMenu.vue";
import SSHSecurityFormFields from "./SSHSecurityFormFields.vue";
import type { SSHSecurityController } from "./ssh-security-contract";

const props = defineProps<{ controller: SSHSecurityController }>();
const {
  details,
  isLoading,
  isSaving,
  isSyncingFirewall,
  saveBlockedReason,
  saveConfig,
  summaryText,
  t,
} = props.controller;
</script>

<template>
  <ConfigCollapsibleCard
    :title="t('admin.sshSecurity.title')"
    :configured="details?.summary.configured === true"
    :ready="details !== null && !isLoading"
    :edit-label="t('admin.sshSecurity.editConfig')"
    collapsed-content-class="min-h-[76px] flex flex-col items-start gap-3 sm:h-[40px] sm:flex-row sm:items-center sm:justify-between"
    summary-class="text-xs text-muted-foreground"
    expanded-content-class="p-0 sm:p-0"
    actions-class="border-t bg-muted/30 px-4 py-4 sm:px-6 flex flex-col-reverse items-stretch gap-2 rounded-b-lg sm:flex-row sm:items-center sm:justify-end"
  >
    <template #summary>{{ summaryText }}</template>

    <template #collapsed-actions>
      <SSHSecurityActionsMenu compact :controller="controller" />
    </template>

    <template #default>
      <SSHSecurityFormFields :controller="controller" />
    </template>

    <template #actions="{ collapse }">
      <Button variant="outline" @click="collapse">
        {{ t("admin.sshSecurity.collapse") }}
      </Button>
      <SSHSecurityActionsMenu :controller="controller" />
      <Button
        :disabled="isSaving || isSyncingFirewall || Boolean(saveBlockedReason)"
        @click="saveConfig"
      >
        <Save class="h-4 w-4" />
        {{
          isSaving
            ? t("admin.sshSecurity.saving")
            : t("admin.sshSecurity.saveConfig")
        }}
      </Button>
    </template>
  </ConfigCollapsibleCard>
</template>
