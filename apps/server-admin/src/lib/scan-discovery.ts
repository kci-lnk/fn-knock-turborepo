import { isIP } from "node:net";
import type { AppConfig } from "./redis";
import { listPrivateIpv4Candidates } from "./local-network";

export const DISCOVER_COMMON_PORTS = [
  80, 81, 88, 443, 3000, 3001, 5000, 5001, 5666, 6688, 7000, 7001, 7080, 7443,
  8000, 8001, 8080, 8081, 8082, 8086, 8088, 8090, 8091, 8096, 8097, 8123, 8443,
  8888, 9000, 9001, 9090, 9091, 9443, 10000, 12345, 16601, 18080, 19999,
] as const;

export const SCAN_DISCOVERY_LIMITS = {
  maxCidrs: 16,
  maxHosts: 1024,
} as const;

export type ScanDiscoveryTargetSource =
  | "docker"
  | "loopback"
  | "interface"
  | "mapping"
  | "custom"
  | "saved";

export interface ScanDiscoveryTarget {
  cidr: string;
  label: string;
  source: ScanDiscoveryTargetSource;
  hostCount: number;
  isAutomatic: boolean;
}

export interface ParsedIpv4Cidr {
  cidr: string;
  address: string;
  prefix: number;
  network: number;
  broadcast: number;
  firstHost: number;
  lastHost: number;
  hostCount: number;
}

export class ScanDiscoveryValidationError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "ScanDiscoveryValidationError";
  }
}

const IPV4_SEGMENT_RE = /^(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)$/;

const ALLOWED_LOCAL_RANGE_BOUNDS = [
  ["127.0.0.0", "127.255.255.255"],
  ["10.0.0.0", "10.255.255.255"],
  ["172.16.0.0", "172.31.255.255"],
  ["192.168.0.0", "192.168.255.255"],
  ["100.64.0.0", "100.127.255.255"],
  ["169.254.0.0", "169.254.255.255"],
] as const;

const ALLOWED_LOCAL_RANGES: Array<[number, number]> =
  ALLOWED_LOCAL_RANGE_BOUNDS.map(([start, end]) => {
    const startNumber = ipv4ToNumber(start);
    const endNumber = ipv4ToNumber(end);
    if (startNumber === null || endNumber === null) {
      throw new Error("Invalid built-in local network range");
    }
    return [startNumber, endNumber];
  });

export function ipv4ToNumber(value: string): number | null {
  const parts = value.trim().split(".");
  if (parts.length !== 4 || parts.some((part) => !IPV4_SEGMENT_RE.test(part))) {
    return null;
  }

  return parts.reduce((acc, part) => acc * 256 + Number.parseInt(part, 10), 0);
}

export function numberToIpv4(value: number): string {
  const normalized = Math.trunc(value) >>> 0;
  return [
    (normalized >>> 24) & 255,
    (normalized >>> 16) & 255,
    (normalized >>> 8) & 255,
    normalized & 255,
  ].join(".");
}

export function isAllowedScanIpv4(value: string): boolean {
  const numeric = ipv4ToNumber(value);
  if (numeric === null) return false;

  return ALLOWED_LOCAL_RANGES.some(
    ([start, end]) => numeric >= start && numeric <= end,
  );
}

const isAllowedScanRange = (firstHost: number, lastHost: number): boolean =>
  ALLOWED_LOCAL_RANGES.some(
    ([start, end]) => firstHost >= start && lastHost <= end,
  );

