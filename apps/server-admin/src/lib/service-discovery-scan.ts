import { execFileSync } from "node:child_process";
import { availableParallelism, freemem, totalmem } from "node:os";
import type { ScanOptions } from "../plugins/scanner/types";
import type { AnalyzedScanService } from "../plugins/scanner/types";
import {
  expandScanCidrs,
  ipv4ToNumber,
  normalizeAllowedScanCidrs,
  parseAllowedScanCidr,
} from "./scan-discovery";

export const LOOPBACK_DISCOVERY_CIDR = "127.0.0.1/32";
export const LOOPBACK_DISCOVERY_HOST = "127.0.0.1";
export const DISCOVERY_PORT_RANGE_START = 80;
export const DISCOVERY_PORT_RANGE_END = 60000;
export const DISCOVERY_LIMITED_PORT_RANGE_END = 9999;
export const DISCOVERY_PORT_RANGE = {
  start: DISCOVERY_PORT_RANGE_START,
  end: DISCOVERY_PORT_RANGE_END,
} as const;
export const DISCOVERY_LIMITED_PORT_RANGE = {
  start: DISCOVERY_PORT_RANGE_START,
  end: DISCOVERY_LIMITED_PORT_RANGE_END,
} as const;
export const DISCOVERY_PORT_COUNT =
  DISCOVERY_PORT_RANGE_END - DISCOVERY_PORT_RANGE_START + 1;
export const DISCOVERY_LIMITED_PORT_COUNT =
  DISCOVERY_LIMITED_PORT_RANGE_END - DISCOVERY_PORT_RANGE_START + 1;
export const LOCAL_SELF_DISCOVERY_SKIP_PORTS = [80] as const;

const DISCOVERY_TIMEOUT_MS = 80;
const MIN_NETWORK_MAX_CONCURRENT = 64;
const MIN_NETWORK_HOST_CONCURRENCY = 6;
const MIN_LOOPBACK_MAX_CONCURRENT = 200;
const MAX_TOTAL_SOCKET_BUDGET = 4096;
const MAX_PORT_CONCURRENCY = 1024;
const MAX_HOST_CONCURRENCY = 32;
const DEFAULT_FILE_DESCRIPTOR_LIMIT = 1024;
const FILE_DESCRIPTOR_RESERVED = 96;
const FILE_DESCRIPTOR_SOCKET_RATIO = 0.6;
const BYTES_PER_MIB = 1024 * 1024;

export interface ServiceDiscoveryScanResult {
  host?: string;
  totalPortsScanned: number;
  foundServices: number;
  scannedHosts?: number;
  services: unknown[];
}

export interface ServiceDiscoveryScanner {
  scanAndAnalyze: (
    host: string,
    options?: ScanOptions,
  ) => Promise<ServiceDiscoveryScanResult>;
  scanAndAnalyzeMany: (
    hosts: string[],
    options?: ScanOptions,
  ) => Promise<ServiceDiscoveryScanResult>;
}

export interface DiscoveryDeviceProfile {
  cpuCount: number;
  totalMemoryMb: number;
  freeMemoryMb: number;
}

export interface DiscoveryConcurrency {
  maxConcurrent: number;
  hostConcurrency: number;
  totalSocketBudget: number;
}

export interface DiscoveryProgressEvent {
  scannedPorts: number;
  totalPorts: number;
  scannedHosts: number;
  totalHosts: number;
  currentHost?: string;
}

export type DiscoveryPortRangeMode = "full" | "limited";

export interface DiscoveryHostGroup {
  cidr: string;
  hosts: string[];
  mode: DiscoveryPortRangeMode;
  portRange: { start: number; end: number };
  skipPorts: number[];
}

const clamp = (value: number, min: number, max: number): number =>
  Math.min(max, Math.max(min, value));

const normalizePositiveInteger = (value: number, fallback: number): number => {
  if (!Number.isFinite(value)) return fallback;
  const normalized = Math.trunc(value);
  return normalized > 0 ? normalized : fallback;
};

