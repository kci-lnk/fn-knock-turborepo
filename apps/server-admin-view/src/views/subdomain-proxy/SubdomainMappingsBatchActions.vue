<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { CalendarClock, Power, PowerOff, Trash2 } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import type { HostMappingGroup } from "@/types";
import SubdomainBatchGroupMenu from "./SubdomainBatchGroupMenu.vue";

defineProps<{
  groups: HostMappingGroup[];
  saving: boolean;
  selectedCount: number;
}>();

const emit = defineEmits<{
  clear: [];
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
    <SubdomainBatchGroupMenu
      :groups="groups"
      :saving="saving"
      @move="emit('move', $event)"
    />
    <Button
      size="sm"
      variant="outline"
      :disabled="saving"
      class="h-10 w-full justify-center sm:h-8 sm:w-auto"
      @click="emit('enable')"
    >
      <Power class="mr-2 h-4 w-4" />
      {{ t("admin.subdomainProxy.enableMapping") }}
    </Button>
    <Button
      size="sm"
      variant="outline"
      :disabled="saving"
      class="h-10 w-full justify-center sm:h-8 sm:w-auto"
      @click="emit('disable')"
    >
      <PowerOff class="mr-2 h-4 w-4" />
      {{ t("admin.subdomainProxy.disableMapping") }}
    </Button>
    <Button
      size="sm"
      variant="outline"
      :disabled="saving"
      class="h-10 w-full justify-center sm:h-8 sm:w-auto"
      @click="emit('schedule')"
    >
      <CalendarClock class="mr-2 h-4 w-4" />
      <span class="sm:hidden">{{
        t("admin.subdomainProxy.batchSchedule")
      }}</span>
      <span class="hidden sm:inline">{{
        t("admin.subdomainProxy.scheduleAvailability")
      }}</span>
    </Button>
    <Button
      size="sm"
      variant="destructive"
      :disabled="saving"
      class="h-10 w-full justify-center sm:h-8 sm:w-auto"
      @click="emit('delete')"
    >
      <Trash2 class="mr-2 h-4 w-4" />
      {{ t("admin.subdomainProxy.delete") }}
    </Button>
  </div>
</template>
