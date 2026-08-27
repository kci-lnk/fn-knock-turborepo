import { computed, ref, watch, type ComputedRef } from "vue";
import { ScanAPI, type ScanDiscoveryHostCandidate } from "@/lib/api/scan";

type Translate = (key: string) => string;

const NATIVE_LOOPBACK_ADDRESS = "127.0.0.1";
const DOCKER_HOST_CANDIDATE_SOURCES = new Set<
  ScanDiscoveryHostCandidate["source"]
>(["configured", "proxy", "request_host"]);

const nativeLoopbackCandidate = (): ScanDiscoveryHostCandidate => ({
  address: NATIVE_LOOPBACK_ADDRESS,
  cidr: `${NATIVE_LOOPBACK_ADDRESS}/32`,
  source: "loopback",
  recommended: true,
  includedInAutomaticScan: true,
});

const effectiveCandidates = (
  candidates: readonly ScanDiscoveryHostCandidate[],
  isDockerDeployment: boolean,
): readonly ScanDiscoveryHostCandidate[] => {
  if (isDockerDeployment) {
    return candidates.filter(
      (candidate) =>
        DOCKER_HOST_CANDIDATE_SOURCES.has(candidate.source) &&
        !candidate.address.startsWith("127."),
    );
  }
  return [
    nativeLoopbackCandidate(),
    ...candidates.filter(
      (candidate) => !candidate.address.startsWith("127."),
    ),
  ];
};

export const buildHostTargetSuggestions = (
  candidates: readonly ScanDiscoveryHostCandidate[],
  isDockerDeployment: boolean,
): string[] => [
  ...new Set(
    effectiveCandidates(candidates, isDockerDeployment).map(
      (candidate) => `${candidate.address}:`,
    ),
  ),
];

export const buildHostTargetPlaceholder = (
  candidates: readonly ScanDiscoveryHostCandidate[],
  isDockerDeployment: boolean,
  dockerFallback: string,
): string => {
  const resolved = effectiveCandidates(candidates, isDockerDeployment);
  const recommended =
    resolved.find((candidate) => candidate.recommended) ?? resolved[0];
  if (!recommended) return dockerFallback;
  return `${recommended.address}:${isDockerDeployment ? 8080 : 5173}`;
};

export const useHostTargetCandidates = ({
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

  const targetSuggestions = computed(() =>
    buildHostTargetSuggestions(candidates.value, isDockerDeployment.value),
  );
  const targetPlaceholder = computed(() =>
    buildHostTargetPlaceholder(
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
    return targetSuggestions.value.length > 0
      ? translate("admin.subdomainProxy.dockerTargetCandidatesHint")
      : translate("admin.subdomainProxy.dockerTargetCandidatesEmpty");
  });

  return {
    targetCandidateHint,
    targetPlaceholder,
    targetSuggestions,
  };
};