export const getLocalDiscoveryDeviceProfile = (): DiscoveryDeviceProfile => ({
  cpuCount: normalizePositiveInteger(availableParallelism(), 1),
  totalMemoryMb: normalizePositiveInteger(totalmem() / BYTES_PER_MIB, 512),
  freeMemoryMb: normalizePositiveInteger(freemem() / BYTES_PER_MIB, 512),
});

export const getProcessFileDescriptorLimit = (): number => {
  if (process.platform === "win32") return DEFAULT_FILE_DESCRIPTOR_LIMIT;

  try {
    const output = execFileSync("/bin/sh", ["-c", "ulimit -n"], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
      timeout: 200,
    }).trim();
    if (output === "unlimited") return Number.POSITIVE_INFINITY;
    return normalizePositiveInteger(
      Number.parseInt(output, 10),
      DEFAULT_FILE_DESCRIPTOR_LIMIT,
    );
  } catch {
    return DEFAULT_FILE_DESCRIPTOR_LIMIT;
  }
};

const calculateFileDescriptorSocketBudget = (
  fileDescriptorLimit: number,
): number => {
  if (!Number.isFinite(fileDescriptorLimit)) return MAX_TOTAL_SOCKET_BUDGET;

  const availableDescriptors = Math.max(
    1,
    fileDescriptorLimit - FILE_DESCRIPTOR_RESERVED,
  );
  return clamp(
    Math.floor(availableDescriptors * FILE_DESCRIPTOR_SOCKET_RATIO),
    1,
    MAX_TOTAL_SOCKET_BUDGET,
  );
};

export const calculateDiscoveryConcurrency = ({
  deviceProfile = getLocalDiscoveryDeviceProfile(),
  fileDescriptorLimit = getProcessFileDescriptorLimit(),
  minimumHostConcurrency,
  minimumMaxConcurrent,
  scanHostCount,
}: {
  deviceProfile?: DiscoveryDeviceProfile;
  fileDescriptorLimit?: number;
  minimumHostConcurrency: number;
  minimumMaxConcurrent: number;
  scanHostCount: number;
}): DiscoveryConcurrency => {
  const hostCount = normalizePositiveInteger(scanHostCount, 1);
  const cpuCount = clamp(
    normalizePositiveInteger(deviceProfile.cpuCount, 1),
    1,
    128,
  );
  const totalMemoryMb = normalizePositiveInteger(
    deviceProfile.totalMemoryMb,
    512,
  );
  const freeMemoryMb = normalizePositiveInteger(
    deviceProfile.freeMemoryMb,
    512,
  );
  const usableMemoryMb = Math.max(
    512,
    Math.min(totalMemoryMb, freeMemoryMb * 2),
  );
  const memorySlots = Math.floor(usableMemoryMb / 512);
  const socketBudgetCap =
    calculateFileDescriptorSocketBudget(fileDescriptorLimit);
  const requestedMinimumSocketBudget =
    minimumMaxConcurrent * minimumHostConcurrency;
  const minimumSocketBudget = Math.min(
    requestedMinimumSocketBudget,
    socketBudgetCap,
  );
  const totalSocketBudget = clamp(
    minimumSocketBudget + cpuCount * 96 + memorySlots * 64,
    minimumSocketBudget,
    socketBudgetCap,
  );
  const maxHostConcurrencyByBudget = Math.max(
    1,
    Math.floor(
      totalSocketBudget / Math.min(minimumMaxConcurrent, totalSocketBudget),
    ),
  );
  const hostConcurrency = clamp(
    Math.ceil(Math.sqrt(hostCount * cpuCount)),
    Math.min(minimumHostConcurrency, maxHostConcurrencyByBudget),
    Math.min(MAX_HOST_CONCURRENCY, maxHostConcurrencyByBudget),
  );
  const maxConcurrent = clamp(
    Math.floor(totalSocketBudget / hostConcurrency),
    Math.min(minimumMaxConcurrent, totalSocketBudget),
    Math.min(MAX_PORT_CONCURRENCY, totalSocketBudget),
  );

  return {
    maxConcurrent,
    hostConcurrency,
    totalSocketBudget,
  };
};

