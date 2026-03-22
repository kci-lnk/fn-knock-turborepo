import { isIP } from "node:net";

export const normalizeIp = (ip: string | null | undefined): string => {
  let candidate = String(ip ?? "").trim();
  if (!candidate) return "";

  const bracketedMatch = candidate.match(/^\[(.+)\](?::\d+)?$/);
  if (bracketedMatch?.[1]) {
    candidate = bracketedMatch[1];
  }

  const ipv4WithPortMatch = candidate.match(/^(\d+\.\d+\.\d+\.\d+):\d+$/);
  if (ipv4WithPortMatch?.[1]) {
    candidate = ipv4WithPortMatch[1];
  }

  if (candidate.includes("%")) {
    candidate = candidate.split("%")[0] || candidate;
  }

  if (candidate.startsWith("::ffff:")) {
    const mapped = candidate.slice("::ffff:".length);
    if (isIP(mapped) === 4) {
      candidate = mapped;
    }
  }

  if (candidate === "::1") {
    candidate = "127.0.0.1";
  }

  return isIP(candidate) > 0 ? candidate : "";
};

const parseIpv6FirstHextet = (ip: string): number | null => {
  const candidate = ip.trim().toLowerCase();
  if (!candidate.includes(":")) return null;

  const [firstSegment] = candidate.split(":");
  if (firstSegment === undefined) return null;
  if (!firstSegment) return 0;

  const parsed = Number.parseInt(firstSegment, 16);
  if (!Number.isInteger(parsed) || parsed < 0 || parsed > 0xffff) {
    return null;
  }

  return parsed;
};

const isPrivateOrLocalIpv6 = (ip: string): boolean => {
  const normalized = ip.trim().toLowerCase();
  if (!normalized) return false;
  if (normalized === "::" || normalized === "::1") return true;

  const firstHextet = parseIpv6FirstHextet(normalized);
  if (firstHextet === null) return false;

  if ((firstHextet & 0xfe00) === 0xfc00) {
    return true;
  }

  if ((firstHextet & 0xffc0) === 0xfe80) {
    return true;
  }

  return false;
};

const parseIpv4Octets = (
  ip: string,
): [number, number, number, number] | null => {
  const parts = ip.split(".");
  if (parts.length !== 4) return null;

  const octets = parts.map((part) => Number.parseInt(part, 10));
  if (octets.some((part) => !Number.isInteger(part) || part < 0 || part > 255)) {
    return null;
  }

  return octets as [number, number, number, number];
};

export const isWhitelistExemptIp = (
  ip: string | null | undefined,
): boolean => {
  const normalizedIp = normalizeIp(ip);
  if (!normalizedIp) return false;

  if (isIP(normalizedIp) === 6) {
    return isPrivateOrLocalIpv6(normalizedIp);
  }

  const octets = parseIpv4Octets(normalizedIp);
  if (!octets) return false;

  const [first, second] = octets;
  if (first === 0) return true;
  if (first === 10) return true;
  if (first === 127) return true;
  if (first === 169 && second === 254) return true;
  if (first === 172 && second >= 16 && second <= 31) return true;
  if (first === 192 && second === 168) return true;
  if (first === 100 && second >= 64 && second <= 127) return true;

  return false;
};

export const shouldCheckWhitelistForIp = (
  ip: string | null | undefined,
): boolean => {
  const normalizedIp = normalizeIp(ip);
  return Boolean(normalizedIp) && !isWhitelistExemptIp(normalizedIp);
};
