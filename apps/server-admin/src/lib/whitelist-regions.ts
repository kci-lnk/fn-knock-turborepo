import { normalizeCidrLines } from "../../../../packages/admin-shared/src/utils/cidr";
import { normalizeWhiteListTarget } from "./whitelist-target";

export type WhitelistRegionInput = {
  province: string;
  query_city?: string | null;
};

export type WhitelistRegionLookupResult = {
  cidrGroups: {
    ipv4: string[];
    ipv6: string[];
  };
};

export type WhitelistRegionLookup = (input: {
  province: string;
  city?: string | null;
}) => Promise<WhitelistRegionLookupResult>;

export type WhitelistRegionResolveResult = {
  regions: WhitelistRegionInput[];
  cidrs: string[];
  total: number;
};

export class WhitelistRegionValidationError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "WhitelistRegionValidationError";
  }
}

const normalizeString = (value: unknown): string => String(value ?? "").trim();

const regionKey = (province: string, queryCity?: string | null): string =>
  `${province.trim()}::${String(queryCity ?? "").trim()}`;

export const normalizeWhitelistRegionInputs = (
  value: unknown,
): WhitelistRegionInput[] => {
  const items = Array.isArray(value) ? value : [];
  const result: WhitelistRegionInput[] = [];
  const seen = new Set<string>();

  for (const item of items) {
    if (!item || typeof item !== "object") continue;
    const raw = item as Partial<WhitelistRegionInput>;
    const province = normalizeString(raw.province);
    const queryCity = normalizeString(raw.query_city) || null;
    if (!province) continue;

    const key = regionKey(province, queryCity);
    if (seen.has(key)) continue;
    seen.add(key);
    result.push({ province, query_city: queryCity });
  }

  return result;
};

export const resolveWhitelistRegionCidrs = async ({
  regions,
  lookupCidrs,
}: {
  regions: unknown;
  lookupCidrs: WhitelistRegionLookup;
}): Promise<WhitelistRegionResolveResult> => {
  const normalizedRegions = normalizeWhitelistRegionInputs(regions);
  if (normalizedRegions.length === 0) {
    throw new WhitelistRegionValidationError("regionRequired");
  }

  const resolvedCidrs: string[] = [];
  for (const region of normalizedRegions) {
    const lookup = await lookupCidrs({
      province: region.province,
      city: region.query_city,
    });
    resolvedCidrs.push(...lookup.cidrGroups.ipv4, ...lookup.cidrGroups.ipv6);
  }

  const uniqueCidrs = normalizeCidrLines(
    resolvedCidrs
      .map((cidr) => normalizeWhiteListTarget(cidr, "cidr"))
      .filter(Boolean),
  );
  if (uniqueCidrs.length === 0) {
    throw new WhitelistRegionValidationError("regionEmpty");
  }

  return {
    regions: normalizedRegions,
    cidrs: uniqueCidrs,
    total: uniqueCidrs.length,
  };
};
