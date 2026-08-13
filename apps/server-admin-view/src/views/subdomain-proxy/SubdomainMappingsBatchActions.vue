<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { ChevronDown, FolderInput } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import type { HostMappingGroup } from "@/types";

defineProps<{
  groups: HostMappingGroup[];
  saving: boolean;
  selectedCount: number;
}>();

const emit = defineEmits<{
  clear: [];
  move: [groupId: string | null];
}>();

const { t } = useI18n();
</script>

<template>
  <div
    class="flex flex-wrap items-center gap-3 rounded-md border bg-muted/35 px-3 py-2"
    role="toolbar"
    :aria-label="t('admin.subdomainProxy.batchActions')"
  >
    <span class="text-sm font-medium">
      {{
        t("admin.subdomainProxy.selectedMappingsCount", {
          count: selectedCount,
        })
      }}
    </span>
    <DropdownMenu>
      <DropdownMenuTrigger as-child>
        <Button size="sm" variant="outline" :disabled="saving">
          <FolderInput class="mr-2 h-4 w-4" />
          {{ t("admin.subdomainProxy.moveToGroup") }}
          <ChevronDown class="ml-2 h-4 w-4" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start">
        <DropdownMenuItem
          v-for="group in groups"
          :key="group.id"
          @select="emit('move', group.id)"
        >
          {{ group.name }}
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        <DropdownMenuItem @select="emit('move', null)">
          {{ t("admin.subdomainProxy.ungrouped") }}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
    <Button size="sm" variant="outline" @click="emit('clear')">
      {{ t("admin.subdomainProxy.clearSelection") }}
    </Button>
  </div>
</template>
