<script setup lang="ts">
import {
  ChevronRight,
  FolderPlus,
  Folders,
  MoreHorizontal,
} from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import type { HostMappingGroupSection } from "./host-mapping-groups";
import type {
  SubdomainMappingsCardProps,
  SubdomainMappingsTableActions,
} from "./subdomain-mappings-card-contract";

defineProps<{
  actions: SubdomainMappingsTableActions;
  collapsed: boolean;
  hasSelectableMappings: boolean;
  model: SubdomainMappingsCardProps;
  partiallySelected: boolean;
  section: HostMappingGroupSection;
  selected: boolean;
  selectionCheckboxClass: string;
  selectionMode: boolean;
}>();

const emit = defineEmits<{
  select: [selected: boolean];
  toggle: [];
}>();

const { t } = useI18n();
</script>

<template>
  <header
    class="flex min-h-12 items-center gap-2 border-b bg-muted/45 px-3 py-2"
  >
    <Checkbox
      v-if="selectionMode && hasSelectableMappings"
      :class="selectionCheckboxClass"
      :model-value="partiallySelected ? 'indeterminate' : selected"
      :aria-label="
        t('admin.subdomainProxy.selectGroupMappings', {
          group: section.name,
        })
      "
      @update:model-value="emit('select', $event === true)"
    />
    <span
      v-else-if="selectionMode"
      class="size-[18px] shrink-0"
      aria-hidden="true"
    ></span>

    <button
      type="button"
      class="inline-flex min-w-0 flex-1 items-center gap-2 rounded-sm text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      :aria-expanded="!collapsed"
      @click="emit('toggle')"
    >
      <ChevronRight
        class="h-4 w-4 shrink-0 transition-transform duration-200 ease-out motion-reduce:transition-none"
        :class="{ 'rotate-90': !collapsed }"
      />
      <span class="truncate text-sm font-medium">{{ section.name }}</span>
      <span
        class="rounded-full bg-background px-2 py-0.5 text-[11px] text-muted-foreground"
      >
        {{ section.mappings.length }}
      </span>
    </button>

    <DropdownMenu>
      <DropdownMenuTrigger as-child>
        <Button
          variant="ghost"
          size="icon"
          class="h-8 w-8 shrink-0"
          :aria-label="`${t('common.moreActions')}: ${section.name}`"
        >
          <MoreHorizontal class="h-4 w-4" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuItem
          :disabled="model.isSavingMappings"
          @select="actions.openCreate(section.groupId)"
        >
          <FolderPlus class="mr-2 h-4 w-4" />
          {{ t("admin.subdomainProxy.addMappingToGroup") }}
        </DropdownMenuItem>
        <DropdownMenuItem @select="actions.manageGroups">
          <Folders class="mr-2 h-4 w-4" />
          {{ t("admin.subdomainProxy.manageGroups") }}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  </header>
</template>
