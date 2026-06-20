import { computed, type ComputedRef, type Ref } from "vue";
import { extractPortFromTarget } from "@admin-shared/utils/extractPortFromTarget";
import type { HostMapping, HostTrafficStats, TrafficStats } from "@/types";
import { getMappingDisplayTitle, normalizeHostLike } from "./model";

export const useSubdomainMappingsView = ({
  allMappings,
  draggableVisibleMappings,
  formatHostWithAccessEntryPort,
  isAuthServiceTarget,
  searchQuery,
  trafficRealtimeStats,
}: {
  allMappings: ComputedRef<HostMapping[]>;
  draggableVisibleMappings: Ref<HostMapping[]>;
  formatHostWithAccessEntryPort: (host: string) => string;
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
  const existingMappingPorts = computed(() => {
    const ports = new Set<number>();

    for (const mapping of allMappings.value) {
      const port = extractPortFromTarget(mapping.target);
      if (port !== null) {
        ports.add(port);
      }
    }

    return ports;
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
    return visibleMappings.value.filter(
      (mapping) =>
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
    existingMappingPorts,
    filteredMappings,
    getHostTrafficSample,
    hasRegularHostMappings,
    syncDraggableVisibleMappings,
    visibleMappings,
  };
};
