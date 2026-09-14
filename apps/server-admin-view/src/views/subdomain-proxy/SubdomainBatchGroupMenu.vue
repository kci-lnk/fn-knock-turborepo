<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { FolderInput } from "lucide-vue-next";
import type { HostMappingGroup } from "@/types";
import {
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
} from "@/components/ui/dropdown-menu";
defineProps<{ groups: HostMappingGroup[]; saving: boolean }>();
const emit = defineEmits<{ move: [groupId: string | null] }>();
const { t } = useI18n();
</script>
<template>
  <DropdownMenuSub v-if="groups.length > 0">
    <DropdownMenuSubTrigger :disabled="saving">
      <FolderInput class="mr-2 h-4 w-4" />{{
        t("admin.subdomainProxy.moveToGroup")
      }}
    </DropdownMenuSubTrigger>
    <DropdownMenuSubContent>
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
    </DropdownMenuSubContent>
  </DropdownMenuSub>
</template>
