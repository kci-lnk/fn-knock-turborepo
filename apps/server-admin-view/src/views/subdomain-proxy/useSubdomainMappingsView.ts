import { computed, type ComputedRef, type Ref } from "vue";
import type {
  HostMapping,
  HostMappingGroup,
  HostTrafficStats,
  TrafficStats,
} from "@/types";
import {
  buildMappingTargetKey,
  getMappingDisplayTitle,
  normalizeHostLike,
} from "./model";

export const useSubdomainMappingsView = ({
  allMappings,
  draggableVisibleMappings,
  formatHostWithAccessEntryPort,
  groups,
  isAuthServiceTarget,
  searchQuery,
  trafficRealtimeStats,
}: {
  allMappings: ComputedRef<HostMapping[]>;
  draggableVisibleMappings: Ref<HostMapping[]>;
  formatHostWithAccessEntryPort: (host: string) => string;
  groups: ComputedRef<HostMappingGroup[]>;
  isAuthServiceTarget: (target: string) => boolean;
  searchQuery: Ref<string>;
  trafficRealtimeStats: Ref<TrafficStats | null>;
}) => {
  const regularHostMappings = computed(() =>
    allMappings.value.filter((mapping) => !isAuthServiceTarget(mapping.target)),
  );
  const hasRegularHostMappings = computed(
    () => regularHostMappings.value.length > 0,
  );
  const existingMappingTargets = computed(() => {
    const targets = new Set<string>();

    for (const mapping of allMappings.value) {
      const targetKey = buildMappingTargetKey(mapping.target);
      if (targetKey) {
        targets.add(targetKey);
      }
    }

    return targets;
  });
  const authServiceMapping = computed(
    () =>
      allMappings.value.find((mapping) =>
        isAuthServiceTarget(mapping.target),
      ) ?? null,
  );
  const discoverButtonVariant = computed(() =>
    authServiceMapping.value ? "default" : "secondary",
  );
  const discoverButtonDividerClass = computed(() =>
    authServiceMapping.value
      ? "border-primary-foreground/20"
      : "border-border/70",
  );
  const visibleMappings = computed(() =>
    allMappings.value.filter((mapping) => !isAuthServiceTarget(mapping.target)),
  );
  const hostTrafficSamples = computed(() => {
    const samples = new Map<string, HostTrafficStats>();
    for (const item of trafficRealtimeStats.value?.by_host ?? []) {
      const host = normalizeHostLike(item.host);
      if (!host) continue;
      samples.set(host, item);
    }
    return samples;
  });
  const getHostTrafficSample = (host: string): HostTrafficStats | null =>
    hostTrafficSamples.value.get(normalizeHostLike(host)) ?? null;

  const filteredMappings = computed(() => {
    const query = searchQuery.value.trim().toLowerCase();
    if (!query) return visibleMappings.value;
    const matchingGroupIds = new Set(
      groups.value
        .filter((group) => group.name.toLowerCase().includes(query))
        .map((group) => group.id),
    );
    return visibleMappings.value.filter(
      (mapping) =>
        (mapping.group_id != null && matchingGroupIds.has(mapping.group_id)) ||
        getMappingDisplayTitle(mapping).toLowerCase().includes(query) ||
        formatHostWithAccessEntryPort(mapping.host)
          .toLowerCase()
          .includes(query) ||
        mapping.host.toLowerCase().includes(query) ||
        mapping.target.toLowerCase().includes(query),
    );
  });

  const syncDraggableVisibleMappings = () => {
    draggableVisibleMappings.value = [...filteredMappings.value];
  };

  return {
    authServiceMapping,
    discoverButtonDividerClass,
    discoverButtonVariant,
    existingMappingTargets,
    filteredMappings,
    getHostTrafficSample,
    hasRegularHostMappings,
    syncDraggableVisibleMappings,
    visibleMappings,
  };
};
