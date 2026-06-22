import { BlockList, isIP } from "node:net";

import {
  isValidCIDR,
  normalizeCidrLines,
} from "../../../../packages/admin-shared/src/utils/cidr";
import { tDefault } from "./i18n";
import { normalizeIp } from "./ip-normalize";

const scannerT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => tDefault(`server.scanner.${key}`, params);

const toCidrInputList = (value: unknown): string[] => {
  if (!Array.isArray(value)) return [];
  return value.map((item) => String(item ?? ""));
};

type ScannerCidrMatcher = ReturnType<typeof buildScannerCidrExemptionMatcher>;

let cachedMatcherKey = "";
let cachedMatcher: ScannerCidrMatcher | null = null;

export class ScannerCidrValidationError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "ScannerCidrValidationError";
  }
}

export const normalizeScannerCidrExemptions = (value: unknown): string[] =>
  normalizeCidrLines(toCidrInputList(value)).filter((cidr) =>
    isValidCIDR(cidr),
  );

export const validateScannerCidrExemptions = (value: unknown): string[] => {
  const normalized = normalizeCidrLines(toCidrInputList(value));
  const invalid = normalized.filter((cidr) => !isValidCIDR(cidr));
  if (invalid.length > 0) {
    throw new ScannerCidrValidationError(
      scannerT("cidrExemptionsInvalid", { cidrs: invalid.join(", ") }),
    );
  }
  return normalized;
};

export const buildScannerCidrExemptionMatcher = (cidrs: unknown) => {
  const blockList = new BlockList();

  for (const cidr of normalizeScannerCidrExemptions(cidrs)) {
    const [address, prefixText] = cidr.split("/");
    const prefix = Number.parseInt(prefixText ?? "", 10);
    if (!address || !Number.isFinite(prefix)) continue;
    const family = isIP(address) === 6 ? "ipv6" : "ipv4";
    blockList.addSubnet(address, prefix, family);
  }

  return {
    contains(ip: string): boolean {
      const normalizedIp = normalizeIp(ip);
      const version = isIP(normalizedIp);
      if (!normalizedIp || version === 0) return false;
      return blockList.check(normalizedIp, version === 6 ? "ipv6" : "ipv4");
    },
  };
};

export const isScannerCidrExemptIp = (
  ip: string,
  cidrs: unknown,
): boolean => {
  const exemptions = normalizeScannerCidrExemptions(cidrs);
  if (exemptions.length === 0) return false;
  const matcherKey = exemptions.join("\n");
  if (!cachedMatcher || cachedMatcherKey !== matcherKey) {
    cachedMatcherKey = matcherKey;
    cachedMatcher = buildScannerCidrExemptionMatcher(exemptions);
  }
  return cachedMatcher.contains(ip);
};
