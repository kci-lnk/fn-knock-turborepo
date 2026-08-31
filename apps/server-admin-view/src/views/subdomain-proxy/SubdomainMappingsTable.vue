<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useMediaQueryMatch } from "@admin-shared/composables/useMediaQueryMatch";
import { normalizeHostLike } from "./model";
import SubdomainMappingsDesktopTable from "./SubdomainMappingsDesktopTable.vue";
import SubdomainMappingsMobileList from "./SubdomainMappingsMobileList.vue";
import SubdomainMappingsBatchActions from "./SubdomainMappingsBatchActions.vue";
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
const isDesktopViewport = useMediaQueryMatch("(min-width: 768px)");
const selectionCheckboxClass =
  "size-[18px] rounded-[5px] border-muted-foreground/40 bg-background shadow-none transition-[color,background-color,border-color,opacity] hover:border-primary/70 data-[state=indeterminate]:border-primary data-[state=indeterminate]:bg-primary data-[state=indeterminate]:text-primary-foreground";
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

  <SubdomainMappingsMobileList
    v-if="!isDesktopViewport"
    :actions="actions"
    :all-visible-selected="allVisibleSelected"
    :drag-disabled="dragDisabled"
    :group-sections="groupSections"
    :handle-sort-end="handleSortEnd"
    :is-deep-monitor-active="isDeepMonitorActive"
    :is-mapping-selected="isMappingSelected"
    :is-section-collapsed="isSectionCollapsed"
    :is-section-partially-selected="isSectionPartiallySelected"
    :is-section-selected="isSectionSelected"
    :model="model"
    :selected-count="selectedCount"
    :selection-checkbox-class="selectionCheckboxClass"
    :selection-mode="isSelectionMode"
    :set-all-visible-selected="setAllVisibleSelected"
    :set-mapping-selected="setMappingSelected"
    :set-section-selected="setSectionSelected"
    :show-grouped-view="showGroupedView"
    :some-visible-selected="someVisibleSelected"
    :toggle-section-collapsed="toggleSectionCollapsed"
    :update-section-mappings="updateSectionMappings"
  />

  <SubdomainMappingsDesktopTable
    v-else
    :actions="actions"
    :all-visible-selected="allVisibleSelected"
    :drag-disabled="dragDisabled"
    :group-sections="groupSections"
    :handle-sort-end="handleSortEnd"
    :is-deep-monitor-active="isDeepMonitorActive"
    :is-mapping-selected="isMappingSelected"
    :is-section-collapsed="isSectionCollapsed"
    :is-section-partially-selected="isSectionPartiallySelected"
    :is-section-selected="isSectionSelected"
    :model="model"
    :selection-checkbox-class="selectionCheckboxClass"
    :selection-mode="isSelectionMode"
    :selection-visibility-class="mappingSelectionVisibilityClass"
    :set-all-visible-selected="setAllVisibleSelected"
    :set-mapping-selected="setMappingSelected"
    :set-section-selected="setSectionSelected"
    :show-grouped-view="showGroupedView"
    :some-visible-selected="someVisibleSelected"
    :toggle-section-collapsed="toggleSectionCollapsed"
    :update-section-mappings="updateSectionMappings"
  />
</template>
