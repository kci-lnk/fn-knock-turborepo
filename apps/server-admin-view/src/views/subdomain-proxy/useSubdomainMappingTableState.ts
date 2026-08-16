import { computed, nextTick, ref, watch } from "vue";
import type { HostMapping, HostMappingGroup } from "@/types";
import {
  buildHostMappingGroupSections,
  type HostMappingGroupSection,
} from "./host-mapping-groups";

const COLLAPSE_STORAGE_KEY = "fnknock.admin.hostMappingGroups.collapsed";
const HIDDEN_SELECTION_CLASS =
  "opacity-60 group-hover:opacity-100 group-focus-within:opacity-100";

interface SubdomainMappingTableStateOptions {
  filteredMappings: () => HostMapping[];
  groups: () => HostMappingGroup[];
  isSavingMappings: () => boolean;
  isMappingSelectable: (mapping: HostMapping) => boolean;
  searchQuery: () => string;
  showGroupedView: () => boolean;
  ungroupedLabel: () => string;
  onSaveFlatOrder: (mappings: HostMapping[]) => void;
  onSaveGroupedOrder: (sections: HostMappingGroupSection[]) => void;
  collapseStorage?: Pick<Storage, "getItem" | "setItem"> | null;
}

const resolveCollapseStorage = () =>
  typeof window === "undefined" ? null : window.localStorage;

const loadCollapsedGroupKeys = (
  storage: Pick<Storage, "getItem"> | null,
): Set<string> => {
  if (!storage) return new Set();
  try {
    const stored: unknown = JSON.parse(
      storage.getItem(COLLAPSE_STORAGE_KEY) || "[]",
    );
    if (!Array.isArray(stored)) return new Set();
    return new Set(
      stored.filter((item): item is string => typeof item === "string"),
    );
  } catch {
    return new Set();
  }
};

export const useSubdomainMappingTableState = (
  options: SubdomainMappingTableStateOptions,
) => {
  const collapseStorage =
    options.collapseStorage === undefined
      ? resolveCollapseStorage()
      : options.collapseStorage;
  const groupSections = ref<HostMappingGroupSection[]>([]);
  const selectedHosts = ref(new Set<string>());
  const isSelectionMode = ref(false);
  const collapsedGroupKeys = ref(loadCollapsedGroupKeys(collapseStorage));

  const dragDisabled = computed(
    () =>
      options.isSavingMappings() ||
      isSelectionMode.value ||
      Boolean(options.searchQuery().trim()) ||
      options.filteredMappings().length < 2,
  );
  const selectedCount = computed(() => selectedHosts.value.size);
  const mappingSelectionVisibilityClass = computed(() =>
    selectedCount.value > 0
      ? "pointer-events-auto opacity-100"
      : HIDDEN_SELECTION_CLASS,
  );
  const allVisibleSelected = computed(() => {
    const mappings = options
      .filteredMappings()
      .filter(options.isMappingSelectable);
    return (
      mappings.length > 0 &&
      mappings.every((mapping) => selectedHosts.value.has(mapping.host))
    );
  });
  const someVisibleSelected = computed(
    () =>
      !allVisibleSelected.value &&
      options
        .filteredMappings()
        .filter(options.isMappingSelectable)
        .some((mapping) => selectedHosts.value.has(mapping.host)),
  );

  const clearSelection = () => {
    selectedHosts.value = new Set();
  };

  const syncGroupSections = () => {
    const filteredMappings = options.filteredMappings();
    groupSections.value = buildHostMappingGroupSections(
      filteredMappings,
      options.showGroupedView() ? options.groups() : [],
      options.ungroupedLabel(),
      options.showGroupedView() && !options.searchQuery().trim(),
    );
    const visibleHosts = new Set(
      filteredMappings
        .filter(options.isMappingSelectable)
        .map((item) => item.host),
    );
    selectedHosts.value = new Set(
      [...selectedHosts.value].filter((host) => visibleHosts.has(host)),
    );
  };

  watch(
    () => [
      options.filteredMappings(),
      options.groups(),
      options.searchQuery(),
      options.showGroupedView(),
      options.ungroupedLabel(),
    ],
    syncGroupSections,
    { deep: true, immediate: true },
  );

  watch(options.isSavingMappings, (isSaving, wasSaving) => {
    if (wasSaving && !isSaving) syncGroupSections();
  });
  watch(options.showGroupedView, clearSelection);
  watch(options.searchQuery, clearSelection);

  const setSelectionMode = (enabled: boolean) => {
    isSelectionMode.value = enabled;
    if (!enabled) clearSelection();
  };

  const updateSectionMappings = (key: string, mappings: HostMapping[]) => {
    const section = groupSections.value.find((item) => item.key === key);
    if (section) section.mappings = mappings;
  };

  const handleSortEnd = async () => {
    await nextTick();
    if (options.showGroupedView()) {
      options.onSaveGroupedOrder(
        groupSections.value.map((section) => ({
          ...section,
          mappings: [...section.mappings],
        })),
      );
      return;
    }
    options.onSaveFlatOrder(groupSections.value[0]?.mappings ?? []);
  };

  const isSectionCollapsed = (section: HostMappingGroupSection) =>
    options.searchQuery().trim()
      ? false
      : collapsedGroupKeys.value.has(section.key);

  const toggleSectionCollapsed = (section: HostMappingGroupSection) => {
    const next = new Set(collapsedGroupKeys.value);
    if (next.has(section.key)) next.delete(section.key);
    else next.add(section.key);
    collapsedGroupKeys.value = next;
    try {
      collapseStorage?.setItem(COLLAPSE_STORAGE_KEY, JSON.stringify([...next]));
    } catch {
      // Storage can be unavailable in hardened or quota-constrained browsers.
    }
  };

  const isMappingSelected = (host: string) => selectedHosts.value.has(host);
  const setMappingSelected = (host: string, selected: boolean) => {
    const next = new Set(selectedHosts.value);
    if (selected) next.add(host);
    else next.delete(host);
    selectedHosts.value = next;
  };
  const isSectionSelected = (section: HostMappingGroupSection) =>
    section.mappings.some(options.isMappingSelectable) &&
    section.mappings
      .filter(options.isMappingSelectable)
      .every((mapping) =>
      selectedHosts.value.has(mapping.host),
    );
  const isSectionPartiallySelected = (section: HostMappingGroupSection) =>
    !isSectionSelected(section) &&
    section.mappings
      .filter(options.isMappingSelectable)
      .some((mapping) => selectedHosts.value.has(mapping.host));
  const setSectionSelected = (
    section: HostMappingGroupSection,
    selected: boolean,
  ) => {
    const next = new Set(selectedHosts.value);
    for (const mapping of section.mappings.filter(options.isMappingSelectable)) {
      if (selected) next.add(mapping.host);
      else next.delete(mapping.host);
    }
    selectedHosts.value = next;
  };
  const setAllVisibleSelected = (selected: boolean) => {
    const next = new Set(selectedHosts.value);
    for (const mapping of options
      .filteredMappings()
      .filter(options.isMappingSelectable)) {
      if (selected) next.add(mapping.host);
      else next.delete(mapping.host);
    }
    selectedHosts.value = next;
  };
  const takeSelectedHosts = () => {
    const hosts = [...selectedHosts.value];
    clearSelection();
    return hosts;
  };
  const getSelectedHosts = () => [...selectedHosts.value];

  return {
    allVisibleSelected,
    clearSelection,
    dragDisabled,
    groupSections,
    getSelectedHosts,
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
    takeSelectedHosts,
    toggleSectionCollapsed,
    updateSectionMappings,
  };
};