export function parseIpv4Cidr(value: string): ParsedIpv4Cidr | null {
  const normalized = value.trim();
  const slashIndex = normalized.indexOf("/");
  if (slashIndex <= 0 || slashIndex !== normalized.lastIndexOf("/")) {
    return null;
  }

  const address = normalized.slice(0, slashIndex).trim();
  const prefixRaw = normalized.slice(slashIndex + 1).trim();
  if (isIP(address) !== 4 || !/^\d+$/.test(prefixRaw)) {
    return null;
  }

  const addressNumber = ipv4ToNumber(address);
  const prefix = Number.parseInt(prefixRaw, 10);
  if (
    addressNumber === null ||
    !Number.isInteger(prefix) ||
    prefix < 0 ||
    prefix > 32
  ) {
    return null;
  }

  const mask = prefix === 0 ? 0 : (0xffffffff << (32 - prefix)) >>> 0;
  const network = (addressNumber & mask) >>> 0;
  const hostSize = 2 ** (32 - prefix);
  const broadcast = network + hostSize - 1;
  const firstHost = prefix >= 31 ? network : network + 1;
  const lastHost = prefix >= 31 ? broadcast : broadcast - 1;
  const hostCount = prefix >= 31 ? hostSize : Math.max(0, hostSize - 2);
  const cidr = `${numberToIpv4(network)}/${prefix}`;

  return {
    cidr,
    address,
    prefix,
    network,
    broadcast,
    firstHost,
    lastHost,
    hostCount,
  };
}

export function parseAllowedScanCidr(value: string): ParsedIpv4Cidr | null {
  const parsed = parseIpv4Cidr(value);
  if (!parsed || parsed.hostCount <= 0) return null;
  if (!isAllowedScanRange(parsed.firstHost, parsed.lastHost)) return null;
  return parsed;
}

export function normalizeAllowedScanCidrs(values: Iterable<string>): string[] {
  const result: string[] = [];
  const seen = new Set<string>();

  for (const raw of values) {
    const parsed = parseAllowedScanCidr(String(raw ?? ""));
    if (!parsed || seen.has(parsed.cidr)) continue;
    seen.add(parsed.cidr);
    result.push(parsed.cidr);
  }

  return result;
}

export function validateScanCidrs(values: Iterable<string>): string[] {
  const result: string[] = [];
  const seen = new Set<string>();
  const invalid: string[] = [];

  for (const raw of values) {
    const value = String(raw ?? "").trim();
    if (!value) continue;

    const parsed = parseAllowedScanCidr(value);
    if (!parsed) {
      invalid.push(value);
      continue;
    }
    if (seen.has(parsed.cidr)) continue;
    seen.add(parsed.cidr);
    result.push(parsed.cidr);
  }

  if (invalid.length > 0) {
    throw new ScanDiscoveryValidationError(
      `扫描网段仅支持本地 IPv4 CIDR：${invalid.slice(0, 3).join("、")}`,
    );
  }

  if (result.length > SCAN_DISCOVERY_LIMITS.maxCidrs) {
    throw new ScanDiscoveryValidationError(
      `单次最多选择 ${SCAN_DISCOVERY_LIMITS.maxCidrs} 个扫描网段`,
    );
  }

  const hostCount = countScanHosts(result);
  if (hostCount > SCAN_DISCOVERY_LIMITS.maxHosts) {
    throw new ScanDiscoveryValidationError(
      `单次最多扫描 ${SCAN_DISCOVERY_LIMITS.maxHosts} 台主机，当前为 ${hostCount} 台`,
    );
  }

  return result;
}

export function expandScanCidrs(cidrs: Iterable<string>): string[] {
  const hosts: string[] = [];
  const seen = new Set<number>();

  for (const cidr of cidrs) {
    const parsed = parseAllowedScanCidr(cidr);
    if (!parsed) continue;

    for (let host = parsed.firstHost; host <= parsed.lastHost; host += 1) {
      if (seen.has(host)) continue;
      seen.add(host);
      hosts.push(numberToIpv4(host));
      if (hosts.length > SCAN_DISCOVERY_LIMITS.maxHosts) {
        throw new ScanDiscoveryValidationError(
          `单次最多扫描 ${SCAN_DISCOVERY_LIMITS.maxHosts} 台主机`,
        );
      }
    }
  }

  return hosts;
}

export function countScanHosts(cidrs: Iterable<string>): number {
  return expandScanCidrs(cidrs).length;
}

export function buildIpv4Cidr(value: string, prefix = 24): string | null {
  if (isIP(value) !== 4 || prefix < 0 || prefix > 32) return null;
  const parsed = parseIpv4Cidr(`${value}/${prefix}`);
  return parsed?.cidr ?? null;
}

