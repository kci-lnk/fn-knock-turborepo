<script setup lang="ts">
import SubdomainBatchGroupMenu from "./SubdomainBatchGroupMenu.vue";
import { useI18n } from "vue-i18n";
import {
  CalendarClock,
  ChevronDown,
  Pencil,
  Power,
  PowerOff,
  Trash2,
} from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import type { HostMappingGroup } from "@/types";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";

defineProps<{
  groups: HostMappingGroup[];
  saving: boolean;
  selectedCount: number;
}>();

const emit = defineEmits<{
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
  <DropdownMenu>
    <DropdownMenuTrigger as-child>
      <Button
        size="sm"
        variant="outline"
        :disabled="saving || selectedCount === 0"
        class="col-span-2 h-10 w-full sm:ml-auto sm:h-8 sm:w-auto"
      >
        {{ t("admin.subdomainProxy.batchActions") }}
        <ChevronDown class="ml-2 h-4 w-4" />
      </Button>
    </DropdownMenuTrigger>
    <DropdownMenuContent align="end">
      <DropdownMenuItem :disabled="saving" @select="emit('edit')">
        <Pencil class="mr-2 h-4 w-4" />{{
          t("admin.subdomainProxy.batchEdit.action")
        }}
      </DropdownMenuItem>
      <SubdomainBatchGroupMenu
        :groups="groups"
        :saving="saving"
        @move="emit('move', $event)"
      />
      <DropdownMenuItem :disabled="saving" @select="emit('enable')">
        <Power class="mr-2 h-4 w-4" />{{
          t("admin.subdomainProxy.enableMapping")
        }}
      </DropdownMenuItem>
      <DropdownMenuItem :disabled="saving" @select="emit('disable')">
        <PowerOff class="mr-2 h-4 w-4" />{{
          t("admin.subdomainProxy.disableMapping")
        }}
      </DropdownMenuItem>
      <DropdownMenuItem :disabled="saving" @select="emit('schedule')">
        <CalendarClock class="mr-2 h-4 w-4" />{{
          t("admin.subdomainProxy.scheduleAvailability")
        }}
      </DropdownMenuItem>
      <DropdownMenuSeparator />
      <DropdownMenuItem
        variant="destructive"
        :disabled="saving"
        @select="emit('delete')"
      >
        <Trash2 class="mr-2 h-4 w-4" />{{ t("admin.subdomainProxy.delete") }}
      </DropdownMenuItem>
    </DropdownMenuContent>
  </DropdownMenu>
</template>
