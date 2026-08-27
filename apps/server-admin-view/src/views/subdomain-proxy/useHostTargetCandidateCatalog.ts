import { computed, ref, watch, type ComputedRef } from "vue";
import { ScanAPI, type ScanDiscoveryHostCandidate } from "@/lib/api/scan";
import { resolveEffectiveHostTargetCandidates } from "./host-target-candidates";

export const useHostTargetCandidateCatalog = ({
  isDockerDeployment,
  open,
}: {
  isDockerDeployment: ComputedRef<boolean>;
  open: ComputedRef<boolean>;
}) => {
  const candidates = ref<ScanDiscoveryHostCandidate[]>([]);
  const isLoading = ref(false);
  const loadFailed = ref(false);
  let requestId = 0;

  const loadCandidates = async () => {
    const currentRequestId = ++requestId;
    isLoading.value = true;
    loadFailed.value = false;
    try {
      const response = await ScanAPI.getDiscoverTargets();
      if (currentRequestId === requestId) {
        candidates.value = response.hostCandidates ?? [];
      }
    } catch (error) {
      if (currentRequestId === requestId) {
        candidates.value = [];
        loadFailed.value = true;
        console.warn("load host target candidates failed", error);
      }
    } finally {
      if (currentRequestId === requestId) isLoading.value = false;
    }
  };

  watch(
    [open, isDockerDeployment],
    ([isOpen]) => {
      if (isOpen) void loadCandidates();
    },
    { immediate: true },
  );

  const effectiveCandidates = computed(() =>
    resolveEffectiveHostTargetCandidates(
      candidates.value,
      isDockerDeployment.value,
    ),
  );

  return {
    candidates,
    effectiveCandidates,
    isLoading,
    loadCandidates,
    loadFailed,
  };
};
