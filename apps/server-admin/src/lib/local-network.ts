import { networkInterfaces } from "node:os";

export interface LocalIpv4Candidate {
  label: string;
  value: string;
  interface: string;
  netmask?: string;
  prefix?: number;
}

const EXCLUDED_INTERFACE_PATTERNS = [
  /^lo$/i,
  /^docker/i,
  /^br-/i,
  /^veth/i,
  /^tailscale/i,
  /^zt/i,
  /^tun/i,
  /^tap/i,
  /^wg/i,
] as const;

const isExcludedInterface = (name: string): boolean =>
  EXCLUDED_INTERFACE_PATTERNS.some((pattern) => pattern.test(name));

const isIpv4Family = (family: string | number): boolean =>
  family === "IPv4" || family === 4;

const parseIpv4Segments = (value: string): number[] | null => {
  const segments = value.split(".").map((segment) => Number.parseInt(segment, 10));
  if (
    segments.length !== 4 ||
    segments.some((segment) => !Number.isInteger(segment) || segment < 0 || segment > 255)
  ) {
    return null;
  }

  return segments;
};

export const ipv4NetmaskToPrefix = (value: string): number | null => {
  const segments = parseIpv4Segments(value.trim());
  if (!segments) return null;

  const mask = segments.reduce((acc, segment) => (acc << 8) | segment, 0) >>> 0;
  let prefix = 0;
  let hasSeenZero = false;

  for (let bit = 31; bit >= 0; bit -= 1) {
    const isOne = Boolean(mask & (1 << bit));
    if (isOne && hasSeenZero) return null;
    if (isOne) {
      prefix += 1;
    } else {
      hasSeenZero = true;
    }
  }

  return prefix;
};

export const isPrivateIpv4Address = (value: string): boolean => {
  const [a, b] = parseIpv4Segments(value) || [];
  if (
    a === undefined ||
    b === undefined ||
    !Number.isInteger(a) ||
    !Number.isInteger(b)
  ) {
    return false;
  }

  if (a === 10) return true;
  if (a === 172 && b >= 16 && b <= 31) return true;
  if (a === 192 && b === 168) return true;
  return false;
};

export const listPrivateIpv4Candidates = (): LocalIpv4Candidate[] => {
  const seen = new Set<string>();
  const results: LocalIpv4Candidate[] = [];

  for (const [name, items] of Object.entries(networkInterfaces())) {
    if (!items || isExcludedInterface(name)) {
      continue;
    }

    for (const item of items) {
      if (item.internal || !isIpv4Family(item.family)) {
        continue;
      }

      const address = String(item.address ?? "").trim();
      if (!address || !isPrivateIpv4Address(address) || seen.has(address)) {
        continue;
      }

      seen.add(address);
      const netmask = String(item.netmask ?? "").trim();
      const prefix = netmask ? ipv4NetmaskToPrefix(netmask) : null;
      results.push({
        label: `${address} (${name})`,
        value: address,
        interface: name,
        ...(netmask ? { netmask } : {}),
        ...(prefix !== null ? { prefix } : {}),
      });
    }
  }

  return results.sort((left, right) =>
    left.interface === right.interface
      ? left.value.localeCompare(right.value)
      : left.interface.localeCompare(right.interface),
  );
};
