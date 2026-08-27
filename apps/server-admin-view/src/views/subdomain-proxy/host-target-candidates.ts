import type { ScanDiscoveryHostCandidate } from "@/lib/api/scan";

export const NATIVE_LOOPBACK_ADDRESS = "127.0.0.1";

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

export const resolveEffectiveHostTargetCandidates = (
  candidates: readonly ScanDiscoveryHostCandidate[],
  isDockerDeployment: boolean,
): ScanDiscoveryHostCandidate[] => {
  const resolved = isDockerDeployment
    ? candidates.filter(
        (candidate) =>
          DOCKER_HOST_CANDIDATE_SOURCES.has(candidate.source) &&
          !candidate.address.startsWith("127."),
      )
    : [
        nativeLoopbackCandidate(),
        ...candidates.filter(
          (candidate) =>
            candidate.source === "interface" &&
            !candidate.address.startsWith("127."),
        ),
      ];

  const seen = new Set<string>();
  return resolved.filter((candidate) => {
    if (seen.has(candidate.address)) return false;
    seen.add(candidate.address);
    return true;
  });
};

export const buildHostTargetSuggestions = (
  candidates: readonly ScanDiscoveryHostCandidate[],
  isDockerDeployment: boolean,
): string[] =>
  resolveEffectiveHostTargetCandidates(candidates, isDockerDeployment).map(
    (candidate) => `${candidate.address}:`,
  );

export const buildHostTargetPlaceholder = (
  candidates: readonly ScanDiscoveryHostCandidate[],
  isDockerDeployment: boolean,
  dockerFallback: string,
): string => {
  const resolved = resolveEffectiveHostTargetCandidates(
    candidates,
    isDockerDeployment,
  );
  const recommended =
    resolved.find((candidate) => candidate.recommended) ?? resolved[0];
  if (!recommended) return dockerFallback;
  return `${recommended.address}:${isDockerDeployment ? 8080 : 5173}`;
};
