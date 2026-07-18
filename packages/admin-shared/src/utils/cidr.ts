const IPV4_SEGMENT_RE = /^(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)$/;
const IPV6_SEGMENT_RE = /^[0-9a-f]{1,4}$/i;

export const isValidIPv4Address = (value: string): boolean => {
  const parts = value.split(".");
  if (parts.length !== 4) return false;
  return parts.every((part) => IPV4_SEGMENT_RE.test(part));
};

export const isValidIPv6Address = (value: string): boolean => {
  const normalized = value.trim();
  if (!normalized || normalized.includes("%")) {
    return false;
  }

  let working = normalized;
  let embeddedIPv4Hextets = 0;

  if (working.includes(".")) {
    const lastColon = working.lastIndexOf(":");
    if (lastColon < 0) return false;
    const ipv4Part = working.slice(lastColon + 1);
    if (!isValidIPv4Address(ipv4Part)) return false;
    working = working.slice(0, lastColon);
    embeddedIPv4Hextets = 2;
  }

  const sections = working.split("::");
  if (sections.length > 2) {
    return false;
  }

  const parseSide = (input: string): string[] | null => {
    if (!input) return [];
    const parts = input.split(":");
    if (parts.some((part) => !part || !IPV6_SEGMENT_RE.test(part))) {
      return null;
    }
    return parts;
  };

  const leftParts = parseSide(sections[0] ?? "");
  if (leftParts == null) return false;

  const rightParts = parseSide(sections[1] ?? "");
  if (rightParts == null) return false;

  const totalHextets =
    leftParts.length + rightParts.length + embeddedIPv4Hextets;

  if (sections.length === 1) {
    return totalHextets === 8;
  }

  return totalHextets < 8;
};

export const isValidCIDR = (value: string): boolean => {
  const normalized = value.trim();
  const slashIndex = normalized.indexOf("/");
  if (slashIndex <= 0 || slashIndex !== normalized.lastIndexOf("/")) {
    return false;
  }

  const address = normalized.slice(0, slashIndex).trim();
  const prefixRaw = normalized.slice(slashIndex + 1).trim();
  if (!address || !/^\d+$/.test(prefixRaw)) {
    return false;
  }

  const prefix = Number.parseInt(prefixRaw, 10);
  if (!Number.isFinite(prefix) || prefix < 0) {
    return false;
  }

  if (isValidIPv4Address(address)) {
    return prefix <= 32;
  }

  if (isValidIPv6Address(address)) {
    return prefix <= 128;
  }

  return false;
};

export const normalizeCidrLines = (values: Iterable<string>): string[] => {
  const result: string[] = [];
  const seen = new Set<string>();

  for (const value of values) {
    const normalized = value.trim();
    if (!normalized) continue;
    const key = normalized.toLowerCase();
    if (seen.has(key)) continue;
    seen.add(key);
    result.push(normalized);
  }

  return result;
};

export const splitCidrTextarea = (value: string): string[] =>
  value
    .split(/\r?\n/u)
    .map((line) => line.trim())
    .filter(Boolean);

export const parseCidrTextarea = (value: string) => {
  const lines = splitCidrTextarea(value);
  const cidrs = normalizeCidrLines(lines);
  const invalid = cidrs.filter((cidr) => !isValidCIDR(cidr));

  return {
    cidrs,
    invalid,
  };
};
