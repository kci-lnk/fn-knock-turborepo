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
}>();

const emit = defineEmits<{
  move: [groupId: string | null];
}>();

const { t } = useI18n();
</script>

<template>
  <div v-if="groups.length > 0" class="col-span-2 sm:contents">
    <DropdownMenu>
      <DropdownMenuTrigger as-child>
        <Button
          size="sm"
          variant="outline"
          :disabled="saving"
          class="h-10 w-full justify-center sm:h-8 sm:w-auto"
        >
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
  </div>
</template>
