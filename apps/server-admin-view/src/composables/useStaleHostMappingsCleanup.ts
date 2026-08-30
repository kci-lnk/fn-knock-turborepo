import { computed, ref, type ComputedRef, type Ref } from "vue";
import { ScanAPI, type HostMappingProbeResult } from "@/lib/api/scan";
import type { HostMapping } from "../types";
import { isProxyHostMapping } from "../lib/host-mapping-target";

type MappingsSource = Ref<HostMapping[]> | ComputedRef<HostMapping[]>;

export interface UseStaleHostMappingsCleanupOptions {
  mappings: MappingsSource;
  saveMappings: (mappings: HostMapping[]) => Promise<unknown>;
  isAuthServiceTarget: (target: string) => boolean;
}

const hostKey = (value: string): string => value.trim().toLowerCase();

export const useStaleHostMappingsCleanup = (
  options: UseStaleHostMappingsCleanupOptions,
) => {
  const open = ref(false);
  const results = ref<HostMappingProbeResult[]>([]);
  const selectedHostKeys = ref(new Set<string>());
  const isProbing = ref(false);
  const isCleaning = ref(false);

  const probeableMappings = computed(() =>
    options.mappings.value.filter(
      (mapping) =>
        isProxyHostMapping(mapping) &&
        mapping.host.trim() &&
        mapping.target.trim() &&
        !options.isAuthServiceTarget(mapping.target),
    ),
  );

  const staleResults = computed(() =>
    results.value.filter((result) => result.status === "stale"),
  );

  const selectedCount = computed(() => selectedHostKeys.value.size);
  const isAllStaleSelected = computed(
    () =>
      staleResults.value.length > 0 &&
      staleResults.value.every((result) =>
        selectedHostKeys.value.has(hostKey(result.host)),
      ),
  );

  const setSelectedHosts = (hosts: Iterable<string>) => {
    selectedHostKeys.value = new Set([...hosts].map(hostKey).filter(Boolean));
  };

  const setHostSelected = (host: string, selected: boolean) => {
    const next = new Set(selectedHostKeys.value);
    const key = hostKey(host);
    if (selected) next.add(key);
    else next.delete(key);
    selectedHostKeys.value = next;
  };

  const isHostSelected = (host: string): boolean =>
    selectedHostKeys.value.has(hostKey(host));

  const setAllStaleSelected = (selected: boolean) => {
    setSelectedHosts(
      selected ? staleResults.value.map((result) => result.host) : [],
    );
  };

  const reset = () => {
    results.value = [];
    selectedHostKeys.value = new Set();
  };

  const openDialog = () => {
    open.value = true;
  };

  const closeDialog = () => {
    open.value = false;
    reset();
  };

  const probe = async () => {
    isProbing.value = true;
    try {
      const response = await ScanAPI.probeHostMappings({
        hosts: probeableMappings.value.map((mapping) => mapping.host),
      });
      results.value = response.results;
      setSelectedHosts(
        response.results
          .filter((result) => result.status === "stale")
          .map((result) => result.host),
      );
      return response.results;
    } finally {
      isProbing.value = false;
    }
  };

  const cleanSelected = async (): Promise<number> => {
    const staleSelectedKeys = new Set(
      staleResults.value
        .map((result) => hostKey(result.host))
        .filter((key) => selectedHostKeys.value.has(key)),
    );
    if (staleSelectedKeys.size === 0) return 0;

    const nextMappings = options.mappings.value.filter(
      (mapping) => !staleSelectedKeys.has(hostKey(mapping.host)),
    );
    const removedCount = options.mappings.value.length - nextMappings.length;
    if (removedCount <= 0) return 0;

    isCleaning.value = true;
    try {
      await options.saveMappings(nextMappings);
      reset();
      return removedCount;
    } finally {
      isCleaning.value = false;
    }
  };

  return {
    open,
    results,
    probeableMappings,
    staleResults,
    selectedCount,
    isAllStaleSelected,
    isProbing,
    isCleaning,
    openDialog,
    closeDialog,
    probe,
    cleanSelected,
    setHostSelected,
    isHostSelected,
    setAllStaleSelected,
  };
};
