<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Pencil } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import type { HostMappingGroup } from "@/types";
import SubdomainBatchGroupMenu from "./SubdomainBatchGroupMenu.vue";
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
    class="flex flex-col gap-2 rounded-md border bg-muted/35 px-3 py-2 sm:flex-row sm:flex-wrap sm:items-center sm:justify-between sm:gap-3"
    role="toolbar"
    :aria-label="t('admin.subdomainProxy.batchActions')"
  >
    <div
      class="flex min-w-0 items-center justify-between gap-2 sm:justify-start"
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
        class="h-10 shrink-0 px-2 text-xs sm:h-8 sm:px-3 sm:text-sm"
        @click="emit('clear')"
      >
        {{ t("admin.subdomainProxy.clearSelection") }}
      </Button>
    </div>
    <div
      class="grid min-w-0 gap-2 sm:flex sm:flex-wrap"
      :class="
        groups.length
          ? 'grid-cols-[minmax(0,1fr)_minmax(0,1fr)_2.5rem]'
          : 'grid-cols-[minmax(0,1fr)_2.5rem]'
      "
    >
      <Button
        size="sm"
        :disabled="saving || selectedCount === 0"
        :title="t('admin.subdomainProxy.batchEdit.action')"
        class="h-10 min-w-0 gap-1 px-2 text-xs sm:h-8 sm:px-3 sm:text-sm"
        data-testid="batch-edit-trigger"
        @click="emit('edit')"
      >
        <Pencil class="hidden h-4 w-4 shrink-0 sm:block" />
        <span class="truncate">{{
          t("admin.subdomainProxy.batchEdit.shortAction")
        }}</span>
      </Button>
      <SubdomainBatchGroupMenu
        :groups="groups"
        :saving="saving || selectedCount === 0"
        @move="emit('move', $event)"
      />
      <SubdomainBatchOperationsMenu
        :saving="saving"
        :selected-count="selectedCount"
        @enable="emit('enable')"
        @disable="emit('disable')"
        @schedule="emit('schedule')"
        @delete="emit('delete')"
      />
    </div>
  </div>
</template>