export const buildDiscoveryPortModeLabel = (
  _isDockerRuntime: boolean,
  scanCidrs: readonly string[],
  fullRangeCidrs: readonly string[] = [],
) => {
  const groups = buildDiscoveryHostGroups(scanCidrs, fullRangeCidrs);
  const hasFullRange = groups.some((group) => group.mode === "full");
  const hasLimitedRange = groups.some((group) => group.mode === "limited");

  if (hasFullRange && hasLimitedRange) {
    return `local=${DISCOVERY_PORT_RANGE_START}-${DISCOVERY_PORT_RANGE_END}, other=${DISCOVERY_PORT_RANGE_START}-${DISCOVERY_LIMITED_PORT_RANGE_END}`;
  }

  return hasLimitedRange
    ? `${DISCOVERY_PORT_RANGE_START}-${DISCOVERY_LIMITED_PORT_RANGE_END}`
    : `${DISCOVERY_PORT_RANGE_START}-${DISCOVERY_PORT_RANGE_END}`;
};

const countPortsInRange = (
  range: { start: number; end: number },
  excludePorts: readonly number[],
): number => {
  const excluded = new Set(
    excludePorts.filter((port) => port >= range.start && port <= range.end),
  );
  return range.end - range.start + 1 - excluded.size;
};

export const countDiscoveryPorts = (
  excludePorts: readonly number[],
): number => {
  return countPortsInRange(DISCOVERY_PORT_RANGE, excludePorts);
};

export const countLimitedDiscoveryPorts = (
  excludePorts: readonly number[],
): number => countPortsInRange(DISCOVERY_LIMITED_PORT_RANGE, excludePorts);

const mergeDiscoverySkipPorts = (
  ...portLists: Array<readonly number[] | undefined>
): number[] =>
  Array.from(
    new Set(
      portLists
        .flatMap((ports) => [...(ports || [])])
        .filter((port) => Number.isFinite(port) && port > 0),
    ),
  );

const isFullRangeDiscoveryHost = (
  host: string,
  fullRangeCidrs: readonly string[],
): boolean => {
  if (host === LOOPBACK_DISCOVERY_HOST) return true;

  const hostNumber = ipv4ToNumber(host);
  if (hostNumber === null) return false;

  return fullRangeCidrs.some((fullRangeCidr) => {
    const parsed = parseAllowedScanCidr(fullRangeCidr);
    return (
      parsed !== null &&
      hostNumber >= parsed.firstHost &&
      hostNumber <= parsed.lastHost
    );
  });
};

const buildDiscoveryHostGroup = (
  cidr: string,
  hosts: string[],
  mode: DiscoveryPortRangeMode,
  skipPorts: readonly number[] = [],
): DiscoveryHostGroup => ({
  cidr,
  hosts,
  mode,
  portRange:
    mode === "full" ? DISCOVERY_PORT_RANGE : DISCOVERY_LIMITED_PORT_RANGE,
  skipPorts: [...skipPorts],
});

const buildSelfScanHostSet = (selfScanHosts?: readonly string[]): Set<string> =>
  new Set(
    [LOOPBACK_DISCOVERY_HOST, ...(selfScanHosts || [])]
      .map((host) => String(host || "").trim())
      .filter(Boolean),
  );

const pushDiscoveryHostGroups = ({
  cidr,
  groups,
  hosts,
  mode,
  selfScanHosts,
}: {
  cidr: string;
  groups: DiscoveryHostGroup[];
  hosts: string[];
  mode: DiscoveryPortRangeMode;
  selfScanHosts: ReadonlySet<string>;
}) => {
  const regularHosts: string[] = [];
  const localSelfHosts: string[] = [];
  for (const host of hosts) {
    if (selfScanHosts.has(host)) {
      localSelfHosts.push(host);
    } else {
      regularHosts.push(host);
    }
  }

  if (regularHosts.length > 0) {
    groups.push(buildDiscoveryHostGroup(cidr, regularHosts, mode));
  }
  if (localSelfHosts.length > 0) {
    groups.push(
      buildDiscoveryHostGroup(
        cidr,
        localSelfHosts,
        mode,
        LOCAL_SELF_DISCOVERY_SKIP_PORTS,
      ),
    );
  }
};

