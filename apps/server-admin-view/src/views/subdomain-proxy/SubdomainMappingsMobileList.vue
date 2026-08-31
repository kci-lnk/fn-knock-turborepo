<script setup lang="ts">
import { computed } from "vue";
import { VueDraggable } from "vue-draggable-plus";
import { useI18n } from "vue-i18n";
import { Checkbox } from "@/components/ui/checkbox";
import type { HostMapping } from "@/types";
import {
  buildHostMappingDragRenderKey,
  type HostMappingGroupSection,
} from "./host-mapping-groups";
import SubdomainMappingMobileGroupHeader from "./SubdomainMappingMobileGroupHeader.vue";
import SubdomainMappingMobileRow from "./SubdomainMappingMobileRow.vue";
import type {
  SubdomainMappingsCardProps,
  SubdomainMappingsTableActions,
} from "./subdomain-mappings-card-contract";

const props = defineProps<{
  actions: SubdomainMappingsTableActions;
  allVisibleSelected: boolean;
  dragDisabled: boolean;
  groupSections: HostMappingGroupSection[];
  isMappingSelected: (host: string) => boolean;
  isSectionCollapsed: (section: HostMappingGroupSection) => boolean;
  isSectionPartiallySelected: (section: HostMappingGroupSection) => boolean;
  isSectionSelected: (section: HostMappingGroupSection) => boolean;
  model: SubdomainMappingsCardProps;
  selectedCount: number;
  selectionCheckboxClass: string;
  selectionMode: boolean;
  showGroupedView: boolean;
  someVisibleSelected: boolean;
  setAllVisibleSelected: (selected: boolean) => void;
  setMappingSelected: (host: string, selected: boolean) => void;
  setSectionSelected: (
    section: HostMappingGroupSection,
    selected: boolean,
  ) => void;
  toggleSectionCollapsed: (section: HostMappingGroupSection) => void;
  updateSectionMappings: (key: string, mappings: HostMapping[]) => void;
  handleSortEnd: () => Promise<void>;
  isDeepMonitorActive: (host: string) => boolean;
}>();

const { t } = useI18n();
const hasSelectableMappings = computed(() =>
  props.model.filteredMappings.some(
    (mapping) => !props.model.isAuthServiceTarget(mapping.target),
  ),
);
const showEmptyState = computed(
  () =>
    props.model.filteredMappings.length === 0 &&
    (!props.showGroupedView ||
      Boolean(props.model.searchQuery.trim()) ||
      props.groupSections.length === 0),
);
const updateMappings = (key: string, mappings: HostMapping[]) =>
  props.updateSectionMappings(key, mappings);
</script>

<template>
  <div class="overflow-hidden rounded-md border md:hidden">
    <div
      v-if="showEmptyState"
      class="flex min-h-40 items-center justify-center px-4 py-8 text-center text-sm text-muted-foreground"
    >
      {{ t("admin.subdomainProxy.emptyMappings") }}
    </div>
    <template v-else>
      <div
        v-if="selectionMode"
        class="flex items-center justify-between gap-3 border-b bg-muted/20 px-3 py-2.5"
      >
        <label class="flex min-w-0 items-center gap-2 text-xs">
          <Checkbox
            :class="selectionCheckboxClass"
            :model-value="
              someVisibleSelected ? 'indeterminate' : allVisibleSelected
            "
            :aria-label="t('admin.subdomainProxy.selectAllMappings')"
            :disabled="!hasSelectableMappings"
            @update:model-value="setAllVisibleSelected($event === true)"
          />
          <span class="truncate">{{ t("common.selectAll") }}</span>
        </label>
        <span class="shrink-0 text-[11px] text-muted-foreground">
          {{
            t("admin.subdomainProxy.selectedMappingsCount", {
              count: selectedCount,
            })
          }}
        </span>
      </div>

      <section
        v-for="section in groupSections"
        :key="section.key"
        class="border-b last:border-b-0"
      >
        <SubdomainMappingMobileGroupHeader
          v-if="showGroupedView"
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
          :selection-mode="selectionMode"
          @select="setSectionSelected(section, $event)"
          @toggle="toggleSectionCollapsed(section)"
        />

        <VueDraggable
          v-if="!showGroupedView || !isSectionCollapsed(section)"
          :key="buildHostMappingDragRenderKey(section.mappings)"
          :model-value="section.mappings"
          tag="div"
          class="[&>.mapping-mobile-row:last-child]:border-b-0"
          handle=".mapping-drag-handle"
          draggable=".mapping-mobile-row"
          ghost-class="bg-muted/60"
          chosen-class="bg-muted/80"
          :animation="180"
          :disabled="dragDisabled"
          :group="{ name: 'host-mapping-groups', pull: true, put: true }"
          @update:model-value="updateMappings(section.key, $event)"
          @end="handleSortEnd"
        >
          <div
            v-if="section.mappings.length === 0"
            class="flex min-h-20 items-center justify-center px-4 text-center text-xs text-muted-foreground"
          >
            {{ t("admin.subdomainProxy.emptyGroup") }}
          </div>
          <SubdomainMappingMobileRow
            v-for="mapping in section.mappings"
            :key="mapping.host"
            :actions="actions"
            :deep-monitor-active="isDeepMonitorActive(mapping.host)"
            :drag-disabled="dragDisabled"
            :mapping="mapping"
            :model="model"
            :selected="isMappingSelected(mapping.host)"
            :selectable="!model.isAuthServiceTarget(mapping.target)"
            :selection-checkbox-class="selectionCheckboxClass"
            :selection-mode="selectionMode"
            @select="setMappingSelected(mapping.host, $event)"
          />
        </VueDraggable>
      </section>
    </template>
  </div>
</template>
