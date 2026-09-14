<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { ChevronDown, FolderInput } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import type { HostMappingGroup } from "@/types";
import {
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
defineProps<{ groups: HostMappingGroup[]; saving: boolean }>();
const emit = defineEmits<{ move: [groupId: string | null] }>();
const { t } = useI18n();
</script>
<template>
  <DropdownMenu v-if="groups.length > 0">
    <DropdownMenuTrigger as-child>
      <Button
        size="sm"
        variant="outline"
        :disabled="saving"
        :title="t('admin.subdomainProxy.moveToGroup')"
        data-testid="batch-group-trigger"
        class="h-10 min-w-0 w-full gap-1 px-1.5 has-[>svg]:px-1.5 text-xs sm:h-8 sm:w-auto sm:px-3 sm:has-[>svg]:px-3 sm:text-sm"
      >
        <FolderInput class="hidden h-4 w-4 shrink-0 sm:block" />
        <span class="truncate">{{
          t("admin.subdomainProxy.batchEdit.moveGroup")
        }}</span>
        <ChevronDown class="size-3 shrink-0 sm:size-4" />
      </Button>
    </DropdownMenuTrigger>
    <DropdownMenuContent align="end">
      <DropdownMenuItem
        v-for="group in groups"
        :key="group.id"
        :disabled="saving"
        @select="emit('move', group.id)"
      >
        {{ group.name }}
      </DropdownMenuItem>
      <DropdownMenuSeparator />
      <DropdownMenuItem :disabled="saving" @select="emit('move', null)">{{
        t("admin.subdomainProxy.ungrouped")
      }}</DropdownMenuItem>
    </DropdownMenuContent>
  </DropdownMenu>
</template>