export const buildDiscoveryHostGroups = (
  scanCidrs: readonly string[],
  fullRangeCidrs: readonly string[] = [],
  scanHosts?: readonly string[],
  selfScanHosts?: readonly string[],
): DiscoveryHostGroup[] => {
  const normalizedFullRangeCidrs = normalizeAllowedScanCidrs([
    LOOPBACK_DISCOVERY_CIDR,
    ...fullRangeCidrs,
  ]);
  const allowedHosts = scanHosts ? new Set(scanHosts) : null;
  const selfScanHostSet = buildSelfScanHostSet(selfScanHosts);
  const seenHosts = new Set<string>();
  const groups: DiscoveryHostGroup[] = [];

  for (const cidr of normalizeAllowedScanCidrs(scanCidrs)) {
    const hosts = expandScanCidrs([cidr]).filter((host) => {
      if (allowedHosts && !allowedHosts.has(host)) return false;
      if (seenHosts.has(host)) return false;
      seenHosts.add(host);
      return true;
    });
    if (hosts.length === 0) continue;

    const fullRangeHosts: string[] = [];
    const limitedRangeHosts: string[] = [];
    for (const host of hosts) {
      if (isFullRangeDiscoveryHost(host, normalizedFullRangeCidrs)) {
        fullRangeHosts.push(host);
      } else {
        limitedRangeHosts.push(host);
      }
    }

    if (fullRangeHosts.length > 0) {
      pushDiscoveryHostGroups({
        cidr,
        groups,
        hosts: fullRangeHosts,
        mode: "full",
        selfScanHosts: selfScanHostSet,
      });
    }
    if (limitedRangeHosts.length > 0) {
      pushDiscoveryHostGroups({
        cidr,
        groups,
        hosts: limitedRangeHosts,
        mode: "limited",
        selfScanHosts: selfScanHostSet,
      });
    }
  }

  return groups;
};

export const countDiscoveryScanPorts = ({
  excludePorts,
  fullRangeCidrs,
  scanCidrs,
  scanHosts,
  selfScanHosts,
}: {
  excludePorts: readonly number[];
  fullRangeCidrs?: readonly string[];
  scanCidrs: readonly string[];
  scanHosts?: readonly string[];
  selfScanHosts?: readonly string[];
}): number =>
  buildDiscoveryHostGroups(
    scanCidrs,
    fullRangeCidrs,
    scanHosts,
    selfScanHosts,
  ).reduce((total, group) => {
    const skipPorts = mergeDiscoverySkipPorts(excludePorts, group.skipPorts);
    return (
      total + countPortsInRange(group.portRange, skipPorts) * group.hosts.length
    );
  }, 0);

export const buildDiscoveryScanOptions = ({
  excludePorts,
  minimumHostConcurrency,
  minimumMaxConcurrent,
  portRange = DISCOVERY_PORT_RANGE,
  scanHostCount,
}: {
  excludePorts: readonly number[];
  minimumHostConcurrency: number;
  minimumMaxConcurrent: number;
  portRange?: { start: number; end: number };
  scanHostCount: number;
}): ScanOptions => {
  const concurrency = calculateDiscoveryConcurrency({
    minimumHostConcurrency,
    minimumMaxConcurrent,
    scanHostCount,
  });

  return {
    skipPorts: [...excludePorts],
    timeout: DISCOVERY_TIMEOUT_MS,
    maxConcurrent: concurrency.maxConcurrent,
    hostConcurrency: concurrency.hostConcurrency,
    portRanges: [{ ...portRange }],
  };
};

const buildNetworkScanOptions = (
  excludePorts: readonly number[],
  scanHostCount: number,
  portRange: { start: number; end: number },
): ScanOptions =>
  buildDiscoveryScanOptions({
    excludePorts,
    minimumHostConcurrency: MIN_NETWORK_HOST_CONCURRENCY,
    minimumMaxConcurrent: MIN_NETWORK_MAX_CONCURRENT,
    portRange,
    scanHostCount,
  });

