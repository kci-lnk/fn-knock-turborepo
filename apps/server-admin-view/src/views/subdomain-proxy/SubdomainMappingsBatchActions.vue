<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import type { HostMappingGroup } from "@/types";
import SubdomainBatchOperationsMenu from "./SubdomainBatchOperationsMenu.vue";

defineProps<{
  groups: HostMappingGroup[];
  saving: boolean;
  selectedCount: number;
}>();

const emit = defineEmits<{
  clear: [];
  edit: [];
  disable: [];
  enable: [];
  move: [groupId: string | null];
  schedule: [];
  delete: [];
}>();

const { t } = useI18n();
</script>

<template>
  <div
    class="grid grid-cols-2 items-center gap-2 rounded-md border bg-muted/35 px-3 py-3 sm:flex sm:flex-wrap sm:gap-3 sm:py-2"
    role="toolbar"
    :aria-label="t('admin.subdomainProxy.batchActions')"
  >
    <span class="min-w-0 truncate text-sm font-medium">
      {{
        t("admin.subdomainProxy.selectedMappingsCount", {
          count: selectedCount,
        })
      }}
    </span>
    <Button
      size="sm"
      variant="outline"
      :disabled="saving"
      class="h-10 w-full justify-center sm:h-8 sm:w-auto"
      @click="emit('clear')"
    >
      {{ t("admin.subdomainProxy.clearSelection") }}
    </Button>
    <SubdomainBatchOperationsMenu
      :groups="groups"
      :saving="saving"
      :selected-count="selectedCount"
      @edit="emit('edit')"
      @enable="emit('enable')"
      @disable="emit('disable')"
      @schedule="emit('schedule')"
      @delete="emit('delete')"
      @move="emit('move', $event)"
    />
  </div>
</template>
