import {
  computed,
  ref,
  watch,
  type ComputedRef,
  type Ref,
} from "vue";
import { toast } from "@admin-shared/utils/toast";
import type { HostMapping } from "@/types";
import {
  buildTargetOptimizationDestinations,
  buildTargetOptimizationPreviews,
  resolveDefaultTargetOptimizationDestination,
} from "./subdomain-target-optimization";
import { useHostTargetCandidateCatalog } from "./useHostTargetCandidateCatalog";

type AsyncActionRun = <T>(action: () => Promise<T>) => Promise<T | undefined>;
type Translate = (key: string, params?: Record<string, number>) => string;

export const useSubdomainTargetOptimization = ({
  allMappings,
  isAuthServiceTarget,
  isDockerDeployment,
  isSavingMappings,
  runSaveMappings,
  saveHostMappings,
  translate,
}: {
  allMappings: ComputedRef<HostMapping[]>;
  isAuthServiceTarget: (target: string) => boolean;
  isDockerDeployment: ComputedRef<boolean>;
  isSavingMappings: Ref<boolean>;
  runSaveMappings: AsyncActionRun;
  saveHostMappings: (mappings: HostMapping[]) => Promise<unknown>;
  translate: Translate;
}) => {
  const isOpen = ref(false);
  const destinationAddress = ref("");
  const selectedHosts = ref(new Set<string>());
  let initializedForOpen = false;

  const {
    effectiveCandidates,
    isLoading: isLoadingCandidates,
    loadCandidates,
    loadFailed: candidateLoadFailed,
  } = useHostTargetCandidateCatalog({
    isDockerDeployment,
    open: computed(() => isOpen.value),
  });
  const destinations = computed(() =>
    buildTargetOptimizationDestinations(
      effectiveCandidates.value,
      isDockerDeployment.value,
    ),
  );
  const destinationSignature = computed(() =>
    destinations.value.map((destination) => destination.address).join("\0"),
  );
  const previews = computed(() =>
    buildTargetOptimizationPreviews({
      candidates: effectiveCandidates.value,
      destinationAddress: destinationAddress.value,
      isAuthServiceTarget,
      isDockerDeployment: isDockerDeployment.value,
      mappings: allMappings.value,
    }),
  );
  const selectedPreviews = computed(() =>
    previews.value.filter((preview) => selectedHosts.value.has(preview.host)),
  );
  const selectedCount = computed(() => selectedPreviews.value.length);
  const allSelected = computed(
    () => previews.value.length > 0 && selectedCount.value === previews.value.length,
  );
  const partiallySelected = computed(
    () => selectedCount.value > 0 && !allSelected.value,
  );

  const selectAllPreviews = () => {
    selectedHosts.value = new Set(previews.value.map((preview) => preview.host));
  };
  const setDestinationAddress = (address: string) => {
    destinationAddress.value = address;
    selectAllPreviews();
  };
  const initializeDialog = () => {
    const address = resolveDefaultTargetOptimizationDestination({
      candidates: effectiveCandidates.value,
      isAuthServiceTarget,
      isDockerDeployment: isDockerDeployment.value,
      mappings: allMappings.value,
    });
    destinationAddress.value = address;
    selectAllPreviews();
    initializedForOpen = true;
  };
  const openDialog = () => {
    if (isSavingMappings.value) return;
    initializedForOpen = false;
    destinationAddress.value = "";
    selectedHosts.value = new Set();
    isOpen.value = true;
  };
  const closeDialog = () => {
    if (isSavingMappings.value) return;
    isOpen.value = false;
    destinationAddress.value = "";
    selectedHosts.value = new Set();
  };
  const handleOpenChange = (open: boolean) => {
    if (open) openDialog();
    else closeDialog();
  };
  const retryCandidates = async () => {
    initializedForOpen = false;
    destinationAddress.value = "";
    selectedHosts.value = new Set();
    await loadCandidates();
  };
  const setAllSelected = (selected: boolean) => {
    selectedHosts.value = selected
      ? new Set(previews.value.map((preview) => preview.host))
      : new Set();
  };
  const setMappingSelected = (host: string, selected: boolean) => {
    const next = new Set(selectedHosts.value);
    if (selected) next.add(host);
    else next.delete(host);
    selectedHosts.value = next;
  };
  const isMappingSelected = (host: string) => selectedHosts.value.has(host);

  const saveOptimizedTargets = async () => {
    if (
      isSavingMappings.value ||
      isLoadingCandidates.value ||
      selectedCount.value === 0
    ) {
      return;
    }
    const selected = new Map(
      selectedPreviews.value.map((preview) => [preview.host, preview.nextTarget]),
    );
    if (selected.size === 0) return;
    const savedCount = await runSaveMappings(async () => {
      await saveHostMappings(
        allMappings.value.map((mapping) => {
          const nextTarget = selected.get(mapping.host);
          return nextTarget ? { ...mapping, target: nextTarget } : mapping;
        }),
      );
      toast.success(
        translate("admin.subdomainProxy.targetOptimizationSaved", {
          count: selected.size,
        }),
      );
      return selected.size;
    });
    if (savedCount !== undefined) {
      isOpen.value = false;
      destinationAddress.value = "";
      selectedHosts.value = new Set();
    }
  };

  watch(
    [
      isOpen,
      isLoadingCandidates,
      destinationAddress,
      destinationSignature,
    ],
    ([open, loading]) => {
      const destinationIsValid = destinations.value.some(
        (destination) => destination.address === destinationAddress.value,
      );
      if (
        open &&
        !loading &&
        (!initializedForOpen || !destinationIsValid)
      ) {
        initializeDialog();
      }
    },
    { flush: "post" },
  );

  return {
    allSelected,
    candidateLoadFailed,
    closeDialog,
    destinationAddress,
    destinations,
    handleOpenChange,
    isLoadingCandidates,
    isMappingSelected,
    isOpen,
    isSavingMappings,
    openDialog,
    partiallySelected,
    previews,
    retryCandidates,
    saveOptimizedTargets,
    selectedCount,
    setAllSelected,
    setDestinationAddress,
    setMappingSelected,
  };
};

export type SubdomainTargetOptimizationController = ReturnType<
  typeof useSubdomainTargetOptimization
>;
