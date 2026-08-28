import { isProxyTargetProtocol } from "@admin-shared/utils/proxyTargetInput";
import type { ScanDiscoveryHostCandidate } from "@/lib/api/scan";
import type { HostMapping } from "@/types";
import { NATIVE_LOOPBACK_ADDRESS } from "./host-target-candidates";

export type TargetOptimizationDirection =
  | "loopback_to_lan"
  | "lan_to_loopback";

export interface TargetOptimizationDestination {
  address: string;
  direction: TargetOptimizationDirection;
  source: ScanDiscoveryHostCandidate["source"];
}

export interface TargetOptimizationPreview {
  direction: TargetOptimizationDirection;
  host: string;
  nextTarget: string;
  target: string;
}

const PROTECTED_LOOPBACK_TARGET_PORT = "7998";

const isIpv4Address = (value: string): boolean => {
  const parts = value.split(".");
  return (
    parts.length === 4 &&
    parts.every((part) => {
      if (!/^\d{1,3}$/u.test(part)) return false;
      const number = Number(part);
      return number >= 0 && number <= 255 && String(number) === part;
    })
  );
};

export const parseOptimizableTargetHostname = (target: string): string | null => {
  const normalized = target.trim();
  if (!normalized) return null;
  try {
    const parsed = new URL(normalized);
    if (
      parsed.hostname === NATIVE_LOOPBACK_ADDRESS &&
      parsed.port === PROTECTED_LOOPBACK_TARGET_PORT
    ) {
      return null;
    }
    return isProxyTargetProtocol(parsed.protocol) &&
      isIpv4Address(parsed.hostname)
      ? parsed.hostname
      : null;
  } catch {
    return null;
  }
};

export const rewriteTargetHostname = (
  target: string,
  sourceAddresses: ReadonlySet<string>,
  destinationAddress: string,
): string | null => {
  if (!isIpv4Address(destinationAddress)) return null;
  const sourceAddress = parseOptimizableTargetHostname(target);
  if (!sourceAddress || !sourceAddresses.has(sourceAddress)) return null;

  const contentStart = target.length - target.trimStart().length;
  const contentEnd = target.trimEnd().length;
  const normalized = target.slice(contentStart, contentEnd);
  const schemeEnd = normalized.indexOf("://");
  if (schemeEnd < 0) return null;
  const authorityStart = schemeEnd + 3;
  const suffixOffset = normalized.slice(authorityStart).search(/[/?#]/u);
  const authorityEnd =
    suffixOffset < 0 ? normalized.length : authorityStart + suffixOffset;
  const authority = normalized.slice(authorityStart, authorityEnd);
  const userInfoEnd = authority.lastIndexOf("@");
  const hostStartInAuthority = userInfoEnd < 0 ? 0 : userInfoEnd + 1;
  const hostAndPort = authority.slice(hostStartInAuthority);
  if (
    hostAndPort.slice(0, sourceAddress.length).toLowerCase() !==
      sourceAddress.toLowerCase() ||
    !["", ":"].includes(hostAndPort[sourceAddress.length] ?? "")
  ) {
    return null;
  }

  const absoluteHostStart = contentStart + authorityStart + hostStartInAuthority;
  return `${target.slice(0, absoluteHostStart)}${destinationAddress}${target.slice(
    absoluteHostStart + sourceAddress.length,
  )}`;
};

export const buildTargetOptimizationDestinations = (
  candidates: readonly ScanDiscoveryHostCandidate[],
  isDockerDeployment: boolean,
): TargetOptimizationDestination[] => {
  const destinations: TargetOptimizationDestination[] = [];
  for (const candidate of candidates) {
    if (candidate.address === NATIVE_LOOPBACK_ADDRESS) {
      if (!isDockerDeployment) {
        destinations.push({
          address: candidate.address,
          direction: "lan_to_loopback",
          source: candidate.source,
        });
      }
      continue;
    }
    if (
      (!isDockerDeployment && candidate.source !== "interface") ||
      !isIpv4Address(candidate.address)
    ) {
      continue;
    }
    destinations.push({
      address: candidate.address,
      direction: "loopback_to_lan",
      source: candidate.source,
    });
  }
  return destinations;
};

const buildOptimizationSourceAddresses = (
  destination: TargetOptimizationDestination,
  candidates: readonly ScanDiscoveryHostCandidate[],
): ReadonlySet<string> =>
  destination.direction === "loopback_to_lan"
    ? new Set([NATIVE_LOOPBACK_ADDRESS])
    : new Set(
        candidates
          .filter(
            (candidate) =>
              candidate.source === "interface" &&
              candidate.address !== NATIVE_LOOPBACK_ADDRESS,
          )
          .map((candidate) => candidate.address),
      );

export const buildTargetOptimizationPreviews = ({
  candidates,
  destinationAddress,
  isAuthServiceTarget,
  isDockerDeployment,
  mappings,
}: {
  candidates: readonly ScanDiscoveryHostCandidate[];
  destinationAddress: string;
  isAuthServiceTarget: (target: string) => boolean;
  isDockerDeployment: boolean;
  mappings: readonly HostMapping[];
}): TargetOptimizationPreview[] => {
  const destination = buildTargetOptimizationDestinations(
    candidates,
    isDockerDeployment,
  ).find((candidate) => candidate.address === destinationAddress);
  if (!destination) return [];
  const sourceAddresses = buildOptimizationSourceAddresses(
    destination,
    candidates,
  );

  return mappings.flatMap((mapping) => {
    if (isAuthServiceTarget(mapping.target)) return [];
    const nextTarget = rewriteTargetHostname(
      mapping.target,
      sourceAddresses,
      destination.address,
    );
    if (!nextTarget || nextTarget === mapping.target) return [];
    return [
      {
        direction: destination.direction,
        host: mapping.host,
        nextTarget,
        target: mapping.target,
      },
    ];
  });
};

export const resolveDefaultTargetOptimizationDestination = ({
  candidates,
  isAuthServiceTarget,
  isDockerDeployment,
  mappings,
}: {
  candidates: readonly ScanDiscoveryHostCandidate[];
  isAuthServiceTarget: (target: string) => boolean;
  isDockerDeployment: boolean;
  mappings: readonly HostMapping[];
}): string => {
  const destinations = buildTargetOptimizationDestinations(
    candidates,
    isDockerDeployment,
  );
  const firstLanDestination = destinations.find(
    (destination) => destination.direction === "loopback_to_lan",
  );
  if (
    firstLanDestination &&
    buildTargetOptimizationPreviews({
      candidates,
      destinationAddress: firstLanDestination.address,
      isAuthServiceTarget,
      isDockerDeployment,
      mappings,
    }).length > 0
  ) {
    return firstLanDestination.address;
  }

  const loopbackDestination = destinations.find(
    (destination) => destination.direction === "lan_to_loopback",
  );
  if (
    loopbackDestination &&
    buildTargetOptimizationPreviews({
      candidates,
      destinationAddress: loopbackDestination.address,
      isAuthServiceTarget,
      isDockerDeployment,
      mappings,
    }).length > 0
  ) {
    return loopbackDestination.address;
  }
  return firstLanDestination?.address ?? loopbackDestination?.address ?? "";
};
