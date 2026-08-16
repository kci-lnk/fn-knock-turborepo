<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Table,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { normalizeHostLike } from "./model";
import SubdomainMappingGroupHeaderRow from "./SubdomainMappingGroupHeaderRow.vue";
import SubdomainMappingGroupRows from "./SubdomainMappingGroupRows.vue";
import SubdomainMappingsBatchActions from "./SubdomainMappingsBatchActions.vue";
import SubdomainMappingTableRow from "./SubdomainMappingTableRow.vue";
import type {
  SubdomainMappingsCardProps,
  SubdomainMappingsTableActions,
} from "./subdomain-mappings-card-contract";
import { useSubdomainMappingTableState } from "./useSubdomainMappingTableState";

const props = defineProps<{
  actions: SubdomainMappingsTableActions;
  model: SubdomainMappingsCardProps;
  showGroupedView: boolean;
}>();
const { t } = useI18n();
const selectionCheckboxClass =
  "size-[18px] rounded-[5px] border-muted-foreground/40 bg-background shadow-none transition-[color,background-color,border-color,opacity] hover:border-primary/70 data-[state=indeterminate]:border-primary data-[state=indeterminate]:bg-primary data-[state=indeterminate]:text-primary-foreground";
const isScrolled = ref(false);
const activeDeepMonitorHostSet = computed(
  () => new Set(props.model.activeDeepMonitorHosts.map(normalizeHostLike)),
);
const isDeepMonitorActive = (host: string) =>
  activeDeepMonitorHostSet.value.has(normalizeHostLike(host));
const {
  allVisibleSelected,
  clearSelection,
  dragDisabled,
  getSelectedHosts,
  groupSections,
  handleSortEnd,
  isMappingSelected,
  isSelectionMode,
  isSectionCollapsed,
  isSectionPartiallySelected,
  isSectionSelected,
  mappingSelectionVisibilityClass,
  selectedCount,
  setAllVisibleSelected,
  setMappingSelected,
  setSelectionMode,
  setSectionSelected,
  someVisibleSelected,
  toggleSectionCollapsed,
  updateSectionMappings,
} = useSubdomainMappingTableState({
  filteredMappings: () => props.model.filteredMappings,
  groups: () => props.model.groups,
  isSavingMappings: () => props.model.isSavingMappings,
  searchQuery: () => props.model.searchQuery,
  showGroupedView: () => props.showGroupedView,
  isMappingSelectable: (mapping) =>
    !props.model.isAuthServiceTarget(mapping.target),
  ungroupedLabel: () => t("admin.subdomainProxy.ungrouped"),
  onSaveFlatOrder: props.actions.saveFlatOrder,
  onSaveGroupedOrder: props.actions.saveGroupedOrder,
});
const moveSelected = (groupId: string | null) =>
  props.actions.moveMappings(getSelectedHosts(), groupId, clearSelection);
const runBatchAction = (
  action: (hosts: string[], onComplete: () => void) => void,
) => action(getSelectedHosts(), clearSelection);
const handleScroll = (event: Event) => {
  if (event.currentTarget instanceof HTMLElement) {
    isScrolled.value = event.currentTarget.scrollLeft > 0;
  }
};

defineExpose({ clearSelection, setSelectionMode });
</script>

<template>
  <SubdomainMappingsBatchActions
    v-if="isSelectionMode && selectedCount > 0"
    :groups="model.groups"
    :saving="model.isSavingMappings"
    :selected-count="selectedCount"
    @clear="clearSelection"
    @delete="runBatchAction(actions.batchDelete)"
    @disable="runBatchAction(actions.batchDisable)"
    @enable="runBatchAction(actions.batchEnable)"
    @move="moveSelected"
    @schedule="runBatchAction(actions.batchSchedule)"
  />

  <slot name="notices" />

  <div class="overflow-hidden rounded-md border">
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
                v-if="isSelectionMode"
                :class="[
                  selectionCheckboxClass,
                  mappingSelectionVisibilityClass,
                ]"
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
            :selection-visibility-class="mappingSelectionVisibilityClass"
            :selection-mode="isSelectionMode"
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
            :selection-visibility-class="mappingSelectionVisibilityClass"
            :selection-mode="isSelectionMode"
            :show-grouped-view="showGroupedView"
            @select="setMappingSelected(mapping.host, $event)"
          />
        </template>
      </SubdomainMappingGroupRows>
    </Table>
  </div>
</template>