const toTarget = (
  cidr: string,
  label: string,
  source: ScanDiscoveryTargetSource,
  isAutomatic: boolean,
): ScanDiscoveryTarget | null => {
  const parsed = parseAllowedScanCidr(cidr);
  if (!parsed) return null;

  return {
    cidr: parsed.cidr,
    label,
    source,
    hostCount: parsed.hostCount,
    isAutomatic,
  };
};

export function dedupeTargets(
  targets: Array<ScanDiscoveryTarget | null>,
): ScanDiscoveryTarget[] {
  const seen = new Set<string>();
  const result: ScanDiscoveryTarget[] = [];

  for (const target of targets) {
    if (!target || seen.has(target.cidr)) continue;
    seen.add(target.cidr);
    result.push(target);
  }

  return result;
}

export function buildDockerDiscoverTarget(
  ip: string | null,
): ScanDiscoveryTarget | null {
  if (!ip || !isAllowedScanIpv4(ip)) return null;
  const cidr = buildIpv4Cidr(ip, 24);
  return cidr
    ? toTarget(cidr, `${cidr}（Docker 宿主机局域网）`, "docker", true)
    : null;
}

export function buildLoopbackDiscoverTarget(): ScanDiscoveryTarget {
  return toTarget(
    "127.0.0.1/32",
    "127.0.0.1/32（本机回环）",
    "loopback",
    true,
  )!;
}

export function buildInterfaceDiscoverTargets(): ScanDiscoveryTarget[] {
  return dedupeTargets(
    listPrivateIpv4Candidates().map((candidate) => {
      const cidr = buildIpv4Cidr(candidate.value, 24);
      return cidr
        ? toTarget(cidr, `${cidr}（${candidate.interface}）`, "interface", true)
        : null;
    }),
  );
}

const extractIpv4FromTarget = (value: string): string | null => {
  const trimmed = value.trim();
  if (!trimmed) return null;

  const parseHost = (input: string): string | null => {
    try {
      return new URL(input).hostname.trim();
    } catch {
      return null;
    }
  };

  const host = parseHost(trimmed) || parseHost(`http://${trimmed}`);
  if (!host || isIP(host) !== 4 || !isAllowedScanIpv4(host)) {
    return null;
  }

  return host;
};

export function buildMappingDiscoverTargets(
  config: Pick<AppConfig, "proxy_mappings" | "host_mappings">,
): ScanDiscoveryTarget[] {
  const targets = [
    ...(config.proxy_mappings || []).map((mapping) => mapping.target),
    ...(config.host_mappings || []).map((mapping) => mapping.target),
  ];

  return dedupeTargets(
    targets.map((target) => {
      const ip = extractIpv4FromTarget(target || "");
      if (!ip) return null;
      const cidr = buildIpv4Cidr(ip, ip.startsWith("127.") ? 32 : 24);
      return cidr
        ? toTarget(cidr, `${cidr}（已有映射目标）`, "mapping", true)
        : null;
    }),
  );
}

export function buildCustomDiscoverTargets(
  cidrs: Iterable<string>,
): ScanDiscoveryTarget[] {
  return normalizeAllowedScanCidrs(cidrs)
    .map((cidr) => toTarget(cidr, `${cidr}（自定义）`, "custom", false))
    .filter((target): target is ScanDiscoveryTarget => Boolean(target));
}

export function buildSavedDiscoverTargets(
  cidrs: Iterable<string>,
): ScanDiscoveryTarget[] {
  return normalizeAllowedScanCidrs(cidrs)
    .map((cidr) => toTarget(cidr, `${cidr}（已保存）`, "saved", false))
    .filter((target): target is ScanDiscoveryTarget => Boolean(target));
}

export function buildScanScope(cidrs: string[]): string | null {
  if (cidrs.length === 0) return null;
  if (cidrs.length === 1) return cidrs[0]!;
  return cidrs.join(", ");
}

export const buildSingletonPortRanges = (ports: readonly number[]) =>
  ports.map((port) => ({ start: port, end: port }));
