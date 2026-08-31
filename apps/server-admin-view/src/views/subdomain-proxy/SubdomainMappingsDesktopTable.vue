<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Table,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import type { HostMapping } from "@/types";
import type { HostMappingGroupSection } from "./host-mapping-groups";
import SubdomainMappingGroupHeaderRow from "./SubdomainMappingGroupHeaderRow.vue";
import SubdomainMappingGroupRows from "./SubdomainMappingGroupRows.vue";
import SubdomainMappingTableRow from "./SubdomainMappingTableRow.vue";
import type {
  SubdomainMappingsCardProps,
  SubdomainMappingsTableActions,
} from "./subdomain-mappings-card-contract";

defineProps<{
  actions: SubdomainMappingsTableActions;
  allVisibleSelected: boolean;
  dragDisabled: boolean;
  groupSections: HostMappingGroupSection[];
  handleSortEnd: () => Promise<void>;
  isDeepMonitorActive: (host: string) => boolean;
  isMappingSelected: (host: string) => boolean;
  isSectionCollapsed: (section: HostMappingGroupSection) => boolean;
  isSectionPartiallySelected: (section: HostMappingGroupSection) => boolean;
  isSectionSelected: (section: HostMappingGroupSection) => boolean;
  model: SubdomainMappingsCardProps;
  selectionCheckboxClass: string;
  selectionMode: boolean;
  selectionVisibilityClass: string;
  setAllVisibleSelected: (selected: boolean) => void;
  setMappingSelected: (host: string, selected: boolean) => void;
  setSectionSelected: (
    section: HostMappingGroupSection,
    selected: boolean,
  ) => void;
  showGroupedView: boolean;
  someVisibleSelected: boolean;
  toggleSectionCollapsed: (section: HostMappingGroupSection) => void;
  updateSectionMappings: (key: string, mappings: HostMapping[]) => void;
}>();

const { t } = useI18n();
const isScrolled = ref(false);
const handleScroll = (event: Event) => {
  if (event.currentTarget instanceof HTMLElement) {
    isScrolled.value = event.currentTarget.scrollLeft > 0;
  }
};
</script>

<template>
  <div class="hidden overflow-hidden rounded-md border md:block">
    <Table
      :container-class="[
        'mapping-table-scroll',
        {
          'mapping-table-scroll--grouped': showGroupedView,
          'mapping-table-scroll--scrolled': isScrolled,
        },
      ]"
      @scroll.passive="handleScroll"
    >
      <TableHeader>
        <TableRow class="group">
          <TableHead
            class="mapping-sticky-cell mapping-order-cell mapping-icon-cell"
          >
            <div class="flex h-7 w-full items-center justify-center">
              <Checkbox
                v-if="selectionMode"
                :class="[selectionCheckboxClass, selectionVisibilityClass]"
                :model-value="
                  someVisibleSelected ? 'indeterminate' : allVisibleSelected
                "
                :aria-label="t('admin.subdomainProxy.selectAllMappings')"
                @update:model-value="setAllVisibleSelected($event === true)"
              />
            </div>
          </TableHead>
          <TableHead
            class="mapping-sticky-cell mapping-favicon-cell mapping-icon-cell"
          >
            <span class="sr-only">Icon</span>
          </TableHead>
          <TableHead class="mapping-sticky-cell mapping-title-cell">
            {{ t("admin.subdomainProxy.columns.title") }}
          </TableHead>
          <TableHead>{{ t("admin.subdomainProxy.columns.domain") }}</TableHead>
          <TableHead>{{ t("admin.subdomainProxy.columns.target") }}</TableHead>
          <TableHead class="w-[7rem] min-w-[7rem] max-w-[7rem]">
            {{ t("admin.subdomainProxy.columns.traffic") }}
          </TableHead>
          <TableHead class="w-[8rem] min-w-[8rem]">
            {{ t("admin.subdomainProxy.columns.status") }}
          </TableHead>
          <TableHead class="text-right">
            {{ t("admin.subdomainProxy.columns.actions") }}
          </TableHead>
        </TableRow>
      </TableHeader>
      <tbody v-if="groupSections.length === 0">
        <TableRow>
          <TableCell colspan="8" class="py-8 text-center text-muted-foreground">
            {{ t("admin.subdomainProxy.emptyMappings") }}
          </TableCell>
        </TableRow>
      </tbody>
      <SubdomainMappingGroupRows
        v-for="section in groupSections"
        :key="section.key"
        :mappings="section.mappings"
        :collapsed="showGroupedView && isSectionCollapsed(section)"
        :disabled="dragDisabled"
        :empty-label="t('admin.subdomainProxy.emptyGroup')"
        :show-header="showGroupedView"
        @update:mappings="updateSectionMappings(section.key, $event)"
        @end="handleSortEnd"
      >
        <template #header>
          <SubdomainMappingGroupHeaderRow
            :actions="actions"
            :collapsed="isSectionCollapsed(section)"
            :has-selectable-mappings="
              section.mappings.some(
                (mapping) => !model.isAuthServiceTarget(mapping.target),
              )
            "
            :model="model"
            :partially-selected="isSectionPartiallySelected(section)"
            :section="section"
            :selected="isSectionSelected(section)"
            :selection-checkbox-class="selectionCheckboxClass"
            :selection-visibility-class="selectionVisibilityClass"
            :selection-mode="selectionMode"
            @select="setSectionSelected(section, $event)"
            @toggle="toggleSectionCollapsed(section)"
          />
        </template>
        <template #default="{ mapping }">
          <SubdomainMappingTableRow
            :actions="actions"
            :deep-monitor-active="isDeepMonitorActive(mapping.host)"
            :drag-disabled="dragDisabled"
            :mapping="mapping"
            :model="model"
            :selected="isMappingSelected(mapping.host)"
            :selectable="!model.isAuthServiceTarget(mapping.target)"
            :selection-checkbox-class="selectionCheckboxClass"
            :selection-visibility-class="selectionVisibilityClass"
            :selection-mode="selectionMode"
            :show-grouped-view="showGroupedView"
            @select="setMappingSelected(mapping.host, $event)"
          />
        </template>
      </SubdomainMappingGroupRows>
    </Table>
  </div>
</template>