const buildLoopbackScanOptions = (
  excludePorts: readonly number[],
): ScanOptions =>
  buildDiscoveryScanOptions({
    excludePorts,
    minimumHostConcurrency: 1,
    minimumMaxConcurrent: MIN_LOOPBACK_MAX_CONCURRENT,
    scanHostCount: 1,
  });

const mergeDiscoveryScanResults = (
  primaryHost: string,
  results: ServiceDiscoveryScanResult[],
): ServiceDiscoveryScanResult => {
  const services = results.flatMap((result) => result.services || []);

  return {
    host: primaryHost,
    totalPortsScanned: results.reduce(
      (total, result) => total + result.totalPortsScanned,
      0,
    ),
    foundServices: services.length,
    scannedHosts: results.reduce(
      (total, result) => total + (result.scannedHosts || 0),
      0,
    ),
    services,
  };
};

export async function runServiceDiscoveryScan({
  excludePorts,
  fullRangeCidrs,
  onProgress,
  onService,
  scanCidrs,
  scanHosts,
  selfScanHosts,
  signal,
  scannerService,
}: {
  excludePorts: readonly number[];
  fullRangeCidrs?: readonly string[];
  isDockerRuntime: boolean;
  onProgress?: (progress: DiscoveryProgressEvent) => void;
  onService?: (service: AnalyzedScanService) => void | Promise<void>;
  scanCidrs: readonly string[];
  scanHosts: readonly string[];
  selfScanHosts?: readonly string[];
  signal?: AbortSignal;
  scannerService: ServiceDiscoveryScanner;
}): Promise<ServiceDiscoveryScanResult> {
  const groups = buildDiscoveryHostGroups(
    scanCidrs,
    fullRangeCidrs,
    scanHosts,
    selfScanHosts,
  );
  const totalHosts = groups.reduce(
    (total, group) => total + group.hosts.length,
    0,
  );
  const totalPorts = groups.reduce(
    (total, group) => {
      const skipPorts = mergeDiscoverySkipPorts(excludePorts, group.skipPorts);
      return (
        total +
        countPortsInRange(group.portRange, skipPorts) * group.hosts.length
      );
    },
    0,
  );
  const completedHosts = new Set<string>();
  let scannedPorts = 0;
  const buildProgressOptions = (options: ScanOptions): ScanOptions => ({
    ...options,
    signal,
    onPortScanned: (progress) => {
      scannedPorts += 1;
      if (progress.scannedPorts >= progress.totalPorts) {
        completedHosts.add(progress.host);
      }
      options.onPortScanned?.(progress);
      onProgress?.({
        scannedPorts,
        totalPorts,
        scannedHosts: completedHosts.size,
        totalHosts,
        currentHost: progress.host,
      });
    },
    onService,
  });

  if (groups.length === 0) {
    return {
      host: "",
      totalPortsScanned: 0,
      foundServices: 0,
      scannedHosts: 0,
      services: [],
    };
  }

  const results: ServiceDiscoveryScanResult[] = [];

  for (const group of groups) {
    const skipPorts = mergeDiscoverySkipPorts(excludePorts, group.skipPorts);
    const shouldScanAsLoopback =
      group.hosts.length === 1 && group.hosts[0] === LOOPBACK_DISCOVERY_HOST;
    if (shouldScanAsLoopback) {
      results.push(
        await scannerService.scanAndAnalyze(
          LOOPBACK_DISCOVERY_HOST,
          buildProgressOptions(buildLoopbackScanOptions(skipPorts)),
        ),
      );
      continue;
    }

    results.push(
      await scannerService.scanAndAnalyzeMany(
        group.hosts,
        buildProgressOptions(
          buildNetworkScanOptions(
            skipPorts,
            group.hosts.length,
            group.portRange,
          ),
        ),
      ),
    );
  }

  return mergeDiscoveryScanResults(
    scanHosts[0] || groups[0]?.hosts[0] || "",
    results,
  );
}
