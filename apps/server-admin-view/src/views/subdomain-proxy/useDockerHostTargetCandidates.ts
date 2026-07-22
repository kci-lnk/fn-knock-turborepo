import { computed, ref, watch, type ComputedRef } from "vue";
import { ScanAPI, type ScanDiscoveryHostCandidate } from "@/lib/api";

type Translate = (key: string) => string;

export const buildDockerHostTargetSuggestions = (
  candidates: readonly ScanDiscoveryHostCandidate[],
  isDockerDeployment: boolean,
): string[] => {
  if (!isDockerDeployment) return ["127.0.0.1:"];
  return [...new Set(candidates.map((candidate) => `${candidate.address}:`))];
};

export const buildDockerHostTargetPlaceholder = (
  candidates: readonly ScanDiscoveryHostCandidate[],
  isDockerDeployment: boolean,
  dockerFallback: string,
): string => {
  if (!isDockerDeployment) return "127.0.0.1:5173";
  const recommended =
    candidates.find((candidate) => candidate.recommended) ?? candidates[0];
  return recommended ? `${recommended.address}:8080` : dockerFallback;
};

export const useDockerHostTargetCandidates = ({
  isDockerDeployment,
  open,
  translate,
}: {
  isDockerDeployment: ComputedRef<boolean>;
  open: ComputedRef<boolean>;
  translate: Translate;
}) => {
  const candidates = ref<ScanDiscoveryHostCandidate[]>([]);
  const isLoading = ref(false);
  let requestId = 0;

  const loadCandidates = async () => {
    if (!isDockerDeployment.value) {
      candidates.value = [];
      return;
    }
    const currentRequestId = ++requestId;
    isLoading.value = true;
    try {
      const response = await ScanAPI.getDiscoverTargets();
      if (currentRequestId === requestId) {
        candidates.value = response.hostCandidates ?? [];
      }
    } catch (error) {
      if (currentRequestId === requestId) {
        candidates.value = [];
        console.warn("load Docker host target candidates failed", error);
      }
    } finally {
      if (currentRequestId === requestId) isLoading.value = false;
    }
  };

  watch(
    [open, isDockerDeployment],
    ([isOpen, isDocker]) => {
      if (!isDocker) {
        candidates.value = [];
        return;
      }
      if (isOpen) void loadCandidates();
    },
    { immediate: true },
  );

  const targetSuggestions = computed(() =>
    buildDockerHostTargetSuggestions(
      candidates.value,
      isDockerDeployment.value,
    ),
  );
  const targetPlaceholder = computed(() =>
    buildDockerHostTargetPlaceholder(
      candidates.value,
      isDockerDeployment.value,
      translate("admin.subdomainProxy.dockerTargetPlaceholder"),
    ),
  );
  const targetCandidateHint = computed(() => {
    if (!isDockerDeployment.value) return "";
    if (isLoading.value) {
      return translate("admin.subdomainProxy.dockerTargetCandidatesLoading");
    }
    return candidates.value.length > 0
      ? translate("admin.subdomainProxy.dockerTargetCandidatesHint")
      : translate("admin.subdomainProxy.dockerTargetCandidatesEmpty");
  });

  return {
    targetCandidateHint,
    targetPlaceholder,
    targetSuggestions,
  };
};
